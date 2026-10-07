//! Minimal synchronized Gameplay binding, generated from the Rust protocol.
//! Normal publication requires the validated Gameplay acquisition/payment set.
//! The explicit experiment keeps its old inspection-only opt-in contract; it
//! never enables or bootstraps normal accounting. Neither path proves that a
//! native runtime is installed: Rust still checks the current native Governor.
//! The output cache lives only in this Lua context. A native-only session
//! rebuild must request canonical republication or reload Lua. No native epoch
//! is inferred from saved properties or claimed observed by this glue.
use crate::paid_catalog::{BITS, COMMIT_KEY, COUNT, EXPERIMENT, SCHEMA, WIDTH};

pub fn source() -> String {
    TEMPLATE
        .replace("@BITS@", &BITS.to_string())
        .replace("@WIDTH@", &WIDTH.to_string())
        .replace("@COUNT@", &COUNT.to_string())
        .replace("@COMMIT@", &COMMIT_KEY.to_string())
        .replace("@SCHEMA@", SCHEMA)
        .replace("@EXPERIMENT@", EXPERIMENT)
}

const TEMPLATE: &str = r#"-- BEGIN RUST GENERATED PAID ASSIGNMENT GLUE
-- Regenerate with civ6-native-bridge-rs/generate-paid-glue.
-- Volatile acknowledged output, never authority; a new Lua context loses it.
local normalPaidPublished;
local function ClosePaidAdmissionOwners()
    normalPaidPublished = nil;
    local confirmed = true;
    for owner=0,63 do
        -- Protect acquisition of the player and method as well as the call.
        -- Failure in either namespace must not skip the other or next owner.
        for _,key in ipairs({907000 + owner, ADMISSION_RESET + owner}) do
            local ok = pcall(function()
                local p = Player(owner, false);
                if p then p:GetProperty(key); end
            end);
            if not ok then confirmed = false; end
        end
    end
    admissionSnapshots = {};
    return confirmed;
end
local function NormalPaidPublicationActive(ids, rows)
    if not PersonaEvents or PersonaEvents.Enabled() or
       Game:GetProperty('JAC_TEST853_ADMISSION_ACTIVE') ~= nil or
       Game:GetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT') ~= nil or
       type(PersonaEvents.NormalStatus) ~= 'function' or
       type(PersonaEvents.NormalPaid) ~= 'function' then return false; end
    local view = PersonaEvents.NormalStatus(ids[1], rows[1].row.LeaderType);
    return type(view) == 'table' and view.active == true;
end
local function PaidPublicationEligible(owner, row, forceClosed, diagnostic)
    local cost = PersonaEvents.Cost(row);
    if forceClosed or Get(row.LeaderType, 'OWNER') ~= owner or Get(row.LeaderType, 'STATE') ~= 'GOVERNOR' or
       Get(row.LeaderType, 'UNIT') ~= nil or not cost then
        return false;
    end
    local person, ready, count = FindPerson(owner, row);
    if not ready or person ~= nil or count ~= 0 then return false; end
    -- A diagnostic true cell only requests native inspection. Normal cells
    -- additionally require this person's validated, confirmed Gameplay set.
    if diagnostic then return true; end
    local paid, readable = PersonaEvents.NormalPaid(owner, row, cost);
    return readable == true and paid == cost;
