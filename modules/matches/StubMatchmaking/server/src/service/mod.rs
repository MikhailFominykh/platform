use std::collections::HashMap;

use jwt_tonic_user_uuid::JWTUserTokenParser;
use tokio::sync::RwLock;
use tonic::transport::Uri;
use tonic::{Code, Request, Response, Status};
use uuid::Uuid;

use cheetah_libraries_microservice::trace::Trace;
use factory::internal::factory_client::FactoryClient;
use factory::internal::CreateMatchRequest;
use matchmaking::external::matchmaking_server::Matchmaking;
use matchmaking::external::{TicketRequest, TicketResponse};
use realtime::internal::CreateMemberRequest;

use crate::proto::matches::factory;
use crate::proto::matches::factory::internal::CreateMatchResponse;
use crate::proto::matches::matchmaking;
use crate::proto::matches::realtime;

pub struct StubMatchmakingService {
	pub jwt_public_key: String,
	pub factory_service_uri: Uri,
	pub matches: RwLock<HashMap<String, MatchInfo>>,
}

#[derive(Clone)]
pub struct MatchInfo {
	pub realtime_server_grpc_host: String,
	pub realtime_server_grpc_port: u16,
	pub realtime_server_host: String,
	pub realtime_server_port: u16,
	pub room_id: u64,
}

impl StubMatchmakingService {
	pub fn new(factory_service: Uri, jwt_public_key: String) -> Self {
		StubMatchmakingService {
			jwt_public_key,
			factory_service_uri: factory_service,
			matches: RwLock::new(HashMap::new()),
		}
	}
	#[async_recursion::async_recursion]
	async fn matchmaking(
		&self,
		ticket: TicketRequest,
		user_id: Uuid,
	) -> Result<TicketResponse, String> {
		let template = ticket.match_template.clone();
		let match_info = self.find_or_create_match(&template).await?;
		match StubMatchmakingService::attach_user(&ticket, &match_info).await {
			Ok(member_attach_response) => Ok(TicketResponse {
				private_key: member_attach_response.private_key,
				member_id: member_attach_response.user_id,
				room_id: match_info.room_id,
				realtime_server_host: match_info.realtime_server_host,
				realtime_server_port: match_info.realtime_server_port as u32,
			}),
			Err(e) => {
				tracing::error!("Cannot attach_user {}", e);
				// если  такой комнаты нет - то удаляем ее из существующих
				let mut matches = self.matches.write().await;
				matches.remove(&template);
				drop(matches);
				// и создаем снова
				self.matchmaking(ticket, user_id).await
			}
		}
	}

	async fn attach_user(
		ticket: &TicketRequest,
		match_info: &MatchInfo,
	) -> Result<realtime::internal::CreateMemberResponse, String> {
		let mut relay = realtime::internal::realtime_client::RealtimeClient::connect(
			cheetah_libraries_microservice::make_internal_srv_uri(
				match_info.realtime_server_grpc_host.as_str(),
				match_info.realtime_server_grpc_port,
			),
		)
		.await
		.map_err(|e| format!("Connect to relay error {:?}", e))?;

		match relay
			.create_member(Request::new(CreateMemberRequest {
				room_id: match_info.room_id,
				user: Some(realtime::internal::UserTemplate {
					groups: ticket.user_groups,
					objects: Default::default(),
				}),
			}))
			.await
		{
			Ok(user_attach_response) => Ok(user_attach_response.into_inner()),
			Err(status) => match status.code() {
				Code::NotFound => Err("Relay server not found".to_string()),
				e => Err(format!("Relay server unknown status {:?}", e)),
			},
		}
	}

	async fn find_or_create_match(&self, template: &str) -> Result<MatchInfo, String> {
		let mut matches = self.matches.write().await;
		match matches.get(template) {
			None => {
				let mut factory = FactoryClient::connect(self.factory_service_uri.clone())
					.await
					.unwrap();

				let create_match_response = factory
					.create_match(Request::new(CreateMatchRequest {
						template: template.to_string(),
					}))
					.await
					.map_err(|e| format!("Create match error {:?}", e))?
					.into_inner();
				let match_info = create_match_info(create_match_response);
				matches.insert(template.to_string(), match_info.clone());
				Ok(match_info)
			}
			Some(match_info) => Ok(match_info.clone()),
		}
	}
}

