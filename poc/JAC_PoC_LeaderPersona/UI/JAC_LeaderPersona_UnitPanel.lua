-- Official GS panel remains intact; only the portrait lookup for persona individuals changes.
include("UnitPanel_Expansion2");
local basePortraits = GetUnitPortraitIconNames;
function GetUnitPortraitIconNames(unit)
    local gp = unit and unit:GetGreatPerson();
    if gp and gp:IsGreatPerson() then
        local person = GameInfo.GreatPersonIndividuals[gp:GetIndividual()];
        if person then
            for row in GameInfo.JAC_LeaderPersonas() do
                if row.PersonType == person.GreatPersonIndividualType then
                    return row.PortraitIcon, row.PortraitIcon, row.PortraitIcon, row.PortraitIcon;
                end
            end
        end
    end
    return basePortraits(unit);
end
