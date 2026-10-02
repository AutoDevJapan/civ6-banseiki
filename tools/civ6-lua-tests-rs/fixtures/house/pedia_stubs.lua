
CAPTIONS = {}
local function record(value) CAPTIONS[#CAPTIONS + 1] = value end
function Do_AddChapterWithParagraphs(_, _, header, paragraphs) record(header); for _, p in ipairs(paragraphs) do record(p) end end
function Do_AddHeader(_, caption) record(caption) end
function Do_AddParagraph(_, paragraph) record(paragraph) end
function Do_AddHeaderBody(_, header, body) record(header); record(body) end
function Do_AddIconHeaderBody(_, icon, header, body) record(header); record(body) end
function HookupIcon(icon) record(icon[2]); record(icon[1]) end
function AddQuote(quote) record(quote) end
function AddRightColumnStatBox(title, populate)
  record(title)
  local box = {}
  function box:AddHeader(c) record(c) end
  function box:AddLabel(c) record(c) end
  function box:AddSmallLabel(c) record(c) end
  function box:AddIconLabel(icon, c) record(icon[2] == c and "same" or "different"); record(c) end
  function box:AddIconNumberLabel(icon, value, c) record(c) end
  populate(box)
end
ContextPtr = { GetID = function() return "Civilopedia" end }
