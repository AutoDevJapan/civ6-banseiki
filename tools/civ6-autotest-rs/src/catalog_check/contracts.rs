//! Explicit wire versions. Never regenerate an accepted contract from current
//! catalog_scenario code, and never normalize a submitted Lua code string.
use super::{digest, json, require};
use crate::Result;
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SourceContractVersion {
    Legacy4cd,
    NilAdjusted068,
}

pub(super) struct Contract {
    pub version: SourceContractVersion,
    pub data: Value,
    pub sha256: &'static str,
}

impl Contract {
    pub fn load(version: SourceContractVersion) -> Result<Self> {
        let (raw, sha256) = match version {
            SourceContractVersion::Legacy4cd => (
                include_str!("../../tests/fixtures/catalog-check/source-contract.json"),
                "2ce6c4df27af3f82a9fd4f9f8d4bce52ddfba604241b4e17a1258d8d80d0f3f2",
            ),
            SourceContractVersion::NilAdjusted068 => (
                include_str!(
                    "../../tests/fixtures/catalog-check/source-contract-nil-adjusted-068.json"
                ),
                "cbebf7fe0cb633e8d541205f667762b637c4cadd3ca19e90337a9738bec67e42",
            ),
        };
        require(
            digest(raw) == sha256,
            "embedded source contract hash is outside the explicit allowlist",
        )?;
        Ok(Self {
            version,
            data: json(raw)?,
            sha256,
        })
    }
}
