# EQOA Enhanced

A native Rust client for EverQuest Online Adventures: Frontiers, forked from the vanilla client
(`origin`, github.com/DabDavis/eqoa-rust-client-vanilla) and published as LoopRook/eqoa-enhanced
(`looprook`). Vanilla keeps the game as it was; this fork adds to it: modding (native Lua addons
that run unchanged FrontiersForge packages), the native maps and guidance, and quality-of-life
features. Additions are this fork's purpose. Each one is the owner's call: propose it, then build
it once approved.

The owner does not write code. Agents implement; the owner plays the build and decides acceptance.
Report results in a player's terms, with what was tested and what was not.

## Starting a session

1. Read `FORK.md`. It is the source of truth for scope, file ownership and merge policy.
2. Read the current handoff for the work at hand. Handoffs live in `docs/` on the branch that holds
   the work (Forge/addons: `docs/FORGE-COMPATIBILITY-HANDOFF.md`). `docs/backlog.md` holds deferred
   requests.
3. Read `C:\Users\hocka\Paseo\Agent_Learning_Instructions.md` and the experience log beside it. Add an
   entry there after each meaningful task, and update the existing entry when the owner's test
   results arrive.
4. Check `git status`, the branch, and `git worktree list` before touching anything.

## Branches and worktrees

| Branch | Role | Who moves it |
|---|---|---|
| `master` | Upstream mirror: fast-forward to `origin/master` only. This is the sync branch. | Sync only |
| `enhanced` | **Stable.** What players run. Release tags `vX.Y.Z` are cut here. GitHub's default branch. | Release merges from `develop`, hotfixes |
| `develop` | **Development.** Integration of finished, tested features; previews are built from here. | Merges of feature branches |
| feature branches | One per piece of work (`forge-compatibility`, `fix/...`, `feat/...`), branched from `develop`. | The agent doing that work |

- A **worktree** is a separate folder with its own branch checked out, sharing one repository. Give
  each concurrent agent its own Paseo worktree (`create_workspace`, isolation `worktree`) so agents
  never edit the same folder. The main checkout (`C:/Users/hocka/Github Projects/EQOA Native Rust`)
  stays on `develop`.
- A feature merges into `develop` after its tests pass, the footprint check passes and an independent
  review is done. `develop` merges into `enhanced` only for a release the owner approved, after the
  owner's live test.
- Upstream sync: branch `sync/<date>` from `develop`, merge `origin/master` (procedure:
  `docs/UPSTREAM-SYNC.md`), test, merge into `develop`. A CLAUDE.md conflict always resolves to this
  file; refresh `docs/upstream/CLAUDE.md` from the new upstream, read upstream's diff for rules worth
  adopting here, and move `reviewed_base` in `docs/fork-footprint.json` deliberately.
- Hotfix: branch from `enhanced`, release, then merge `enhanced` back into `develop`.
- Commit as LoopRook (the repo-local git config). Pushing to `looprook`, tagging and merging into
  `enhanced` need the owner's go-ahead each time. Nothing of the fork goes to `origin` or `master`.

## Agents (Paseo)

Claude leads and spends its allowance on decisions, integration and review; bounded work goes to
workers. Read `list_profiles` and pick by each profile's notes. Defaults:

- Scouting (read-only): Scout-Free (DeepSeek free), then Scout-Backup (GLM).
- Implementation: Worker-Deepseek (free) or Worker-DeepSeek-Go for scoped fixes; Worker-Build (Kimi)
  for multi-file features; Worker-Second (MiniMax) for an alternate approach or a second lane.
- Hard bugs: Worker-Debug (GLM high). Review: Review-Codex when Codex has quota, otherwise MiniMax
  or GLM, never the same model that wrote the change.
- Antigravity (Gemini) runs with no permission prompts. Give it its own worktree and a feature
  branch, tell it to commit only there (no push, merge, tag, or other folders), and review its diff
  before anything merges.

One cargo build at a time across all agents: builds are slow and share the CPU. Keep each worktree on
its own `CARGO_TARGET_DIR` unless the handoff names a shared one.

## Engineering rules kept from upstream

Upstream's full instructions are preserved verbatim in `docs/upstream/CLAUDE.md`. Read the matching
section there before changing a ported system (anything under `crates/vi`, `ee`, `dnas`, `snd`,
`drdp`, the pregame, input, the renderer, the patcher, S2). These always apply:

- New fork code goes in fork-owned files (`crates/eqoa/src/fork_ext/`, `map/`, `questguide/`, ...);
  shared upstream files get a hook call within their budget. `python tools/check_fork_footprint.py
  --base <reviewed_base>` must PASS; a budget grows only with a reviewed reason.
- Game-logic floats and compares go through `ee::*`. `vi` structs stay `#[repr(C)]` in the EE layout.
- Port game behaviour from the decomp, line by line. The decomp, eqoa-port's oracle and the 2002
  disc are not on this machine, so oracle tests skip here: say so in the report.
- What the game counts in frames steps on `frames.rs`'s 30 Hz clock, never once per frame drawn.
- Windows: the main thread links an 8 MB stack (`crates/eqoa/build.rs`); the program is a GUI
  subsystem binary.
- Tests: behaviour tests with a negative control (show the test fails without the fix). A test that
  logs SKIP and returns Ok proved nothing; count skips in the report.
- Never commit game data, disc images, decrypted modules, server patches, credentials, or anything
  under `run/` or `Testing Resources/`.

## Where upstream's rules give way to this fork

`docs/upstream/CLAUDE.md` is written for the vanilla client. For this fork:

- Scope: `FORK.md` and the owner's decisions replace the "Vanilla" rule.
- Identity: commits are LoopRook's.
- Releases: versioned tags on `enhanced` through `enhanced-release.yml` (`docs/RELEASING.md`);
  upstream's `dev.yml`/`dev` release is not used.
- Notes: player notes go in `FORK-UPDATE-NOTES.md`; `UPDATE-NOTES.md` stays upstream's.
- Reports: submission is off by default (`FORK.md`); test against a loopback fixture only.

## Platform

Windows 11, PowerShell and Git Bash. Rust in `C:/Users/hocka/.cargo/bin`, Python at
`C:/Python314/python.exe`. Managed worktrees need `git -c safe.directory=*` per command; leave global
git config alone. The owner's disc: `C:\Users\hocka\Documents\PS2 Games\`.
