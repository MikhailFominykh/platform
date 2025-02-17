use std::convert::AsRef;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tonic::Status;

use cheetah_libraries_microservice::tonic::{Request, Response};
use cheetah_libraries_microservice::trace::Trace;
use cheetah_matches_realtime_common::commands::c2s::C2SCommand;
use cheetah_matches_realtime_common::commands::s2c::S2CCommand;
use cheetah_matches_realtime_common::commands::FieldType;
use cheetah_matches_realtime_common::room::owner::GameObjectOwner;
use cheetah_matches_realtime_common::room::RoomId;

use crate::debug::proto::admin;
use crate::debug::proto::shared;
use crate::debug::tracer::{
	SessionId, TracedBothDirectionCommand, TracedCommand, TracerSessionCommand,
};
use crate::server::manager::ServerManager;

pub struct CommandTracerGRPCService {
	pub manager: Arc<Mutex<ServerManager>>,
}

impl CommandTracerGRPCService {
	pub fn new(relay_server: Arc<Mutex<ServerManager>>) -> Self {
		Self {
			manager: relay_server,
		}
	}

	///
	/// Выполнить задачу в relay сервере (в другом потоке), дождаться результата и преобразовать
	/// его в нужный для grpc формат
	///
	pub async fn execute_task<TaskResult, GrpcType>(
		&self,
		room_id: RoomId,
		task: TracerSessionCommand,
		receiver: std::sync::mpsc::Receiver<TaskResult>,
		converter: fn(TaskResult) -> Result<GrpcType, Status>,
	) -> Result<Response<GrpcType>, Status> {
		let manager = self.manager.lock().await;

		manager
			.execute_command_trace_sessions_task(room_id, task.clone())
			.trace_err(format!("Schedule tracer command {} {:?}", room_id, task))
			.map_err(Status::internal)?;

		let result = receiver
			.recv_timeout(Duration::from_millis(100))
			.trace_err(format!("Wait tracer command {} {:?}", room_id, task))
			.map_err(Status::internal)?;

		converter(result).map(Response::new)
	}
}

#[tonic::async_trait]
impl admin::command_tracer_server::CommandTracer for CommandTracerGRPCService {
	async fn create_session(
		&self,
		request: Request<admin::CreateSessionRequest>,
	) -> Result<Response<admin::CreateSessionResponse>, Status> {
		let (sender, receiver) = std::sync::mpsc::channel();
		let task = TracerSessionCommand::CreateSession(sender);
		self.execute_task(
			request.get_ref().room as RoomId,
			task,
			receiver,
			|session_id| {
				Ok(admin::CreateSessionResponse {
					id: session_id as u32,
				})
			},
		)
		.await
	}

	async fn set_filter(
		&self,
		request: Request<admin::SetFilterRequest>,
	) -> Result<Response<admin::SetFilterResponse>, Status> {
		let (sender, receiver) = std::sync::mpsc::channel();
		let request = request.get_ref();
		let task = TracerSessionCommand::SetFilter(
			request.session as SessionId,
			request.filter.clone(),
			sender,
		);
		self.execute_task(request.room as RoomId, task, receiver, |_| {
			Ok(admin::SetFilterResponse {})
		})
		.await
	}

	async fn get_commands(
		&self,
		request: Request<admin::GetCommandsRequest>,
	) -> Result<Response<admin::GetCommandsResponse>, Status> {
		let (sender, receiver) = std::sync::mpsc::channel();
		let request = request.get_ref();
		let task = TracerSessionCommand::GetCommands(request.session as SessionId, sender);
		self.execute_task(request.room as RoomId, task, receiver, |result| {
			result
				.trace_err("Get commands for trace")
				.map_err(Status::internal)
				.map(|commands| admin::GetCommandsResponse {
					commands: commands.into_iter().map(admin::Command::from).collect(),
				})
		})
		.await
	}

	async fn close_session(
		&self,
		request: Request<admin::CloseSessionRequest>,
	) -> Result<Response<admin::CloseSessionResponse>, Status> {
		let (sender, receiver) = std::sync::mpsc::channel();
		let request = request.get_ref();
		let task = TracerSessionCommand::CloseSession(request.session as SessionId, sender);
		self.execute_task(request.room as RoomId, task, receiver, |result| {
			result
				.trace_err("Close tracer session")
				.map_err(Status::internal)
				.map(|_| admin::CloseSessionResponse {})
		})
		.await
	}
}

