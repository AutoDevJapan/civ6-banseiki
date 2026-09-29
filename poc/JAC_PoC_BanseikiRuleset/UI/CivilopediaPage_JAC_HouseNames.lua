-- 家固有名の表示切替（JAC_HouseNames.lua）をシヴィロペディアへ取り付ける。
-- CivilopediaScreen.lua:13 の include("CivilopediaPage_", true) で読まれる。ページ定義は含まない。
-- CivilopediaScreen.lua は CivilopediaSupport を読んだ後（:6）にここを読み、その後で Initialize（:17）するため、
-- 見出し（page.Title、CivilopediaSupport.lua:317）は包んだ Locale.Lookup を通る。
include("JAC_HouseNames")
JAC_InstallHouseNames(ContextPtr and ContextPtr:GetID() or "Civilopedia")

-- ページの部品はキーをそのまま LocalizeAndSetText / LocalizeAndSetToolTip に渡し、エンジンが翻訳する（Locale.Lookup を通らない）。
-- 例：時代名 s:AddLabel(era.Name)（CivilopediaPage_Civic.lua:201）、前提の社会制度 s:AddIconLabel(icon, kPreReqData.Name)（:210）。
-- 家固有名を持つ家が見ているときだけ、部品へ渡す前に Locale.Lookup で文にする。
-- 翻訳済みの文を渡すのは公式と同じ使い方（page.TabName：CivilopediaSupport.lua:316 と :745）。
local function Localize(value)
	if type(value) == "string" then return Locale.Lookup(value) end
	if type(value) == "table" then
		local localized = {}
		for i, v in ipairs(value) do localized[i] = Localize(v) end
		return localized
	end
	return value
end

-- アイコンの指定 { アイコン名, ツールチップ, 検索語, 色 }（CivilopediaSupport.lua:1374）のうち、ツールチップだけを文にする。
local function LocalizeIcon(icon)
	if type(icon) ~= "table" then return icon end
	return { icon[1], Localize(icon[2]), icon[3], icon[4] }
end

local function Active()
	return JAC_LocalHouseHasNames()
end

local BASE_Do_AddChapterWithParagraphs = Do_AddChapterWithParagraphs
function Do_AddChapterWithParagraphs(chapterManager, paragraphManager, header, paragraphs)
	if Active() then header, paragraphs = Localize(header), Localize(paragraphs) end
	return BASE_Do_AddChapterWithParagraphs(chapterManager, paragraphManager, header, paragraphs)
end

local BASE_Do_AddHeader = Do_AddHeader
function Do_AddHeader(manager, caption)
	if Active() then caption = Localize(caption) end
	return BASE_Do_AddHeader(manager, caption)
end

local BASE_Do_AddParagraph = Do_AddParagraph
function Do_AddParagraph(manager, paragraph)
	if Active() then paragraph = Localize(paragraph) end
	return BASE_Do_AddParagraph(manager, paragraph)
end

local BASE_Do_AddHeaderBody = Do_AddHeaderBody
function Do_AddHeaderBody(manager, header, body)
	if Active() then header, body = Localize(header), Localize(body) end
	return BASE_Do_AddHeaderBody(manager, header, body)
end

local BASE_Do_AddIconHeaderBody = Do_AddIconHeaderBody
function Do_AddIconHeaderBody(manager, icon, header, body)
	if Active() then header, body = Localize(header), Localize(body) end
	return BASE_Do_AddIconHeaderBody(manager, icon, header, body)
end

local BASE_HookupIcon = HookupIcon
function HookupIcon(iconData, iconControl, buttonControl)
	if Active() then iconData = LocalizeIcon(iconData) end
	return BASE_HookupIcon(iconData, iconControl, buttonControl)
end

local BASE_AddQuote = AddQuote
function AddQuote(quote, audio)
	if Active() then quote = Localize(quote) end
	return BASE_AddQuote(quote, audio)
end

-- 右列の枠は部品の関数を枠ごとに作る（CivilopediaSupport.lua:1641-1779）ので、中身を埋める関数を包んで差し替える。
local BASE_AddRightColumnStatBox = AddRightColumnStatBox
function AddRightColumnStatBox(title, populateMethod)
	if not Active() then return BASE_AddRightColumnStatBox(title, populateMethod) end
	return BASE_AddRightColumnStatBox(Localize(title), function(statBox)
		for _, name in ipairs({ "AddHeader", "AddLabel", "AddSmallLabel" }) do
			local original = statBox[name]
			statBox[name] = function(self, caption) return original(self, Localize(caption)) end
		end
		local addIconLabel = statBox.AddIconLabel
		-- ツールチップと見出しが同じなら隠す比較（:1708）が崩れないよう、両方を文にしてから渡す。
		statBox.AddIconLabel = function(self, icon, caption) return addIconLabel(self, LocalizeIcon(icon), Localize(caption)) end
		local addIconNumberLabel = statBox.AddIconNumberLabel
		statBox.AddIconNumberLabel = function(self, icon, value, caption) return addIconNumberLabel(self, icon, value, Localize(caption)) end
		if populateMethod then return populateMethod(statBox) end
	end)
end
