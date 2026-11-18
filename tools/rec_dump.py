#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Inspect a local EQOA game recording."""
import argparse
from pathlib import Path
import struct

MAGIC = b"EQREC\x01\r\n"
HEADER = struct.Struct("<QBBI")
MAX_PAYLOAD = (32 << 20) - len(MAGIC) - HEADER.size


def records(stream):
    if stream.read(len(MAGIC)) != MAGIC:
        raise ValueError("invalid or unsupported recording header")
    while header := stream.read(HEADER.size):
        if len(header) != HEADER.size:
            raise ValueError("truncated message header")
        micros, direction, kind, size = HEADER.unpack(header)
        if direction not in (0, 1) or size > MAX_PAYLOAD:
            raise ValueError("invalid message header")
        data = stream.read(size)
        if len(data) != size:
            raise ValueError("truncated message payload")
        yield micros, direction, kind, data


def parse_opcode(value):
    opcode = int(value, 0)
    if not 0 <= opcode <= 0xFFFF:
        raise argparse.ArgumentTypeError("opcode must fit in 16 bits")
    return opcode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("file", type=Path)
    parser.add_argument("--op", type=parse_opcode, help="filter by opcode, for example 0x0018")
    args = parser.parse_args()
    try:
        with args.file.open("rb") as stream:
            for micros, direction, kind, data in records(stream):
                opcode = int.from_bytes(data[:2], "little") if kind in (251, 252) and len(data) >= 2 else None
                if args.op is not None and args.op != opcode:
                    continue
                op = f" op=0x{opcode:04X}" if opcode is not None else ""
                print(f"{micros} us {('received', 'sent')[direction]} type=0x{kind:02X}{op} {len(data)} bytes {data.hex()}")
    except (OSError, ValueError) as error:
        parser.exit(1, f"Error: {error}\n")


if __name__ == "__main__":
    main()
