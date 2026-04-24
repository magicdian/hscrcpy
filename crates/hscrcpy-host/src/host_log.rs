use std::fmt;
use std::io::{self, Write};
use std::sync::OnceLock;

const LOG_ENV: &str = "HSCRCPY_LOG";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HostLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Off,
}

impl HostLogLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Off => "off",
        }
    }

    fn from_env_value(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warn" | "warning" => Self::Warn,
            "error" => Self::Error,
            "off" | "none" | "silent" => Self::Off,
            _ => Self::Debug,
        }
    }
}

impl fmt::Display for HostLogLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn configured_level() -> HostLogLevel {
    static LEVEL: OnceLock<HostLogLevel> = OnceLock::new();
    *LEVEL.get_or_init(|| {
        std::env::var(LOG_ENV)
            .map(|raw| HostLogLevel::from_env_value(&raw))
            .unwrap_or(HostLogLevel::Debug)
    })
}

pub fn enabled(level: HostLogLevel) -> bool {
    let configured = configured_level();
    configured != HostLogLevel::Off && level >= configured
}

pub fn emit(level: HostLogLevel, subsystem: &str, operation: &str, message: impl AsRef<str>) {
    if !enabled(level) {
        return;
    }
    println!(
        "log level={} subsystem={} operation={} {}",
        level,
        subsystem,
        operation,
        message.as_ref()
    );
    let _ = io::stdout().flush();
}

pub fn trace(subsystem: &str, operation: &str, message: impl AsRef<str>) {
    emit(HostLogLevel::Trace, subsystem, operation, message);
}

pub fn debug(subsystem: &str, operation: &str, message: impl AsRef<str>) {
    emit(HostLogLevel::Debug, subsystem, operation, message);
}

pub fn info(subsystem: &str, operation: &str, message: impl AsRef<str>) {
    emit(HostLogLevel::Info, subsystem, operation, message);
}

pub fn warn(subsystem: &str, operation: &str, message: impl AsRef<str>) {
    emit(HostLogLevel::Warn, subsystem, operation, message);
}

pub fn error(subsystem: &str, operation: &str, message: impl AsRef<str>) {
    emit(HostLogLevel::Error, subsystem, operation, message);
}
