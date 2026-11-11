//! Generate the remaining Gameplay/UI price consumers from one price contract.
//! Only these bounded fragments are replaced; no Lua output is edited by hand.

pub fn gameplay(source: &str) -> Result<String, String> {
    let source = super::route(
        source,
        "local function ReservedOutstandingCost(owner, rows, spent)",
        "local function CorePaymentComplete",
        RESERVED,
    )?;
    let source = super::route(
        &source,
        "local function AdmissionAllowed(owner, row, rows, forceClosed)",
        "-- Volatile publication snapshots",
        ADMISSION,
    )?;
    let source = super::route(
        &source,
        "local function EvolutionCost(leader)",
        "local function AvailableTitles",
        EVOLUTION,
    )?;
    let start = if source.contains("-- BEGIN RUST PERSONA JOIN TRANSACTION") {
        "-- BEGIN RUST PERSONA JOIN TRANSACTION"
    } else {
        "local function Join(owner, source, personality, trait)"
    };
    let source = super::route(
        &source,
        start,
        "local function EvolutionCost(leader)",
        super::join_transaction::SOURCE,
    )?;
    Ok(source.replace(
        "for step=1,row.EvolutionCost-1 do",
        "for step=1,(PersonaEvents.Cost(row) or 1)-1 do",
    ))
}

const EVOLUTION: &str = r#"local function EvolutionCost(leader)
    return PersonaEvents.Cost(Definition(leader));
end
"#;

const RESERVED: &str = r#"local function ReservedOutstandingCost(owner, rows, spent)
    local total, attributed = 0, 0;
    for _,entry in ipairs(rows) do
        local row = entry.row;
        if Get(row.LeaderType, "OWNER") == owner and Get(row.LeaderType, "STATE") == "EVOLVING" then
            local cost = PersonaEvents.Cost(row);
            if not cost then return nil; end
            local paid, readable = CorePaymentRead(owner, row, cost);
            if readable and paid == nil then return nil; end
            -- Without native identity proof, retain the whole reservation.
            paid = paid or 0;
            total = total + cost - paid;
            attributed = attributed + paid;
        end
    end
    if spent < attributed then return nil; end
    return total;
end
"#;

const ADMISSION: &str = r#"local function AdmissionAllowed(owner, row, rows, forceClosed)
    if forceClosed or Get(row.LeaderType, "OWNER") ~= owner then return false; end
    local cost = PersonaEvents.Cost(row);
    if not cost then return false; end
    local available, spent = OfficialTitleBalance(owner);
    if not available then return false; end
    local person, ready, count = FindPerson(owner, row);
    local state = Get(row.LeaderType, "STATE");
    if state == "EVOLVING" then
        local reserved = ReservedOutstandingCost(owner, rows, spent);
        return ready and person ~= nil and count == 1 and
            Get(row.LeaderType, "UNIT") == person:GetID() and
            reserved ~= nil and available >= reserved;
    end
    if state == "GOVERNOR" then
        return ready and person == nil and count == 0 and
            Get(row.LeaderType, "UNIT") == nil and
            CorePaymentComplete(owner, row, cost, spent);
    end
    return false;
end
"#;

fn replace(source: &str, old: &str, new: &str) -> Result<String, String> {
    if source.matches(old).count() == 1 {
        return Ok(source.replace(old, new));
    }
    if source.matches(new).count() == 1 {
        return Ok(source.to_owned());
    }
    Err("price consumer fragment missing or ambiguous".into())
}

pub fn ui(source: &str) -> Result<String, String> {
    let source = replace(source,
        "local reserved, full = JAC_PersonaTreeRules.ReservedAppointments(owner);\n    local ownReserved = full and (surcharge + 1) or remaining;",
        "local reserved, full, own = JAC_PersonaTreeRules.ReservedAppointments(owner, leader);\n    local ownReserved = own or (full and (surcharge + 1) or remaining);")?;
    let source = replace(&source,
        "local cost = Game:GetProperty(\"JAC_PERSONA_\" .. leader .. \"_EVOLUTION_COST\") or row.EvolutionCost;",
        "local cost = JAC_PersonaTreeRules.EvolutionCost(leader);\n        if not cost then return nil; end")?;
    let source = replace(
        &source,
        "            local cost = Get(leader, \"EVOLUTION_COST\") or row.EvolutionCost;",
        "            local cost = JAC_PersonaTreeRules.EvolutionCost(leader);",
    )?;
    let source = replace(&source,
        "if type(surcharge) ~= \"number\" or surcharge < 0 then return false; end",
        "local cost = JAC_PersonaTreeRules.EvolutionCost(leader);\n    if not cost or surcharge ~= cost - 1 then return false; end")?;
    let source = replace(
        &source,
        "(available < cost or not native or native.appointed);",
        "(not cost or available < cost or not native or native.appointed);",
    )?;
    let source = replace(
        &source,
        "Locale.Lookup(\"LOC_JAC_PERSONA_EVOLVE_COST\", cost)",
        "Locale.Lookup(\"LOC_JAC_PERSONA_EVOLVE_COST\", cost or \"?\")",
    )?;
    replace(
        &source,
        "Locale.Lookup(\"LOC_JAC_PERSONA_COST_HELP\", cost, available)",
        "Locale.Lookup(\"LOC_JAC_PERSONA_COST_HELP\", cost or \"?\", available)",
    )
}

