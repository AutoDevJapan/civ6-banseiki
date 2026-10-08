#[allow(dead_code)]
mod support_safety;

use civ6_autotest_rs::{
    judge,
    runner::{autostart, LOADSCREEN_READY},
    scenario,
    time::Clock,
    Error, ErrorKind,
};
use serde_json::json;
use std::{
    fs,
    sync::{Arc, Mutex},
};
use support_safety::Fixture;

fn step(f: &Fixture, load: bool) -> serde_json::Value {
    if load {
        fs::write(f.saves.join("existing.Civ6Save"), b"save").unwrap();
        json!({"action":"load","name":"existing","exact_name":true,"timeout":2})
    } else {
        json!({"action":"start_game","timeout":2})
    }
}

#[test]
fn dedicated_api_scripts_validate_boolean_and_setting_in_lua51() {
    let lua = mlua::Lua::new();
    for setup in ["Automation=nil", "Automation={IsAutoStartEnabled=function() return 1 end,SetAutoStartEnabled=function() end}"] {
        lua.load(setup).exec().unwrap();
        assert!(lua.load(autostart::READ).eval::<bool>().is_err());
    }
    lua.load("flag=false; writes=0; Automation={IsAutoStartEnabled=function() return flag end,SetAutoStartEnabled=function(v) flag=v; writes=writes+1 end}").exec().unwrap();
    assert!(!lua.load(autostart::READ).eval::<bool>().unwrap());
    assert!(lua.load(autostart::write(true)).eval::<bool>().unwrap());
    assert!(lua.load(autostart::READ).eval::<bool>().unwrap());
    assert!(lua.load(autostart::write(false)).eval::<bool>().unwrap());
    assert!(!lua.load(autostart::READ).eval::<bool>().unwrap());
    assert_eq!(lua.globals().get::<i64>("writes").unwrap(), 2);
    lua.load("Automation.SetAutoStartEnabled=function() end")
        .exec()
        .unwrap();
    assert!(lua.load(autostart::write(true)).eval::<bool>().is_err());
}

#[test]
fn host_and_load_restore_both_initial_values_without_input_or_transition_replay() {
    for load in [false, true] {
        for initial in [false, true] {
            let f = Fixture::new();
            let step = step(&f, load);
            let flag = Arc::new(Mutex::new(initial));
            let value = flag.clone();
            let log = f.logs.join("Lua.log");
            let mut backend = f.backend();
            backend.on_states = Box::new(|_| {
                Ok(vec![
                    "FrontEnd".into(),
                    "LoadScreen".into(),
                    "InGame".into(),
                ])
            });
            backend.on_lua = Box::new(move |_, code, _| {
                if code == autostart::READ {
                    return Ok(value.lock().unwrap().to_string());
                }
                if code == autostart::write(true) {
                    *value.lock().unwrap() = true;
                }
                if code == autostart::write(false) {
                    *value.lock().unwrap() = false;
                }
                if code.contains("Network.HostGame") || code.contains("Network.LoadGame") {
                    assert!(*value.lock().unwrap());
                    fs::write(&log, format!("{LOADSCREEN_READY}\n")).unwrap();
                }
                Ok("true".into())
            });
            let mut r = f.direct_runner(json!({"loadscreen_autostart":true}), backend);
            r.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
            r.run_step(&step).unwrap();
            assert_eq!(*flag.lock().unwrap(), initial);
            assert!(f.trace().presses.is_empty());
            assert!(!f
                .trace()
                .lua
                .iter()
                .any(|(_, code, _)| code.contains("OnActivateButtonClicked")));
            assert_eq!(
                f.trace()
                    .lua
                    .iter()
                    .filter(|(_, code, _)| code.contains(if load {
                        "Network.LoadGame"
                    } else {
                        "Network.HostGame"
                    }))
                    .count(),
                1
            );
            assert_eq!(
                f.trace()
                    .lua
                    .iter()
                    .filter(|(_, code, _)| *code == autostart::write(false))
                    .count(),
                usize::from(!initial)
            );
        }
    }
}

