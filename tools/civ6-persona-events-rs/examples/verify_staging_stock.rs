//! Local primary-source verification; CI compiles this example without game files.
//! Run explicitly with the installed, hash-pinned StagingRoom.lua. No game process.
use civ6_persona_events_rs::mp_staging as staging;
use mlua::Lua;
fn function(source: &str, name: &str) -> String {
    let start = source.find(&format!("function {name}(")).unwrap();
    let rest = &source[start..];
    let end = rest[1..]
        .find("\nfunction ")
        .map(|n| n + 1)
        .unwrap_or(rest.len());
    let mut code = rest[..end].to_owned();
    // Only these selected stock functions use Firaxis' optional declaration
    // annotations; their executable statements are retained for Lua5.1.
    for kind in ["number", "boolean", "string", "table"] {
        code = code
            .replace(&format!(": {kind}"), "")
            .replace(&format!(":{kind}"), "");
    }
    code
}
fn world(source: &str, patched: bool) -> Lua {
    let lua = Lua::new();
    lua.load(r#"
print=function()end;hotseat=true;ruleset='RULESET_JAC_BANSEIKI';map='{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map';maximum=22;minimum=22
g_currentMaxPlayers=12;g_currentMinPlayers=22;MAX_EVER_PLAYERS=12;MIN_EVER_PLAYERS=2
g_hotseatNumHumanPlayers=2;g_hotseatNumAIPlayers=20;m_bTeamsValid=true;rebuilt=0;launches=0;readyWrites=0
sameTeam=false;duplicate=false;hidden=false;humans=2;count=22
SlotStatus={SS_TAKEN=3,SS_COMPUTER=1,SS_OBSERVER=4};CivilizationLevelTypes={CIVILIZATION_LEVEL_FULL_CIV=624367792};GameStateTypes={GAMESTATE_PREGAME=0,GAMESTATE_LAUNCHED=1};TeamTypes={NO_TEAM=-1}
local function ids()local v={};for i=0,count-1 do v[#v+1]=i end;return v end
GameConfiguration={IsHotseat=function()return hotseat end,GetRuleSet=function()return ruleset end,GetValue=function()return true end,GetMultiplayerPlayerIDs=ids,GetParticipatingPlayerIDs=ids,GetGameState=function()return 0 end,IsPlayByCloud=function()return false end,IsMatchMaking=function()return false end}
MapConfiguration={GetValue=function()return map end,GetMaxMajorPlayers=function()return maximum end,GetMinMajorPlayers=function()return minimum end}
PlayerConfigurations={};for i=0,21 do local id=i;PlayerConfigurations[id]={GetSlotStatus=function()return id<humans and 3 or 1 end,GetCivilizationLevelTypeID=function()return 624367792 end,GetReady=function()return false end,GetModReady=function()return false end,IsAlive=function()return true end,IsHumanRequired=function()return false end,IsParticipant=function()return true end,GetTeam=function()return sameTeam and 0 or id end}end
Network={IsPlayerHotJoining=function()return false end,GetLocalPlayerID=function()return 0 end,IsCloudLaunching=function()return false end,IsPlayerConnected=function()return false end,IsEveryoneConnected=function()return true end,IsGameHost=function()return true end,GetSessionID=function()return 7 end,GetFriends=function()return nil end,LaunchGame=function()launches=launches+1 end}
ContextPtr={IsHidden=function()return hidden end};Automation={IsActive=function()return false end}
Locale={Lookup=function(v)return v end,ToUpper=function(v)return v end}
local noop=function()end
Controls={StartLabel={SetText=noop,LocalizeAndSetText=noop},ReadyButton={SetText=noop,LocalizeAndSetToolTip=noop,SetDisabled=function(self,v)disabled=v end,IsDisabled=function()return disabled end,SetOffsetY=noop}}
IsCloudInProgress=function()return false end;IsCloudInProgressAndNotTurn=function()return false end
GetPlayerParameterError=function(id)if duplicate and id==1 then return {Id='InvalidDomainValue',Reason='LOC_SETUP_ERROR_NO_DUPLICATE_LEADERS'} end end
IsLaunchCountdownActive=function()return false end;StartLaunchCountdown=noop;StopCountdown=noop;IsUseReadyCountdown=function()return false end
InitializeReadyUI=noop;ShowHideInviteButton=noop;ShowHideTopLeftButtons=noop;RealizeGameSetup=noop;PopulateTargetPull=noop;ShowHideChatPanel=noop;UpdateFriendsList=noop;RealizeInfoTabs=noop;RealizeGridSize=noop
SetLocalReady=function()readyWrites=readyWrites+1 end
UpdateReadyButton=function()UpdateReadyButton_Hotseat()end
BuildPlayerList=function()rebuilt=rebuilt+1;g_hotseatNumHumanPlayers=humans;g_hotseatNumAIPlayers=count-humans;CheckTeamsValid();CheckGameAutoStart()end
"#).exec().unwrap();
    let names = [
        "SetCurrentMaxPlayers",
        "SetCurrentMinPlayers",
        "OnMapMaxMajorPlayersChanged",
        "OnMapMinMajorPlayersChanged",
        "CheckTeamsValid",
        "CheckGameAutoStart",
        "ResetAutoStartFlags",
        "UpdateReadyButton_Hotseat",
        "OnReadyButton",
        "OnShow",
    ];
    let mut code = if patched {
        staging::policy()
    } else {
        String::new()
    };
    for name in names {
        code.push_str(&function(source, name));
        code.push('\n');
    }
    lua.load(code).exec().unwrap();
    lua
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("pass installed StagingRoom.lua")?;
    let bytes = std::fs::read(path)?;
    let patched = staging::transform(&bytes)?;
    if let Some(bundle_dir) = std::env::args().nth(2) {
        let dir = std::path::Path::new(&bundle_dir);
        let bundle: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("bundle.json"))?)?;
        let n = bundle["nonce"].as_str().ok_or("boundary nonce missing")?;
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../civ6-autotest/scenarios");
        let normal =
            serde_json::from_slice(&std::fs::read(root.join("leader-persona-15km.json"))?)?;
        let hotseat = serde_json::from_slice(&std::fs::read(
            root.join("multiplayer-banseiki-hotseat-growth.json"),
        )?)?;
        let boundary = civ6_persona_events_rs::mp_staging_boundary::build(&normal, &hotseat, n)?;
        let observer = civ6_persona_events_rs::mp_staging_boundary::observer(&normal, &hotseat, n)?;
        let expected =
            patched.replacen("\nInitialize();", &format!("{observer}\nInitialize();"), 1);
        let actual = std::fs::read(dir.join(staging::MOD_FOLDER).join("UI/StagingRoom.lua"))?;
        assert_eq!(
            actual,
            expected.as_bytes(),
            "generated Lua differs from current Rust source"
        );
        assert_eq!(
            staging::sha(&actual),
            bundle["generated_sha256"].as_str().unwrap()
        );
        assert_eq!(
            std::fs::read_to_string(
                dir.join(staging::MOD_FOLDER)
                    .join(format!("{}.modinfo", staging::MOD_FOLDER))
            )?,
            staging::manifest()
        );
        let scene = std::fs::read(dir.join("boundary.json"))?;
        assert_eq!(
            staging::sha(&scene),
            bundle["scene_sha256"].as_str().unwrap()
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&scene)?,
            boundary
        );
        println!("PASS actual CLI bundle/current source: Lua/manifest exact bytes; scenario exact values/arrays; raw artifact hashes");
    }
    let stock = String::from_utf8(bytes)?.replace("\r\n", "\n");
    // Existing official rejection and launch logic is byte-for-byte preserved.
    for name in [
        "CheckTeamsValid",
        "CheckGameAutoStart",
        "ResetAutoStartFlags",
        "UpdateReadyButton_Hotseat",
        "OnReadyButton",
    ] {
        assert_eq!(
            function(&stock, name),
            function(&patched, name),
            "{name} changed"
        );
    }
    let old = world(&stock, false);
    old.load("OnShow();assert(g_currentMaxPlayers==12 and disabled==true and g_badPlayerForMapSize==true and launches==0)").exec()?;
    let lua = world(&patched, true);
    for (label,code) in [
        ("22 initial/reentry","OnShow();assert(g_currentMaxPlayers==22 and not disabled and launches==0);OnShow();assert(g_currentMaxPlayers==22 and not disabled)"),
        ("sameMax out of scope","map='OtherMap.lua';OnMapMaxMajorPlayersChanged(22);assert(g_currentMaxPlayers==12 and disabled and g_badPlayerForMapSize)"),
        ("Pangaea20 retains original refresh","map='Pangaea.lua';maximum=20;local before=rebuilt;OnMapMaxMajorPlayersChanged(20);OnMapMaxMajorPlayersChanged(20);assert(g_currentMaxPlayers==12 and rebuilt==before+2);maximum=22"),
        ("sameMax back in scope","map='{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map';OnMapMaxMajorPlayersChanged(22);assert(g_currentMaxPlayers==22 and not disabled)"),
        ("ruleset refresh","ruleset='RULESET_EXPANSION_2';OnMapMaxMajorPlayersChanged(22);assert(g_currentMaxPlayers==12);ruleset='RULESET_JAC_BANSEIKI';OnMapMaxMajorPlayersChanged(22);assert(g_currentMaxPlayers==22)"),
        ("network session refresh","hotseat=false;OnMapMaxMajorPlayersChanged(22);assert(g_currentMaxPlayers==12);hotseat=true;OnShow();assert(g_currentMaxPlayers==22)"),
        ("duplicate original refusal","duplicate=true;BuildPlayerList();assert(disabled and g_duplicateLeaders);duplicate=false;BuildPlayerList();assert(not disabled)"),
        ("teams original refusal","sameTeam=true;BuildPlayerList();assert(disabled and not m_bTeamsValid);sameTeam=false;BuildPlayerList();assert(not disabled)"),
        ("human/total original refusal","humans=0;BuildPlayerList();assert(disabled);humans=1;count=1;BuildPlayerList();assert(disabled);humans=2;count=22;BuildPlayerList();assert(not disabled)"),
        ("map cap still obeyed","maximum=20;OnMapMaxMajorPlayersChanged(20);assert(g_currentMaxPlayers==20 and disabled);maximum=22;OnMapMaxMajorPlayersChanged(22);assert(g_currentMaxPlayers==22 and not disabled)"),
        ("no autonomous launch and original one click","assert(launches==0 and readyWrites==0);OnReadyButton();assert(launches==1 and readyWrites==1)"),
    ]{lua.load(code).exec()?;println!("PASS {label}");}
    println!("PASS stock22 rejection reproduced; 5 official gate/Ready bodies preserved; 11 real-source Lua51 cases; no game");
    Ok(())
}
