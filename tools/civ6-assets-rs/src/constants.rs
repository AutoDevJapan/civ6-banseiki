use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::LazyLock;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Seat {
    pub slug: String,
    pub ja: String,
    pub en: String,
    pub ru: String,
    pub castle: String,
    pub lon: f64,
    pub lat: f64,
    pub leader: String,
    pub civ: String,
    pub head_en: String,
    pub head_ru: String,
}
pub static CONSTANTS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../tests/fixtures/constants.json")).unwrap()
});
pub static SEATS: LazyLock<Vec<Seat>> = LazyLock::new(|| {
    let definitions: Vec<Seat> = serde_json::from_value(CONSTANTS["seats"].clone()).unwrap();
    civ6_data_rs::asset_dependencies::SEATS
        .into_iter()
        .map(|slug| {
            definitions
                .iter()
                .find(|seat| seat.slug == slug)
                .expect("seat geography")
                .clone()
        })
        .collect()
});
pub const SCALES: [i32; 2] = [20, 15];
pub const ISLAND_ANCHORS: [(&str, f64, f64); 5] = [
    ("Yonaguni", 123., 24.460),
    ("Ishigaki", 124.160, 24.340),
    ("Miyako", 125.280, 24.800),
    ("Tokunoshima", 128.990, 27.780),
    ("Amami", 129.490, 28.370),
];
pub fn seat_index(slug: &str) -> i32 {
    SEATS
        .iter()
        .position(|s| s.slug == slug)
        .expect("known faction") as i32
}
pub fn seat_castle(slug: &str) -> Option<&'static str> {
    CONSTANTS["seat_castles"][slug].as_str()
}
pub fn extra_capital(slug: &str) -> Option<&'static str> {
    match slug {
        "ODA" => Some("那古野城"),
        "DATE" => Some("米沢城"),
        "ASHIKAGA" => Some("室町殿"),
        _ => None,
    }
}
