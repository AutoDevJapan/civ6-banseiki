//! Rust port of `build_scenario` from the shipped Assassin scenario generator.
//! The numbered Lua fixtures are the generator's original triple-quoted literals.
//! A visible `␠` encodes each trailing space; decode it before concatenation.

use civ6_lua_tests_rs::read_repo;
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Number;
use std::fmt;

const COURT_SCENARIO: &str = "tools/civ6-autotest/scenarios/court-arbitration.json";
const ORIGINAL_GENERATOR: &str = "tools/generate_assassin_poc.py";
const MOD_ID: &str = "fbb5219d-d7b9-4b31-884f-057f623476a0";
const COURT_ID: &str = "4785cfe4-8d9a-4deb-89ea-d2d566551fec";
const EXTRA_DISABLED_ID: &str = "57a3af09-fb38-41da-b367-818de11a995c";
const PREFIX: &str = "local R=ExposedMembers.JAC_Assassin; assert(R,'Assassin gameplay absent'); local t=Game:GetProperty('JAC_ASN_LIVE_TEST'); ";

// serde_json::Map sorts keys with this crate's enabled features. Python's
// json.dumps preserves insertion order, including the two copied court steps.
// Keep that order through deserialization, mutation, and serialization.
#[derive(Clone, Debug, PartialEq)]
enum Ordered {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Self>),
    Object(Vec<(String, Self)>),
}

impl Ordered {
    fn object(entries: impl IntoIterator<Item = (&'static str, Self)>) -> Self {
        Self::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        )
    }

    fn get(&self, key: &str) -> &Self {
        match self {
            Self::Object(entries) => &entries.iter().find(|(k, _)| k == key).unwrap().1,
            _ => panic!("{key} is not an object field"),
        }
    }

    fn get_mut(&mut self, key: &str) -> &mut Self {
        match self {
            Self::Object(entries) => &mut entries.iter_mut().find(|(k, _)| k == key).unwrap().1,
            _ => panic!("{key} is not an object field"),
        }
    }

    fn array(&self) -> &Vec<Self> {
        match self {
            Self::Array(values) => values,
            _ => panic!("expected an array"),
        }
    }

    fn array_mut(&mut self) -> &mut Vec<Self> {
        match self {
            Self::Array(values) => values,
            _ => panic!("expected an array"),
        }
    }

    fn string(&self) -> &str {
        match self {
            Self::String(value) => value,
            _ => panic!("expected a string"),
        }
    }
}

impl From<&str> for Ordered {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for Ordered {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<i64> for Ordered {
    fn from(value: i64) -> Self {
        Self::Number(value.into())
    }
}

impl From<bool> for Ordered {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor;
        impl<'de> Visitor<'de> for OrderedVisitor {
            type Value = Ordered;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any JSON value")
            }

            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(Ordered::Bool(value))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(Ordered::Number(value.into()))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(Ordered::Number(value.into()))
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                Ok(Ordered::Number(Number::from_f64(value).unwrap()))
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(Ordered::String(value.to_owned()))
            }

            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(Ordered::String(value))
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(Ordered::Null)
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(Ordered::Null)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element()? {
                    values.push(value);
                }
                Ok(Ordered::Array(values))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(Ordered::Object(entries))
            }
        }
        deserializer.deserialize_any(OrderedVisitor)
    }
}

impl Serialize for Ordered {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Number(value) => value.serialize(serializer),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(values) => {
                let mut seq = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    seq.serialize_element(value)?;
                }
                seq.end()
            }
            Self::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

fn template(number: u8) -> String {
    read_repo(format!(
        "tools/civ6-lua-tests-rs/fixtures/assassin/scenario/{number:02}.lua"
    ))
    .replace("\r\n", "\n")
    .replace('␠', " ")
}

