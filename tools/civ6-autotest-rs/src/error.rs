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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Rust's IO display keeps the numeric OS code even with localized text.
    /// Only a terminal Windows sharing/lock code is a temporary sharing error.
    pub fn is_windows_sharing_violation(&self) -> bool {
        self.kind == ErrorKind::Io
            && (self.message.ends_with("(os error 32)") || self.message.ends_with("(os error 33)"))
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sharing_retry_never_turns_permission_or_cancellation_into_transient_io() {
        for code in [32, 33] {
            assert!(
                Error::new(ErrorKind::Io, format!("共有できません (os error {code})"))
                    .is_windows_sharing_violation()
            );
        }
        for (kind, message) in [
            (ErrorKind::Io, "denied (os error 5)"),
            (ErrorKind::Interrupted, "cancelled (os error 32)"),
            (ErrorKind::Input, "unknown record (os error 33)"),
            (ErrorKind::Io, "old (os error 32); now denied"),
            (ErrorKind::Io, "other (os error 320)"),
        ] {
            assert!(!Error::new(kind, message).is_windows_sharing_violation());
        }
    }
}
