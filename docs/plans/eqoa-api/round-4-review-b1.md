# Round 4 Part B, independent review b1 (API additions and migration)

Range `938ba4c..fe996e3` (6 commits), branch `feat/eqoa-api-r4`. Read-only review: no cargo run,
nothing executed except attribute checks of this machine's Desktop/OneDrive folders. The worktree
has uncommitted edits by another agent (file_io.rs, native_events.rs, native.rs, mod.rs,
options.rs, option_buttons.rs); where that matters I read the committed blob
(`git show fe996e3:...`). `player_files.rs`, `exchange.rs`, `map_migration.rs`, `discovery.rs`,
`bindpage.rs` are unmodified in the worktree, so their line numbers are the committed ones.
native_events.rs line numbers below are from the committed blob.

## Verdict: APPROVE WITH CHANGES

The design is sound where it matters most. The shared-file mount is fail-closed (exact string
match against the owner's declared shares, running owners only, no write path). The click rule is
a scoped, per-VM, spend-once slot rather than a flag that can leak. Export uses `create_new`, so
it cannot overwrite. The migration never touches the original and is marker-guarded. What needs
changing before merge: export will drop any file name or extension on the Desktop (HIGH), plus
four MEDIUM issues (queue eviction by cross-mod commands, reparse-point policy applied to
Desktop and to player-picked files, unbounded growth of `imports/`, no rate/total limit on
exports from any click). Nothing in the bundled mods is in this range, so mod-level behaviour
(command ids `import_journal`/`export_journal`, bind ids `open`/`clear`) is unverified here.

## HIGH

### H1. Export lets a mod write any file name and any bytes to the Desktop (player_files.rs:308-357)

The only checks on `file_name` are: nonempty, at most 128 bytes, no `/` or `\`, and that
`m.key("imports/<name>")` accepts it (no `:`, `..`, trailing dot/space, reserved device names).
There is no extension policy. The source is any private file, and a mod can put arbitrary bytes
in it (`eqoa.files.write`, `save_json_async`, and Lua strings are binary-safe).

Failure scenario: a mod with `"permissions":["export"]` writes a `.bat`, `.cmd`, `.ps1`, `.lnk`,
`.url`, `.js`, `.vbs`, `.scr`, `.exe` or `desktop.ini` payload into its private data, then calls
`eqoa.files.export("data/x","Open Me.bat")` from any click handler. It lands on the player's
Desktop. The player only saw a "Permissions: export" text line in the manager and a normal click
elsewhere. §9 promises no process execution and no ambient file authority; this is a one-click
staged-execution drop point.

Fix: export extension allowlist (`json`, `txt`, `csv`, `png`, maybe `log`), refuse names that are
only an extension or that change the source file's extension class, and refuse `desktop.ini` and
dot-files. Add a test that `.bat`/`.lnk`/`.exe` and `desktop.ini` are refused (negative control:
the current code accepts them).

## MEDIUM

### M1. Cross-mod commands can evict the target's queued events (exchange.rs:73-147, native_events.rs:853-864, 729-747)

`payload()` allows 256 nodes and 4096 bytes per string, about 1 MiB per command. `command()` only
caps the number of pending `mod.command` events at 64 and then `enqueue()`s. `enqueue` evicts the
oldest events of any kind once `pending_bytes > 2 MiB` or 1024 events.

Scenario: a third-party mod (no permissions needed, only knowledge that the journal mod declares
`import_journal`) sends three 1 MiB `import_journal` commands in one frame. The journal mod's
pending `entity.*`, `resource.ready` (its own save acknowledgement), `shared.changed` and any
earlier player-marked command are dropped (reported only as `events.dropped`). The journal loses
sightings and can lose the player's own Import click. Also, the 64-slot cap is shared by all
senders: one sender fills it and the search mod's real `import_journal` gets "queue is full".

Fix: per-command size cap (for example 8 to 16 KiB serialised, the brief says "bounded"), a
per-sender-per-target pending cap (for example 4 to 8), and do not let a command push out other
event kinds (reject instead). Test: flood from mod A, assert mod B's other events survive and a
command from mod C still gets in.

The same eviction lever exists in smaller form for `shared.changed` (publish() to every active
VM, exchange.rs:65-70), but it needs a mod with `shares` and a working write loop, so I rate it
LOW.

### M2. The reparse-point/symlink sandbox policy is applied to the Desktop and to player-picked files (player_files.rs:161,181-182; sandbox.rs:140-189, 216-247)

`unique_copy()` calls `no_links(dir)` and `lock_parents()` on the Desktop path, which rejects any
ancestor with `FILE_ATTRIBUTE_REPARSE_POINT` (0x400). Import reads the player's chosen file via
`read_bounded` -> `lock_resource` -> the same checks on every ancestor. That policy is right for
mod-owned storage. For OS-provided and player-chosen locations it fails closed on legitimate
setups: OneDrive Known Folder Move placeholders/dehydrated folders (cloud-file reparse tags),
junction-redirected profile folders, and UNC folder redirection (`\\server\share` cannot be
opened as a directory by `lock_parents`). The error text the mod sees is "reparse point in addon
file path", which the player never sees explained.

On this machine the Desktop is plain (`%USERPROFILE%\Desktop`, no reparse attribute) and the
OneDrive root is also not a reparse point, so I could not reproduce it here; I mark it MEDIUM
because it is likely on OneDrive-backed Desktops and it is untested: the tests only use a temp
`TestIo.desktop`, and `#[cfg(test)]` replaces both `pick()` and `desktop()`, so neither the real
`rfd` path nor `desktop_dir()` runs in any test.

Fix: for the Desktop destination and the picked source use plain `std::fs` with `create_new` and
a bounded read (the player/OS chose the location; the sandbox goal is protecting the mod's
private tree, which the private-side checks still cover). Add a `Desktop folder unavailable` /
`not writable` player-facing diagnostic. If the Desktop folder is missing, current code does
create it (`create_dir_all`, line 162); that is acceptable but should be a deliberate choice.

### M3. `imports/` grows without limit and can starve the journal's own saves (player_files.rs:283-298)

Each import writes `imports/import-<id>.json` (up to 16 MiB). `<id>` restarts at 1 every time the
VM starts, so names collide and fall into the ` (n)` loop (up to 10000 files). Only
`check_quota` limits it (16 MiB total private quota, 4096 entries; the 64-file `overlay_room`
limit is bypassed because the copy does not go through `Mounts::write`). Nothing in the native
side removes the file after the mod has loaded it. Scenario: a player imports a 1.4 MB Forge
journal a few times, trying to fix a bad import; the private quota fills and the journal's
`save_json_async` then fails with "addon storage exceeds 16 MiB" while the mod still shows the
journal. The bundled journal mod is outside this range, so confirm it removes `imports/*` after
parsing (`eqoa.files.remove` works for these paths); otherwise add a native cap (keep only the
most recent import) or have `import` replace `imports/last.json` atomically.

### M4. Any player click or key anywhere authorises a dialog; no rate or total limit on exports (native_events.rs:484-486; player_files.rs:105-145,173-204)

`direct` is true for any real `input.key` down (not repeat/synthetic) and any `pointer.pressed`
inside the window, with nothing tying it to a visible control of the mod. The brief asks for
exactly this ("a pointer/key event handler"), so this is a design note with an operational
consequence rather than a bypass: a mod can open a native file dialog or export on the very first
key/click after enabling, and on every click thereafter, up to 10000 `(n)` copies of up to
16 MiB each on the Desktop (160 GB), one per click. There is no per-mod rate limit, no total
Desktop-bytes cap, and the dialog title is mod-controlled text.

Suggested: one export per N seconds per mod (for example 5 s), a cap on Desktop copies per
session per mod, and prefix the dialog title with the mod name so a mod cannot impersonate the
OS. For import/export that must come from a visible control, the option-button path in the
uncommitted `option.activated` work is the better trigger; leave the raw key/click path as a
documented fallback.

## LOW

- L1. Rfd is only a dependency on linux/macos/windows (`Cargo.toml:64`), yet `pick()`
  (player_files.rs:222-233) uses `rfd::FileDialog` unconditionally; on `target_os = "android"` that
  does not compile, and on Linux/macOS every file op already fails in `lock_parents`
  ("unsupported"). The old `map/journal.rs` had the same cfg gap, so it is not a regression, but
  gate this `pick()` and return an `unavailable` error. Parity gap with the old journal: it fell
  back to `Desktop/EQOA-Creature-Journal.json` when the dialog gave nothing; the new import returns
  `cancelled` (also for a missing portal/zenity on Linux).
- L2. `poll()` reports a worker disconnect as `kind:"file"` with `capability eqoa.files.file`
  (player_files.rs:63-67), so a mod waiting for kind `import`/`export` gets an unfamiliar kind.
  Keep the kind in `Service`.
- L3. Disable-while-open: `close()` flips the permit and drops the receiver, so nothing is copied
  and no event is raised (good), but the OS dialog stays on screen until closed, and a re-enable
  can open a second dialog (the new Service has no `pending`). Acceptable; note it in the spec
  and add a test with a blocking `pick`.
- L4. Export writes straight to the final name; a crash mid-write leaves a truncated file on the
  Desktop (only removed on error or permit loss, lines 186-197). Write to a `.tmp` and `hard_link`
  or rename with `create_new` semantics.
- L5. `shared.changed` fires on save and write but not on `remove`, so a reader keeps a stale
  journal when the owner deletes it (exchange.rs:65-70, sandbox.rs:101).
- L6. A player-marked command sits in the target queue until its next `dispatch`; if the queue is
  backed up (delivery budget 256/iteration) it can arrive some frames later than the click.
  Bounded, but add an expiry (for example the mark is dropped after 2 frames).
- L7. Command ping-pong (A sends B, B answers A) is allowed forever, one hop per boundary. It is
  bounded by the 64 cap and the 256/iteration budget, but the cost lands on both mods. Optional
  per-pair rate limit.
- L8. Migration ordering: the marker means the copy happens once, at first start of the first
  build that has it. Native `journal.rs` still records into `config/journal.json` at this commit;
  sightings after the copy and before native removal ship are not carried. Fine if the migration
  and the native removal ship in the same release; say so in the report.
- L9. Migration retries forever (and `eprintln`s each start) if the copy fails (journal near or
  above the 16 MiB private quota, or `hard_link` unsupported on a non-NTFS config volume).
  Fall back to `fs::copy` to a temp plus a `create_new` rename, and write the marker with an
  error note after N failures.
- L10. Key migration has no collision check: it keeps the player's key as written, and
  `dispatch_keypress` just returns the first bind with that key. In practice native swap logic
  saved both sides, so I found no reachable collision, but there is no test with a legacy file
  holding only `key.map_search=` while a different mod owns that key. Also the cleanup rewrites
  the native binds file (removes the migrated lines), so a downgrade loses the rebound search
  key; documented behaviour for stats_box already, so just keep it consistent.
- L11. Spec (`EQOA-API-SPEC.md`) is missing: error codes (`permission_denied`, `requires_player`,
  `busy`, `invalid_argument`, `quota`, `unavailable`, `io_error`), the 16 MiB cap, "one request at
  a time", that imports are always stored as `.json`, that the extension filter is a hint (a
  player can pick any file in the OS dialog), and that a command received from a click may be
  used once and may not be re-delegated.