pub fn shared(source: &str) -> Result<String, String> {
    let begin = "-- BEGIN RUST PERSONA PRICE READER\n";
    let end = "-- END RUST PERSONA PRICE READER\n";
    let block = format!("{begin}{PRICE_READER}{end}");
    let source = if let Some(start) = source.find(begin) {
        let finish = source[start..]
            .find(end)
            .ok_or("price reader end missing")?
            + start
            + end.len();
        format!("{}{}{}", &source[..start], block, &source[finish..])
    } else {
        let at = source
            .find("function Rules.ReservedAppointments(owner)")
            .ok_or("shared price reader anchor missing")?;
        format!("{}{}{}", &source[..at], block, &source[at..])
    };
    super::route(
        &source,
        "function Rules.ReservedAppointments(",
        "function Rules.Owned(node)",
        SHARED_RESERVATIONS,
    )
}

const SHARED_RESERVATIONS: &str = r#"function Rules.ReservedAppointments(owner, leader)
    local api = ExposedMembers and ExposedMembers.JAC_LeaderPersona;
    if api and type(api.ReservedEvolutionTitles) == "function" then
        local total, own = api.ReservedEvolutionTitles(owner, leader);
        if total == math.huge and own == 0 then return total, true, own; end
        if type(total) == "number" and total >= 0 and total < 9007199254740991 and total % 1 == 0 and
           type(own) == "number" and own >= 0 and own <= total and own % 1 == 0 then
            return total, false, own;
        end
    end
    local reserved, fullCost, observedPaid, completedPaid = 0, 0, 0, 0;
    for person in GameInfo.JAC_LeaderPersonas() do
        if Rules.Get(person.LeaderType, "OWNER") == owner then
            local cost = Rules.EvolutionCost(person.LeaderType);
            if not cost then return math.huge, true; end
            local state = Rules.Get(person.LeaderType, "STATE");
            if state == "GOVERNOR" then
                completedPaid = completedPaid + cost;
            elseif state == "EVOLVING" then
                local native = Rules.NativeGovernor(owner, person.LeaderType);
                local paid = native and native.appointed and native.base and
                    type(native.surchargePaid) == "number" and native.surchargePaid >= 0 and
                    native.surchargePaid % 1 == 0 and (1 + native.surchargePaid) or 0;
                if paid < 0 or paid > cost then paid = 0; end
                fullCost = fullCost + cost;
                observedPaid = observedPaid + paid;
                reserved = reserved + math.max(0, cost - paid);
            end
        end
    end
    -- Earlier completed people and other title users cannot be mistaken for
    -- the payment of a current person. Until the official and native totals
    -- agree exactly, reserve every pending person's full cost.
    local player = Players[owner];
    local governors = player and player:GetGovernors();
    local spent = governors and type(governors.GetGovernorPointsSpent) == "function" and
        governors:GetGovernorPointsSpent();
    if type(spent) ~= "number" or spent ~= completedPaid + observedPaid then return fullCost, true; end
    return reserved, false;
end
"#;

const PRICE_READER: &str = r#"-- Generated by civ6-persona-events-rs::price_routes.
function Rules.EvolutionCost(leader)
    local api = ExposedMembers and ExposedMembers.JAC_LeaderPersona;
    if api and type(api.EvolutionCost) == "function" then return api.EvolutionCost(leader); end
    -- A joined price needs the independent Gameplay contract. UI startup may
    -- display current prices only for people who have not joined yet.
    local row = GameInfo.JAC_LeaderPersonas[leader];
    local cost = row and row.EvolutionCost;
    if Rules.Get(leader, "OWNER") == nil and type(cost) == "number" and cost % 1 == 0 and cost >= 1 and cost <= 3 then return cost; end
end
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shipped_price_consumers_are_generated_and_idempotent() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (path, render) in [
            (
                "poc/JAC_PoC_LeaderPersona/UI/JAC_LeaderPersona_View.lua",
                ui as fn(&str) -> Result<String, String>,
            ),
            (
                "poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua",
                shared,
            ),
        ] {
            let text = std::fs::read_to_string(root.join(path))
                .unwrap()
                .replace("\r\n", "\n");
            assert_eq!(render(&text).unwrap(), text, "{path}");
        }
    }
}
