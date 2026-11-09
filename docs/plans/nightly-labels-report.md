# Nightly labels and release notes fix

Branch: `fix/nightly-labels` (from `develop`). No push, merge or tag was made.

## What changed

### A. Nightly release body

- `FORK-UPDATE-NOTES.md` uses `## Unreleased` again (its bullets are kept and it
  gains a short note about the Nightly label). `tools/dev_release.py` therefore
  publishes those bullets instead of "Same as Stable.".
- `tools/dev_release.py` now rejects a malformed Unreleased heading in
  `FORK-UPDATE-NOTES.md` during preflight (exactly one `## Unreleased`, or no
  Unreleased heading at all). The same check is covered by tooling tests.
- `docs/RELEASING.md` documents the stable release step explicitly: at a
  versioned release, rename `## Unreleased` to
  `## <YYYY-MM-DD> · Enhanced <version>`, keeping its bullets.
- `crates/eqoa/src/update.rs`:
  - the built-in-notes test checks the dated sections and ignores the moving
    `Unreleased` heading;
  - `whats_new` treats an `Unreleased` section as new when its bullets changed,
    so a Nightly update still shows its notes (instead of "Nothing").
- The update window already displays a section title as written, so it shows
  `Unreleased`; no crash or reordering occurs.

### B. Player-facing version label

New fork-owned `crates/eqoa/src/fork_ext/version.rs`:

```rust
display_version(channel, commit, version)
```

- Stable (`enhanced`) and local/unknown channels: plain `VERSION` (e.g.
  `0.4.14`).
- Nightly (`enhanced-dev`): `0.4.14+nightly (07b915a)`, or
  `0.4.14+nightly` without a stamped commit.

It feeds:

- `logfile.rs` run head (shared file: one replaced line);
- `fork_ext/report_core.rs` version, used by crash files, the crash offer text
  and the crash JSON body;
- `fork_ext/report.rs` `enhanced_label()`, used by the log header, report ZIP
  manifest and report state snapshot.

The report label keeps Stable/local output unchanged
(`EQOA Enhanced 0.4.14 (abcdef1)`) and prints
`EQOA Enhanced 0.4.14+nightly (abcdef1)` for Nightly. The in-game window title
is plain `EQOA` and About/options show no version; the update window names no
build version, so there was nothing to change there.

## Tests run

- `python tools/check_fork_footprint.py --base 8435f2c8a00c631d716771f8189fa0011a1fbf50` — PASS.
- `python -X utf8 -m unittest discover -s tools -p "test_*.py"` — 94 passed.
- `cargo test --locked --workspace --target x86_64-pc-windows-msvc -- --test-threads=1`
  with scratch `EQOA_CONFIG`, `CARGO_TARGET_DIR=C:/Users/hocka/Paseo/targets/nightly-labels`
  and `RUSTFLAGS=-C target-feature=+crt-static` — 1,939 passed, 0 failed,
  27 ignored (26 in the workspace plus the existing ignored `fog16` GPU test).
- Focused rerun after the final notes edit: `update::tests::` 21 passed;
  `fork_ext::version::tests` 2 passed.
- `cargo build --release --locked -p eqoa --target x86_64-pc-windows-msvc`
  with `EQOA_UPDATE_CHANNEL=enhanced-dev` and the current HEAD stamped — succeeded.
  Existing compiler warnings only.

New tests include positive and negative controls:

- real `FORK-UPDATE-NOTES.md` notes publish bullets; renaming the heading makes
  the body fall back to "Same as Stable.";
- malformed Unreleased headings are rejected, including by preflight;
- `display_version` for Stable, local, Nightly, missing commit and blank commit;
- `whats_new` shows a changed `Unreleased` section and hides an unchanged one.

## Footprint

`--base 8435f2c8a00c631d716771f8189fa0011a1fbf50` PASS.

Two reviewed budgets changed:

| Shared file | Old budget | New budget | Reason |
| --- | ---: | ---: | --- |
| `crates/eqoa/src/update.rs` | +3/-3 | +9/-6 | Nightly publishes `## Unreleased`; `whats_new` compares that moving section by content and the built-in-notes test checks dated sections separately. Upstream updater mechanics remain unchanged. |
| `crates/eqoa/src/logfile.rs` | +8/-0 | +9/-1 | One upstream log-label line now uses the fork build label so Nightly names its build and Stable/local show `VERSION`; the other eight hook lines are unchanged. |

Both reasons are recorded in `docs/fork-footprint.json` and
`docs/FORK-OWNERSHIP.md`.

## What is not tested

- No live game run and no GitHub publication (no disc/server session, no push).
  The Nightly release page itself was verified only through the release-body
  and tooling tests.
- The stable release conversion of `## Unreleased` remains a documented manual
  step in `docs/RELEASING.md`; it was not performed here.
- The plain `python tools/check_fork_footprint.py` (without `--base`) expects
  CI's step that points `refs/remotes/origin/master` at the reviewed base.
  This worktree's `origin/master` is elsewhere, so only the required
  `--base 8435f2c8...` form was used locally.
- The compiled Nightly label was checked by unit tests and by a release build
  stamped with `enhanced-dev`; its visual appearance in the running game was
  not inspected.
