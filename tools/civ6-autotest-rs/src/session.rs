use crate::{Error, Result};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionType {
    SinglePlayer,
    Hotseat,
}
impl SessionType {
    pub fn from_scenario(data: &Value) -> Result<Self> {
        match data.get("session_type") {
            None => Ok(Self::SinglePlayer),
            Some(v) if v == "single_player" => Ok(Self::SinglePlayer),
            Some(v) if v == "hotseat" => Ok(Self::Hotseat),
            _ => Err(Error::scenario(
                "session_type must be single_player or hotseat",
            )),
        }
    }
    pub fn save_folder(self) -> &'static str {
        match self {
            Self::SinglePlayer => "Single",
            Self::Hotseat => "Multi",
        }
    }
    pub fn server_type(self) -> &'static str {
        match self {
            Self::SinglePlayer => "SERVER_TYPE_NONE",
            Self::Hotseat => "SERVER_TYPE_HOTSEAT",
        }
    }
    pub fn guard_code(self) -> &'static str {
        match self {
            Self::SinglePlayer => "if GameConfiguration.IsAnyMultiplayer() then error('single-player session required') end",
            Self::Hotseat => "if not GameConfiguration.IsHotseat() then error('hotseat session required') end",
        }
    }
}