fn create_match_info(create_match_response: CreateMatchResponse) -> MatchInfo {
	let addrs = create_match_response.addrs.unwrap();
	let grpc_addr = addrs.grpc_internal.unwrap();
	let game_addr = addrs.game.unwrap();
	MatchInfo {
		realtime_server_grpc_host: grpc_addr.host,
		realtime_server_grpc_port: grpc_addr.port as u16,
		realtime_server_host: game_addr.host,
		realtime_server_port: game_addr.port as u16,
		room_id: create_match_response.id,
	}
}

#[tonic::async_trait]
impl Matchmaking for StubMatchmakingService {
	async fn matchmaking(
		&self,
		request: Request<TicketRequest>,
	) -> Result<Response<TicketResponse>, Status> {
		let user = JWTUserTokenParser::new(self.jwt_public_key.clone())
			.get_user_uuid_from_grpc(request.metadata())
			.trace_err(format!("Get user uuid {:?}", request.metadata()))
			.map_err(|_| Status::unauthenticated(""))?;

		let ticket_request = request.into_inner();
		self.matchmaking(ticket_request, user)
			.await
			.trace_err("Matchmaking error")
			.map_err(|_| Status::internal(""))
			.map(Response::new)
	}
}

#[cfg(test)]
pub mod tests {
	use tokio::net::TcpListener;
	use tokio::sync::RwLock;
	use tokio_stream::wrappers::ReceiverStream;
	use tonic::transport::Server;
	use tonic::{Request, Response, Status};

	use factory::internal::factory_server::Factory;
	use factory::internal::{CreateMatchRequest, CreateMatchResponse};
	use matchmaking::external::TicketRequest;
	use realtime::internal::CreateMemberResponse;

	use crate::proto::matches::factory;
	use crate::proto::matches::matchmaking;
	use crate::proto::matches::realtime;
	use crate::proto::matches::realtime::internal::{
		CreateMemberRequest, CreateSuperMemberRequest, EmptyRequest, ProbeRequest, ProbeResponse,
		RoomIdResponse,
	};
	use crate::proto::matches::registry::internal::{Addr, RelayAddrs};
	use crate::service::StubMatchmakingService;

	#[tokio::test]
	async fn should_create_match() {
		let matchmaking = setup(100).await;
		let response = matchmaking
			.matchmaking(
				TicketRequest {
					user_groups: 0,
					match_template: Default::default(),
				},
				Default::default(),
			)
			.await
			.unwrap();
		assert_eq!(response.room_id, StubFactory::ROOM_ID);
		assert_eq!(response.member_id, StubRealtimeService::MEMBER_ID);
	}

	///
	/// Повторный матчинг для одного и того же шаблона
	/// не должен привести к изменению id комнаты
	///
	#[tokio::test]
	async fn should_dont_create_match_if_exist() {
		let matchmaking = setup(100).await;
		matchmaking
			.matchmaking(
				TicketRequest {
					user_groups: Default::default(),
					match_template: "some-template".to_owned(),
				},
				Default::default(),
			)
			.await
			.unwrap();
		let response = matchmaking
			.matchmaking(
				TicketRequest {
					user_groups: Default::default(),
					match_template: "some-template".to_owned(),
				},
				Default::default(),
			)
			.await
			.unwrap();
		assert_eq!(response.room_id, StubFactory::ROOM_ID);
	}
	///
	/// Для каждого шаблона должен быть собственный матч     
	///
	#[tokio::test]
	async fn should_create_different_match_for_different_template() {
		let matchmaking = setup(100).await;
		let response_a = matchmaking
			.matchmaking(
				TicketRequest {
					user_groups: Default::default(),
					match_template: "some-template-a".to_owned(),
				},
				Default::default(),
			)
			.await
			.unwrap();
		let response_b = matchmaking
			.matchmaking(
				TicketRequest {
					user_groups: Default::default(),
					match_template: "some-template-b".to_owned(),
				},
				Default::default(),
			)
			.await
			.unwrap();
		assert_eq!(response_a.room_id, StubFactory::ROOM_ID);
		assert_eq!(response_b.room_id, StubFactory::ROOM_ID + 1);
	}

