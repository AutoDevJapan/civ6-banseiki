use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    Scenario,
    Tuner,
    LuaResponseUnknown,
    LuaExecution,
    Input,
    Capture,
    Io,
    Interrupted,
    LiveLease,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    pub fn scenario(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Scenario, message)
    }
    pub fn tuner(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Tuner, message)
    }
    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::LuaResponseUnknown, message)
    }
    pub fn lua(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::LuaExecution, message)
    }
    pub fn input(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Input, message)
    }
    pub fn capture(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Capture, message)
    }
    pub fn interrupted(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Interrupted, message)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::new(ErrorKind::Io, value.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::scenario(value.to_string())
    }
}
impl From<csv::Error> for Error {
    fn from(value: csv::Error) -> Self {
        Self::new(ErrorKind::Io, value.to_string())
    }
}
