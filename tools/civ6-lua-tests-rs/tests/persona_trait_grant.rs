//! Test-only SQL and Gameplay ordering. Native trait admission needs the live scenario.
use civ6_lua_tests_rs::strict::ChunkExt;
use civ6_lua_tests_rs::{new_lua, official, read_repo, repo_root};
use mlua::Lua;
use roxmltree::Document;
use rusqlite::Connection;
use std::collections::BTreeSet;

const ROOT: &str = "poc/JAC_Test_PersonaTraitGrant";
const SCRIPT: &str = "poc/JAC_Test_PersonaTraitGrant/Scripts/JAC_PersonaTraitGrant_Gameplay.lua";

fn count(db: &Connection, sql: &str) -> i64 {
    db.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn foreign_keys(db: &Connection) -> BTreeSet<(String, Option<i64>, String, i64)> {
    db.prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn candidate_sql_changes_only_ashina_and_has_no_static_owner_grant() {
    let db = official::official_db();
    db.execute_batch("INSERT OR IGNORE INTO Kinds (Kind) VALUES ('KIND_TRAIT');")
        .unwrap();
    for path in [
        "poc/JAC_PoC_CustomCivs/Data/JAC_CustomCivs_Gameplay.sql",
        "poc/JAC_PoC_CustomCivs/Data/JAC_Japan1534_Gameplay.sql",
    ] {
        db.execute_batch(&read_repo(path)).unwrap();
    }
    official::run_sengoku(&db);
    for path in [
        "poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona.sql",
        "poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona_Trees.sql",
    ] {
        db.execute_batch(&read_repo(path)).unwrap();
    }
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas) AND TraitType='TRAIT_JAC_PERSONA_ADMISSION_CLOSED'"
        ),
        22
    );
    let before_fk = foreign_keys(&db);
    let sql = read_repo(format!("{ROOT}/Data/JAC_PersonaTraitGrant.sql"))
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    for statement in sql.split(';').filter(|sql| !sql.trim().is_empty()) {
        db.execute_batch(statement)
            .unwrap_or_else(|error| panic!("trait grant SQL: {statement}: {error}"));
    }
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas) AND TraitType='TRAIT_JAC_PERSONA_ADMISSION_CLOSED'"
        ),
        21
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM Governors WHERE GovernorType=(SELECT GovernorType FROM JAC_LeaderPersonas WHERE LeaderType='LEADER_JAC_ASHINA_MORIKIYO') AND TraitType='TRAIT_JAC_TEST_ASHINA_JOINED'"
        ),
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM LeaderTraits WHERE TraitType='TRAIT_JAC_TEST_ASHINA_JOINED'"
        ) + count(
            &db,
            "SELECT COUNT(*) FROM CivilizationTraits WHERE TraitType='TRAIT_JAC_TEST_ASHINA_JOINED'"
        ),
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM DynamicModifiers WHERE ModifierType='MODIFIER_JAC_TEST_OWNER_GRANT_TRAIT' AND CollectionType='COLLECTION_OWNER' AND EffectType='EFFECT_GRANT_TRAIT'"
        ),
        1
    );
    let after_fk = foreign_keys(&db);
    assert!(after_fk.difference(&before_fk).next().is_none());

    let manifest = read_repo(format!("{ROOT}/JAC_Test_PersonaTraitGrant.modinfo"));
    let doc = Document::parse(&manifest).unwrap();
    assert_eq!(
        doc.root_element().attribute("id"),
        Some("2b20f0d9-64cd-42b8-9841-74067419b81e")
    );
    let text = manifest.as_str();
    assert!(text.contains("RULESET_JAC_BANSEIKI"));
    assert!(text.contains("JAC_Japan_15km.Civ6Map"));
    assert!(text.contains("Data/JAC_PersonaTraitGrant.sql"));
    assert!(text.contains("Scripts/JAC_PersonaTraitGrant_Gameplay.lua"));
    assert!(repo_root().join(SCRIPT).is_file());
}

