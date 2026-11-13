//! Read the existing Steam error window; never invoke controls or game code.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = civ6_autotest_rs::steam_dialog::Session::begin_if_running()?
        .ok_or("registered Steam process absent")?;
    let mut view = civ6_autotest_rs::steam_dialog::probe(&session)?;
    let windows = view["windows"].as_array_mut().ok_or("windows missing")?;
    windows.retain(|w| {
        w["title"]
            .as_str()
            .is_some_and(|title| title.eq_ignore_ascii_case("Steam - Game Launch Failed"))
    });
    println!("{}", serde_json::to_string_pretty(&view)?);
    Ok(())
}
