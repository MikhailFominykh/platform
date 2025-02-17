use std::collections::VecDeque;
use std::sync::Mutex;

use lazy_static::lazy_static;
use tracing::{log, Event};
use tracing_core::field::Visit;
use tracing_core::{Field, LevelFilter};
use tracing_log::LogTracer;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::{fmt, Layer, Registry};

use crate::ffi::logs::LogLevel;

lazy_static! {
	pub static ref TRACER_COLLECTOR: Mutex<TracerCollector> = Mutex::new(TracerCollector::setup());
}

///
/// Сохранение трейсов для передачи в Unity/Unreal/etc
///
#[derive(Debug)]
pub struct TracerCollector {
	pub level: tracing_core::Level,
	pub items: VecDeque<Trace>,
}

#[derive(Debug, Clone)]
pub struct Trace {
	pub level: LogLevel,
	pub message: String,
}

impl TracerCollector {
	fn setup() -> Self {
		LogTracer::builder()
			.with_max_level(log::LevelFilter::Info)
			.init()
			.unwrap();
		let fmt_layer = fmt::layer().with_target(false);
		let subscriber = Registry::default()
			.with(fmt_layer)
			.with(TracerCollectorLayer);
		tracing::subscriber::set_global_default(subscriber)
			.expect("Setting default subscriber failed");
		Self {
			level: tracing_core::Level::INFO,
			items: Default::default(),
		}
	}

	pub fn set_log_level(&mut self, log_level: LogLevel) {
		self.level = match log_level {
			LogLevel::Info => tracing_core::Level::INFO,
			LogLevel::Warn => tracing_core::Level::WARN,
			LogLevel::Error => tracing_core::Level::ERROR,
		};
	}

	fn on_event(&mut self, event: &Event<'_>) {
		let filter = LevelFilter::from_level(self.level);
		if filter >= *event.metadata().level() {
			let mut visitor = ValueVisitor::new("message");
			event.record(&mut visitor);
			let message = visitor.result.unwrap_or_else(|| "".to_string());
			let level = match *event.metadata().level() {
				tracing_core::Level::INFO => LogLevel::Info,
				tracing_core::Level::WARN => LogLevel::Warn,
				tracing_core::Level::ERROR => LogLevel::Error,
				tracing_core::Level::DEBUG => LogLevel::Info,
				tracing_core::Level::TRACE => LogLevel::Info,
			};
			let message = format!(
				"{} in {}:{}",
				message,
				event.metadata().file().unwrap_or(""),
				event.metadata().line().unwrap_or(0)
			);
			self.items.push_back(Trace { level, message })
		}
	}
}

struct TracerCollectorLayer;

impl<S: tracing::Subscriber> Layer<S> for TracerCollectorLayer {
	fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
		let collector = &mut TRACER_COLLECTOR.lock().unwrap();
		collector.on_event(event);
	}
}

#[derive(Default)]
pub struct ValueVisitor {
	name: String,
	result: Option<String>,
}

impl ValueVisitor {
	pub fn new<S: AsRef<str>>(name: S) -> Self {
		Self {
			name: name.as_ref().to_string(),
			result: None,
		}
	}
}

impl<'a> Visit for ValueVisitor {
	fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
		if field.name() == self.name {
			self.result = Some(format!("{:?}", value));
		}
	}
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;
	use std::sync::{LockResult, Mutex, MutexGuard};

	use lazy_static::lazy_static;

	use crate::ffi::logs::LogLevel;
	use crate::tracer::TRACER_COLLECTOR;

	lazy_static! {
		pub static ref LOCK: Mutex<()> = Mutex::new(());
	}

	#[test]
	fn should_collect_trace() {
		let _lock = setup(LogLevel::Error);
		tracing::error!("some error");

		let mut path = PathBuf::new();
		for v in ["matches", "Realtime", "client", "Rust", "src", "tracer.rs"] {
			path.push(v);
		}
		let view_path = path.display();
		let error = format!("some error in {view_path}");

		assert!(contains(&error));
	}

	#[test]
	fn should_not_collect_trace_if_wrong_level() {
		let _lock = setup(LogLevel::Error);
		let msg = "should_not_collect_trace_if_wrong_level";
		tracing::info!("{}", msg);
		assert!(!contains(msg));
	}

	#[test]
	fn should_set_level() {
		let _lock = setup(LogLevel::Info);
		let msg = "should_set_level";
		tracing::info!("{}", msg);
		assert!(contains(msg));
	}

	fn setup(log_level: LogLevel) -> LockResult<MutexGuard<'static, ()>> {
		let lock = LOCK.lock();
		{
			let collector = &mut TRACER_COLLECTOR.lock().unwrap();
			collector.set_log_level(log_level)
		}
		lock
	}

	fn contains(item: &str) -> bool {
		let items = &TRACER_COLLECTOR.lock().unwrap().items;
		items.iter().any(|t| t.message.contains(item))
	}
}
