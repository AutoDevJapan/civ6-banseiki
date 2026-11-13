// The production accounting path requires the normal 22-person, fresh-game
// fixture. Permission-revocation coverage must not manufacture a private UI
// receipt just to get past that boundary.
#[test]
fn nonthrowing_appointment_rejection_revokes_both_native_permissions() {
    for failure in ["schema", "collision", "readback"] {
        let lua = normal_context(r#"
            appointment,paid={},{}
            injected=false
            for owner=0,21 do
                appointment[owner]={}; paid[owner]={}
                local p=Players[owner]; local getter=p.GetProperty
                p.GetProperty=function(self,key)
                    if key==902000+owner then appointment[owner]={} end
                    if key==907000+owner then paid[owner]={} end
                    local value=getter(self,key)
                    if injected and failure=='readback' and owner==1 and key==900005 then return false end
                    if type(key)=='number' and key>=901000 and key<901256 then appointment[owner][key-901000]=value==true end
                    if type(key)=='number' and key>=905000 and key<905256 then paid[owner][key-905000]=value==true end
                    return value
                end
            end
        "#);
        lua.globals().set("failure", failure).unwrap();
        run(&lua, r#"
            local api=ExposedMembers.JAC_LeaderPersona
            local leader,row=normalJoin(0,3)
            used[0]=1; success(0,row,0); appointed(0,row); finalize(0,leader)
            assert(playerProps[0].JAC_PERSONA_RESULT=='EVOLVED')
            local other,otherRow=normalJoin(1,0)
            local index=GameInfo.Governors[row.GovernorType].Index
            local otherIndex=GameInfo.Governors[otherRow.GovernorType].Index
            assert(index==32 and otherIndex==5)
            -- Enable only the native paid projection after ordinary Gameplay
            -- acquisition and finalization have supplied its real preconditions.
            Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true)
            api.Reconcile()
            assert(appointment[0][index] and paid[0][index] and appointment[1][otherIndex],
                'precondition must prove both permissions open')
            local before=used[0]
            if failure=='schema' then Game:SetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1',99) end
            if failure=='collision' then
                Game:SetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1',nil)
                Players[1]:SetProperty(902001,'foreign')
            end
            injected=true; api.Reconcile()
            for owner=0,21 do
                for _,values in ipairs({appointment[owner],paid[owner]}) do
                    for _,allowed in pairs(values) do
                        assert(not allowed,'rejection stranded native permission: '..failure)
                    end
                end
            end
            assert(used[0]==before,'rejection changed official payment')
            injected=false
            Game:SetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1',3)
            Players[1]:SetProperty(902001,true)
            api.Reconcile()
            assert(appointment[0][index] and paid[0][index] and appointment[1][otherIndex],
                'successful projection did not recover both permissions')
            assert(used[0]==before and uiReads==0 and operations==0,'recovery replayed payment or consulted UI')
        "#);
    }
}
