use crate::*;
pub use constants::SEATS;
pub fn data() -> Result<Value> {
    read_json(root().join("documents/spec/data/japan-1534-start.json"))
}
pub fn leader_id(slug: &str) -> String {
    SEATS[constants::seat_index(slug) as usize].leader.clone()
}
pub fn civ_id(slug: &str) -> String {
    format!("CIVILIZATION_JAC_{slug}")
}
pub fn heads() -> Value {
    Value::Object(
        SEATS
            .iter()
            .map(|seat| {
                (
                    seat.slug.clone(),
                    serde_json::json!([
                        seat.leader
                            .strip_prefix(&format!("LEADER_JAC_{}_", seat.slug))
                            .unwrap(),
                        seat.head_en,
                        seat.head_ru
                    ]),
                )
            })
            .collect(),
    )
}
pub fn head_override(slug: &str) -> Option<&str> {
    match slug {
        "SHIMAZU" => Some("島津貴久"),
        "AINU" => Some("タリコナ"),
        _ => None,
    }
}
