use log::{LevelFilter, Log, Metadata, Record};
use std::sync::Mutex;

/// A simple in-memory log collector that also delegates to an inner logger.
/// Stored messages can be exported on demand via `drain()`.
static BUFFER: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct BufferLogger {
    inner: Box<dyn Log>,
    level: LevelFilter,
}

impl Log for BufferLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level || self.inner.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if record.level() <= self.level {
            let line = format!(
                "[{}] {} — {}",
                record.level(),
                record.target(),
                record.args()
            );
            if let Ok(mut buf) = BUFFER.lock() {
                buf.push(line);
            }
        }
        self.inner.log(record);
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

/// Initialise the buffer logger, wrapping `inner` (e.g. env_logger).
/// Call once at startup.
pub fn init(inner: Box<dyn Log>, max_level: LevelFilter) {
    let logger = BufferLogger {
        inner,
        level: max_level,
    };
    log::set_max_level(max_level);
    log::set_boxed_logger(Box::new(logger)).ok();
}

/// A minimal logger that forwards to the browser console via `web_sys`.
#[cfg(target_arch = "wasm32")]
struct WebConsoleLogger;

#[cfg(target_arch = "wasm32")]
impl Log for WebConsoleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        let msg = format!("[{}] {} — {}", record.level(), record.target(), record.args());
        match record.level() {
            log::Level::Error => web_sys::console::error_1(&msg.into()),
            log::Level::Warn => web_sys::console::warn_1(&msg.into()),
            _ => web_sys::console::log_1(&msg.into()),
        }
    }
    fn flush(&self) {}
}

/// Convenience: initialise buffer + web console logger for WASM.
#[cfg(target_arch = "wasm32")]
pub fn init_wasm(max_level: LevelFilter) {
    init(Box::new(WebConsoleLogger), max_level);
}

/// Return all buffered log lines and clear the buffer.
pub fn drain() -> Vec<String> {
    if let Ok(mut buf) = BUFFER.lock() {
        std::mem::take(&mut *buf)
    } else {
        Vec::new()
    }
}
