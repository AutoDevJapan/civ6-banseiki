-- Dependency-enforcing FrontEnd model copied from the legacy test fixture.
local definitions = ...
MODS, INSTALLED, DISABLED, ENABLED = {}, {}, {}, {}
UPDATES = 0
for _, definition in ipairs(definitions) do
    local id = string.lower(definition.Id)
    local info = {Id = definition.Id, Handle = id, Enabled = definition.Enabled,
                  Dependencies = definition.Dependencies}
    MODS[id] = info
    INSTALLED[#INSTALLED + 1] = info
end
UI = {IsInFrontEnd = function() return true end}
GameConfiguration = {UpdateEnabledMods = function() UPDATES = UPDATES + 1 end}
Modding = {
    GetInstalledMods = function() return INSTALLED end,
    GetModHandle = function(id) local key = string.lower(id); return MODS[key] and key end,
    GetModInfo = function(handle) return assert(MODS[handle]) end,
    GetModAssociations = function(handle)
        local dependencies = {}
        for _, id in ipairs(MODS[handle].Dependencies) do
            dependencies[#dependencies + 1] = {Id = id}
        end
        return dependencies, {}, {}
    end,
    CanDisableMod = function(handle)
        if REFUSE_DISABLE_REASON == handle then return 'OwnershipRequired' end
        local sources = {}
        for _, info in ipairs(INSTALLED) do
            if info.Enabled then
                for _, dependency in ipairs(info.Dependencies) do
                    if string.lower(dependency) == handle then sources[#sources + 1] = {Id = info.Id} end
                end
            end
        end
        if #sources > 0 then return 'MissingDependencies', {}, sources end
        return 'OK'
    end,
    CanEnableMod = function(handle)
        if REFUSE_ENABLE_REASON == handle then return 'OwnershipRequired' end
        return 'OK'
    end,
}
Modding.DisableMod = function(handle)
    if Modding.CanDisableMod(handle) ~= 'OK' or REFUSE_DISABLE == handle then return end
    MODS[handle].Enabled = false
    DISABLED[#DISABLED + 1] = handle
end
Modding.EnableMod = function(handle, dependencies)
    if REFUSE_ENABLE == handle then return end
    if dependencies then
        for _, id in ipairs(MODS[handle].Dependencies) do
            local dependency = Modding.GetModHandle(id)
            if not MODS[dependency].Enabled then Modding.EnableMod(dependency, true) end
        end
    end
    if FAIL_AFTER_DEPENDENCIES == handle then error('engine enable failed') end
    MODS[handle].Enabled = true
    ENABLED[#ENABLED + 1] = handle
end
