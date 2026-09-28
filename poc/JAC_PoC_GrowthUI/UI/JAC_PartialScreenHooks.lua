-- 右上フックバーの List of Reports の左に Notebooks（手帳）を追加する。
-- XP2 の PartialScreenHooks_Expansion2.lua を include し、Era と同じ型で1件足す。
include("PartialScreenHooks_Expansion2");

XP2_IsPartialScreenOpen	= IsPartialScreenOpen;
XP2_LateInitialize		= LateInitialize;

local m_isNotebooksOpen = false;
local m_isAttachLogged = false;

-- 開いた後は /InGame/PartialScreens へ移りパス検索で見つからないため、Era と同じくフラグで判定する。
function IsPartialScreenOpen( optionalScreenName )
	if (optionalScreenName == nil or optionalScreenName == "JAC_Notebooks") and m_isNotebooksOpen then
		return true;
	elseif optionalScreenName == "JAC_Notebooks" then
		return false;
	end
	return XP2_IsPartialScreenOpen( optionalScreenName );
end

function OnToggleNotebooks()
	if IsPartialScreenOpen("JAC_Notebooks") then
		LuaEvents.JAC_Notebooks_Close();
	else
		if IsPartialScreenOpen() then
			LuaEvents.PartialScreenHooks_CloseAllExcept("JAC_Notebooks");
		end
		LuaEvents.JAC_Notebooks_Open();
	end
end

function AddNotebooksHook()
	AddScreenHook("JAC_Notebooks", "LaunchBar_Hook_Timeline", "LOC_JAC_NOTEBOOKS", OnToggleNotebooks);
	-- Realize はターンごとに呼ばれるため、ログは最初の1回だけ出す。
	if not m_isAttachLogged then
		print("[JAC_POC] notebooks button attached parent=" .. Controls.ButtonStack:GetID());
		m_isAttachLogged = true;
	end
end

-- PartialScreenHooks_Expansion1.lua の AddScreenHooks に1行足しただけ。
function AddScreenHooks()
	AddWorldRankingsHook();
	AddCityStateHook();
	AddTradeHook();
	AddEspionageHook();
	AddEraHook();
	AddNotebooksHook();
	AddReportsHook();
end

function LateInitialize()
	LuaEvents.JAC_Notebooks_Opened.Add(function() m_isNotebooksOpen = true; end);
	LuaEvents.JAC_Notebooks_Closed.Add(function() m_isNotebooksOpen = false; end);
	XP2_LateInitialize();
end
