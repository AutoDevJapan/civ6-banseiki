//! Minimal Civ6 Lua glue emitted by the Rust generator.
//! The official InGame loader creates add-in contexts with isHidden=true.
pub fn initialize(panel: &str) -> String {
    const INITIALIZER: &str = "ContextPtr:SetHide(false)\n";
    format!("{INITIALIZER}{}", panel.trim_start_matches(INITIALIZER))
}

#[cfg(test)]
mod tests {
    use super::initialize;

    #[test]
    fn repeated_generation_preserves_body_and_one_engine_initializer() {
        let body = "-- existing panel\ninclude 'InstanceManager'\nRefresh()\n";
        let expected = format!("ContextPtr:SetHide(false)\n{body}");
        for input in [
            body.to_string(),
            expected.clone(),
            format!("ContextPtr:SetHide(false)\n{expected}"),
        ] {
            let first = initialize(&input);
            assert_eq!(first, expected);
            assert_eq!(initialize(&first), expected);
        }
    }
}
