mod support_fixture;

use mlua::Lua;
use support_fixture::{action, code, scenario, steps};

fn engine() -> Lua {
    let lua = Lua::new();
    lua.load(
        r#"
        local function rows(values)
            return function()
                local i=0
                return function() i=i+1; return values[i] end
            end
        end
        owned={}; calls={}; points=3; spent=2; failResearch=false; leakTarget=false
        Game={GetProperty=function() return nil end}
        GameInfo={
            Governments={GOVERNMENT_MERCHANT_REPUBLIC={PrereqCivic='TARGET'}},
            Civics={ROOT={Index=1,CivicType='ROOT'},PASSAGE={Index=2,CivicType='PASSAGE'},TARGET={Index=3,CivicType='TARGET'}},
            CivicPrereqs=rows({{Civic='TARGET',PrereqCivic='PASSAGE'},{Civic='PASSAGE',PrereqCivic='ROOT'}}),
            CivicModifiers=rows({{CivicType='ROOT',ModifierId='TITLE'}}),
            Modifiers={TITLE={ModifierType='GRANT'}},
            DynamicModifiers={GRANT={EffectType='EFFECT_ADJUST_PLAYER_GOVERNOR_POINTS'}},
            ModifierArguments=rows({{ModifierId='TITLE',Name='Delta',Value='1'}})
        }
        local culture={
            HasCivic=function(_,i) return owned[i]==true end,
            SetCivic=function(_,i,value)
                assert(i~=3 and value==true,'target must remain for regular fixture')
                if i==2 then assert(owned[1],'ancestor researched after child') end
                calls[#calls+1]=i
                if not failResearch then owned[i]=true end
                if i==1 then points=points+1 end
                if leakTarget then owned[3]=true end
            end
        }
        local governors={GetGovernorPoints=function() return points end,GetGovernorPointsSpent=function() return spent end}
        Players={[0]={GetCulture=function() return culture end,GetGovernors=function() return governors end}}
        "#,
    )
    .exec()
    .unwrap();
    lua
}

fn preparation() -> String {
    let data = scenario("leader-persona-spy-neutralization-retry-capacity-tier2-civic.json");
    code(
        steps(&data)
            .iter()
            .find(|s| action(s, "ui_lua") && s["state_name"] == "GameCore_Tuner")
            .unwrap(),
    )
    .to_owned()
}

#[test]
fn researches_ancestors_in_order_and_preserves_target_and_spending() {
    let lua = engine();
    lua.load(preparation()).eval::<String>().unwrap();
    lua.load("assert(#calls==2 and calls[1]==1 and calls[2]==2 and points==4 and spent==2 and not owned[3])")
        .exec()
        .unwrap();
    lua.load(preparation()).eval::<String>().unwrap();
    lua.load("assert(#calls==2 and points==4 and not owned[3])")
        .exec()
        .unwrap();
}

#[test]
fn cycles_fail_before_any_research_and_unaccepted_research_is_rejected() {
    let lua = engine();
    lua.load("GameInfo.CivicPrereqs=function() local rows={{Civic='TARGET',PrereqCivic='PASSAGE'},{Civic='PASSAGE',PrereqCivic='TARGET'}}; local i=0; return function() i=i+1; return rows[i] end end")
        .exec()
        .unwrap();
    assert!(lua.load(preparation()).eval::<String>().is_err());
    lua.load("assert(#calls==0 and points==3)").exec().unwrap();
    for flag in ["failResearch", "leakTarget"] {
        let lua = engine();
        lua.globals().set(flag, true).unwrap();
        assert!(lua.load(preparation()).eval::<String>().is_err());
    }
}
