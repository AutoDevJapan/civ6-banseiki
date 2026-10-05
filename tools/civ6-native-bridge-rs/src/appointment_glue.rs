//! Synchronized appointment publication with an explicit commit result.
pub fn source() -> &'static str {
    TEMPLATE
}

const TEMPLATE: &str = r#"-- BEGIN RUST GENERATED APPOINTMENT GLUE
local function SyncAppointmentAdmission(forceClosed, onlyOwner)
    local rows = AdmissionRows();
    local ids = PlayerManager.GetAliveMajorIDs();
    if not AdmissionOwnerIDsValid(ids) then CloseAllAdmissionOwners(); return false; end
    local schema = Game:GetProperty(ADMISSION_SCHEMA);
    if (schema ~= nil and schema ~= 1 and schema ~= 2 and schema ~= 3) or not rows then
        -- Even an invalid table/schema after reload must invalidate the old
        -- native generation. A reset read closes it without owning the value.
        CloseAllAdmissionOwners();
        return false;
    end
    if schema == 1 then
        for owner=0,63 do
            local p = Player(owner, false);
            if p then
                for _,entry in ipairs(rows) do
                    if not AdmissionKeyVacant(p, ADMISSION_CLOSE + entry.index) then
                        CloseAllAdmissionOwners(); return false;
                    end
                end
            end
        end
        Game:SetProperty(ADMISSION_SCHEMA, 2);
        schema = 2;
    end
    if schema == 2 then
        -- Version 2 owned only key 902000 on each player. Claim the new
        -- owner-specific reset keys only when no saved foreign value occupies
        -- one; the old key on owners 1..63 is left untouched.
        for owner=1,63 do
            local p = Player(owner, false);
            if p and not AdmissionKeyVacant(p, ADMISSION_RESET + owner) then
                CloseAllAdmissionOwners(); return false;
            end
        end
        Game:SetProperty(ADMISSION_SCHEMA, 3);
        schema = 3;
    end
    local fingerprint = "JAC:";
    for _,entry in ipairs(rows) do
        fingerprint = fingerprint .. entry.row.LeaderType .. ":" .. tostring(entry.index) .. ";";
    end
    if admissionRowsFingerprint and admissionRowsFingerprint ~= fingerprint then
        for _,snapshot in pairs(admissionSnapshots) do CloseAdmissionOwner(snapshot.player); end
    end
    admissionRowsFingerprint = fingerprint;
    if schema == nil then
        -- Every listed owner must have a live Core player before claiming the
        -- namespace. Dead major players can later return to the alive list,
        -- so their saved properties must be checked too.
        for _,owner in ipairs(ids) do
            if not Player(owner, true) then CloseAllAdmissionOwners(); return false; end
        end
        -- Check every existing major, including the dead, before taking the
        -- reset, manifest, admission and close ranges for this saved game.
        for owner=0,63 do
            local p = Player(owner, false);
            if p then
                if not AdmissionKeyVacant(p, ADMISSION_RESET + owner) then CloseAllAdmissionOwners(); return false; end
                for _,entry in ipairs(rows) do
                    for _,base in ipairs({ADMISSION_MANIFEST, ADMISSION_ALLOW, ADMISSION_CLOSE}) do
                        if not AdmissionKeyVacant(p, base + entry.index) then CloseAllAdmissionOwners(); return false; end
                    end
                end
            end
        end
        Game:SetProperty(ADMISSION_SCHEMA, 3);
    end
    local alive = {};
    for _,owner in ipairs(ids) do alive[owner] = true; end
    for owner,snapshot in pairs(admissionSnapshots) do
        if not alive[owner] then CloseAdmissionOwner(snapshot.player); end
    end
    -- Attempt every initialization reset even if an earlier owner fails.
    -- No manifest can be published until the whole close phase is confirmed.
    local initialized = true;
    for _,owner in ipairs(ids) do
        local p = Player(owner, true);
        local snapshot = admissionSnapshots[owner];
        if not p or owner >= 64 then initialized = false;
        elseif not snapshot or snapshot.player ~= p then
            local resetOK, closed = pcall(CloseAdmissionOwner, p);
            if not resetOK or not closed then initialized = false;
            else admissionSnapshots[owner] = {player=p, values={}}; end
        end
    end
    if not initialized then CloseAllAdmissionOwners(); return false; end
    for _,owner in ipairs(ids) do
        local p = Player(owner, true);
        local snapshot = admissionSnapshots[owner];
        if p and snapshot and (onlyOwner == nil or onlyOwner == owner) then
            local changes = {};
            for _,entry in ipairs(rows) do
                local desired = AdmissionAllowed(owner, entry.row, rows, forceClosed) == true;
                if snapshot.values[entry.index] ~= desired then
                    changes[#changes + 1] = {entry=entry, desired=desired};
                end
            end
            -- Close every affected cell before exposing any new value. Reading
            -- close invalidates it in the native consumer before the getter.
            local valid = true;
            for _,change in ipairs(changes) do
                local index = change.entry.index;
                p:SetProperty(ADMISSION_CLOSE + index, true);
                if p:GetProperty(ADMISSION_CLOSE + index) ~= true then valid = false; break; end
                p:SetProperty(ADMISSION_ALLOW + index, false);
                if p:GetProperty(ADMISSION_ALLOW + index) ~= false then valid = false; break; end
            end
            if not valid then CloseAllAdmissionOwners(); return false; end
            for _,change in ipairs(changes) do
                local index = change.entry.index;
                p:SetProperty(ADMISSION_MANIFEST + index, true);
                if p:GetProperty(ADMISSION_MANIFEST + index) ~= true then valid = false; break; end
                if change.desired then
                    p:SetProperty(ADMISSION_ALLOW + index, true);
                    if p:GetProperty(ADMISSION_ALLOW + index) ~= true then valid = false; break; end
                end
                snapshot.values[index] = change.desired;
            end
            if not valid then CloseAllAdmissionOwners(); return false; end
        end
    end
    return true;
end
-- END RUST GENERATED APPOINTMENT GLUE
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn checked_in_appointment_matches_rust_generator() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua");
        let file = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        assert_eq!(
            file.matches("-- BEGIN RUST GENERATED APPOINTMENT GLUE")
                .count(),
            1
        );
        assert!(
            file.contains(super::source()),
            "run generate-paid-glue before testing"
        );
    }
}
