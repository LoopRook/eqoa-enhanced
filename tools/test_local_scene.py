# SPDX-License-Identifier: AGPL-3.0-or-later
import unittest

from local_scene import read_gm_password, scene_checks


WORLD = "in the world as 0x1dcd6501: Player { name: \"Lsgm\" } after 1s, 5 packets sent"
SPAWN = "chat: [GM] Spawned fire bat. Target it and #despawn to take it away."
TARGET = "script: target fire bat: true"
KILL = "chat: [GM] Killed fire bat"
OPEN = "play: 0x0018 (123 bytes) in mode 0"
REQUEST = "play: loot request One(0, 1)"
LOOT = "chat: You receive 1 torn fire bat wings from the corpse."
CLOSE = "play: 0x0016 (0 bytes) in mode 1"
REFRESH = "play: 0x0018 (3 bytes) in mode 0"
SCENE = [WORLD, SPAWN, TARGET, KILL, OPEN, REQUEST, LOOT, CLOSE, REFRESH]
BAG_KEY = "script: mod key pressed"
BAG_TARGET = "script: mod key pressed"
BAG_TAKE = "script: mouse click Left at 650 615"
MEAT = "chat: You receive sliver of fire bat meat from the corpse."
BAG_LOOT = "scene: Bag Viewer loot log 14:25 sliver of fire bat meat x1, from fire bat"
R65_OPEN = "takeover: mod open_loot -> corpse 0x1234, opcode 0x0015"
R65_HIDDEN = "takeover: loot window HIDDEN for the mod (matched target)"
R65_CLOSE = "takeover: loot hidden session closed (released); 0x0016 sent"


class LocalSceneTests(unittest.TestCase):
    def test_password_uses_only_the_named_gm_row(self):
        accounts = "Local test server\nregular secret (normal)\nlsgm sample! (GM, acctlevel 2)\n"
        self.assertEqual(read_gm_password(accounts, "lsgm"), "sample!")
        for invalid in (accounts, accounts + "lsgm second (GM, acctlevel 2)\n"):
            with self.assertRaises(ValueError) as raised:
                read_gm_password(invalid, "regular" if invalid == accounts else "lsgm")
            self.assertNotIn("sample!", str(raised.exception))
            self.assertNotIn("second", str(raised.exception))

    def test_success_requires_ordered_client_evidence(self):
        self.assertTrue(all(scene_checks("\n".join(SCENE)).values()))
        for index in range(len(SCENE)):
            with self.subTest(missing=SCENE[index]):
                log = "\n".join(SCENE[:index] + SCENE[index + 1:])
                self.assertFalse(all(scene_checks(log).values()))
        early_loot = "\n".join([LOOT, *SCENE[:6], CLOSE, REFRESH])
        self.assertFalse(scene_checks(early_loot)["item received"])

    def test_empty_or_wrong_evidence_cannot_pass(self):
        for index, wrong in (
            (0, WORLD.replace('"Lsgm"', '"Other"')),
            (1, "chat: Lsgm says, 'Spawned fire bat. Target it and #despawn to take it away.'"),
            (2, "script: target fire bat: false"),
            (4, "play: 0x0018 (123 bytes) in mode 10"),
            (6, LOOT.replace("receive 1", "receive 0")),
            (6, "chat: You receive 2 tunar from the corpse."),
        ):
            with self.subTest(wrong=wrong):
                log = "\n".join([*SCENE[:index], wrong, *SCENE[index + 1:]])
                self.assertFalse(all(scene_checks(log).values()))

    def test_bag_requires_loot_after_the_click_and_rejects_junk(self):
        bag = [*SCENE[:4], BAG_KEY, BAG_TARGET, OPEN, BAG_TAKE, MEAT, BAG_LOOT]
        self.assertTrue(all(scene_checks("\n".join(bag), "bag-viewer").values()))
        for log in (bag[:-1], [*bag, LOOT], [*bag[:-3], MEAT, BAG_TAKE, BAG_LOOT]):
            self.assertFalse(all(scene_checks("\n".join(log), "bag-viewer").values()))

    def test_r65_requires_open_hidden_and_closed_loot_session(self):
        bag = [*SCENE[:4], BAG_KEY, R65_OPEN, R65_HIDDEN, BAG_TAKE.replace("650", "750"), MEAT, R65_CLOSE, BAG_LOOT]
        self.assertTrue(all(scene_checks("\n".join(bag), "bag-viewer", True).values()))
        for missing in (R65_OPEN, R65_HIDDEN, R65_CLOSE):
            log = "\n".join(line for line in bag if line != missing)
            self.assertFalse(all(scene_checks(log, "bag-viewer", True).values()))

    def test_auto_loot_requires_its_mode_and_not_your_kill_lines(self):
        bag = [*SCENE[:4], BAG_KEY, R65_OPEN, R65_HIDDEN, BAG_TAKE.replace("650", "750"), MEAT, R65_CLOSE, BAG_LOOT]
        mode = "addon eqoa_bag_viewer: [info] auto-loot: mode Everything but junk for Lsgm"
        skipped = ("addon eqoa_bag_viewer: [info] auto-loot: corpse 0x1234: fire bat: skipped, "
                   "not your kill (you did not hit it)")
        full = [WORLD, mode, *bag[1:4], skipped, *bag[4:]]
        self.assertTrue(all(scene_checks("\n".join(full), "bag-viewer", True, True).values()))
        for missing in (mode, skipped):
            log = "\n".join(line for line in full if line != missing)
            self.assertFalse(all(scene_checks(log, "bag-viewer", True, True).values()))
        self.assertNotIn("Auto-loot mode logged", scene_checks("\n".join(full), "bag-viewer", True))


if __name__ == "__main__":
    unittest.main()
