# Owner test session recorder

Native owner test packages enable recording from both Play launchers. Outside the test package,
set `EQOA_RECORD_SESSION=1` before launching the client. The client starts a file when a character's
game connection opens or its first world dump arrives. The files sit beside `eqoa.log` in the
configuration directory. They are local diagnostic files and may contain character names, player
chat and mail. Keep them on the test machine.

After reproducing the issue and closing the game, run the collector and search the relevant part.

```text
python tools/collect_owner_test.py <package>
python tools/rec_dump.py <recording.eqrec> --op 0x0018
```

The next package launch moves the preceding recordings and logs to the same timestamped
`results/` folder. The collector prints the filename, part size, time between its first and last
messages and receive/send counts. The dump tool prints the full payload in hex. It derives the
little-endian opcode from guaranteed or best-effort messages. State channels display their channel
number in the type field.

## Capture and format

Each part starts with eight bytes `45 51 52 45 43 01 0d 0a`. Every row then has a little-endian
`u64` elapsed microsecond timestamp, one direction byte (0 received, 1 sent), one DRDP type byte,
a little-endian `u32` payload length and the complete payload. Timestamps are monotonic across
parts of a run. Normal boots use the game's UTC log run stamp and the process ID in the name.
Command-line play uses UTC epoch seconds and the process ID. Part numbers increment from 0001.

One part holds at most 32 MiB. Rotation happens at a message boundary and keeps the newest four
parts of that process run. Files from previous runs are retained. A row too large for an entire
part is skipped. The writer opens files exclusively and stops recording after an I/O error.

Inbound payloads are observed after DRDP reassembly and before protocol dispatch, including
messages that change session state without producing a Play event. Outbound rows come from
`Session.record`, in enqueue order. They are timestamped when drained before a received
message, at the next Play iteration or when Play is dropped. This includes the normal logout
send and gives frame-level send timing. A forced exit can lose buffered rows or queued sends.
The capture covers logical game messages, so wire acknowledgements, retransmissions and
connectionless probes are outside its scope.

## Privacy

Station TCP account operations and the initial UDP lobby, shard login and character selection
traffic are outside the capture. The recorder arms on a DRDP game promotion in character
selection, with the first world dump as a fallback for servers that use the existing connection.
It also drops these opcodes in either direction after the capture starts.

- `0x0000`, `0x0001`, `0x0904`, `0x07B3` for version/device hello, shard and lobby authentication
  and shard discovery.
- `0x7F53`, `0x7F54` for the client challenge and response.
- `0x0790`, `0x0791`, `0x0992`, `0x0993` for probe negotiation, token replies and probes.
- `0x7E0F`, `0x7E65` for bug-note and crash-report bodies.

Before writing, the recorder drops any whole payload containing the account name or password
that the client registered at login, in either UTF-8 or UTF-16LE, or a derived UDP login token.
This also catches literal copies of those values in gameplay text. It cannot identify unrelated
credentials or altered spellings in free text. The collector reports metadata only. The package
builder excludes recordings when it copies a starting configuration, and Git ignores `.eqrec`
files at any depth.

## Replay and checks

Use `fork_ext::session_replay::rows` in native tests with bytes from one part, or take the needed
slice of decoded rows. `Row::deliver` uses DRDP's local endpoint to pass server messages through
`Session::pump` in an existing `Session::connect_with` rig. Advance the test's state and trigger
client inputs between rows as needed. Outgoing rows are available for comparison with the
test client's sends. Replay skips connection notifications and runs without network access or
wall-clock delays.

The hand-made round 6 fixture contains a two-item `0x0018`, the outgoing `0x0019` take, the
`0x003B` inventory confirmation, `0x0016` close and one-item `0x0018` reopen. The test checks
the outbound bytes, the close and reopened list, and one Bag Viewer loot-log entry. Its negative
control omits the inventory confirmation and expects an empty loot log after the same reopen.

The recorder privacy test supplies lobby traffic, excluded opcode canaries, and gameplay
messages containing a fake account, a fake password in both encodings and its derived token.
It compares the resulting file to the exact allowed rows in order, including a state channel
whose prefix matches a blocked opcode. The rotation test forces one complete row per part and
checks that six messages leave exactly the newest four parts. The Python tests cover both
launchers, cross-run archival, exclusion from copied config, metadata, truncation and opcode
filtering with a state-channel negative control.

## Ownership

Two lines were added to shared network code. Both use the footprint reason
`session recorder seam, lead-approved 2026-11-18`.

- `crates/net/src/lib.rs` declares `pub mod fork_record;`.
- `crates/net/src/session.rs` calls `crate::fork_record::receive(self, &m);` before dispatch in
  `Session::pump`.

The disabled receive path is an atomic check with no allocation. Recorder implementation, app
integration and replay live in fork-owned files. The search tool runs in Python.

## Verification

- The owner package and collector suite passed all 26 tests, and the recording search CLI
  passed its focused test. An opcode search on the loot fixture found both corpse lists.
  Before implementation, the owner tools suite failed on the missing launcher flag and
  missing recording metadata.
- The focused `net` tests failed as expected when the filter allowed the lobby login opcode
  `0x0904` and when rotation kept a fifth part. Both passed again after restoring the source.
  The add-on gate ran the GPU-backed replay with both the confirmed take and the
  no-confirmation control.
- `python tools/run_gates.py --release` passed add-ons (515 passed, 25 ignored), WASM,
  footprint, local path checks, player-release tooling and the native release build. The
  initial full step compiled the client without rebuilding the cached `net` library and failed
  to find the recorder module. Rerunning the full step under the same lock with fresh source
  timestamps rebuilt both crates. The retry reported 2,396 passed, 0 failed and 30 ignored.
- The add-on gate reported nine prerequisite skips. Four Windows symlink tests lacked
  privilege, two Forge tests lacked the pinned spike checkout and three picture tests lacked
  reviewed goldens. The full suite printed 50 skip lines (30 distinct), also covering missing
  extracted client files, live S2 patches and save state, 2002 disc and server art. The gates
  used the linked Frontiers disc. There was no live server test.