end
local function PublishProductionAdmission(forceClosed, onlyOwner)
    local close = ClosePaidAdmissionOwners;
    local schema = Game:GetProperty("@SCHEMA@");
    local diagnostic = Game:GetProperty("@EXPERIMENT@") == true;
    local ids = PlayerManager.GetAliveMajorIDs();
    if not AdmissionOwnerIDsValid(ids) or (schema ~= nil and schema ~= 1) then close(); return; end
    local rows = AdmissionRows();
    if not rows or #rows ~= @COUNT@ then close(); return; end
    if not diagnostic then
        -- A private appointment experiment cannot reuse normal paid authority,
        -- including a previously cached true from before that flag appeared.
        if Game:GetProperty('JAC_TEST853_ADMISSION_ACTIVE') ~= nil or
           Game:GetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT') ~= nil then
            -- Normal mode closes even a schemaless old native generation.
            -- Only the independently opted-in event diagnostic with no paid
            -- namespace/cache uses its existing appointment reset transaction.
            if not PersonaEvents.Enabled() or schema ~= nil or normalPaidPublished then
                if not close() then return; end
            end
            -- Preserve the separately opted-in event diagnostic's appointment
            -- publication, without publishing any paid catalog or permission.
            if PersonaEvents.Enabled() and SyncAppointmentAdmission(forceClosed, onlyOwner) ~= true then close(); end
            return;
        end
        -- Bootstrap the normal event store only at the existing all-closed
        -- appointment boundary. Never set the experiment flag to get there.
        -- On reload, close the previous paid generation before opening cells.
        if schema ~= nil and not normalPaidPublished and not close() then return; end
        local normalActive = NormalPaidPublicationActive(ids, rows);
        if normalPaidPublished or not normalActive then
            -- A fresh store needs the initial appointment handshake. A valid
            -- restored store gets that handshake in the full transaction below.
            if SyncAppointmentAdmission(forceClosed, onlyOwner) ~= true then close(); return; end
            normalActive = NormalPaidPublicationActive(ids, rows);
        end
        if not normalActive then
            if schema ~= nil then
                if not close() then return; end
                -- The independent event diagnostic may still have a valid
                -- appointment path, exactly as when paid publication was off.
                -- Closing paid state must not silently opt that path in or out.
                if SyncAppointmentAdmission(forceClosed, onlyOwner) ~= true then close(); end
            end
            return;
        end
    else
        -- Retain the original opt-in diagnostic path and its unconditional
        -- reset/republication, including saves without normal event receipts.
        if not close() then return; end
    end
    local function integer(n, maximum)
        return type(n) == "number" and n >= 0 and n < maximum and n % 1 == 0;
    end
    local packed, used = {}, {};
    for slot,entry in ipairs(rows) do
        local row = entry.row;
        -- One person has one joined owner and one immutable payment prefix.
        -- Unjoined people use the current price for future joins.
        -- Pack the independent definition even if one mutable prepare receipt
        -- is damaged. Eligibility below still requires the complete receipt,
        -- and other owners must keep their valid appointment/placement cells.
        local cost = PersonaEvents.CatalogCost(row);
        local governor = GameInfo.Governors[row.GovernorType];
        if not integer(cost, 4) or cost == 0 or not governor or governor.GovernorType ~= row.GovernorType or
           row.BasePromotionType ~= "GOVERNOR_PROMOTION_JAC_PERSONA_" .. string.sub(row.LeaderType,12) .. "_BASE" then close(); return; end
        local values = {};
        for step=0,cost-1 do
            local name = row.BasePromotionType;
            if step > 0 then name = name .. "_TITLE_" .. step; end
            local promotion = GameInfo.GovernorPromotions[name];
            if not promotion or promotion.GovernorPromotionType ~= name or
               not integer(promotion.Index, 4096) or used[promotion.Index] then close(); return; end
            used[promotion.Index] = true;
            values[step+1] = promotion.Index;
        end
        packed[slot] = entry.index + cost * 256 + values[1] * 1024 +
            (values[2] or 0) * 4194304 + (values[3] or 0) * 17179869184;
    end
    local signature = table.concat(packed, ':');
    local current = {};
    for _,owner in ipairs(ids) do
        local p = Player(owner, true);
        if not p then close(); return; end
        local values = {};
        for _,entry in ipairs(rows) do
            values[#values+1] = PaidPublicationEligible(owner, entry.row, forceClosed, diagnostic) and '1' or '0';
        end
        current[#current+1] = {owner=owner, player=p, values=table.concat(values)};
    end
    if not diagnostic and not forceClosed and schema == 1 and normalPaidPublished and
       normalPaidPublished.appointments == admissionSnapshots and
       normalPaidPublished.signature == signature and #normalPaidPublished.owners == #current then
        local same = true;
        for i,entry in ipairs(current) do
            local old = normalPaidPublished.owners[i];
            if old.owner ~= entry.owner or old.player ~= entry.player or old.values ~= entry.values then same = false; break; end
        end
        if same then return; end
    end
    -- A changed normal output uses the same full reset/COMMIT transaction as
    -- diagnostics. Unchanged turns reuse only output after rechecking authority.
    if not close() then return; end
    local function bitkey(bit)
        if bit < 744 then return 905256 + bit; end
        return 906256 + bit - 744;
    end
    -- Claim every reserved key, including currently unused catalog slots.
    if schema == nil then
        local function vacant(p, key)
            local ok, value = pcall(p.GetProperty, p, key);
            return ok and value == nil;
        end
        for owner=0,63 do
            local p = Player(owner, false);
            if p then
                for _,base in ipairs({904000,905000,906000}) do
                    for index=0,255 do
                        if not vacant(p, base+index) then close(); return; end
                    end
                end
                for id=0,63 do if not vacant(p, 907000+id) then close(); return; end end
                if not vacant(p, @COMMIT@) then close(); return; end
                for bit=0,@BITS@-1 do if not vacant(p, bitkey(bit)) then close(); return; end end
            end
        end
        Game:SetProperty("@SCHEMA@", 1);
        if Game:GetProperty("@SCHEMA@") ~= 1 then close(); return; end
    end
    local function write(p, key, value)
        local written = pcall(p.SetProperty, p, key, value);
        if not written then return false; end
        local ok, actual = pcall(p.GetProperty, p, key);
        return ok and actual == value;
    end
    -- Reset each owner once, then let appointment initialization use its own
    -- reset before either catalog or paid cells are published in that binding.
    for _,owner in ipairs(ids) do
        local p = Player(owner, true);
        if not p or not write(p, 907000+owner, true) then close(); return; end
    end
    if SyncAppointmentAdmission(forceClosed, nil) ~= true then close(); return; end
    for i,owner in ipairs(ids) do
        local p = Player(owner, true);
        if not p then close(); return; end
        if not write(p, @COMMIT@, false) then close(); return; end
        for bit=0,@BITS@-1 do
            local slot = math.floor(bit / @WIDTH@) + 1;
            local value = math.floor(packed[slot] / (2 ^ (bit % @WIDTH@))) % 2 == 1;
            if not write(p, bitkey(bit), value) then close(); return; end
        end
        -- All 22 are closed and acknowledge typed false before a single true.
        for _,entry in ipairs(rows) do
            if not write(p, 906000+entry.index, true) or not write(p, 905000+entry.index, false) or
               not write(p, 904000+entry.index, true) then close(); return; end
        end
        if not write(p, @COMMIT@, true) then close(); return; end
        if not diagnostic and not NormalPaidPublicationActive(ids, rows) then close(); return; end
        local values = {};
        for _,entry in ipairs(rows) do
            local row = entry.row;
            -- A request to inspect this same native person, not proof of paid
            -- titles. Rust still requires BASE and all TITLE bits in this call.
            local eligible = PaidPublicationEligible(owner, row, forceClosed, diagnostic);
            values[#values+1] = eligible and '1' or '0';
            if eligible and not write(p, 905000+entry.index, true) then close(); return; end
        end
        current[i] = {owner=owner, player=p, values=table.concat(values)};
    end
    if not diagnostic then normalPaidPublished = {signature=signature, owners=current, appointments=admissionSnapshots}; end
end
local function SyncProductionAdmission(forceClosed, onlyOwner)
    -- A failed projection must not strand a catalog published for an earlier
    -- owner. Reclose every owner and let the next normal callback retry from
    -- the saved ledger; never retry a title payment here.
    local ok = pcall(PublishProductionAdmission, forceClosed, onlyOwner);
    if not ok then
        ClosePaidAdmissionOwners();
    end
end
-- Load boundary. The forced pass closes every saved generation and lets a
-- fresh store commit at its all-closed handshake. Only after that pass
-- published, the normal pass revalidates the restored paid sets read-only:
-- loading never confirms a saved debit or changes the acquisition store.
local function InitializeProductionAdmission()
    SyncProductionAdmission(true);
    if normalPaidPublished then PersonaEvents.ReadOnly(SyncProductionAdmission, false); end
end
-- Explicit synchronized refresh after a controller install/rebuild. The caller
-- supplies no eligibility, payment, flag or native-generation authority.
GameEvents.JAC_LeaderPersonaRepublish.Add(function(owner, params)
    if type(owner) ~= 'number' or owner % 1 ~= 0 or owner < 0 or owner >= 64 or
       type(params) ~= 'table' or not Player(owner, true) or
       Game:GetProperty("@EXPERIMENT@") == true or PersonaEvents.Enabled() then return; end
    if not ClosePaidAdmissionOwners() then return; end
    if Game:GetProperty('JAC_TEST853_ADMISSION_ACTIVE') ~= nil or
       Game:GetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT') ~= nil then return; end
    SyncProductionAdmission(false, nil);
end);
-- A diagnostic projection is a synchronized script operation. It never
-- reconciles persons, finalizes evolution or sends a native title operation.
GameEvents.JAC_LeaderPersonaProject.Add(function(owner, params)
    local flag=Game:GetProperty("@EXPERIMENT@");
    local paidSchema=Game:GetProperty("@SCHEMA@");
    local appointmentSchema=Game:GetProperty(ADMISSION_SCHEMA);
    print('JAC_PERSONA_PROJECT received owner='..tostring(owner)..' paramsType='..type(params)..
        ' flagType='..type(flag)..' flag='..tostring(flag)..' paidSchema='..tostring(paidSchema)..
        ' appointmentSchema='..tostring(appointmentSchema));
    if flag ~= true or not Player(owner, true) or
       type(params) ~= 'table' or type(params.ProjectionSerial) ~= 'number' or
       params.ProjectionSerial % 1 ~= 0 or params.ProjectionSerial < 1 or
       params.ProjectionSerial >= 9007199254740992 then
        print('JAC_PERSONA_PROJECT rejected'); return;
    end
    print('JAC_PERSONA_PROJECT begin owner='..owner..' serial='..params.ProjectionSerial);
    if params.InspectOnly == true then
        local p=Player(owner,true);
        local g=p:GetGovernors();
        local samples={};
        for _,key in ipairs({900021,901021,904021,905021,904999}) do
            local value=p:GetProperty(key);
            samples[#samples+1]=tostring(key)..'='..type(value)..':'..tostring(value);
        end
        print('JAC_PERSONA_PROJECT inspect '..table.concat(samples,';')..
            ' HasGovernor='..type(g.HasGovernor)..' GetGovernor='..type(g.GetGovernor));
        p:SetProperty('JAC_PERSONA_PROJECT_INSPECT_RECEIPT',params.ProjectionSerial);
        print('JAC_PERSONA_PROJECT inspect-done'); return;
    end
    SyncProductionAdmission(false, nil);
    print('JAC_PERSONA_PROJECT end owner='..owner..' serial='..params.ProjectionSerial);
    Player(owner, true):SetProperty('JAC_PERSONA_PROJECT_RECEIPT', params.ProjectionSerial);
end);
-- END RUST GENERATED PAID ASSIGNMENT GLUE
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn checked_in_glue_matches_rust_generator() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua");
        let file = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        let begin = "-- BEGIN RUST GENERATED PAID ASSIGNMENT GLUE\n";
        let end = "-- END RUST GENERATED PAID ASSIGNMENT GLUE\n";
        assert_eq!(file.matches(begin).count(), 1, "paid block must be unique");
        assert_eq!(
            file.matches(end).count(),
            1,
            "paid block end must be unique"
        );
        let first = file.find(begin).unwrap();
        let last = file[first..].find(end).unwrap() + first + end.len();
        assert_eq!(
            &file[first..last],
            super::source(),
            "run generate-paid-glue before testing"
        );
        assert!(
            file.ends_with("\nInitializeProductionAdmission();\n"),
            "Gameplay must end with the generated load entry"
        );
    }
}
