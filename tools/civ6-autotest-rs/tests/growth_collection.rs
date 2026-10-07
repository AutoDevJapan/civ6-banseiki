use civ6_autotest_rs::growth_sync::{compare, Checkpoint, COLLECTION_QUERY};
use mlua::Lua;

fn game() -> Lua {
    let lua = Lua::new();
    lua.load(r#"
      lan=true; network=true; hotseat=false; localPlayer=0; worldTurn=12
      initialized=true; majors={0,1}; humans={[0]=true,[1]=true}
      changedTurn=false; changedActive=false; reads=0; activeReads={}
      GameConfiguration={IsLANMultiplayer=function() return lan end,
        IsNetworkMultiplayer=function() return network end, IsHotseat=function() return hotseat end}
      Game={GetProperty=function(_,name) assert(name=='JAC_POC_GROWTHUI_INITIALIZED'); return initialized end,
        GetCurrentGameTurn=function() reads=reads+1; return worldTurn+(changedTurn and reads>1 and 1 or 0) end,
        GetLocalPlayer=function() return localPlayer end}
      PlayerManager={GetAliveMajors=function()
        local result={}; for _,id in ipairs(majors) do
          local playerID=id; result[#result+1]=Players[playerID] or {GetID=function() return playerID end}
        end; return result
      end}
      Players={}; records={}; counts={[0]=2,[1]=2}
      for pid=0,1 do
        local owner=pid; records[owner]={}
        for id=10,11 do
          records[owner][id]={Level=1,XP=20,Personality='brave',Trait='guard',Equipment={'blade','cloak'},LastXPTurn=12}
        end
        Players[owner]={GetID=function() return owner end, IsHuman=function() return humans[owner] end,
          IsTurnActive=function() activeReads[owner]=(activeReads[owner] or 0)+1; return not(changedActive and activeReads[owner]>1) end,
          GetUnits=function() return {Members=function()
            local id=9; return function()
              id=id+1; if id>=10+counts[owner] then return nil end
              local unitID=id; return unitID, {GetID=function() return unitID end,
                GetProperty=function(_,name) assert(name=='JAC_POC_GROWTHUI_WARLORD'); return records[owner][unitID] end}
            end
          end} end}
      end
      -- Any attempted game/property mutation would fail rather than silently pass.
      Game.SetProperty=function() error('mutation') end
      UnitManager={InitUnit=function() error('mutation') end}
    "#).exec().unwrap();
    lua
}

fn capture(lua: &Lua, endpoint: &str) -> Checkpoint {
    Checkpoint {
        endpoint_id: endpoint.into(),
        session_tag: "session-1".into(),
        barrier_tag: "round-12".into(),
        phase: "after-growth".into(),
        observation: lua.load(COLLECTION_QUERY).eval().unwrap(),
    }
}

#[test]
fn fixed_query_reads_each_endpoint_and_preserves_all_four_records() {
    let host = game();
    let peer = game();
    peer.globals().set("localPlayer", 1).unwrap();
    let before: String = host
        .load("return records[0][10].XP..':'..records[0][10].LastXPTurn")
        .eval()
        .unwrap();
    let a = capture(&host, "host");
    let b = capture(&peer, "peer");
    assert_eq!(a.observation,"12|0|0~10~1~20~brave~guard~blade,cloak~none~12|0~11~1~20~brave~guard~blade,cloak~none~12|1~10~1~20~brave~guard~blade,cloak~none~12|1~11~1~20~brave~guard~blade,cloak~none~12");
    compare(&a, &b).unwrap();
    let after: String = host
        .load("return records[0][10].XP..':'..records[0][10].LastXPTurn")
        .eval()
        .unwrap();
    assert_eq!(before, after);
    peer.load("records[1][11].XP=40").exec().unwrap();
    assert!(compare(&a, &capture(&peer, "peer"))
        .unwrap_err()
        .to_string()
        .contains("XP"));
}

#[test]
fn rejects_wrong_session_roster_and_collection_boundaries() {
    for (setup, expected) in [
        ("lan=false", "LAN multiplayer"),
        ("network=false", "network multiplayer"),
        ("hotseat=true", "hotseat"),
        ("initialized=false", "growth not initialized"),
        ("localPlayer=2", "local player"),
        ("majors={0,1,2}", "exactly two"),
        ("majors={0,0}", "unexpected major"),
        ("humans[1]=false", "two human"),
        ("counts[1]=1", "two growth records"),
        ("changedTurn=true", "turn boundary"),
        ("changedActive=true", "activation boundary"),
    ] {
        let lua = game();
        lua.load(setup).exec().unwrap();
        let error = lua
            .load(COLLECTION_QUERY)
            .eval::<String>()
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{setup}: {error}");
    }
}

#[test]
fn malformed_and_fractional_observations_cannot_be_published_as_checkpoints() {
    for setup in [
        "records[0][10].XP=4.5",
        "records[0][10].Level=0",
        "records[0][10].LastXPTurn=13",
        "records[0][10].Personality='bad~field'",
        "records[0][10].Equipment={'bad|field'}",
    ] {
        let lua = game();
        lua.load(setup).exec().unwrap();
        assert!(capture(&lua, "host").parse().is_err(), "{setup}");
    }
}

#[test]
fn capture_cli_sends_fixed_query_once_and_emits_only_validated_data() {
    use civ6_autotest_rs::tuner::{recv_frame, send_frame, COMMAND, HANDSHAKE};
    use std::{net::TcpListener, process::Command, thread, time::Duration};
    let observation = capture(&game(), "fixture").observation;
    // Mock TCP peers exercise the executable, not live LAN synchronization.
    for reply in [Some(observation.clone()), Some("invalid".into()), None] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let expected_reply = reply.clone();
        let server = thread::spawn(move || {
            let (mut connection, _) = listener.accept().unwrap();
            connection
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut commands = 0;
            while let Ok(frame) = recv_frame(&mut connection, None) {
                match frame.payload.as_str() {
                    "APP:" => send_frame(&mut connection, HANDSHAKE, "Civilization VI").unwrap(),
                    "LSQ:" => send_frame(&mut connection, HANDSHAKE, "LSQ:17\nInGame").unwrap(),
                    payload => {
                        assert!(payload.starts_with("CMD:17:"));
                        assert!(payload.contains(COLLECTION_QUERY));
                        commands += 1;
                        if let Some(value) = &expected_reply {
                            let start = payload.find("JAC_AUTOTEST_").unwrap();
                            let token = &payload[start..start + "JAC_AUTOTEST_".len() + 32];
                            send_frame(&mut connection, COMMAND, &format!("{token}|OK|{value}"))
                                .unwrap();
                        } else {
                            break; // Reply lost: capture must fail without sending again.
                        }
                    }
                }
            }
            commands
        });
        let output = Command::new(env!("CARGO_BIN_EXE_civ6-growth-sync"))
            .args([
                "capture",
                "--endpoint",
                "host",
                "--session",
                "s",
                "--barrier",
                "b",
                "--phase",
                "p",
                "--port",
                &port.to_string(),
            ])
            .output()
            .unwrap();
        assert_eq!(server.join().unwrap(), 1);
        if reply.as_deref() == Some(observation.as_str()) {
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let recorded: Checkpoint = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(recorded.observation, observation);
            assert_eq!(recorded.endpoint_id, "host");
            assert_eq!(recorded.barrier_tag, "b");
        } else {
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn compare_cli_reports_differences_with_failure_exit() {
    use std::{fs, process::Command};
    let root = tempfile::tempdir().unwrap();
    let host = capture(&game(), "host");
    let peer = game();
    peer.globals().set("localPlayer", 1).unwrap();
    let left = root.path().join("host.json");
    let right = root.path().join("peer.json");
    fs::write(&left, serde_json::to_vec(&host).unwrap()).unwrap();
    for changed in [false, true] {
        if changed {
            peer.load("records[1][11].XP=40").exec().unwrap();
        }
        fs::write(&right, serde_json::to_vec(&capture(&peer, "peer")).unwrap()).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_civ6-growth-sync"))
            .arg("compare")
            .arg(&left)
            .arg(&right)
            .output()
            .unwrap();
        assert_eq!(output.status.success(), !changed);
        if changed {
            assert!(String::from_utf8_lossy(&output.stderr).contains("owner=1 unitID=11.XP"));
        } else {
            assert!(String::from_utf8_lossy(&output.stdout).contains("supplied growth records"));
        }
    }
}
