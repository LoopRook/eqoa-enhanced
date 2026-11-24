# SPDX-License-Identifier: AGPL-3.0-or-later
import unittest
from hashlib import sha256

import scene_suite as ss


class SceneSuiteToolTests(unittest.TestCase):
    def test_addon_key_matches_the_key_local_scene_writes_for_bag_viewer(self):
        digest = sha256(b"eqoa-native-addons/storage/v2\0")
        for field in (b"eqoa_bag_viewer/main.lua", b"eqoa_bag_viewer", b"eqoa_bag_viewer"):
            digest.update(len(field).to_bytes(8, "little") + field)
        self.assertEqual(ss.addon_key("eqoa_bag_viewer"), digest.hexdigest())
        self.assertNotEqual(ss.addon_key("eqoa_chat"), ss.addon_key("eqoa_dialogs"))

    def test_expect_lines_must_come_in_order_and_absent_lines_must_not_appear(self):
        log = "a one\nb two\nc three"
        expect = [("a", r"a one"), ("c", r"c three")]
        self.assertTrue(all(ok for _, ok in ss.evaluate(log, expect, [])))
        out = dict(ss.evaluate("c three\nb two\na one", expect, []))
        self.assertFalse(out["c"], "c before a is out of order")   # negative control: order matters
        self.assertFalse(all(ok for _, ok in ss.evaluate("a one", expect, [])), "a missing line fails")
        bad = ss.evaluate(log, expect, [("two", r"^b two$")])
        self.assertFalse(dict(bad)["never: two"], "a forbidden line present fails")
        self.assertTrue(dict(ss.evaluate(log, expect, [("zzz", r"zzz")]))["never: zzz"])

    def test_a_scene_with_no_checks_or_an_error_never_passes(self):
        self.assertEqual(ss.verdict([]), ss.FAIL)
        self.assertEqual(ss.verdict([("x", True)]), ss.PASS)
        self.assertEqual(ss.verdict([("x", True)], "client timed out"), ss.FAIL)
        self.assertEqual(ss.verdict([("x", True), ("y", False)]), ss.FAIL)

    def test_perf_stats_skip_the_first_window_and_take_medians(self):
        log = "\n".join(f"perf: {f} fps, cpu {c} ms/frame (worst 9.0, 0 of 50 ms or more, 0 of 100), 1 models in 1 draws: x"
                         for f, c in [(10.0, 50.0), (60.0, 5.0), (62.0, 4.0), (58.0, 6.0)])
        stats = ss.perf_stats(log)
        self.assertEqual((stats["fps"], stats["cpu_ms"], stats["windows"]), (60.0, 5.0, 3))
        self.assertIsNone(ss.perf_stats("nothing here"))

    def test_perf_delta_names_the_previous_build(self):
        self.assertIn("first recorded", ss.perf_delta([], {"fps": 60, "cpu_ms": 5}, "b"))
        text = ss.perf_delta([{"build": "abc", "fps": 60.0, "cpu_ms": 5.0}], {"fps": 54.0, "cpu_ms": 6.5}, "def")
        self.assertIn("abc", text)
        self.assertIn("-6.0", text)
        self.assertIn("+1.50", text)

    def test_the_stall_assertion_catches_a_stall_line(self):
        absent = [("a stall line (a frame of a second or more)", r"^stall: ")]
        clean = "chat: hello" + chr(10) + "perf: 40.0 fps"
        stalled = clean + chr(10) + "stall: 1516 ms from one frame to the next, to the settings write"
        self.assertTrue(all(ok for _, ok in ss.evaluate(clean, [], absent)))
        self.assertFalse(all(ok for _, ok in ss.evaluate(stalled, [], absent)))   # negative control

    def test_scene_ids_select_by_digit_prefix(self):
        wanted = {"5"}
        for number, picked in (("5a", True), ("5b", True), ("6", False), ("1", False)):
            self.assertEqual(number in wanted or number.rstrip("abc") in wanted, picked)

    def test_screenshot_checks_tell_a_window_from_the_world(self):
        try:
            from PIL import Image
        except ImportError:
            self.skipTest("Pillow is not installed")
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as d:
            world, panel, other = (Path(d) / n for n in ("world.png", "panel.png", "other.png"))
            Image.new("RGB", (200, 100), (200, 140, 40)).save(world)
            img = Image.new("RGB", (200, 100), (200, 140, 40))
            img.paste(Image.new("RGB", (100, 60), (17, 17, 17)), (50, 20))
            img.save(panel)
            Image.new("RGB", (200, 100), (200, 140, 40)).save(other)
            box = (60, 30, 140, 70)
            self.assertTrue(ss.window_up(panel, box))
            self.assertFalse(ss.window_up(world, box))                     # negative control: nothing drawn
            self.assertGreater(ss.region_diff(world, panel, box), 100)
            self.assertEqual(ss.region_diff(world, other, box), 0.0)
            self.assertIsNone(ss.region_diff(world, Path(d) / "missing.png", box))
            self.assertFalse(ss.window_up(Path(d) / "missing.png", box))

    def test_hud_median_is_zero_when_the_phase_is_below_the_print_floor(self):
        line = "perf: 40.0 fps, cpu 20.0 ms/frame (worst 9.0, 0 of 50 ms or more, 0 of 100), 1 models in 1 draws: walk 0.30, hud 0.40, gpu wait 3.0"
        self.assertEqual(ss.hud_stats(chr(10).join([line, line.replace("hud 0.40", "hud 0.60")])), 0.5)
        self.assertEqual(ss.hud_stats(line.replace(", hud 0.40", "")), 0.0)
        self.assertIsNone(ss.hud_stats("no perf lines"))


if __name__ == "__main__":
    unittest.main()
