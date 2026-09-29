"""Guard the 22-leader source inventory and atlas layout."""

from pathlib import Path
import sys
import unittest
import xml.etree.ElementTree as ET

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools" / "jac_art"))
from build_art import LEADER_ORDER, LEADERS, atlas  # noqa: E402
from generate_portraits import SUBJECTS, filename, prompt  # noqa: E402


class PortraitArtTests(unittest.TestCase):
    def test_atlas_places_all_22_faces_in_eight_columns(self):
        masters = [Image.new("RGBA", (2, 2), (index + 1, 0, 0, 255))
                   for index in range(22)]
        image = atlas(masters, 2, 8)
        self.assertEqual(image.size, (16, 6))
        for index in range(22):
            self.assertEqual(image.getpixel(((index % 8) * 2, (index // 8) * 2)),
                             (index + 1, 0, 0, 255))
        self.assertEqual(image.getpixel((15, 5))[3], 0)

    def test_source_keys_match_frontend_leaders(self):
        mod = ROOT / "poc" / "JAC_PoC_CustomCivs" / "Config"
        names = [row.attrib["LeaderType"].removeprefix("LEADER_JAC_")
                 for path in (mod / "JAC_CustomCivs_Players.xml",
                              mod / "JAC_Japan1534_Players.xml")
                 for row in ET.parse(path).findall("./Players/Row")]
        self.assertEqual(tuple(names), LEADER_ORDER)
        self.assertEqual(set(SUBJECTS), set(names))
        for key in LEADER_ORDER:
            self.assertEqual(set(LEADERS[key]), {"portrait", "figure", "background"})
            for kind in LEADERS[key]:
                self.assertEqual(LEADERS[key][kind], f"{key.lower()}/{filename(key, kind)}")

    def test_prompts_require_three_distinct_files_and_keyless_codex(self):
        for key in SUBJECTS:
            text = prompt(key)
            self.assertIn("official Codex CLI", text)
            self.assertIn("No API keys", text)
            for kind in ("portrait", "figure", "background"):
                self.assertEqual(text.count(filename(key, kind)), 1)


if __name__ == "__main__":
    unittest.main()