fn fixture() -> Lua {
    let lua = new_lua();
    lua.load(
        r#"
local leader='LEADER_JAC_ASHINA_MORIKIYO'
ORDER={}; PROPERTIES={}; JOIN_OK=true; POINTS=1; SPENT=0; ATTACHED=0
Game={GetProperty=function(_,key) return PROPERTIES[key] end,
      SetProperty=function(_,key,value) PROPERTIES[key]=value end}
GameEvents={JAC_TestPersonaTraitGrantPrepare={Add=function(f) HANDLER=f end}}
local player={IsHuman=function() return true end, IsAlive=function() return true end,
    GetCulture=function() return {} end,
    GetGovernors=function() return {GetGovernorPoints=function() return POINTS end,
                              GetGovernorPointsSpent=function() return SPENT end} end,
    AttachModifierByID=function(_,modifier)
        assert(modifier=='JAC_TEST_ASHINA_JOINED_GRANT')
        ORDER[#ORDER+1]='attach'; ATTACHED=ATTACHED+1
    end}
Players={[0]=player,[2]={}}
PlayerManager={GetAliveMajorIDs=function() return {0,2} end}
PlayerConfigurations={[0]={GetLeaderTypeName=function() return 'LEADER_JAC_HOJO_UJITSUNA' end},
                      [2]={GetLeaderTypeName=function() return leader end}}
GameInfo={JAC_LeaderPersonas={[leader]={EvolutionCost=1}},
          JAC_PersonaTitleCivics=function() return function() return nil end end}
ExposedMembers={JAC_LeaderPersona={
    Join=function(owner,source)
        ORDER[#ORDER+1]='join'
        if not JOIN_OK then return 'INVALID' end
        assert(owner==0 and source==2)
        PROPERTIES['JAC_PERSONA_'..leader..'_OWNER']=owner
        PROPERTIES['JAC_PERSONA_'..leader..'_STATE']='GREAT_PERSON'
        return 'JOINED'
    end,
    PrepareEvolution=function(owner,who)
        ORDER[#ORDER+1]='reserve'
        assert(owner==0 and who==leader and POINTS-SPENT>=1)
        PROPERTIES['JAC_PERSONA_'..leader..'_STATE']='EVOLVING'
        return 'EVOLVING'
    end}}
function fire(owner) HANDLER(owner) end
function print() end
"#,
    )
    .exec()
    .unwrap();
    lua.load(read_repo(SCRIPT)).set_name(SCRIPT).exec().unwrap();
    lua
}

#[test]
fn owner_modifier_is_attached_only_after_real_join_and_full_reservation() {
    let lua = fixture();
    lua.load("fire(0)").exec().unwrap();
    let (order, attached, owner, state): (String, i64, i64, String) = lua
        .load(
            "return table.concat(ORDER,','),ATTACHED,PROPERTIES.JAC_PERSONA_LEADER_JAC_ASHINA_MORIKIYO_OWNER,PROPERTIES.JAC_PERSONA_LEADER_JAC_ASHINA_MORIKIYO_STATE",
        )
        .strict_eval()
        .unwrap();
    assert_eq!(order, "join,reserve,attach");
    assert_eq!(attached, 1);
    assert_eq!(owner, 0);
    assert_eq!(state, "EVOLVING");

    let rejected = fixture();
    rejected.load("JOIN_OK=false").exec().unwrap();
    assert!(rejected.load("fire(0)").exec().is_err());
    let (order, attached): (String, i64) = rejected
        .load("return table.concat(ORDER,','),ATTACHED")
        .strict_eval()
        .unwrap();
    assert_eq!(order, "join");
    assert_eq!(attached, 0);

    let no_title = fixture();
    no_title.load("POINTS=0").exec().unwrap();
    assert!(no_title.load("fire(0)").exec().is_err());
    let (order, attached): (String, i64) = no_title
        .load("return table.concat(ORDER,','),ATTACHED")
        .strict_eval()
        .unwrap();
    assert_eq!(order, "join");
    assert_eq!(attached, 0);
}
