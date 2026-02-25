use std::sync::OnceLock;

use anyhow::{Result, bail};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error,
    Info,
    Debug,
}

static LOG_LEVEL: OnceLock<LogLevel> = OnceLock::new();

pub fn init(level: LogLevel) {
    // Ignore repeated init calls so tests and main can both call safely.
    let _ = LOG_LEVEL.set(level);
}

pub fn error(component: &str, message: &str) {
    if enabled(LogLevel::Error) {
        eprintln!("[{}] ERROR {}", component, message);
    }
}

pub fn info(component: &str, message: &str) {
    if enabled(LogLevel::Info) {
        eprintln!("[{}] INFO {}", component, message);
    }
}

pub fn debug(component: &str, message: &str) {
    if enabled(LogLevel::Debug) {
        eprintln!("[{}] DEBUG {}", component, message);
    }
}

pub fn enabled(level: LogLevel) -> bool {
    let configured = LOG_LEVEL.get().copied().unwrap_or(LogLevel::Info);
    level <= configured
}

impl LogLevel {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "error" => Ok(Self::Error),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            _ => bail!(
                "invalid log_level value: {} (expected: error|info|debug)",
                value
            ),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
}
