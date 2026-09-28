-- SSOT 3.1「長尾→上杉」の家名表示の切替を、束1では武田家で検証する。
-- ゲームプレイ側では文明名を変えられない（2026-09-28 実機確認）ため、UI の各コンテキストで
-- PlayerConfiguration の名前ゲッターを差し替え、表示だけを上書きする（公開MOD GCO_PlayerConfig と同じ方式）。
-- 目標名はゲームプレイ側が保存した Game プロパティ（同期済みの共有状態）から読み、UI からは書かない。
local RENAME_NAME_PROPERTY = "JAC_POC_CUSTOM_CIVS_RENAMED_NAME"
local TAKEDA_CIV = "CIVILIZATION_JAC_TAKEDA"
local NAME_GETTERS = { "GetCivilizationShortDescription", "GetCivilizationDescription" }

function JAC_InstallCivNameOverride(contextName)
	local config = PlayerConfigurations and PlayerConfigurations[0]
	local mt = config and getmetatable(config)
	local methods = mt and mt.__index
	if type(methods) ~= "table" then
		print("[JAC_POC] nameOverride context=" .. contextName .. " result=no_method_table")
		return
	end
	-- 差し替え先の表がコンテキスト間で共有なら、2つ目以降のコンテキストはここに来る。
	if methods.JAC_NameOverrideContext then
		print(string.format("[JAC_POC] nameOverride context=%s result=shared_with_%s", contextName, methods.JAC_NameOverrideContext))
		return
	end
	local applied = false
	for _, getter in ipairs(NAME_GETTERS) do
		local original = methods[getter]
		methods[getter] = function(self)
			if self:GetCivilizationTypeName() == TAKEDA_CIV then
				local target = Game:GetProperty(RENAME_NAME_PROPERTY)
				if target then
					if not applied then
						applied = true
						print(string.format("[JAC_POC] nameOverride context=%s result=applied getter=%s target=%s",
							contextName, getter, tostring(target)))
					end
					return target
				end
			end
			return original(self)
		end
	end
	methods.JAC_NameOverrideContext = contextName
	print("[JAC_POC] nameOverride context=" .. contextName .. " result=installed")
end
