//! Reads normal ledger and real rowcatalog; creates no join or payment state.
use super::*;
pub const READ: &str = r#"
assert(Game.GetLocalPlayer()==0 and Players[0]:IsTurnActive(),'reader requires local human turn')
for _,key in ipairs({'JAC_TEST853_ADMISSION_ACTIVE','JAC_TEST853_ADMISSION_MANUAL_APPOINT','JAC_TEST853_PAID_ASSIGNMENT_V1','JAC_TEST324_EVENTS_PROBE'}) do
 assert(Game:GetProperty(key)==nil,'private fixture forbidden: '..key)
end
local api=assert(ExposedMembers.JAC_LeaderPersona,'normal Gameplay absent')
local target=nil; local count=0
for row in GameInfo.JAC_LeaderPersonas() do
 count=count+1
 if row.LeaderType=='LEADER_JAC_DATE_TANEMUNE' then assert(not target,'duplicate Date'); target=row end
end
assert(count==22 and target and target.EvolutionCost==2,'actual rowcatalog differs')
local def=assert(GameInfo.Governors[target.GovernorType]);local base=assert(GameInfo.GovernorPromotions[target.BasePromotionType])
local title=assert(GameInfo.GovernorPromotions[target.BasePromotionType..'_TITLE_1'])
local function integer(n) return type(n)=='number' and n>=0 and n==math.floor(n) end
assert(integer(def.Index) and integer(base.Index) and integer(title.Index),'actual Index missing')
local turn=Game.GetCurrentGameTurn();assert(integer(turn),'turn missing')
local candidates={}
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local id=p:GetID()
 if p:IsHuman()==false and p:IsAlive()==true and p:IsMajor()==true then
  local s=api.EventsGameplayStatus(id,target.LeaderType)
  local g=p:GetGovernors();local spent=g and g:GetGovernorPointsSpent();local total=g and g:GetGovernorPoints()
  assert(s.active==true and s.source=='civ6-persona-events-rs/v3/normal-Gameplay','normal ledger unavailable')
  assert(s.registrations.GovernorAppointed and s.registrations.GovernorPromoted,'normal subscriptions absent')
  assert(integer(id) and integer(spent) and s.spent==spent,'actual native/normal Spent differs')
  assert(integer(total) and total>=spent,'actual total unavailable')
  assert(s.paid==nil or (integer(s.paid) and s.paid<=target.EvolutionCost),'normal personal paid invalid')
  candidates[#candidates+1]={id=id,available=total-spent,json='{"owner":'..id..',"index":'..def.Index..',"base":'..base.Index..',"title":'..title.Index..',"cost":2,"paid":'..(s.paid==nil and 'null' or tostring(s.paid))..',"spent":'..spent..',"total":'..total..',"turn":'..turn..',"source":"'..s.source..'","ledger_active":true,"human":false,"alive":true,"major":true}'}
 end
end
table.sort(candidates,function(a,b)if a.available~=b.available then return a.available>b.available end;return a.id<b.id end)
local rows={};for _,c in ipairs(candidates) do rows[#rows+1]=c.json end
assert(#rows>0,'no eligible actual AI owner')
return 'AI1466_READER_CANDIDATES {"candidates":['..table.concat(rows,',')..']}'
"#;
impl Runner {
    pub(super) fn ai_reader_step(&mut self, step: &Value, deadline: f64) -> Result<String> {
        let expected = self
            .resident_runtime_identity
            .ok_or_else(|| Error::input("resident reader identity absent"))?;
        if self.backend.validate_native_bridge_target()? != Some(expected) {
            return Err(Error::input("reader process identity changed"));
        }
        let request = if step["action"] == "start_native_ai_reader" {
            let raw = self.lua("InGame", READ, self.remaining(deadline)?.min(15.0))?;
            let mut value: Value = serde_json::from_str(
                raw.trim()
                    .strip_prefix("AI1466_READER_CANDIDATES ")
                    .ok_or_else(|| Error::input("actual reader response missing"))?,
            )?;
            civ6_native_bridge_rs::ai_reader::candidates(&value).map_err(Error::input)?;
            value["action"] = "start_native_ai_reader".into();
            value
        } else {
            json!({"action":"stop_native_ai_reader"})
        };
        if self.remaining(deadline)? < 25.0 {
            return Err(Error::input(
                "reader lacks its bounded controller-response budget",
            ));
        }
        let receipt = self.backend.native_ai_diagnostic(&request)?;
        self.remaining(deadline)?;
        Ok(format!(
            "actual reader registration only; world/install/drain unknown; {receipt}"
        ))
    }
}
