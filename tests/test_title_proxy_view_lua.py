"""A consumed stage can finalize at zero available titles."""
from pathlib import Path
import unittest
from lupa import LuaRuntime

CODE = (Path(__file__).resolve().parents[1] / "poc/JAC_PoC_LeaderPersona/UI/JAC_TitleProxy_View.lua").read_text(encoding="utf-8")


class PaymentViewTests(unittest.TestCase):
    def test_already_consumed_native_stage_finalizes_without_another_available_title(self):
        lua = LuaRuntime()
        lua.execute("""
            include=function() end; print=function() end
            ContextPtr={SetHide=function() end}
            Game={GetLocalPlayer=function() return 0 end,GetProperty=function()
                return {cursor=0,flight={request=1,stage=0},requests={{id=1,cost=1,payments={},status='PENDING',leader='A'}}}
            end}
            PlayerOperations={EXECUTE_SCRIPT=10}
            REQUESTS={}; UI={RequestPlayerOperation=function(owner,operation,params)
                REQUESTS[#REQUESTS+1]={owner=owner,operation=operation,params=params}
            end}
            GameInfo={Governors={GOVERNOR_JAC_TITLE_PROXY={Hash=9000,Index=200}}}
            Players={[0]={IsHuman=function() return true end,IsTurnActive=function() return true end,
                GetProperty=function() return 1 end,GetGovernors=function() return {
                    HasGovernor=function() return true end,GetGovernor=function() return {} end,
                    GetGovernorPoints=function() return 1 end,GetGovernorPointsSpent=function() return 1 end
                } end}}
            JAC_TitlePaymentRules={Valid=function() return true end,Next=function()
                error('new spending validation must not block an already consumed stage')
            end}
        """)
        lua.execute(CODE)
        self.assertTrue(lua.globals().JAC_ProxyPayNext())
        self.assertEqual(len(lua.globals().REQUESTS), 1)
        self.assertEqual(lua.globals().REQUESTS[1]["params"]["OnStart"], "JAC_TitleProxyAdvance")
        self.assertEqual(lua.globals().REQUESTS[1]["operation"], 10)


if __name__ == "__main__":
    unittest.main()