#[test]
fn lost_enable_reply_restores_setting_and_never_hosts_or_loads() {
    for load in [false, true] {
        let f = Fixture::new();
        let step = step(&f, load);
        let flag = Arc::new(Mutex::new(false));
        let value = flag.clone();
        let mut b = f.backend();
        b.on_lua = Box::new(move |_, code, _| {
            if code == autostart::READ {
                return Ok(value.lock().unwrap().to_string());
            }
            if code == autostart::write(true) {
                *value.lock().unwrap() = true;
                return Err(Error::unknown("setter reply lost"));
            }
            if code == autostart::write(false) {
                *value.lock().unwrap() = false;
            }
            Ok("true".into())
        });
        let mut r = f.direct_runner(json!({"loadscreen_autostart":true}), b);
        r.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
        assert_eq!(
            r.run_step(&step).unwrap_err().kind,
            ErrorKind::LuaResponseUnknown
        );
        assert!(!*flag.lock().unwrap());
        assert!(!f
            .trace()
            .lua
            .iter()
            .any(|(_, code, _)| code.contains("Network.HostGame")
                || code.contains("Network.LoadGame")));
    }
}

#[test]
fn lost_restore_reply_is_read_back_without_repeating_a_completed_setter() {
    let f = Fixture::new();
    let log = f.logs.join("Lua.log");
    let flag = Arc::new(Mutex::new(false));
    let value = flag.clone();
    let mut b = f.backend();
    b.on_lua = Box::new(move |_, code, _| {
        if code == autostart::READ {
            return Ok(value.lock().unwrap().to_string());
        }
        if code == autostart::write(true) {
            *value.lock().unwrap() = true;
        }
        if code == autostart::write(false) {
            *value.lock().unwrap() = false;
            return Err(Error::unknown("restore reply lost"));
        }
        if code.contains("Network.HostGame") {
            fs::write(&log, format!("{LOADSCREEN_READY}\n")).unwrap();
        }
        Ok("true".into())
    });
    let mut r = f.direct_runner(json!({"loadscreen_autostart":true}), b);
    r.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    r.run_step(&step(&f, false)).unwrap();
    assert!(!*flag.lock().unwrap());
    assert_eq!(
        f.trace()
            .lua
            .iter()
            .filter(|(_, code, _)| *code == autostart::write(false))
            .count(),
        1
    );
}

#[test]
fn transition_error_and_readiness_timeout_restore_and_preserve_original_failure() {
    for immediate in [false, true] {
        let f = Fixture::new();
        let log = f.logs.join("Lua.log");
        let flag = Arc::new(Mutex::new(false));
        let value = flag.clone();
        let mut b = f.backend();
        b.on_lua = Box::new(move |_, code, _| {
            if code == autostart::READ {
                return Ok(value.lock().unwrap().to_string());
            }
            if code == autostart::write(true) {
                *value.lock().unwrap() = true;
            }
            if code == autostart::write(false) {
                *value.lock().unwrap() = false;
            }
            if code.contains("Network.HostGame") {
                if immediate {
                    return Err(Error::lua("host rejected"));
                }
                fs::write(&log, format!("{LOADSCREEN_READY}\n")).unwrap();
            }
            if code.contains("Game.GetLocalPlayer") {
                return Ok("false".into());
            }
            Ok("true".into())
        });
        let mut r = f.direct_runner(json!({"loadscreen_autostart":true}), b);
        r.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
        let e = r.run_step(&step(&f, false)).unwrap_err();
        assert!(e.to_string().contains(if immediate {
            "host rejected"
        } else {
            "Timed out"
        }));
        assert!(!*flag.lock().unwrap());
        assert!(f.trace().presses.is_empty());
        assert!(f.clock.now() <= 12.0);
    }
}

#[test]
fn invalid_option_and_hotseat_are_rejected_before_running() {
    for value in [json!(1), json!("true"), json!(null)] {
        assert!(scenario::validate_scenario(&json!({"name":"invalid","loadscreen_autostart":value,"steps":[{"action":"start_game"}]})).is_err());
    }
    for extra in [
        json!({"session_type":"hotseat"}),
        json!({"loadscreen_input":"escape"}),
    ] {
        let mut s =
            json!({"name":"invalid","loadscreen_autostart":true,"steps":[{"action":"start_game"}]});
        for (k, v) in extra.as_object().unwrap() {
            s[k] = v.clone();
        }
        assert!(scenario::validate_scenario(&s).is_err());
    }
}

