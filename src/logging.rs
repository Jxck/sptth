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

#[cfg(test)]
mod tests {
    use super::LogLevel;

    #[test]
    fn parse_valid_levels() {
        assert_eq!(LogLevel::parse("error").unwrap(), LogLevel::Error);
        assert_eq!(LogLevel::parse("info").unwrap(), LogLevel::Info);
        assert_eq!(LogLevel::parse("debug").unwrap(), LogLevel::Debug);
    }

    #[test]
    fn parse_invalid_level() {
        let err = LogLevel::parse("warn").expect_err("should fail for unknown level");
        assert!(err.to_string().contains("invalid log_level"));
    }

    #[test]
    fn level_ordering() {
        assert!(LogLevel::Error < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Debug);
    }

    #[test]
    fn as_str_roundtrip() {
        for level in [LogLevel::Error, LogLevel::Info, LogLevel::Debug] {
            assert_eq!(LogLevel::parse(level.as_str()).unwrap(), level);
        }
    }
}
