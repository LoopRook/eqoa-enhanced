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


if __name__ == "__main__":
    unittest.main()
