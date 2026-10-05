//! Reproducible host measurement: 22 factions, 100/1,000/5,000 nearby Units,
//! 300 publish notifications in each open/closed phase. No game or API key.
use civ6_lua_tests_rs::{
    assassin_panel::{baseline_source, PanelWorld, BASE, PANEL},
    read_repo,
};
use serde_json::json;
use std::time::Instant;

fn main() {
    let baseline = baseline_source();
    let current = read_repo(PANEL);
    let mut results = Vec::new();
    for units in [100, 1000, 5000] {
        for (version, source) in [("baseline", &baseline), ("visible_only", &current)] {
            let world = PanelWorld::new(source, units);
            let closed = world.sample(300);
            let start = Instant::now();
            world.click("OpenButton");
            let open_ms = start.elapsed().as_secs_f64() * 1000.0;
            let rows = world.rows().len();
            let visible = world.sample(300);
            world.click("CloseButton");
            let reclosed = world.sample(300);
            assert_eq!(visible.rows, rows);
            assert!(visible.counts.candidate_resolutions >= units * 300);
            if version == "visible_only" {
                assert_eq!(closed.counts.candidate_resolutions, 0);
                assert_eq!(closed.counts.state_reads, 0);
                assert_eq!(reclosed.counts.unit_classifications, 0);
                assert_eq!(reclosed.counts.layouts, 0);
            }
            results.push(
                json!({"version":version,"target_units":units,"major_factions":22,
                "closed":closed,"open_ms":open_ms,"visible":visible,"reclosed":reclosed}),
            );
        }
    }
    println!("{}", serde_json::to_string_pretty(&json!({"baseline_commit":BASE,
        "scope":"Lua 5.1 / deterministic stubs / host elapsed time; not an in-game or 300-turn measurement",
        "profile":if cfg!(debug_assertions) {"debug"} else {"release"},
        "results":results})).unwrap());
}