## Answers to the seven checks

1. Sandbox. Cross-mod reads go through `exchange::read` (exchange.rs:31-64): the path must be
   `@id/<key>`, an active owner VM with that id must exist, and `<key>` must equal one of the
   owner's declared `shares` byte for byte, so case, `..`, `data/./x`, `::$DATA`, backslash and
   `\\?\` variants all fail before any filesystem call. The key is then re-normalised through
   `Mounts::key` and must be unchanged, resolved with `read_path` (private overlay and tombstone
   only; a `data` key cannot fall through to package files), then `no_links` again. Disabled
   owner: `clear()` sets `active=false`, the Weak bus dies with the VM, read fails with
   `permission_denied`. `files.read/write/list/remove` reject a leading `@` in `relative()`
   (file_io.rs:209). Shares are validated to `data/...` with at most 8 components and no
   `\:*?<>|` or control characters (discovery.rs:44-52). Nothing outside the owner's private
   `files/data` is reachable. I found no escape. (Not tested: case variants and Windows forms;
   the test covers undeclared path, `..`, write, owner disabled.)
2. Click rule. The slot is `(vm, kind)` in a thread-local, set by `player_scope` around
   one event's handlers and restored on drop (exchange.rs:199-240). Timers, `after`, other
   events and render run with kind 0, so a stored callback or timer never has authority
   (tested with `eqoa.events.after(0, ...)`). There is no Lua-level emit, so re-entrancy
   cannot create a click. `spend_player(lua,false)` zeroes the slot, so a second call in
   the same click fails with `requires_player` (tested: `again.json`). Delegation:
   `send` spends the sender's kind-1 slot only if the command is actually queued and marks that
   event; the receiver gets kind 2, which can spend once for a file action but cannot delegate
   again (tested through `forward_denied`). Gaps: `pointer.pressed` and the `inside_window`
   false / `synthetic` / `repeat` negatives are untested (only an `input.key` press is used);
   see M4 for the breadth of what counts as a click.
3. Export. `create_new` plus a `(n)` loop up to 10000 never overwrites, including under
   concurrent exports (tested against an existing file). Name checks are decent but extension
   policy is missing (H1). Only private `data/...` files, via `m.key` + `starts_with("data")` +
   `read_path`; `../outside` is refused (tested); resources and package files are refused.
   Desktop missing: `desktop_dir()` returns None only if neither known folder nor fallbacks
   exist, then `unavailable`; a missing directory is created. OneDrive/reparse: see M2.
4. Import. Dialog and the 16 MiB bounded read run on a worker thread, not the frame
   thread. Cancel yields `resource.failed` code `cancelled` (tested). Huge file: `read_bounded`
   uses `take(max+1)`, so no unbounded read. The copy goes only to `files/data/imports/` with a
   fixed `import-<id>.json` name (a picked file's name and extension never reach the
   filesystem), with `check_quota`. Disabled while open: permit stops the copy (L3). Growth: M3.
   Picked-file reparse policy: M2.
5. Commands. Declared only (`commands` list), missing/disabled/undeclared targets return
   `nil,Error` (tested), queue cap 64 per VM, payload depth 8, 256 nodes, strings 4096 bytes,
   keys 256 (tested "huge" at 1024 elements). Self-send works. Loops are bounded but not
   prevented (L7). The size/fairness gap is M1.
6. Migration. Runs once (marker `addons/journal-migration-v1`, written only after success); source
   is only read; `!target.exists()` guards an existing mod journal; uses a synced temp plus
   `hard_link` so a racing second start cannot overwrite; tested including "deleted journal is
   not resurrected" and "original unchanged". Keys: `key.map_search`/`key.map_stop` map to
   `mod.eqoa_map_search.open`/`.clear`; an existing `mod.*` line wins, including an explicit
   empty (unbound) value (tested). Unverified: that the bundled mod's bind ids are really
   `open` and `clear` (mod not in range). See L8-L10.
7. Tests. The behaviour tests run through the real Lua host with `CORE.emit` (protocol-shaped
   `input.key`), which is good. Negative controls: the tests contain negative assertions
   (permission denied, requires_player, undeclared, traversal, quota, no overwrite,
   no-resurrect), but the commits carry no record of a run without the fix and the Part B report
   section is not written yet, so I could not confirm any control was actually run. Not covered:
   real `rfd`/`desktop_dir` paths (replaced by `cfg(test)`), disabled-while-dialog, pointer-click
   authority, reparse/OneDrive, flood fairness, per-sender caps, shares case/Windows forms,
   command ping-pong.
