# Live shard compatibility

The live shard checks that a client is current before it offers the world.
Without the two answers below it withholds its `0x7E00` entity offer and
prints `Your client is outdated`. This code was developed in the former
private repository (`LoopRook/eqoa-enhanced-private`, commit `3848bb6` on
`fix/live-client-check`) and became public source on 2026-11-07, under the
repository's license (AGPL-3.0-or-later for code first published in 0.4.6 or
later; see NOTICE).

## What the client sends

1. **S2 device-block signature.** The 28-byte device block in the shard hello
   ends with `CDSS 01 02 0c 00 00 00 00 00` (bytes 16–27), not zeros.
2. **Client check.** A `0x7F53` message carrying a 16-byte nonce is answered
   with `0x7F54`:

   ```
   key  = 4e 65 c9 fa 23 21 67 b6 98 5f a9 e5 1a 7b 7d 2a
          18 4b 33 e0 1f 77 af b3 ea a2 ef e2 8e e6 94 b4
   msg  = "CDSS" + nonce(16) + 01 02 0c 00
   resp = HMAC-SHA256(key, msg)          // 32 bytes
   ```

3. **Defaults.** NPC/PC Display starts at 64 and NPC/PC Distance at 650.
   Both still go down to the game's 24 and 100.

Code:

- `crates/net/src/session.rs`: `answer_7f53`, `hmac_sha256`, dispatch.
- `crates/net/Cargo.toml` and `Cargo.lock`: the `sha2` dependency.
- `crates/eqoa/src/s2.rs`: `make_device_block` tail.
- `crates/eqoa/src/settings.rs`: `ENTITY_SLOTS_DEFAULT`, `ENTITY_VIEW_DEFAULT`.
- Tests: `the_client_check_is_answered`,
  `the_hello_s_bytes_and_the_install_id`, the Extended-page tests.

## Why the values are what they are

The nonce/response pair was read from a capture of the live client talking to
the live shard. The key is the live client's ipad block at `.rdata`
`0x140b84860` XOR `0x36`. Every distributed copy of that client carries it, so
it is a shared compatibility constant, not a per-user credential or a server
secret.

The capture also shows the shard sends `0x7E00` with `64` slots and `650` view
only after the `0x7F54` answer arrives.

On 2026-11-04 the shard moved the version half of the signature from
`01 02 0a 00` to `01 02 0c 00` in both the S2 device block and the `0x7F53`
message. The key did not change: the current official client's pad block still
decodes to the same 32 bytes, and the 2026-11-03 capture still reproduces under
the old tail. 0.4.6 and earlier sent `0x0a` and are treated as outdated.

## Ground rules

- The server operator has publicly said any client is allowed. Server access is
  governed by the operator's rules, not by this repository's license.
- The change only answers a compatibility gate. It does not touch account
  credentials, payment data, other players, or any security control. No user
  data is collected or sent anywhere new; the Station login path is unchanged.
- If the operator asks for it to stop, stop.
- The operator can change the scheme at any time. After a server or
  implementation change, update this document and its regression tests together.
- Other differences stay per-client: install id, `0x07E1` counters,
  state-channel content and timing. They are not identity claims.

## Verification

- `the_client_check_is_answered` reproduces the capture: nonce
  `9f 7a 2a d6 … 4d 5d` gives `90 f5 f8 46 … 6d 5f` under the old tail
  `01 02 0a 00` and `3d f2 2f fa … 05 c4` under the current `01 02 0c 00`.
  Its control: a short nonce is not answered.
- A live capture after the original change showed `0x7F54` sent, `0x7E00`
  offered (64/650), no "outdated" line, and `play: 64 entity slots`.
- Official builds 0.4.5 through 0.4.10 carried this code from the private
  repository; 0.4.11 is the first built from public source.
