//! Minimal Civ6 Lua glue emitted by the Rust generator.
//! The official InGame loader creates add-in contexts with isHidden=true.
pub fn initialize(panel: &str) -> String {
    const INITIALIZER: &str = "ContextPtr:SetHide(false)\n";
    format!("{INITIALIZER}{}", panel.trim_start_matches(INITIALIZER))
}
