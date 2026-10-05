//! Minimal synchronized Gameplay binding, generated from the Rust protocol.
//! It projects existing GameInfo and ledger values; all native payment policy
//! remains in Rust and rechecks the current Governor on every assignment.
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
-- Regenerate with civ6-native-bridge-rs/generate-paid-glue. Opt-in only.
local function SyncProductionAdmission(forceClosed, onlyOwner)
    local schema = Game:GetProperty("@SCHEMA@");
    local enabled = Game:GetProperty("@EXPERIMENT@") == true;
    local ids = PlayerManager.GetAliveMajorIDs();
    local function close()
        for owner=0,63 do
            local p = Player(owner, false);
            if p then pcall(p.GetProperty, p, 907000 + owner); end
        end
        admissionSnapshots = {};
    end
    if not enabled then
        if schema ~= nil then close(); end
        SyncAppointmentAdmission(forceClosed, onlyOwner);
        return;
    end
    -- Close before reading definitions, including reload and rejected schema.
    close();
    if not AdmissionOwnerIDsValid(ids) or (schema ~= nil and schema ~= 1) then return; end
    local rows = AdmissionRows();
    if not rows or #rows ~= @COUNT@ then return; end
    local function integer(n, maximum)
        return type(n) == "number" and n >= 0 and n < maximum and n % 1 == 0;
    end
    local packed, used = {}, {};
    for slot,entry in ipairs(rows) do
        local row, cost = entry.row, entry.row.EvolutionCost;
        local governor = GameInfo.Governors[row.GovernorType];
        if not integer(cost, 4) or cost == 0 or not governor or governor.GovernorType ~= row.GovernorType or
           row.BasePromotionType ~= "GOVERNOR_PROMOTION_JAC_PERSONA_" .. string.sub(row.LeaderType,12) .. "_BASE" then return; end
        local values = {};
        for step=0,cost-1 do
            local name = row.BasePromotionType .. (step == 0 and "" or "_TITLE_" .. step);
            local promotion = GameInfo.GovernorPromotions[name];
            if not promotion or promotion.GovernorPromotionType ~= name or
               not integer(promotion.Index, 4096) or used[promotion.Index] then return; end
            used[promotion.Index] = true;
            values[step+1] = promotion.Index;
        end
        packed[slot] = entry.index + cost * 256 + values[1] * 1024 +
            (values[2] or 0) * 4194304 + (values[3] or 0) * 17179869184;
    end
    local function bitkey(bit)
        if bit < 744 then return 905256 + bit; end
        return 906256 + bit - 744;
    end
    -- Claim every reserved key, including currently unused catalog slots.
    if schema == nil then
        for owner=0,63 do
            local p = Player(owner, false);
            if p then
                for _,base in ipairs({904000,905000,906000}) do
                    for index=0,255 do if p:GetProperty(base+index) ~= nil then return; end end
                end
                for id=0,63 do if p:GetProperty(907000+id) ~= nil then return; end end
                if p:GetProperty(@COMMIT@) ~= nil then return; end
                for bit=0,@BITS@-1 do if p:GetProperty(bitkey(bit)) ~= nil then return; end end
            end
        end
        Game:SetProperty("@SCHEMA@", 1);
    end
    local function write(p, key, value)
        p:SetProperty(key, value);
        return p:GetProperty(key) == value;
    end
    -- Reset each owner once, then let appointment initialization use its own
    -- reset before either catalog or paid cells are published in that binding.
    for _,owner in ipairs(ids) do
        local p = Player(owner, true);
        if not p or not write(p, 907000+owner, true) then close(); return; end
    end
    SyncAppointmentAdmission(forceClosed, nil);
    for _,owner in ipairs(ids) do
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
        for _,entry in ipairs(rows) do
            local row = entry.row;
            local person, ready, count = FindPerson(owner, row);
            -- A request to inspect this same native person, not proof of paid
            -- titles. Rust still requires BASE and all TITLE bits in this call.
            local eligible = not forceClosed and ready and person == nil and count == 0 and
                Get(row.LeaderType, "OWNER") == owner and Get(row.LeaderType, "STATE") == "GOVERNOR" and
                Get(row.LeaderType, "UNIT") == nil and Get(row.LeaderType, "EVOLUTION_COST") == row.EvolutionCost;
            if eligible and not write(p, 905000+entry.index, true) then close(); return; end
        end
    end
end
-- END RUST GENERATED PAID ASSIGNMENT GLUE
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn checked_in_glue_matches_rust_generator() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua");
        let file = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        assert!(
            file.contains(&super::source()),
            "run generate-paid-glue before testing"
        );
    }
}
