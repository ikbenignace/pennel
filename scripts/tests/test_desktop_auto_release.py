import importlib.util
from pathlib import Path
import unittest


SCRIPTS = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("desktop_auto_release", SCRIPTS / "desktop-auto-release.py")
auto = importlib.util.module_from_spec(spec)
spec.loader.exec_module(auto)


class NextVersionTests(unittest.TestCase):
    def test_first_release_uses_the_configured_version(self):
        self.assertEqual(auto.next_version("0.1.0", None), "0.1.0")

    def test_patch_bumps_the_latest_tag(self):
        self.assertEqual(auto.next_version("0.1.0", "v0.1.7"), "0.1.8")

    def test_a_higher_configured_version_wins(self):
        self.assertEqual(auto.next_version("0.2.0", "v0.1.7"), "0.2.0")

    def test_a_configured_version_equal_to_the_bump_is_used_once(self):
        self.assertEqual(auto.next_version("0.1.8", "v0.1.7"), "0.1.8")
        self.assertEqual(auto.next_version("0.1.8", "v0.1.8"), "0.1.9")

    def test_latest_tag_compares_numerically_and_ignores_other_tags(self):
        tags = ["v0.1.9", "v0.1.10", "v0.2.0-beta.1", "candidate", "v0.1.2"]
        self.assertEqual(auto.latest_release_tag(tags), "v0.1.10")
        self.assertIsNone(auto.latest_release_tag(["nightly"]))


class CaskTests(unittest.TestCase):
    SHA = "a" * 64

    def test_cask_points_at_the_release_dmg(self):
        cask = auto.render_cask("0.1.8", self.SHA, "ikbenignace/pennel")
        self.assertIn('version "0.1.8"', cask)
        self.assertIn(f'sha256 "{self.SHA}"', cask)
        self.assertIn("https://github.com/ikbenignace/pennel/releases/download/v#{version}/Pennel_#{version}_aarch64.dmg", cask)
        self.assertIn('app "Pennel.app"', cask)
        self.assertIn("depends_on arch: :arm64", cask)

    def test_zap_never_removes_meetings_or_models(self):
        cask = auto.render_cask("0.1.8", self.SHA, "ikbenignace/pennel")
        self.assertNotIn("Application Support", cask)

    def test_rejects_malformed_inputs(self):
        with self.assertRaises(ValueError):
            auto.render_cask("0.1.8", "ABC", "ikbenignace/pennel")
        with self.assertRaises(ValueError):
            auto.render_cask("0.1", self.SHA, "ikbenignace/pennel")


if __name__ == "__main__":
    unittest.main()
