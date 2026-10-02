//! Compatibility interface around the shared production diplomacy generator.
use crate::*;
pub use civ6_data_rs::asset_dependencies::feuds::{
    applies, date_key, ACCESS_MODIFIER, ACCESS_SOURCE, ACCESS_TRAIT, ALLY_KINDS,
    ALLY_VISION_MODIFIER, ALLY_VISION_TRAIT, PENALTY, SAMPLE_TAG, TEXT_TAG,
};
pub fn relation_pairs(kinds: &[&str]) -> Result<Vec<(String, String)>> {
    let source =
        civ6_data_rs::asset_dependencies::feuds::data(&civ6_data_rs::Repository::new(root()))?;
    relation_pairs_from(&source, kinds)
}
pub fn relation_pairs_from(source: &Value, kinds: &[&str]) -> Result<Vec<(String, String)>> {
    civ6_data_rs::asset_dependencies::feuds::relation_pairs(source, kinds)
}
pub fn feud_pairs() -> Result<Vec<(String, String)>> {
    civ6_data_rs::asset_dependencies::feuds::feud_pairs(&civ6_data_rs::Repository::new(root()))
}
pub fn build() -> Result<xmltree::Element> {
    let xml =
        civ6_data_rs::asset_dependencies::feuds::build(&civ6_data_rs::Repository::new(root()))?;
    Ok(xmltree::Element::parse(xml.as_bytes())?)
}
