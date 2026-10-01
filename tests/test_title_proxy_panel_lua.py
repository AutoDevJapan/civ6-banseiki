"""Normal and secret-governor UI routes preserve other mods and hide payment."""
from pathlib import Path
import unittest
from lupa import LuaRuntime

CODE = (Path(__file__).resolve().parents[1] / "poc/JAC_PoC_LeaderPersona/UI/JAC_TitleProxy_GovernorPanel.lua").read_text(encoding="utf-8")


class PaymentPanelTests(unittest.TestCase):
    def setUp(self):
        self.lua = LuaRuntime()
        self.lua.execute("""
            include=function() end
            PROXY={GovernorType='GOVERNOR_JAC_TITLE_PROXY'}
            OTHER={GovernorType='GOVERNOR_OTHER_MOD'}
            GameInfo={Governors={[200]=PROXY,[9000]=PROXY,[201]=OTHER}}
            CALLS={}
            for _,name in ipairs({'AddGovernorCandidate','AddSecretGovernorCandidate',
                'AddGovernorAppointed','AddSecretGovernorAppointed','OnAppointGovernor',
                'OnPromoteGovernor','OnNameButton'}) do
                _G[name]=function(...) CALLS[#CALLS+1]=name; return 'native' end
            end
        """)
        self.lua.execute(CODE)

    def test_both_lists_and_all_public_actions_exclude_the_payment_governor(self):
        self.lua.execute("""
            AddGovernorCandidate(PROXY,true); AddSecretGovernorCandidate(PROXY,true)
            AddGovernorAppointed({}, {}, PROXY,true); AddSecretGovernorAppointed({}, {}, PROXY,true)
            OnAppointGovernor(200); OnPromoteGovernor(200,2); OnNameButton(200)
            OnAppointGovernor(9000); OnPromoteGovernor(9000,2); OnNameButton(9000)
        """)
        self.assertEqual(len(self.lua.globals().CALLS), 0)

    def test_another_mods_lists_and_actions_keep_their_original_behavior(self):
        self.lua.execute("""
            AddGovernorCandidate(OTHER,true); AddSecretGovernorCandidate(OTHER,true)
            AddGovernorAppointed({}, {}, OTHER,true); AddSecretGovernorAppointed({}, {}, OTHER,true)
            OnAppointGovernor(201); OnPromoteGovernor(201,2); OnNameButton(201)
        """)
        self.assertEqual(list(self.lua.globals().CALLS.values()), [
            "AddGovernorCandidate", "AddSecretGovernorCandidate", "AddGovernorAppointed",
            "AddSecretGovernorAppointed", "OnAppointGovernor", "OnPromoteGovernor", "OnNameButton"])


if __name__ == "__main__":
    unittest.main()