impl From<TracedCommand> for admin::Command {
	fn from(command: TracedCommand) -> Self {
		let direction = match command.network_command {
			TracedBothDirectionCommand::C2S(_) => "c2s",
			TracedBothDirectionCommand::S2C(_) => "s2c",
		};

		let object_id = match command.network_command.get_object_id() {
			None => "none".to_string(),
			Some(id) => match &id.owner {
				GameObjectOwner::Room => {
					format!("root({})", id.id)
				}
				GameObjectOwner::Member(user) => {
					format!("user({},{})", user, id.id)
				}
			},
		};
		let template = command.template.map(|id| id as u32);
		let command_name: String = match &command.network_command {
			TracedBothDirectionCommand::C2S(command) => command.as_ref().to_string(),
			TracedBothDirectionCommand::S2C(command) => command.as_ref().to_string(),
		};
		let field_id = command
			.network_command
			.get_field_id()
			.map(|field_id| field_id as u32);
		let field_type = command
			.network_command
			.get_field_type()
			.map(|field_type| match field_type {
				FieldType::Long => shared::FieldType::Long,
				FieldType::Double => shared::FieldType::Double,
				FieldType::Structure => shared::FieldType::Structure,
				FieldType::Event => shared::FieldType::Event,
			})
			.map(|field_type| field_type as i32);
		let value = get_string_value(&command);

		Self {
			time: command.time,
			direction: direction.to_string(),
			command: command_name,
			object_id,
			user_id: command.user as u32,
			template,
			value,
			field_id,
			field_type,
		}
	}
}

fn get_string_value(command: &TracedCommand) -> String {
	match &command.network_command {
		TracedBothDirectionCommand::C2S(command) => match command {
			C2SCommand::CreateGameObject(command) => {
				format!(
					"access({:?}), template({:?}) ",
					command.access_groups.0, command.template
				)
			}
			C2SCommand::CreatedGameObject(command) => {
				format!(
					"room_owner({:?}), singleton_key({:?}) ",
					command.room_owner, command.singleton_key
				)
			}
			C2SCommand::SetField(command) => {
				format!("{:?}", command.value)
			}
			C2SCommand::IncrementLongValue(command) => {
				format!("{:?}", command.increment)
			}
			C2SCommand::CompareAndSetLong(command) => {
				format!(
					"new = {:?}, current = {:?}, reset = {:?}",
					command.new, command.current, command.reset
				)
			}
			C2SCommand::CompareAndSetStructure(command) => {
				format!(
					"new = {:?}, current = {:?}, reset = {:?}",
					command.new, command.current, command.reset
				)
			}
			C2SCommand::IncrementDouble(command) => {
				format!("{:?}", command.increment)
			}
			C2SCommand::Event(command) => {
				format!("{:?}", command.event.as_slice())
			}
			C2SCommand::TargetEvent(command) => {
				format!(
					"target_user = {:?}, value = {:?}",
					command.target, command.event.event
				)
			}
			C2SCommand::Delete(_) => "".to_string(),
			C2SCommand::AttachToRoom => "".to_string(),
			C2SCommand::DetachFromRoom => "".to_string(),
			C2SCommand::DeleteField(command) => {
				format!("field_type = {:?}", command.field_type)
			}
		},
		TracedBothDirectionCommand::S2C(command) => match command {
			S2CCommand::Create(command) => format!(
				"access({:?}), template({:?}) ",
				command.access_groups.0, command.template
			),
			S2CCommand::Created(_) => "".to_string(),
			S2CCommand::SetField(command) => format!("{:?}", command.value),
			S2CCommand::Event(command) => format!("{:?}", command.event),
			S2CCommand::Delete(_) => "".to_string(),
			S2CCommand::DeleteField(_) => "".to_string(),
		},
	}
}

#[cfg(test)]
pub mod test {
	use cheetah_matches_realtime_common::commands::binary_value::BinaryValue;
	use cheetah_matches_realtime_common::commands::c2s::C2SCommand;
	use cheetah_matches_realtime_common::commands::types::event::EventCommand;
	use cheetah_matches_realtime_common::room::object::GameObjectId;
	use cheetah_matches_realtime_common::room::owner::GameObjectOwner;

	use crate::debug::proto::admin;
	use crate::debug::proto::shared;
	use crate::debug::tracer::{TracedBothDirectionCommand, TracedCommand};

	#[test]
	pub fn should_convert() {
		let command = TracedCommand {
			time: 1.1,
			template: Some(155),
			user: 255,
			network_command: TracedBothDirectionCommand::C2S(C2SCommand::Event(EventCommand {
				object_id: GameObjectId::new(100, GameObjectOwner::Room),
				field_id: 555,
				event: BinaryValue::from(vec![10, 20, 30].as_slice()),
			})),
		};

		let grpc_command = admin::Command::from(command);
		assert_eq!(
			grpc_command,
			admin::Command {
				time: 1.1,
				direction: "c2s".to_string(),
				command: "Event".to_string(),
				object_id: "root(100)".to_string(),
				user_id: 255,
				template: Some(155),
				value: "[10, 20, 30]".to_string(),
				field_id: Some(555),
				field_type: Some(shared::FieldType::Event as i32)
			}
		)
	}

	#[test]
	pub fn should_convert_with_none_template_and_none_field() {
		let command = TracedCommand {
			time: 1.1,
			template: None,
			user: 255,
			network_command: TracedBothDirectionCommand::C2S(C2SCommand::AttachToRoom),
		};

		let grpc_command = admin::Command::from(command);
		assert_eq!(
			grpc_command,
			admin::Command {
				time: 1.1,
				direction: "c2s".to_string(),
				command: "AttachToRoom".to_string(),
				object_id: "none".to_string(),
				user_id: 255,
				template: None,
				value: "".to_string(),
				field_id: None,
				field_type: None
			}
		)
	}
}
