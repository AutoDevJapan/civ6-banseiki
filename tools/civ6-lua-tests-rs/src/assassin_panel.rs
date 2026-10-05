//! Execute the shipped panel and rules in Lua 5.1 with deterministic game stubs.
//! Counts are assertions; elapsed times describe this host, not the Civ6 engine.

use crate::{new_lua, read_repo, repo_root};
use mlua::{Function, Lua, MultiValue, Table};
use serde::Serialize;
use std::{cell::Cell, process::Command, rc::Rc, time::Instant};

pub const PANEL: &str = "poc/JAC_PoC_Assassin/UI/JAC_AssassinPanel.lua";
pub const BASE: &str = "4d037b5f";

pub fn baseline_source() -> String {
    let result = Command::new("git")
        .current_dir(repo_root())
        .args(["show", &format!("{BASE}:{PANEL}")])
        .output()
        .expect("read baseline from Git");
    assert!(result.status.success(), "baseline is unavailable");
    String::from_utf8(result.stdout).expect("UTF-8 baseline")
}

#[derive(Clone, Default)]
pub struct Counters {
    states: Rc<Cell<usize>>,
    candidates: Rc<Cell<usize>>,
    units: Rc<Cell<usize>>,
    layouts: Rc<Cell<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub state_reads: usize,
    pub candidate_resolutions: usize,
    pub unit_classifications: usize,
    pub layouts: usize,
}

impl Counters {
    fn watch(&self, lua: &Lua, table: &Table, key: &str, count: Rc<Cell<usize>>) {
        let original: Function = table.get(key).unwrap();
        table
            .set(
                key,
                lua.create_function(move |_, args: MultiValue| {
                    count.set(count.get() + 1);
                    original.call::<MultiValue>(args)
                })
                .unwrap(),
            )
            .unwrap();
    }

    pub fn reset(&self) {
        for value in [&self.states, &self.candidates, &self.units, &self.layouts] {
            value.set(0);
        }
    }

    pub fn snapshot(&self) -> Counts {
        Counts {
            state_reads: self.states.get(),
            candidate_resolutions: self.candidates.get(),
            unit_classifications: self.units.get(),
            layouts: self.layouts.get(),
        }
    }
}

pub struct PanelWorld {
    pub lua: Lua,
    pub counts: Counters,
}

impl PanelWorld {
    pub fn new(source: &str, target_units: usize) -> Self {
        let lua = new_lua();
        lua.load(include_str!("../fixtures/assassin/stubs.lua"))
            .exec()
            .unwrap();
        // 22 major factions plus the free-city slot; all targets are nearby and
        // visible, deliberately exercising the expensive candidate path.
        let data: Table = lua.globals().get("DATA").unwrap();
        let clone: Function = lua.globals().get("clone").unwrap();
        let template: Table = data.get(1).unwrap();
        lua.globals().set("MAX_MAJOR", 21).unwrap();
        for id in 1..=22 {
            let nation: Table = clone.call(template.clone()).unwrap();
            nation.set("Alive", id <= 21).unwrap();
            nation.set("Units", lua.create_table().unwrap()).unwrap();
            let cities: Table = nation.get("Cities").unwrap();
            for city in cities.pairs::<i64, Table>() {
                let (_, city) = city.unwrap();
                city.set("Owner", id).unwrap();
                city.set("X", 2).unwrap();
            }
            data.set(id, nation).unwrap();
        }
        let grant: Function = lua.globals().get("GRANT").unwrap();
        let assassin: Table = grant.call((0, 0, 1, 0)).unwrap();
        // GRANT returns a Unit whose GetID is a method.
        let get_id: Function = assassin.get("GetID").unwrap();
        lua.globals()
            .set("U", get_id.call::<i64>(assassin).unwrap())
            .unwrap();
        for index in 0..target_units {
            let _: Table = grant.call((1 + index % 21, 4, 2, 0)).unwrap();
        }
        let rebuild: Function = lua.globals().get("rebuild").unwrap();
        rebuild.call::<()>(()).unwrap();
        lua.load(read_repo(
            "poc/JAC_PoC_Assassin/Scripts/JAC_Assassin_Rules.lua",
        ))
        .exec()
        .unwrap();
        lua.globals()
            .set("A", lua.globals().get::<Table>("JAC_ASN").unwrap())
            .unwrap();
        lua.globals()
            .set(
                "include",
                lua.create_function(|_, _: String| Ok(())).unwrap(),
            )
            .unwrap();
        let fixture = include_str!("../fixtures/assassin/cases.lua");
        let setup = fixture
            .split("-- CASE helper_panel_setup\n-- SEGMENT\n")
            .nth(1)
            .expect("panel setup")
            .split("-- CASE ")
            .next()
            .unwrap();
        lua.load(setup).exec().unwrap();
        let counts = Counters::default();
        let rules: Table = lua.globals().get("JAC_ASN").unwrap();
        counts.watch(&lua, &rules, "State", counts.states.clone());
        counts.watch(&lua, &rules, "ResolveTarget", counts.candidates.clone());
        counts.watch(&lua, &rules, "Classify", counts.units.clone());
        let controls: Table = lua.globals().get("Controls").unwrap();
        counts.watch(
            &lua,
            &controls.get::<Table>("Rows").unwrap(),
            "CalculateSize",
            counts.layouts.clone(),
        );
        lua.load(source).exec().unwrap();
        Self { lua, counts }
    }

    pub fn click(&self, control: &str) {
        let controls: Table = self.lua.globals().get("Controls").unwrap();
        let callback: Function = controls
            .get::<Table>(control)
            .unwrap()
            .get("Callback")
            .unwrap();
        callback.call::<()>(()).unwrap();
    }

    pub fn event(&self, name: &str) -> Function {
        self.lua
            .globals()
            .get::<Table>("UI_HANDLERS")
            .unwrap()
            .get(name)
            .unwrap()
    }

    pub fn rows(&self) -> Vec<(String, String, bool)> {
        self.lua
            .globals()
            .get::<Table>("UI_ROWS")
            .unwrap()
            .sequence_values::<Table>()
            .map(|row| {
                let row = row.unwrap();
                let action: Table = row.get("Action").unwrap();
                (
                    row.get::<Table>("Text").unwrap().get("Text").unwrap(),
                    action.get("Text").unwrap(),
                    action.get("Disabled").unwrap(),
                )
            })
            .collect()
    }

    pub fn sample(&self, events: usize) -> Sample {
        assert!(events > 0, "at least one update is required");
        self.counts.reset();
        let publish = self.event("GameCoreEventPublishComplete");
        let mut times = Vec::with_capacity(events);
        for _ in 0..events {
            let start = Instant::now();
            publish.call::<()>(()).unwrap();
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        Sample {
            events,
            total_ms: times.iter().sum(),
            median_ms: times[events / 2],
            p95_ms: times[(events * 95 / 100).min(events - 1)],
            counts: self.counts.snapshot(),
            rows: self.rows().len(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Sample {
    pub events: usize,
    pub total_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub counts: Counts,
    pub rows: usize,
}
