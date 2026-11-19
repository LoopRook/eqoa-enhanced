#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


class TestDump(unittest.TestCase):
    def test_opcode_filter_distinguishes_state_channel_from_opcode(self):
        tool = Path(__file__).with_name("rec_dump.py")
        magic = b"EQREC\x01\r\n"
        payload = b"\x18\x00\xAA"
        receive = struct.pack("<QBBI", 10, 0, 251, 3) + payload
        state = struct.pack("<QBBI", 20, 1, 0x40, 3) + payload
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "sample.eqrec"
            path.write_bytes(magic + receive + state)
            all_rows = subprocess.check_output([sys.executable, str(tool), str(path)], text=True)
            filtered = subprocess.check_output(
                [sys.executable, str(tool), str(path), "--op", "0x0018"], text=True
            )
        self.assertIn("sent type=0x40", all_rows)
        self.assertEqual(filtered.strip(), "10 us received type=0xFB op=0x0018 3 bytes 1800aa")
