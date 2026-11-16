use std::{env, fs, path::PathBuf};

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    assert!(
        args.is_empty() || args == ["--check"],
        "expected --check or no arguments"
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua");
    let original = fs::read_to_string(&path).unwrap();
    let begin = "-- BEGIN RUST PERSONA MERIT ADAPTER\n";
    let end = "-- END RUST PERSONA MERIT ADAPTER\n";
    let block = format!("{begin}{}{end}", civ6_persona_merit_rs::adapter());
    let mut content = if let Some(start) = original.find(begin) {
        let finish = original[start..].find(end).unwrap() + start + end.len();
        format!("{}{}{}", &original[..start], block, &original[finish..])
    } else {
        format!("{block}{original}")
    };
    // FireTuner has its own globals. Export the exact lexical table used by
    // gameplay callbacks, as the existing cross-context API already does.
    let export = "OnPlayerTurnStarted = OnPlayerTurnStarted, SyncEffects = SyncEffects";
    let exposed = format!("{export}, MeritQueue = MeritQueue");
    if !content.contains(&exposed) {
        assert_eq!(content.matches(export).count(), 1);
        content = content.replace(export, &exposed);
    }
    if args.is_empty() {
        fs::write(path, content).unwrap();
    } else {
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            content,
            "generated merit adapter is stale"
        );
    }
}
