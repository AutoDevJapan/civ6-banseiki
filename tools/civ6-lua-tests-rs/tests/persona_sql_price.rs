//! Rebuild separate databases from real old/new SQL and price parameters.
//! Saved engine flags and Game properties are restored without replaying a debit.
use civ6_lua_tests_rs::strict::{ChunkExt, FromLuaMulti, TableExt};
use civ6_lua_tests_rs::{json_to_lua, official, query_json, read_repo};
use mlua::{Lua, MultiValue, Table, Value};
use serde_json::{json, Value as Json};
use std::sync::LazyLock;

const SQL: &str = "poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona.sql";
const GP: &str = "poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua";
const UI: &str = "poc/JAC_PoC_LeaderPersona/UI/JAC_LeaderPersona_View.lua";
const OLD_SQL: &str = include_str!("../fixtures/persona/persona-d21.sql");
static CATALOGS: LazyLock<Vec<Json>> = LazyLock::new(|| {
    let latest = read_repo(SQL);
    std::iter::once(database(OLD_SQL, None))
        .chain((1..=3).map(|price| database(&latest, Some(price))))
        .collect()
});

fn priced(source: &str, price: Option<u8>) -> String {
    match price {
        None => source.to_owned(),
        Some(n) => source
            .replace("('BASE_TITLES', 1)", &format!("('BASE_TITLES', {n})"))
            .replace("('FAME_TITLE_WEIGHT', 1)", "('FAME_TITLE_WEIGHT', 0)")
            .replace(
                "('YOUTH_TITLE_SURCHARGE', 1)",
                "('YOUTH_TITLE_SURCHARGE', 0)",
            )
            .replace(
                "('REPRESENTATIVE_TITLE_SURCHARGE', 1)",
                "('REPRESENTATIVE_TITLE_SURCHARGE', 0)",
            ),
    }
}

fn database(source: &str, price: Option<u8>) -> Json {
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
    db.execute_batch(&priced(source, price)).unwrap();
    let rows = query_json(
        &db,
        "SELECT rowid-1 AS 'Index',* FROM JAC_LeaderPersonas ORDER BY rowid",
    );
    let governors=query_json(&db,"SELECT g.rowid-1 AS 'Index',g.*,t.Hash FROM Governors g JOIN Types t ON t.Type=g.GovernorType ORDER BY g.rowid");
    let people=query_json(&db,"SELECT p.rowid-1 AS 'Index',p.*,t.Hash FROM GreatPersonIndividuals p JOIN Types t ON t.Type=p.GreatPersonIndividualType WHERE p.GreatPersonIndividualType IN (SELECT PersonType FROM JAC_LeaderPersonas) ORDER BY p.rowid");
    let promotions=query_json(&db,"SELECT p.rowid-1 AS 'Index',p.*,t.Hash FROM GovernorPromotions p JOIN Types t ON t.Type=p.GovernorPromotionType ORDER BY p.rowid");
    let sets = query_json(&db, "SELECT * FROM GovernorPromotionSets ORDER BY rowid");
    let modifiers = query_json(
        &db,
        "SELECT * FROM GovernorPromotionModifiers ORDER BY rowid",
    );
    json!({"rows":rows,"governors":governors,"people":people,"promotions":promotions,"sets":sets,"modifiers":modifiers})
}

fn exec(lua: &Lua, source: &str) {
    lua.load(source).exec().unwrap();
}

