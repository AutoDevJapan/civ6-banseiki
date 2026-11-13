
local texts, language, sources, civilization = ...
local function format(tag, ...)
  assert(type(tag)=='string','string expected')
  local text = texts[tag]
  if text == nil then return tag end
  local args = { ... }
  return (string.gsub(text, "{(%d+)_[^}]*}", function(n)
    local value = args[tonumber(n)]
    return texts[value] or tostring(value)
  end))
end
-- Like the game, the engine's Locale table is read-only: writing any key raises "Attempt to modify read-only table".
local engineLocale = {
  Lookup = function(tag, ...) return format(tag, ...) end,
  ConvertTextKey = function(tag, ...) return format(tag, ...) end,
  ToUpper = function(tag) return string.upper(texts[tag] or tag) end,
  Compare = function(a, b) return a < b end,
}
ENGINE_LOCALE = setmetatable({}, {
  __index = engineLocale,
  __newindex = function() error("Attempt to modify read-only table", 2) end,
})
Locale = ENGINE_LOCALE
LOG = {}
LOCAL_CIV = civilization
Game = { GetLocalPlayer = function() return 0 end }
PlayerConfigurations = { [0] = { GetCivilizationTypeName = function() return LOCAL_CIV end } }
function include(name, wildcard)
  if wildcard then
    for file, source in pairs(sources) do
      if string.sub(file, 1, #name) == name then load(source, file)() end
    end
  elseif sources[name] then
    load(sources[name], name)()
  end
end
function print(line) LOG[#LOG + 1] = line end