#[test]
fn unavailable_api_and_owner_start_never_mutate_or_dispatch_transition() {
    for owner in [false, true] {
        let f = Fixture::new();
        let mut b = f.backend();
        b.on_lua = Box::new(|_, code, _| {
            if code == autostart::READ {
                return Err(Error::lua("dedicated auto-start API unavailable"));
            }
            panic!("unexpected mutating request: {code}");
        });
        let mut r = f.direct_runner(json!({"loadscreen_autostart":true}), b);
        r.owner_start = owner;
        r.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
        let e = r.run_step(&step(&f, false)).unwrap_err();
        assert!(e.to_string().contains(if owner {
            "automatic single-player"
        } else {
            "API unavailable"
        }));
        assert!(!f
            .trace()
            .lua
            .iter()
            .any(|(_, code, _)| code.contains("Network.HostGame")
                || code.contains("SetAutoStartEnabled(true)")));
    }
}

#[test]
fn process_replacement_refuses_restoration_and_does_not_report_success() {
    let f = Fixture::new();
    let hosted = Arc::new(Mutex::new(false));
    let p = hosted.clone();
    let mut b = f.backend();
    b.on_process = Box::new(move |_| Ok(Some((1234, if *p.lock().unwrap() { 2.0 } else { 1.0 }))));
    let log = f.logs.join("Lua.log");
    let h = hosted.clone();
    b.on_lua = Box::new(move |_, code, _| {
        if code == autostart::READ {
            return Ok("false".into());
        }
        if code == autostart::write(false) {
            panic!("must not restore a replacement process");
        }
        if code.contains("Network.HostGame") {
            *h.lock().unwrap() = true;
            fs::write(&log, format!("{LOADSCREEN_READY}\n")).unwrap();
        }
        Ok("true".into())
    });
    let mut r = f.direct_runner(json!({"loadscreen_autostart":true}), b);
    r.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    assert!(r
        .run_step(&step(&f, false))
        .unwrap_err()
        .to_string()
        .contains("process changed"));
    let observations = fs::read_to_string(f.output.join("transition-observations.jsonl")).unwrap();
    assert!(observations.contains("unconfirmed"));
}

#[test]
fn restoration_failure_is_not_accepted_as_successful_transition() {
    let f = Fixture::new();
    let log = f.logs.join("Lua.log");
    let mut enabled = false;
    let mut b = f.backend();
    b.on_lua = Box::new(move |_, code, _| {
        if code == autostart::READ {
            return Ok(enabled.to_string());
        }
        if code == autostart::write(true) {
            enabled = true;
        }
        if code == autostart::write(false) {
            return Err(Error::lua("restore refused"));
        }
        if code.contains("Network.HostGame") {
            fs::write(&log, format!("{LOADSCREEN_READY}\n")).unwrap();
        }
        Ok("true".into())
    });
    let mut r = f.direct_runner(json!({"loadscreen_autostart":true}), b);
    r.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    assert!(r
        .run_step(&step(&f, false))
        .unwrap_err()
        .to_string()
        .contains("restore refused"));
    assert!(
        fs::read_to_string(f.output.join("transition-observations.jsonl"))
            .unwrap()
            .contains("unconfirmed")
    );
}

#[test]
fn process_exit_is_recorded_separately_and_never_claimed_as_setting_restoration() {
    let f = Fixture::new();
    let hosted = Arc::new(Mutex::new(false));
    let p = hosted.clone();
    let mut b = f.backend();
    b.on_process = Box::new(move |_| {
        Ok(if *p.lock().unwrap() {
            None
        } else {
            Some((1234, 1.0))
        })
    });
    let log = f.logs.join("Lua.log");
    let h = hosted.clone();
    b.on_lua = Box::new(move |_, code, _| {
        if code == autostart::READ {
            return Ok("false".into());
        }
        if code == autostart::write(false) {
            panic!("cannot mutate an exited process");
        }
        if code.contains("Network.HostGame") {
            *h.lock().unwrap() = true;
            fs::write(&log, format!("{LOADSCREEN_READY}\n")).unwrap();
        }
        Ok("true".into())
    });
    let mut r = f.direct_runner(json!({"loadscreen_autostart":true}), b);
    r.baseline = Some(judge::snapshot_logs(&f.logs).unwrap());
    assert!(r
        .run_step(&step(&f, false))
        .unwrap_err()
        .to_string()
        .contains("restoration cannot be confirmed"));
    let observations = fs::read_to_string(f.output.join("transition-observations.jsonl")).unwrap();
    assert!(observations.contains("process_exited"));
}
