use civ6_autotest_rs::{native_carrier_witness as witness, repo_root};
use mlua::Lua;
use serde_json::Value;
use std::fs;
fn nonce() -> &'static str {
    static VALUE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VALUE
        .get_or_init(|| uuid::Uuid::new_v4().simple().to_string())
        .as_str()
}
const RULES: &str =
    include_str!("../../../poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Rules.lua");

#[test]
fn generated_deploy_files_match_and_compile_lua51() {
    let run_id = nonce();
    for (path, code) in [
        (
            "poc/JAC_PoC_Absorption/Scripts/JAC_NativeCarrierWitness_GP.lua",
            witness::gameplay_binding(),
        ),
        (
            "poc/JAC_PoC_Absorption/UI/JAC_NativeCarrierWitness_UI.lua",
            witness::ui_binding(),
        ),
        (
            "poc/JAC_PoC_JapanMapScale/UI/JAC_NativeCarrierWitness_Staging.lua",
            witness::staging_binding(),
        ),
    ] {
        assert_eq!(fs::read_to_string(repo_root().join(path)).unwrap(), code);
        Lua::new().load(&code).into_function().unwrap();
        for nonce in ["nil", "'short'", "'zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz'"] {
            let lua = Lua::new();
            lua.load(format!(
                "GameConfiguration={{GetValue=function() return {nonce} end}}"
            ))
            .exec()
            .unwrap();
            lua.load(&code).exec().unwrap(); // all native/global APIs absent: real no-op
        }
    }
    let root = tempfile::tempdir().unwrap();
    assert!(witness::emit(root.path(), &nonce()[..7]).is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    let emitted = witness::emit(root.path(), nonce()).unwrap();
    assert_eq!(
        emitted.mid_save,
        format!("JAC-NativeCarrier-MidLoan-{run_id}")
    );
    assert_eq!(
        emitted.returned_save,
        format!("JAC-NativeCarrier-Returned-{run_id}")
    );
    assert_eq!(emitted.files.len(), 3);
}

fn gp() -> Lua {
    gp_with_ui_getters(true)
}

fn gp_with_ui_getters(ui_getters: bool) -> Lua {
    gp_context(ui_getters, true, true)
}

fn gp_context(ui_getters: bool, multiplayer: bool, confirmed: bool) -> Lua {
    let run_id = nonce();
    let lua = Lua::new();
    lua.load(RULES).exec().unwrap();
    lua.globals()
        .set("NATIVE_MULTIPLAYER", multiplayer)
        .unwrap();
    lua.globals().set("HOTSEAT_CONFIRMED", confirmed).unwrap();
    lua.load(format!(r#"
local R=JAC_AbsorptionRules
GameConfiguration={{GetValue=function(k) if k=='JAC_NATIVE_CARRIER_WITNESS_HOTSEAT' then return HOTSEAT_CONFIRMED end;return '{run_id}' end,IsHotseat=function() return true end,IsAnyMultiplayer=function() return true end}}
PROPS={{}};WRITES={{}};CREATES=0
Game={{IsNetworkMultiplayer=function() return NATIVE_MULTIPLAYER end,GetProperty=function(_,k) return R.Copy(PROPS[k]) end,SetProperty=function(_,k,v) PROPS[k]=R.Copy(v);WRITES[#WRITES+1]=R.Copy(v) end,GetCurrentGameTurn=function() return 1 end}}
local function player(id) return {{IsAlive=function() return true end,IsHuman=function() return id<2 end,IsTurnActive=function() return id==0 end}} end
Players={{[0]=player(0),[1]=player(1),[2]=player(2)}}
PlayerManager={{GetAliveIDs=function() return {{0,1,2}} end}}
ExposedMembers={{}}
JAC_AbsorptionGameplay={{Rules=R,Canonical=R.Canonical,GetState=function() return {{}} end}}
GameEvents={{JAC_NativeCarrierWitness_Operation={{Add=function(f) RECEIVE=f end}}}}
"#)).exec().unwrap();
    if !ui_getters {
        lua.load("GameConfiguration.IsHotseat=nil;GameConfiguration.IsAnyMultiplayer=nil")
            .exec()
            .unwrap();
    }
    lua.load(witness::gameplay_binding()).exec().unwrap();
    lua
}

#[test]
fn gameplay_initializes_without_ui_only_session_getters() {
    let lua = gp_with_ui_getters(false);
    lua.load("assert(type(RECEIVE)=='function' and ExposedMembers.JAC_NativeCarrierWitness.Get().NextOperation==1 and CREATES==0)").exec().unwrap();
}

#[test]
fn gameplay_requires_real_native_mp_and_verified_ui_hotseat_setting() {
    for (network, confirmed) in [(false, true), (true, false), (false, false)] {
        let lua = gp_context(false, network, confirmed);
        lua.load("assert(RECEIVE==nil and ExposedMembers.JAC_NativeCarrierWitness==nil and #WRITES==0 and CREATES==0)").exec().unwrap();
    }
    let lua = gp_with_ui_getters(false);
    lua.load(format!("HOTSEAT_CONFIRMED=false;RECEIVE(0,{{Version=1,Command='FIXED',Nonce='{}',Actor=0,Generation=0,Step=8,Turn=1}});assert(CREATES==0 and next(ExposedMembers.JAC_NativeCarrierWitness.Get().Receipts)==nil and ExposedMembers.JAC_NativeCarrierWitness.Get().Failure)",nonce())).exec().unwrap();
}

#[test]
fn core_persists_attempt_before_failure_and_never_replays_it() {
    let run_id = nonce();
    let lua = gp();
    // First fixed operation is native CREATE at protocol step 8. The missing
    // real site is an intentional failure, not a substitute city creation.
    let packet = format!(
        "{{Version=1,Command='FIXED',Nonce='{run_id}',Actor=0,Generation=0,Step=8,Turn=1}}"
    );
    lua.load(format!("RECEIVE(0,{packet});RECEIVE(0,{packet})"))
        .exec()
        .unwrap();
    lua.load(r#"
local b=ExposedMembers.JAC_NativeCarrierWitness.Get()
assert(b.Receipts['0:8'].Status=='FAIL' and b.NextOperation==1)
local started=0;for _,w in ipairs(WRITES) do if w.Receipts and w.Receipts['0:8'] and w.Receipts['0:8'].Status=='STARTED' then started=started+1 end end
assert(started==1)
"#).exec().unwrap();
    // Reload Gameplay from its actual saved diagnostic property. Its spent
    // receipt survives and prevents a new engine attempt after rejoin.
    lua.load(witness::gameplay_binding()).exec().unwrap();
    lua.load(format!(
        "local n=#WRITES;RECEIVE(0,{packet});assert(#WRITES==n)"
    ))
    .exec()
    .unwrap();
}

#[test]
fn wrong_actor_generation_and_runtime_source_fail_before_mutation() {
    let run_id = nonce();
    for extra in [
        "Actor=1,Generation=0,Step=8",
        "Actor=0,Generation=1,Step=8",
        "Actor=0,Generation=0,Step=24",
        "Actor=0,Generation=0,Step=8,Code='fake()'",
    ] {
        let lua = gp();
        lua.load(format!(
            "RECEIVE(0,{{Version=1,Command='FIXED',Nonce='{run_id}',Turn=1,{extra}}})"
        ))
        .exec()
        .unwrap();
        lua.load("local b=ExposedMembers.JAC_NativeCarrierWitness.Get();assert(next(b.Receipts)==nil and b.NextOperation==1 and b.Failure)").exec().unwrap();
    }
}

#[test]
fn staging_uses_the_guarded_official_button_once_and_frames_are_json() {
    let run_id = nonce();
    let lua = Lua::new();
    lua.load(format!(r#"
GameConfiguration={{GetValue=function(k) if k=='JAC_NATIVE_CARRIER_WITNESS_NONCE' then return '{run_id}' else return 1 end end,
 IsHotseat=function() return true end,IsAnyMultiplayer=function() return true end,GetParticipatingPlayerIDs=function() return {{0,1,2}} end}}
PlayerConfigurations={{[0]={{GetSlotStatus=function() return 1 end}},[1]={{GetSlotStatus=function() return 1 end}},[2]={{GetSlotStatus=function() return 2 end}}}}
SlotStatus={{SS_TAKEN=1}};ContextPtr={{IsHidden=function() return false end}};Network={{IsGameHost=function() return true end}}
CLICKS=0;UPDATES=0;DISABLED=true
UI={{GetElapsedTime=function() return 10.125 end}}
Controls={{ReadyButton={{IsDisabled=function() return DISABLED end,DoLeftClick=function() CLICKS=CLICKS+1 end}}}}
UpdateReadyButton=function() UPDATES=UPDATES+1 end
LOGS={{}};print=function(s) LOGS[#LOGS+1]=s end
"#)).exec().unwrap();
    lua.load(witness::staging_binding()).exec().unwrap();
    lua.load("UpdateReadyButton();assert(CLICKS==0);DISABLED=false;UpdateReadyButton();UpdateReadyButton();assert(CLICKS==1 and UPDATES==3)").exec().unwrap();
    let logs = lua.globals().get::<mlua::Table>("LOGS").unwrap();
    let mut bytes = Vec::new();
    for (expected, line) in (1..).zip(logs.sequence_values::<String>()) {
        let line = line.unwrap();
        let fields: Vec<_> = line.split('|').collect();
        assert_eq!(fields.len(), 10);
        assert_eq!(fields[0], "JAC_NC");
        assert_eq!(fields[1], "1");
        assert_eq!(fields[2], nonce());
        assert_eq!(fields[5], "1");
        assert_eq!(fields[7], "STAGING_READY");
        assert!(fields[9].len() <= 192);
        assert_eq!(
            fields[8]
                .split('/')
                .next()
                .unwrap()
                .parse::<usize>()
                .unwrap(),
            expected
        );
        for pair in fields[9].as_bytes().chunks_exact(2) {
            bytes.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
        }
    }
    let payload: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(payload["generation"], 1);
    assert!(payload["canonical"].as_str().unwrap().contains(nonce()));
}

#[test]
fn product_entry_hooks_precede_publication_and_growth_contract_stays_separate() {
    let hero = fs::read_to_string(
        repo_root().join("poc/JAC_PoC_Absorption/UI/JAC_Absorption_HeroKeys.lua"),
    )
    .unwrap();
    let hook = hero.find("include('JAC_NativeCarrierWitness_UI')").unwrap();
    assert!(hero.find("include('JAC_CarrierMP_UI')").unwrap() < hook);
    assert!(hook < hero.find("include('JAC_NativeInventory_UI')").unwrap());
    assert!(
        hook < hero
            .find("Events.GameCoreEventPublishComplete.Add(ObservePendingHeroes)")
            .unwrap()
    );
    let entry = fs::read_to_string(
        repo_root().join("poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Entry.lua"),
    )
    .unwrap();
    assert!(
        entry.find("include(\"JAC_NativeInventory\")").unwrap()
            < entry
                .find("include('JAC_NativeCarrierWitness_GP')")
                .unwrap()
    );
    let original: Value = serde_json::from_str(include_str!(
        "../../civ6-autotest/scenarios/multiplayer-hotseat-growth.json"
    ))
    .unwrap();
    civ6_autotest_rs::hotseat_bridge::check_scenario(&original).unwrap();
    assert!(civ6_autotest_rs::hotseat_bridge::check_scenario(&witness::fixture()).is_err());
    assert_eq!(
        witness::fixture()["steps"].as_array().unwrap().len() + 1,
        witness::LAST_STEP
    );
}

#[test]
fn human_route_changes_only_fixed_roles_and_keeps_the_ai_tuner_fixture_exact() {
    let original = witness::fixture();
    let human = witness::witness_fixture();
    civ6_autotest_rs::scenario::validate_scenario(&human).unwrap();
    assert_eq!(original["steps"][1], human["steps"][1]);
    assert!(original["steps"][6]["code"]
        .as_str()
        .unwrap()
        .contains("Recipient=0,Donor=2"));
    assert!(human["steps"][6]["code"]
        .as_str()
        .unwrap()
        .contains("Recipient=1,Donor=0"));
    assert!(human["steps"][6]["code"]
        .as_str()
        .unwrap()
        .contains("not Players[2]:IsHuman()"));
    assert!(human["steps"][25]["code"]
        .as_str()
        .unwrap()
        .contains("A.RecoverArchived(e.AssetID,1,e.Token)"));
    for index in [34, 40] {
        let code = human["steps"][index]["code"].as_str().unwrap();
        assert!(code.contains("m.Actor==0 and m.MoveOwner==0"));
        assert!(code.contains("A.GetRoster(1)"));
        assert!(code.contains("final.Locations[1].Owner==1"));
    }
    assert_eq!(original["steps"][35], human["steps"][35]); // complete-world snapshot is unchanged
    assert_eq!(original["steps"][41], human["steps"][41]);
    let actual: Value = serde_json::from_slice(
        &fs::read(
            repo_root()
                .join("tools/civ6-autotest/scenarios/absorption-native-carrier-hotseat-save.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, original);
}

fn loaded_ui(claimed: bool) -> Lua {
    let run_id = nonce();
    // Trusted repository-only test code uses debug upvalues to isolate the
    // save callback ordering. This VM never accepts external game packets.
    let lua = unsafe { Lua::unsafe_new() };
    lua.load(RULES).exec().unwrap();
    lua.globals().set("CLAIMED", claimed).unwrap();
    lua.load(format!(r#"
local R=JAC_AbsorptionRules
local b={{Version=1,Nonce='{run_id}',Generation=1,ResumeStep=33,Receipts={{}}}}
PAUSED=true;SHOWN=true;LOGS={{}};ADD_ORDER={{}}
UI={{GetElapsedTime=function() return 10.125 end}}
E={{Token='fixture-native-hotseat-996',Original={{Index=99,GreatWorkType='RELIC'}},Borrowed={{ID=44,Phase='BORROWED'}},Source={{ID=2}}}}
M={{Phase='BORROWED',Claimed=CLAIMED,Actor=0,MoveOwner=0,Loans={{{{}},{{Building=7,Owner=1}}}}}}
NATIVE={{Recoveries={{['fixture-native-hotseat-996']={{NativeCarrier={{MP=M}}}}}}}}
GameConfiguration={{GetValue=function() return '{run_id}' end,IsHotseat=function() return true end,IsAnyMultiplayer=function() return true end,IsPaused=function() return PAUSED end}}
Game={{GetProperty=function(_,k) if k=='JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996' then return E else return b end end,GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return 1 end}}
GameInfo={{GreatWorks={{RELIC={{Index=37}}}}}}
ExposedMembers={{JAC_Absorption={{GetState=function() return NATIVE end,Canonical=R.Canonical}},
 JAC_NativeCarrierWitness={{Get=function() return R.Copy(b) end,GetHeader=function() return {{Generation=1,ResumeStep=33}} end}}}}
JAC_ObserveNativeCarrierMP=function() return true end
local control={{IsHidden=function() return not SHOWN end,IsDisabled=function() return false end}}
ContextPtr={{SetHide=function() end,LookUpControl=function() return control end,SetUpdate=function(_,f) UPDATE=f end}}
UIManager={{IsInPopupQueue=function() return SHOWN end}}
Players={{[0]={{IsHuman=function() return true end,IsTurnActive=function() return true end}}}}
PlayerConfigurations={{[0]={{GetHotseatPassword=function() return '' end}}}}
Events={{GreatWorkMoved={{Add=function(f) MOVE=f;ADD_ORDER[#ADD_ORDER+1]='MOVE' end,Remove=function() end}},
 LoadScreenClose={{Add=function(f) CLOSE=f;ADD_ORDER[#ADD_ORDER+1]='CLOSE' end}}}}
print=function(s) LOGS[#LOGS+1]=s end
"#)).exec().unwrap();
    lua.load(
        fs::read_to_string(
            repo_root().join("poc/JAC_PoC_Absorption/UI/JAC_NativeCarrierWitness_UI.lua"),
        )
        .unwrap(),
    )
    .exec()
    .unwrap();
    lua
}

#[test]
fn review_staging_ready_is_once_per_generation_in_the_same_context() {
    let run_id = nonce();
    let lua = Lua::new();
    lua.load(RULES).exec().unwrap();
    lua.load(format!(r#"
GENERATION=0
GameConfiguration={{GetValue=function(k) if k=='JAC_NATIVE_CARRIER_WITNESS_NONCE' then return '{run_id}' else return GENERATION end end,
 IsHotseat=function() return true end,IsAnyMultiplayer=function() return true end,GetParticipatingPlayerIDs=function() return {{0,1,2}} end}}
PlayerConfigurations={{[0]={{GetSlotStatus=function() return 1 end}},[1]={{GetSlotStatus=function() return 1 end}},[2]={{GetSlotStatus=function() return 2 end}}}}
SlotStatus={{SS_TAKEN=1}};ContextPtr={{IsHidden=function() return false end}};Network={{IsGameHost=function() return true end}}
CLICKS=0;UPDATES=0
Controls={{ReadyButton={{IsDisabled=function() return false end,DoLeftClick=function() CLICKS=CLICKS+1 end}}}}
UpdateReadyButton=function() UPDATES=UPDATES+1 end
LOGS={{}};print=function(s) LOGS[#LOGS+1]=s end
"#)).exec().unwrap();
    lua.load(
        fs::read_to_string(
            repo_root().join("poc/JAC_PoC_JapanMapScale/UI/JAC_NativeCarrierWitness_Staging.lua"),
        )
        .unwrap(),
    )
    .exec()
    .unwrap();
    lua.load(
        r#"
for g=0,2 do
 GENERATION=g;UpdateReadyButton();UpdateReadyButton()
 assert(CLICKS==g+1,'same context must Ready once for each loaded generation')
end
GENERATION=0;UpdateReadyButton();assert(CLICKS==3 and UPDATES==7)
"#,
    )
    .exec()
    .unwrap();
}

#[test]
fn review_wait_stage_stops_at_its_own_deadline() {
    let lua = loaded_ui(false);
    lua.load("UPDATE(119);assert(MOVE~=nil);UPDATE(1);UPDATE(1)")
        .exec()
        .unwrap();
    let log = lua
        .globals()
        .get::<mlua::Table>("LOGS")
        .unwrap()
        .sequence_values::<String>()
        .collect::<mlua::Result<Vec<_>>>()
        .unwrap()
        .join("\n");
    assert!(
        log.contains("|33|FAIL|"),
        "the fixed 120-second stage must fail while still waiting for the actual dialog"
    );
    assert!(!log.contains("|33|RESULT|"));
}

#[test]
fn review_prebegin_wait_does_not_spend_stage_deadline() {
    let lua = loaded_ui(false);
    lua.load(r#"
local player=Game.GetLocalPlayer
Game.GetLocalPlayer=function() return -1 end
UPDATE(600)
for _,s in ipairs(LOGS) do assert(not s:find('|FAIL|',1,true),'pre-Begin load must not consume the next stage deadline') end
Game.GetLocalPlayer=player
UPDATE(119)
for _,s in ipairs(LOGS) do assert(not s:find('|FAIL|',1,true)) end
UPDATE(1)
local failed=false;for _,s in ipairs(LOGS) do if s:find('|33|FAIL|',1,true) then failed=true end end
assert(failed,'active stage still has its original 120-second deadline')
"#).exec().unwrap();
}

#[test]
fn review_failed_initialization_blocks_only_the_fixture_observer() {
    let lua = loaded_ui(true);
    lua.load(r#"
assert(JAC_ObserveNativeCarrierMP({MP={Recovery='another-flow'}})==true)
assert(JAC_ObserveNativeCarrierMP({MP={Recovery='fixture-native-hotseat-996'}})==false,'failed early MOVE listener must keep fixture stopped')
"#).exec().unwrap();
    let successful = loaded_ui(false);
    successful.load("assert(JAC_ObserveNativeCarrierMP({MP={Recovery='fixture-native-hotseat-996'}})==true)").exec().unwrap();
}

#[test]
fn loaded_scene_arms_real_move_signature_before_publication_and_emits_only_transitions() {
    let lua = loaded_ui(false);
    lua.load(
        r#"
assert(ADD_ORDER[1]=='MOVE' and ADD_ORDER[2]=='CLOSE' and JAC996BorrowObserver==nil)
MOVE(2,2,0,44,7,37);MOVE(0,2,0,45,7,37);assert(#ExposedMembers.JAC996MoveEvents==0)
MOVE(0,2,0,44,7,37);assert(#ExposedMembers.JAC996MoveEvents==1)
UPDATE(0.01);local logs=#LOGS;for i=1,100 do UPDATE(0.01) end;assert(#LOGS==logs)
CLOSE();SHOWN=false;PAUSED=false;UPDATE(0.01);local final=#LOGS
for i=1,100 do UPDATE(0.01) end;assert(#LOGS==final)
"#,
    )
    .exec()
    .unwrap();
    let log = lua
        .globals()
        .get::<mlua::Table>("LOGS")
        .unwrap()
        .sequence_values::<String>()
        .collect::<mlua::Result<Vec<_>>>()
        .unwrap()
        .join("\n");
    assert!(
        log.contains("|BOOT|")
            && log.contains("|LOAD_READY|")
            && log.contains("|DIALOG_OPEN|")
            && log.contains("|DIALOG_CLOSED|")
    );
    assert!(!log.contains("|FAIL|"));
}

#[test]
fn an_already_dispatched_load_never_claims_an_early_ready_receipt() {
    let lua = loaded_ui(true);
    lua.load("UPDATE(0.01);UPDATE(0.01);assert(MOVE==nil)")
        .exec()
        .unwrap();
    let log = lua
        .globals()
        .get::<mlua::Table>("LOGS")
        .unwrap()
        .sequence_values::<String>()
        .collect::<mlua::Result<Vec<_>>>()
        .unwrap()
        .join("\n");
    assert!(log.contains("|FAIL|"));
    assert!(!log.contains("|LOAD_READY|"));
}

#[test]
fn save_generation_requires_the_actual_borrowed_city_and_survives_reload() {
    let run_id = nonce();
    for owner in [0, 1] {
        let lua = gp();
        lua.globals().set("CITY_OWNER", owner).unwrap();
        lua.load(r#"
local R=JAC_AbsorptionRules
local b=ExposedMembers.JAC_NativeCarrierWitness.Get();b.NextOperation=10;Game:SetProperty('JAC_NATIVE_CARRIER_WITNESS_V1',b)
Game:SetProperty('JAC_NATIVE_HOTSEAT_SAVE_FIXTURE_996',{Token='fixture-native-hotseat-996',Target={X=3,Y=4}})
CityManager={GetCityAt=function(x,y) assert(x==3 and y==4);return {GetOwner=function() return CITY_OWNER end,GetID=function() return 44 end} end}
JAC_AbsorptionGameplay.GetState=function() return {Recoveries={['fixture-native-hotseat-996']={NativeCarrier={MP={Phase='BORROWED',Actor=0,MoveOwner=0,Loans={{},{Owner=1,Issued=true,BorrowedID=44}}}}}}} end
"#).exec().unwrap();
        lua.load(format!("RECEIVE(0,{{Version=1,Command='FIXED',Nonce='{run_id}',Actor=0,Generation=0,Step=30,Turn=1}})")).exec().unwrap();
        let b = lua
            .globals()
            .get::<mlua::Table>("ExposedMembers")
            .unwrap()
            .get::<mlua::Table>("JAC_NativeCarrierWitness")
            .unwrap()
            .get::<mlua::Function>("Get")
            .unwrap()
            .call::<mlua::Table>(())
            .unwrap();
        assert_eq!(
            b.get::<i64>("Generation").unwrap(),
            if owner == 0 { 1 } else { 0 }
        );
        if owner == 0 {
            assert_eq!(b.get::<i64>("ResumeStep").unwrap(), 33);
            assert_eq!(
                b.get::<String>("SaveName").unwrap(),
                format!("JAC-NativeCarrier-MidLoan-{run_id}")
            );
            lua.load(witness::gameplay_binding()).exec().unwrap();
            lua.load("local b=ExposedMembers.JAC_NativeCarrierWitness.Get();assert(b.Generation==1 and b.ResumeStep==33 and b.Receipts['0:30'].Status=='COMPLETE')").exec().unwrap();
        }
    }
}

#[test]
fn scene_freezes_saved_generation_without_frontend_game_or_clock_apis() {
    let run_id = nonce();
    let common = include_str!("../src/native_carrier_witness/common.lua");
    let mut scenes = Vec::new();
    for generation in [0, 1, 2] {
        let lua = Lua::new();
        lua.load(RULES).exec().unwrap();
        lua.load(format!(r#"
GameConfiguration={{GetValue=function() return '{run_id}' end,IsHotseat=function() return true end,IsAnyMultiplayer=function() return true end}}
ContextPtr=setmetatable({{}},{{__tostring=function() return 'reusedContextAddress' end}})
LOGS={{}};print=function(s) LOGS[#LOGS+1]=s end
"#)).exec().unwrap();
        lua.load(format!("{common}\nemit('BOOT',{generation},0,JAC_AbsorptionRules.Canonical({{Text='日本語の完全記録',Generation={generation}}}))")).exec().unwrap();
        let lines = lua
            .globals()
            .get::<mlua::Table>("LOGS")
            .unwrap()
            .sequence_values::<String>()
            .collect::<mlua::Result<Vec<_>>>()
            .unwrap();
        scenes.push(lines[0].split('|').nth(3).unwrap().to_owned());
        assert!(scenes
            .last()
            .unwrap()
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.'));
        assert!(scenes.last().unwrap().ends_with(&format!(".g{generation}")));
    }
    assert_ne!(scenes[0], scenes[1]);
    assert_ne!(scenes[1], scenes[2]);
}

#[test]
fn synchronous_save_completion_waits_for_the_actual_api_return() {
    let run_id = nonce();
    let lua = loaded_ui(false);
    // Isolate save transport, not native-world acceptance. The complete-world
    // reader is replaced ONLY inside this test to exercise callback ordering.
    lua.load(format!(r#"
local function up(fn,wanted)
 for i=1,64 do local name,value=debug.getupvalue(fn,i);if not name then break end;if name==wanted then return value end end
 error('missing tested upvalue '..wanted)
end
local tick=up(UPDATE,'tick');SAVE=up(tick,'saveGame')
local actions=up(tick,'actions');actions[37].run=function() return JAC_AbsorptionRules.Canonical({{CompleteWorld='保存対象'}}) end
local oldGet=ExposedMembers.JAC_NativeCarrierWitness.Get
ExposedMembers.JAC_NativeCarrierWitness.Get=function() local b=oldGet();b.SaveName='JAC-NativeCarrier-MidLoan-{run_id}';return b end
local values={{}}
GameConfiguration.SetValue=function(k,v) values[k]=v end
GameConfiguration.GetValue=function(k) return values[k] end
SaveLocations={{LOCAL_STORAGE=0}}
Events.SaveComplete={{Add=function(f) SAVECB=f end,Remove=function() end}}
RETURNED_FROM_API=false
Network={{GetGameConfigurationSaveType=function() return 3 end,SaveGame=function() SAVECB();RETURNED_FROM_API=true;return true end}}
local originalPrint=print
print=function(s) if s:find('|SAVE_RETURNED|',1,true) or s:find('|SAVE_COMPLETE|',1,true) then assert(RETURNED_FROM_API) end;originalPrint(s) end
UPDATE(0.01);SHOWN=false;PAUSED=false
SAVE(30)
local before=#LOGS;for _,s in ipairs(LOGS) do assert(not s:find('|SAVE_COMPLETE|',1,true)) end
UPDATE(0.01);assert(#LOGS>before)
local returned,complete
for i,s in ipairs(LOGS) do if s:find('|SAVE_RETURNED|',1,true) then returned=i elseif s:find('|SAVE_COMPLETE|',1,true) then complete=complete or i end end
assert(returned and complete and returned<complete)
"#)).exec().unwrap();
}