fn verify_original_templates() {
    let source = read_repo(ORIGINAL_GENERATOR).replace("\r\n", "\n");
    let active_start = source.find("def active():").unwrap();
    let build_start = source.find("def build_scenario():").unwrap();
    let active_source = &source[active_start..build_start];
    let open = active_source.find("\"\"\"").unwrap() + 3;
    let close = open + active_source[open..].find("\"\"\"").unwrap();
    let active_literal = &active_source[open..close];
    let active_fixture = read_repo("tools/civ6-lua-tests-rs/fixtures/assassin/scenario/active.lua")
        .replace("\r\n", "\n")
        .replace('␠', " ");
    assert_eq!(
        active_fixture, active_literal,
        "active-turn Lua template changed"
    );

    let build_end = source.find("\ndef scenarios():").unwrap();
    let scope = &source[build_start..build_end];
    let mut cursor = 0;
    let mut number = 0;
    while let Some(relative_start) = scope[cursor..].find("\"\"\"") {
        let literal_start = cursor + relative_start + 3;
        let literal_end = literal_start + scope[literal_start..].find("\"\"\"").unwrap();
        if number > 0 {
            assert_eq!(
                template(number),
                scope[literal_start..literal_end],
                "Lua template {number} changed"
            );
        }
        number += 1;
        cursor = literal_end + 3;
    }
    assert_eq!(number, 45, "unexpected Lua template count");
}

fn active() -> Ordered {
    Ordered::object([
        ("action", "wait_for".into()),
        ("timeout", 600_i64.into()),
        (
            "condition",
            Ordered::object([
                ("type", "lua".into()),
                ("state_name", "InGame".into()),
                (
                    "code",
                    read_repo("tools/civ6-lua-tests-rs/fixtures/assassin/scenario/active.lua")
                        .replace("\r\n", "\n")
                        .replace('␠', " ")
                        .into(),
                ),
            ]),
        ),
    ])
}

struct Builder {
    steps: Vec<Ordered>,
    grant: String,
    equality: String,
    trace: String,
}

impl Builder {
    fn new() -> Self {
        let source: Ordered = serde_json::from_str(&read_repo(COURT_SCENARIO)).unwrap();
        let source_steps = source.get("steps").array();
        let mut enable = source_steps[0].clone();
        enable
            .get_mut("ids")
            .array_mut()
            .retain(|id| id.string() != COURT_ID);
        enable.get_mut("ids").array_mut().push(MOD_ID.into());
        enable
            .get_mut("disable_ids")
            .array_mut()
            .extend([COURT_ID.into(), EXTRA_DISABLED_ID.into()]);
        let mut configure = source_steps[1].clone();
        *configure.get_mut("seed") = 620995_i64.into();
        for player in configure.get_mut("players").array_mut() {
            let human = player.get("civ").string() == "CIVILIZATION_JAC_ODA";
            *player.get_mut("human") = human.into();
        }
        Self {
            steps: vec![
                enable,
                configure,
                Ordered::object([("action", "start_game".into()), ("timeout", 300_i64.into())]),
                active(),
            ],
            grant: template(1),
            equality: template(2),
            trace: template(3),
        }
    }

    fn check(&mut self, code: &str, state: &str) {
        let flattened = code.lines().map(str::trim).collect::<Vec<_>>().join(" ");
        self.steps.push(Ordered::object([
            ("action", "ui_lua".into()),
            ("state_name", state.into()),
            (
                "code",
                format!(
                    "{PREFIX}{}{}{}{flattened}",
                    self.grant, self.equality, self.trace
                )
                .into(),
            ),
        ]));
    }

    fn game_check(&mut self, code: &str) {
        self.check(code, "GameCore_Tuner");
    }

    fn advance(&mut self, turns: i64) {
        self.steps.push(Ordered::object([
            ("action", "end_turn".into()),
            ("turns", turns.into()),
            ("timeout", std::cmp::max(600, turns * 240).into()),
        ]));
        self.steps.push(active());
    }

    fn round_trip(&mut self, name: &str) {
        self.steps.extend([
            Ordered::object([
                ("action", "save".into()),
                ("name", name.into()),
                ("timeout", 60_i64.into()),
            ]),
            Ordered::object([
                ("action", "exit_to_menu".into()),
                ("timeout", 60_i64.into()),
            ]),
            Ordered::object([
                ("action", "load".into()),
                ("name", name.into()),
                ("timeout", 300_i64.into()),
            ]),
            active(),
        ]);
    }

