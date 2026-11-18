# SPDX-License-Identifier: AGPL-3.0-or-later
"""Check always-on takeover diagnostics from a built eqoa libtest executable.

Usage: python crates/eqoa/src/fork_ext/addons/test_round65_takeover_logs.py <eqoa-test-executable>
The fixture sessions never contact a live server. GPU tests run serially.
"""
import re
import subprocess
import sys


def run(binary, name):
    result = subprocess.run(
        [binary, name, "--nocapture", "--test-threads=1"],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        encoding="utf-8", errors="replace",
    )
    assert result.returncode == 0, result.stdout
    assert "1 passed; 0 failed" in result.stdout, result.stdout
    return result.stdout


def main():
    binary = sys.argv[1]
    talk = run(binary, "round65_fix2_merchant_uses_native_flags_and_exact_request")
    assert re.search(r"mod talk -> npc 0x24b9,.*opcode 0x004c", talk), talk
    loot = run(binary, "round65_fix2_open_button_only_displays_never_takes")
    assert re.search(r"mod open_loot -> corpse 0x2329, opcode 0x0015", loot), loot
    assert "loot window HIDDEN" in loot, loot
    declined = run(binary, "round65_fix_m2_pending_open_never_claims_a_player_started_open")
    assert "window left to the game" in declined, declined
    close = run(binary, "round65_fix2_one_close_per_hidden_session")
    assert "hidden session closed (released by the mod)" in close, close
    # Fixture (a): one client close. Fixture (b): one server close. Fixture (c): none.
    # These are local handler echoes, distinct from packets sent over the session.
    assert close.count("play: 0x0016 (0 bytes)") == 2, close
    print("PASS: request ids/opcodes, accept/decline, close reasons, and exactly two close-handler entries")


if __name__ == "__main__":
    main()