	///
	/// Для каждого шаблона должен быть собственный матч     
	///
	#[tokio::test]
	async fn should_recreate_match_if_not_found() {
		let matchmaking = setup(1).await;
		let response_a = matchmaking
			.matchmaking(
				TicketRequest {
					user_groups: Default::default(),
					match_template: "some-template".to_owned(),
				},
				Default::default(),
			)
			.await
			.unwrap();
		let response_b = matchmaking
			.matchmaking(
				TicketRequest {
					user_groups: Default::default(),
					match_template: "some-template".to_owned(),
				},
				Default::default(),
			)
			.await
			.unwrap();
		assert_eq!(response_a.room_id, StubFactory::ROOM_ID);
		assert_eq!(response_b.room_id, StubFactory::ROOM_ID + 1);
	}

	async fn setup(fail_create_user: i8) -> StubMatchmakingService {
		let stub_grpc_service_tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let stub_grpc_service_addr = stub_grpc_service_tcp.local_addr().unwrap();

		let stub_factory = StubFactory {
			relay_grpc_host: stub_grpc_service_addr.ip().to_string(),
			relay_grpc_port: stub_grpc_service_addr.port(),
			room_sequence: RwLock::new(0),
		};
		let stub_relay = StubRealtimeService {
			fail_when_zero: RwLock::new(fail_create_user),
		};
		tokio::spawn(async move {
			Server::builder()
				.add_service(factory::internal::factory_server::FactoryServer::new(
					stub_factory,
				))
				.add_service(realtime::internal::realtime_server::RealtimeServer::new(
					stub_relay,
				))
				.serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(
					stub_grpc_service_tcp,
				))
				.await
		});

		let matchmaking = StubMatchmakingService::new(
			cheetah_libraries_microservice::make_internal_srv_uri(
				stub_grpc_service_addr.ip().to_string().as_str(),
				stub_grpc_service_addr.port(),
			),
			Default::default(),
		);
		matchmaking
	}

	struct StubFactory {
		pub relay_grpc_host: String,
		pub relay_grpc_port: u16,
		pub room_sequence: RwLock<u16>,
	}

	impl StubFactory {
		pub const ROOM_ID: u64 = 555;
	}
	#[tonic::async_trait]
	impl Factory for StubFactory {
		async fn create_match(
			&self,
			_request: Request<CreateMatchRequest>,
		) -> Result<Response<CreateMatchResponse>, Status> {
			let mut sequence = self.room_sequence.write().await;
			let current_seq = *sequence;
			*sequence += 1;
			Ok(Response::new(CreateMatchResponse {
				addrs: Some(RelayAddrs {
					// not used
					game: Some(Addr {
						host: "127.0.0.1".to_string(),
						port: 0,
					}),
					grpc_internal: Some(Addr {
						host: self.relay_grpc_host.clone(),
						port: self.relay_grpc_port as u32,
					}),
				}),
				id: StubFactory::ROOM_ID + current_seq as u64,
			}))
		}
	}

	struct StubRealtimeService {
		pub fail_when_zero: RwLock<i8>,
	}
	impl StubRealtimeService {
		pub const MEMBER_ID: u32 = 777;
	}
	#[tonic::async_trait]
	impl realtime::internal::realtime_server::Realtime for StubRealtimeService {
		async fn create_room(
			&self,
			_request: Request<realtime::internal::RoomTemplate>,
		) -> Result<Response<realtime::internal::RoomIdResponse>, Status> {
			todo!()
		}

		async fn create_member(
			&self,
			_request: Request<CreateMemberRequest>,
		) -> Result<Response<CreateMemberResponse>, Status> {
			let mut fail = self.fail_when_zero.write().await;
			let current = *fail;
			*fail -= 1;
			if current == 0 {
				Err(Status::not_found(""))
			} else {
				Ok(Response::new(CreateMemberResponse {
					user_id: StubRealtimeService::MEMBER_ID,
					private_key: vec![],
				}))
			}
		}

		async fn create_super_member(
			&self,
			_request: Request<CreateSuperMemberRequest>,
		) -> Result<Response<CreateMemberResponse>, Status> {
			todo!()
		}

		async fn probe(
			&self,
			_request: Request<ProbeRequest>,
		) -> Result<Response<ProbeResponse>, Status> {
			Ok(Response::new(ProbeResponse {}))
		}

		type WatchCreatedRoomEventStream = ReceiverStream<Result<RoomIdResponse, Status>>;

		async fn watch_created_room_event(
			&self,
			_request: Request<EmptyRequest>,
		) -> Result<Response<Self::WatchCreatedRoomEventStream>, Status> {
			todo!()
		}
	}
}