    fn branch(&mut self, branch: &str, house: &str) {
        let mut command = format!("{}{}{}", template(16), house, template(17));
        match branch {
            "SUCCESSION" => command.push_str(&template(18)),
            "ABSORPTION" => command.push_str(&template(19)),
            "CONGRESS" => command.push_str(&template(20)),
            _ => unreachable!(),
        }
        command.push_str(&template(21));
        command.push_str(branch);
        command.push_str(&template(22));
        if branch == "ABSORPTION" {
            command.push_str(&template(23));
        }
        if branch == "CONGRESS" {
            command.push_str(&template(24));
        }
        command.push_str(&template(25));
        command.push_str(branch);
        command.push_str(&template(26));
        self.game_check(&command);
        self.advance(1);

        let mut result = format!("{}{}{}", template(27), branch, template(28));
        match branch {
            "SUCCESSION" => result.push_str(&template(29)),
            "ABSORPTION" => result.push_str(&template(30)),
            "CONGRESS" => result.push_str(&template(31)),
            _ => unreachable!(),
        }
        result.push_str(&template(32));
        result.push_str(branch);
        result.push_str(&template(33));
        self.game_check(&result);
    }

    fn lost_actor(&mut self, index: usize, success: i64, detected: i64) {
        if index > 0 {
            self.game_check(&template(34));
            self.advance(1);
        }
        let command = format!(
            "{}{success},Detected={detected}{}{success}, detected={detected}{}",
            template(35),
            template(36),
            template(37)
        );
        self.game_check(&command);
        self.advance(1);
        self.game_check(&template(38));
    }
}

pub fn build_scenario() -> serde_json::Value {
    serde_json::to_value(build_ordered_scenario()).unwrap()
}

pub fn build_scenario_text() -> String {
    let mut result = serde_json::to_string_pretty(&build_ordered_scenario()).unwrap();
    result.push('\n');
    result
}

fn build_ordered_scenario() -> Ordered {
    verify_original_templates();
    let mut builder = Builder::new();
    builder.game_check(&template(4));
    builder.check(&template(5), "JAC_AssassinPanel");
    builder.steps.push(Ordered::object([
        ("action", "wait_for".into()),
        ("timeout", 30_i64.into()),
        (
            "condition",
            Ordered::object([
                ("type", "lua".into()),
                ("state_name", "GameCore_Tuner".into()),
                (
                    "code",
                    format!(
                        "{PREFIX}local s=R.State(t.Actor); return s.Production~=nil and s.Mission~=nil"
                    )
                    .into(),
                ),
            ]),
        ),
    ]));
    builder.game_check(&template(6));
    builder.advance(1);
    builder.game_check(&template(7));
    builder.round_trip("JAC-Assassin-Dynamic");
    builder.game_check(&template(8));
    builder.advance(1);
    builder.game_check(&template(9));
    builder.advance(1);
    builder.game_check(&template(10));
    builder.advance(1);
    builder.game_check(&template(11));
    builder.advance(1);
    builder.game_check(&template(12));
    builder.round_trip("JAC-Assassin-Lock");
    builder.game_check(&template(13));
    builder.advance(1);
    builder.game_check(&template(14));
    builder.advance(1);
    builder.game_check(&template(15));

    for (branch, house) in [
        ("SUCCESSION", "IMAGAWA"),
        ("ABSORPTION", "ASAKURA"),
        ("CONGRESS", "ASHIKAGA"),
    ] {
        builder.branch(branch, house);
    }
    for (index, (success, detected)) in [(1, 0), (1, 1), (0, 0), (0, 1)].into_iter().enumerate() {
        builder.lost_actor(index, success, detected);
    }
    builder.game_check(&template(39));
    builder.round_trip("JAC-Assassin-Outcomes");
    builder.game_check(&template(40));
    builder.check(&template(41), "JAC_AssassinPanel");
    builder.game_check(&template(42));
    builder.advance(1);
    builder.check(&template(43), "JAC_AssassinPanel");
    builder.game_check(&template(44));

    let turns: i64 = builder
        .steps
        .iter()
        .filter(|step| step.get("action").string() == "end_turn")
        .map(|step| match step.get("turns") {
            Ordered::Number(n) => n.as_i64().unwrap(),
            _ => unreachable!(),
        })
        .sum();
    assert_eq!(turns, 18, "unexpected live turn budget");
    Ordered::object([
        ("name", "Assassin: 18 live turns, dynamic 100/120/110/120/70/20 training with diplomacy, decided death outcomes, favor vote, lost actors, three save/load cycles".into()),
        ("judge_profile", "japan_map".into()),
        ("steps", Ordered::Array(builder.steps)),
    ])
}
