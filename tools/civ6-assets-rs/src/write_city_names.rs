//! Compatibility interface around the shared production city-name generator.
use crate::*;
pub use civ6_data_rs::asset_dependencies::city_names::{
    gameplay_sql, names, pools, romanize, tag, RANDOM_DEPTH, SHARED_SORT_BASE,
};
use serde_json::json;
pub fn data() -> Result<Value> {
    civ6_data_rs::asset_dependencies::city_names::data(&civ6_data_rs::Repository::new(root()))
}
pub fn to_hiragana(text: &str) -> String {
    text.chars()
        .map(|c| {
            if ('ァ'..='ヶ').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap()
            } else {
                c
            }
        })
        .collect()
}
pub fn text_xml(source: &Value) -> Result<xmltree::Element> {
    let xml = civ6_data_rs::asset_dependencies::city_names::text_xml(source)?;
    Ok(xmltree::Element::parse(xml.as_bytes())?)
}
pub fn localized_records(source: &Value) -> Result<Value> {
    let xml = text_xml(source)?;
    Ok(json!(rows(&xml, "LocalizedText")
        .iter()
        .map(|r| json!({"tag":attr(r,"Tag"),"lang":attr(r,"Language"),"text":text_of(r,"Text")}))
        .collect::<Vec<_>>()))
}
pub fn main_to(output: &Path) -> Result<()> {
    let data = data()?;
    write(
        output.join("Data/JAC_CityNames_Gameplay.sql"),
        gameplay_sql(&data)?,
    )?;
    write(
        output.join("Text/JAC_CityNames_Text.xml"),
        civ6_data_rs::asset_dependencies::city_names::text_xml(&data)?,
    )
}
