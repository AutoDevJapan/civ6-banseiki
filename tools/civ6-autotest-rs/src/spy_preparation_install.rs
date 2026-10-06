//! Generate the test-only UI module and its owned event call site.
use std::{fs, io, path::Path};
#[path = "spy_prepared_mission.rs"]
#[allow(dead_code)]
mod mission;
#[path = "spy_returned_preparation.rs"]
#[allow(dead_code)]
mod preparation;

pub const OUTPUT: &str = "poc/JAC_Test_PersonaBoundary/UI/JAC_ReturnedPreparation548.lua";
const VIEW: &str = "poc/JAC_Test_PersonaBoundary/UI/JAC_PersonaBoundary_View.lua";
const EVENT: &str = "Events.SpyMissionCompleted.Add(function(attacker, missionID)\n";
const LEGACY_HOOK: &str = "include(\"JAC_ReturnedPreparation548\");\nEvents.SpyMissionCompleted.Add(function(attacker, missionID)\n    if Game:GetProperty(\"JAC_BOUNDARY_RETURNED_PREPARATION\") then\n        local preparation=assert(B.ReturnedPreparation,\"returned preparation UI module missing\");\n        if preparation.RouteEvent(attacker,missionID)~='UNOWNED' then return; end\n    end\n";
const HOOK: &str = "include(\"JAC_ReturnedPreparation548\");\nEvents.SpyMissionCompleted.Add(function(attacker, missionID)\n    if Game:GetProperty(\"JAC_BOUNDARY_PREPARED_MISSION\") then\n        local prepared=assert(B.PreparedMission,\"prepared mission UI module missing\");\n        if prepared.RouteEvent(attacker,missionID)~='UNOWNED' then return; end\n    end\n    if Game:GetProperty(\"JAC_BOUNDARY_RETURNED_PREPARATION\") then\n        local preparation=assert(B.ReturnedPreparation,\"returned preparation UI module missing\");\n        if preparation.RouteEvent(attacker,missionID)~='UNOWNED' then return; end\n    end\n";
const LOAD: &str = "Events.LoadScreenClose.Add(function()\n";
const LEGACY_LOAD_HOOK: &str = "Events.LoadScreenClose.Add(function()\n    if B.ReturnedPreparation then B.ReturnedPreparation.InvalidateOnLoad(); end\n";
const LOAD_HOOK: &str = "Events.LoadScreenClose.Add(function()\n    if B.ReturnedPreparation then B.ReturnedPreparation.InvalidateOnLoad(); end\n    if B.PreparedMission then B.PreparedMission.InvalidateOnLoad(); end\n";

pub fn connection() -> String {
    format!(
        "-- Generated from Rust: spy_preparation_install.rs / spy_returned_preparation.rs / spy_prepared_mission.rs.\nlocal function install()\n{}\nend\ninstall();\nExposedMembers.JAC_PersonaBoundary.ReturnedPreparation=JAC_ReturnedPreparation548;\nlocal function installPrepared()\n{}\nend\ninstallPrepared();\nExposedMembers.JAC_PersonaBoundary.PreparedMission=JAC_PreparedMission548;\n",
        preparation::UI_SETUP,mission::UI_SETUP
    )
}

pub fn hooked_view(source: &str) -> io::Result<String> {
    let source = source.replace("\r\n", "\n");
    let source = if source.matches(HOOK).count() == 1 {
        source
    } else if source.matches(LEGACY_HOOK).count() == 1 {
        source.replacen(LEGACY_HOOK, HOOK, 1)
    } else if source.matches(EVENT).count() != 1 || source.contains("JAC_ReturnedPreparation548") {
        return Err(io::Error::other(
            "owned preparation event call site changed",
        ));
    } else {
        source.replacen(EVENT, HOOK, 1)
    };
    if source.matches(LOAD_HOOK).count() == 1 {
        return Ok(source);
    }
    if source.matches(LEGACY_LOAD_HOOK).count() == 1 {
        return Ok(source.replacen(LEGACY_LOAD_HOOK, LOAD_HOOK, 1));
    }
    if source.matches(LOAD).count() != 1 {
        return Err(io::Error::other("owned load call site changed"));
    }
    Ok(source.replacen(LOAD, LOAD_HOOK, 1))
}

pub fn write(root: &Path) -> io::Result<()> {
    let output = root.join(OUTPUT);
    let expected = connection();
    if fs::read_to_string(&output).ok().as_deref() != Some(expected.as_str()) {
        fs::write(output, expected)?;
    }
    let view = root.join(VIEW);
    let old = fs::read_to_string(&view)?;
    let next = hooked_view(&old)?;
    if old.replace("\r\n", "\n") != next {
        fs::write(view, next)?;
    }
    Ok(())
}
