//! Captures `tracing` log events (bevy's logging) into a shared buffer so the
//! editor's Console panel can display them, while bevy's own stdout logging
//! keeps working.

use bevy::app::App;
use bevy::log::tracing::{self, Subscriber};
use bevy::log::tracing_subscriber::layer::Context;
use bevy::log::tracing_subscriber::{Layer, Registry};
use bevy::log::BoxedLayer;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct LogLine {
    /// Uptime when the line was emitted, formatted mm:ss.mmm.
    pub time: String,
    pub level: tracing::Level,
    pub target: String,
    pub message: String,
}

impl LogLine {
    /// Shortened tracing target, e.g. `bevy_render::camera` -> `camera`.
    pub fn target_short(&self) -> String {
        self.target
            .rsplit("::")
            .next()
            .unwrap_or(&self.target)
            .chars()
            .take(18)
            .collect()
    }
}

static BUFFER: OnceLock<Arc<Mutex<Vec<LogLine>>>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();

const MAX_LINES: usize = 3000;

/// Global log buffer shared between the tracing layer and the UI.
pub fn log_buffer() -> Arc<Mutex<Vec<LogLine>>> {
    BUFFER.get_or_init(|| {
        START.get_or_init(Instant::now);
        Arc::new(Mutex::new(Vec::new()))
    })
    .clone()
}

fn format_uptime(d: Duration) -> String {
    let ms = d.as_millis();
    format!("{:02}:{:02}.{:03}", ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

pub struct EditorLogLayer {
    buffer: Arc<Mutex<Vec<LogLine>>>,
}

impl EditorLogLayer {
    fn new() -> Self {
        Self {
            buffer: log_buffer(),
        }
    }
}

impl<S> Layer<S> for EditorLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut message = String::new();
        event.record(&mut MessageVisitor(&mut message));
        let start = START.get_or_init(Instant::now);
        let line = LogLine {
            time: format_uptime(start.elapsed()),
            level: *event.metadata().level(),
            target: event.metadata().target().to_string(),
            message,
        };
        if let Ok(mut buf) = self.buffer.lock() {
            buf.push(line);
            let len = buf.len();
            if len > MAX_LINES {
                buf.drain(0..len - MAX_LINES);
            }
        }
    }
}

struct MessageVisitor<'a>(&'a mut String);

impl tracing::field::Visit for MessageVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            use std::fmt::Write;
            let _ = write!(self.0, "{value:?}");
        }
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0.push_str(value);
        }
    }
}

/// Hook passed to bevy's `LogPlugin::custom_layer`.
pub fn editor_log_layer(_app: &mut App) -> Option<BoxedLayer> {
    Some(Box::new(EditorLogLayer::new()))
}