fn world(data: &Json) -> Lua {
    let lua = Lua::new();
    for path in [
        "tools/civ6-lua-tests-rs/fixtures/persona/stubs.lua",
        "tools/civ6-lua-tests-rs/fixtures/persona/admission_engine.lua",
        "tools/civ6-lua-tests-rs/fixtures/persona/production_admission_engine.lua",
    ] {
        lua.load(read_repo(path)).exec().unwrap();
    }
    civ6_lua_tests_rs::persona_engine::prepare(&lua).unwrap();
    lua.globals()
        .set("SQL_DATA", json_to_lua(&lua, data))
        .unwrap();
    exec(
        &lua,
        r#"
ROWS,GOVDEFS,INDIVIDUALS,PROMOS=SQL_DATA.rows,SQL_DATA.governors,SQL_DATA.people,SQL_DATA.promotions
GameInfo.JAC_LeaderPersonas=collection(ROWS,'LeaderType')
GameInfo.Governors=collection(GOVDEFS,'GovernorType')
GameInfo.GreatPersonIndividuals=collection(INDIVIDUALS,'GreatPersonIndividualType')
GameInfo.GovernorPromotions=collection(PROMOS,'GovernorPromotionType')
GameInfo.GovernorPromotionSets=collection(SQL_DATA.sets,'GovernorPromotion')
GameInfo.GovernorPromotionModifiers=collection(SQL_DATA.modifiers,'GovernorPromotionType')
for id=0,21 do
 local row=ROWS[id+1]; local cfg=PlayerConfigurations[id]
 cfg.leader=row.LeaderType; cfg.civ=row.CivilizationType
 cfg.GetLeaderTypeName=function() return row.LeaderType end
 cfg.GetCivilizationTypeName=function() return row.CivilizationType end
 Players[id].points=100
end
LOCAL=0
function sourceFor(cost,except)
 for id=1,21 do if ROWS[id+1].EvolutionCost==cost and ROWS[id+1].LeaderType~=except then return id end end
 error('price subject missing')
end
function deliver()
 Players[0].active=false; local result=STEP(); Players[0].active=true
 assert(result and #REQUESTS==0,'operation did not settle once')
end
"#,
    );
    lua.load(read_repo(
        "poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua",
    ))
    .exec()
    .unwrap();
    lua
}

fn boot(lua: &Lua) {
    lua.load(read_repo(GP)).exec().unwrap();
    lua.load(read_repo(UI)).exec().unwrap();
}

fn transfer(lua: &Lua, value: Value) -> Value {
    match value {
        Value::Table(t) => {
            let out = lua.create_table().unwrap();
            for pair in t.pairs::<Value, Value>() {
                let (k, v) = pair.unwrap();
                out.set(transfer(lua, k), transfer(lua, v)).unwrap();
            }
            Value::Table(out)
        }
        Value::String(s) => Value::String(lua.create_string(s.as_bytes()).unwrap()),
        other => other,
    }
}

fn restore(a: &Lua, data: &Json, paid: u8) -> Lua {
    let b = world(data);
    let props: Table = a.globals().strict_get("Game").unwrap();
    let getter: mlua::Function = props.strict_get("GetProperty").unwrap();
    // The engine fixture's property store is restored key by key. These are
    // serialized Game values, not success events or a substitute title balance.
    let names:Table=a.load(r#"
local out={'JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1','JAC_PERSONA_ADMISSION_SCHEMA_V1','JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA'}
for row in GameInfo.JAC_LeaderPersonas() do
 for _,field in ipairs({'SOURCE','NAME','PORTRAIT','PERSONALITY','TRAIT','EVOLUTION_COST','UNIT','OWNER','STATE','MERIT','EVOLUTION_SURCHARGE','NODE_PRESERVED'}) do
  out[#out+1]='JAC_PERSONA_'..row.LeaderType..'_'..field
 end
end
return out
"#).strict_eval().unwrap();
    for name in names.strict_sequence_values::<String>() {
        let name = name.unwrap();
        let value = Value::from_results(
            getter
                .call::<MultiValue>((props.clone(), name.clone()))
                .unwrap(),
        )
        .unwrap();
        let game: Table = b.globals().strict_get("Game").unwrap();
        let setter: mlua::Function = game.strict_get("SetProperty").unwrap();
        setter
            .call::<()>((game, name, transfer(&b, value)))
            .unwrap();
    }
    let leader: String = a.globals().strict_get("LEADER").unwrap();
    b.globals().set("LEADER", leader).unwrap();
    b.globals().set("PAID", paid).unwrap();
    exec(
        &b,
        r#"
ROW=GameInfo.JAC_LeaderPersonas[LEADER]; KEY='JAC_PERSONA_'..LEADER..'_'
Players[0].spent=PAID
if PAID>0 then
 local gov=NATIVE_CANDIDATE(0,LEADER,false)
 for step=1,PAID-1 do
  local p=assert(GameInfo.GovernorPromotions[ROW.BasePromotionType..'_TITLE_'..step],'old payment disappeared from rebuilt SQL')
  gov.promotions[p.Hash]=true
 end
end
if Game:GetProperty(KEY..'STATE')~='GOVERNOR' then
 local individual=GameInfo.GreatPersonIndividuals[ROW.PersonType]
 Game.GetGreatPeople():GrantPerson(individual.Hash,4000,5000,0,0,false)
end
"#,
    );
    boot(&b);
    b
}

#[test]
fn real_sql_rebuild_preserves_d21_and_price_v1_saves_for_prices_123() {
    let old_default = &CATALOGS[0];
    let rebuilt = &CATALOGS[1..];
    for version in 0..3 {
        for old in 1..=3 {
            let before = if version < 2 {
                old_default
            } else {
                &rebuilt[usize::from(old - 1)]
            };
            for new in (1..=3).filter(|n| *n != old) {
                for paid in 0..=old {
                    let a = world(before);
                    boot(&a);
                    a.globals().set("OLD", old).unwrap();
                    exec(
                        &a,
                        r#"
SOURCE=sourceFor(OLD); LEADER=PlayerConfigurations[SOURCE]:GetLeaderTypeName()
ROW=GameInfo.JAC_LeaderPersonas[LEADER]; KEY='JAC_PERSONA_'..LEADER..'_'
assert(ExposedMembers.JAC_LeaderPersona.Join(0,SOURCE)=='JOINED')
assert(ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,LEADER)=='EVOLVING')
"#,
                    );
                    for _ in 0..paid {
                        exec(&a, "assert(JAC_AppointLeader(LEADER)); deliver()");
                    }
                    exec(&a,"ExposedMembers.JAC_LeaderPersona.Reconcile(); Game:SetProperty(KEY..'MERIT',37); Game:SetProperty(KEY..'NODE_PRESERVED',1)");
                    if version == 0 {
                        exec(&a,"local s=Game:GetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'); s.prices=nil; s.priceVersion=nil; Game:SetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1',s)");
                    }
                    let b = restore(&a, &rebuilt[usize::from(new - 1)], paid);
                    b.globals().set("OLD", old).unwrap();
                    b.globals().set("NEW", new).unwrap();
                    exec(
                        &b,
                        r#"
local api=ExposedMembers.JAC_LeaderPersona
assert(api.EventsGameplayStatus(0,LEADER).active,'rebuilt SQL closed the saved catalog')
assert(api.EvolutionCost(LEADER)==OLD and ROW.EvolutionCost==NEW,'SQL repricing changed the joining price')
assert(api.EventsGameplayStatus(0,LEADER).reserved==OLD-PAID)
local def=GameInfo.Governors[ROW.GovernorType]
api.Reconcile(); assert(Players[0]:GetProperty(901000+def.Index)==true,'appointment qualification lost')
assert(Players[1]:GetProperty(905000+def.Index)==false,'other owner inherited payment')
if PAID<OLD then
 assert(Game:GetProperty(KEY..'STATE')=='EVOLVING' and DESTROYS==0)
 assert(not Players[0]:GetProperty(905000+def.Index),'partial payment counted as complete')
 for step=PAID,OLD-1 do assert(JAC_AppointLeader(LEADER)); deliver() end
 assert(JAC_AppointLeader(LEADER)); deliver()
end
assert(Game:GetProperty(KEY..'STATE')=='GOVERNOR' and Players[0].spent==OLD)
assert(Players[0]:GetProperty(905000+def.Index)==true,'placement qualification lost')
assert(Game:GetProperty(KEY..'MERIT')==37 and Game:GetProperty(KEY..'NODE_PRESERVED')==1)
api.Reconcile(); assert(#REQUESTS==0 and Players[0].spent==OLD,'old operation was replayed')
-- New joins use the new SQL formula, while the old contract remains fixed.
local source=sourceFor(NEW,LEADER); local fresh=PlayerConfigurations[source]:GetLeaderTypeName()
assert(api.Join(0,source)=='JOINED' and api.EvolutionCost(fresh)==NEW)
assert(api.EvolutionCost(LEADER)==OLD)
"#,
                    );
                }
            }
        }
    }
}

#[test]
fn rebuilt_sql_rejects_unknown_cross_person_owner_and_damaged_receipts() {
    let a = world(&CATALOGS[0]);
    boot(&a);
    exec(
        &a,
        r#"
SOURCE=sourceFor(2); LEADER=PlayerConfigurations[SOURCE]:GetLeaderTypeName()
ROW=GameInfo.JAC_LeaderPersonas[LEADER]; KEY='JAC_PERSONA_'..LEADER..'_'
assert(ExposedMembers.JAC_LeaderPersona.Join(0,SOURCE)=='JOINED')
assert(ExposedMembers.JAC_LeaderPersona.PrepareEvolution(0,LEADER)=='EVOLVING')
assert(JAC_AppointLeader(LEADER)); deliver(); ExposedMembers.JAC_LeaderPersona.Reconcile()
"#,
    );
    for fault in [
        "Game:SetProperty(KEY..'EVOLUTION_COST',nil)",
        "Game:SetProperty(KEY..'EVOLUTION_COST',1); Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',0)",
        "Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',nil)",
        "local s=Game:GetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'); s.prices[LEADER].cost=1; Game:SetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1',s)",
        "local s=Game:GetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'); s.people[LEADER].acquired[1].raw[3].value=4095; Game:SetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1',s)",
    ] {
        let b=restore(&a,&CATALOGS[1],1); exec(&b,fault);
        exec(&b,r#"
ExposedMembers.JAC_LeaderPersona.Reconcile()
ExposedMembers.JAC_LeaderPersona.CompleteEvolution(0,LEADER)
assert(Game:GetProperty(KEY..'STATE')=='EVOLVING' and Players[0].spent==1 and DESTROYS==0 and #REQUESTS==0)
assert(not Players[0]:GetProperty(905000+GameInfo.Governors[ROW.GovernorType].Index))
"#);
    }
    let b = restore(&a, &CATALOGS[1], 1);
    exec(
        &b,
        r#"
local api=ExposedMembers.JAC_LeaderPersona
local gov=GameInfo.Governors[ROW.GovernorType]
local base=GameInfo.GovernorPromotions[ROW.BasePromotionType]
local title=GameInfo.GovernorPromotions[ROW.BasePromotionType..'_TITLE_1']
local other=ROWS[1].LeaderType==LEADER and ROWS[2] or ROWS[1]
local otherGov=GameInfo.Governors[other.GovernorType]
local otherPromotion=GameInfo.GovernorPromotions[other.BasePromotionType]
FIREUI('GovernorPromoted',1,gov.Index,title.Index)
FIREUI('GovernorPromoted',1,gov.Index,otherPromotion.Index)
FIREUI('GovernorPromoted',1,otherGov.Index,4095)
FIREUI('GovernorPromoted','0',gov.Index,title.Index)
FIREUI('GovernorAppointed',0,gov.Index,99)
assert(api.EventsGameplayStatus(0,LEADER).paid==1 and api.EvolutionCost(LEADER)==2)
assert(Game:GetProperty(KEY..'STATE')=='EVOLVING' and DESTROYS==0)
local s=Game:GetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1')
local serial=s.accounts[0].serial
FIREUI('GovernorPromoted',0,gov.Index,base.Index); FIREUI('GovernorAppointed',0,gov.Index)
assert(Game:GetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1').accounts[0].serial==serial,'owned base was charged twice')
assert(JAC_AppointLeader(LEADER)); deliver(); assert(JAC_AppointLeader(LEADER)); deliver()
assert(Game:GetProperty(KEY..'STATE')=='GOVERNOR' and Players[0].spent==2 and DESTROYS==1)
assert(Players[1]:GetProperty(905000+gov.Index)==false)
"#,
    );
}
