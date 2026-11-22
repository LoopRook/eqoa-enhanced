# Round 8 WP7: draft note to Winfro and PR descriptions (2026-11-22)

Draft only. Nothing is sent. Places the owner must choose are marked `[OWNER: ...]`.
Numbers come from `round-8-upstream-pr-plan.md` (the plan) and `round-8-host-map.md` (the map). Each is counted
there; none is new.

---

## Part 1. The note

**Subject: Would you welcome a few small PRs from a fork?**

Hi Winfro,

I play EQOA Frontiers on your client, and I run a fork of it called EQOA Enhanced. I don't write code. AI agents
write it for me, and I test it by playing. I'm writing before I send anything, because your guide says to ask
first.

The fork keeps your game as it is and adds mods. Players can drop in Lua addons, including existing
FrontiersForge addons, and they run on a stable, documented API called `eqoa/1`. We built it so that the fork
ends up as your client plus mods. Right now the fork changes 907 lines and removes 138 in 43 of your files. About
200 of those lines are general-purpose hooks. I'd like to offer those, so the fork gets smaller and syncing with
you gets cheaper.

If you are open to it, here is what I'd send, in this order. Each one is its own PR. You can take any and leave
the rest.

1. **Small fixes.** About 245 lines in all, each with a test. [OWNER: offer the fixes, or leave them out?]
2. **Observer and overlay.** The game reports its events to a listener, and a layer can draw over the finished
   frame. About 45 call lines plus one new file. This is the smallest useful piece.
3. **Window seams.** Read-only window state, HUD parts, and a map overlay. About 95 lines.
4. **Option rows declared once.** A cleanup of the options tables.
5. **The mod host.** The Lua runtime, the `eqoa/1` API and the Mods & Addons window. It is large, about 100,000
   lines, almost all in new files. In your files it needs about 25 lines plus Cargo.lock. Nothing runs until a
   player turns a mod on. [OWNER: offer the host, or only the seams?]

Some things hold for all of them. The fork's code is AGPL-3.0-or-later, like your client. [OWNER: confirm the
licence and which PRs it covers.] Names are neutral, with no fork names. A player who runs no mods sees no
change. Each PR is cut from your current master and comes with tests that fail without the change. I won't
reformat your code.

Any of these would add features to your client, so I'm asking first. A "no" is fine, and so is "only the
fixes". The fork carries on either way.

Thank you for the client,
[OWNER: name]

(Word count about 330.)

---

## Part 2. PR descriptions

All are cut from `origin/master`. Titles and text use neutral names.

### PR 0 (one PR per fix). Fixes

[OWNER: include these only if you want to offer the fixes.]

Five separate PRs, about 245 lines with tests in total:
- **Animation bounds** (`vi`: csprite, hsprite). A broken animation no longer crashes. Needs the oracle case
  in the same change. [OWNER: the oracle is not on the fork's machine, so this one may wait.]
- **Login reply** (Station api). A refused login reads its reply before the close, so the reason shows.
- **Phantom controller** (input). No pad is reported when none is connected.
- **Windows test path** (texpack). A test path that works on Windows.
- **Power-bar reset** (net session). The plan marks this one "to confirm"; confirm before offering it.

Each is a no-op for anyone not hitting the bug. Each test fails first without its fix, then passes.
They do not change game behaviour beyond the bug.

### PR 1. Add a game-event observer and an overlay layer

- Adds a listener the game calls around each server event and at a few play steps, and a slot in the renderer
  for a layer drawn over the finished frame (input offered to it first).
- About 45 call lines in `play.rs` and `viewer.rs`, plus one new file.
- No-op by default: with no listener and an empty slot nothing is called, drawn or allocated.
- Tested with a recorder: a scripted fight is seen in order at 30 Hz and 165 Hz. Each call removed in turn fails
  a test. The overlay survives a lost graphics device.
- Builds for `wasm32` and the three desktop systems. Upstream's tests pass.
- Does not add Lua, a mod system or a dependency. Does not change any line's order or formatting.

### PR 2. Add read-only window state, HUD-part visibility and a map overlay

- Adds read-only accessors for the Bank, Dialog, Items, Loot, Merchant, Quest log, Spellbook, Start menu and
  Trainer windows. Adds one shared question, "is this HUD part shown?", used at both the draw and the click. Adds
  a map overlay slot.
- About 95 lines. [OWNER: the plan adds two lines the planned bag, spellbook and journal mod needs. That mod is
  built only after you approve its spec. Include them or not.]
- No-op by default: in the stock game Hide UI and its options answer the question as before, and the map
  overlay is empty.
- Tested: a same-picture test for the HUD, one test per part hidden, and one test per accessor so none is
  unused.
- Does not let anything act on a window. Does not change what the player sees.

### PR 3. Declare option, settings and Controls rows once

- Refactors `options.rs`, `settings.rs` and `bindpage.rs` so each row is declared once, instead of in six
  `pages()` lists plus more.
- About 43 lines changed in your files. Ids 1000 and up are kept for other builds.
- No-op: the saved settings file is byte-identical.
- Tested: your page walk tests pass unchanged, a byte-identity test on the saved file, and a planted duplicate
  id fails.
- Does not add options. Does not change any default.

### PR 4. Add an opt-in mod host

[OWNER: send only if Winfro asks for it.]

- Adds the Lua runtime, the `eqoa/1` API, the Mods & Addons window, the docs and a frozen record of the API
  with a guard test that its names only grow.
- Mostly new files. In your files: about 25 lines plus Cargo.lock. New dependencies are `mlua` and three
  dear-imgui crates.
- No mod is bundled and nothing runs until a player enables one. A player with none sees no change.
- Tested with a zero-addon gate for the measured cost with no mod, the docs against the running client, and the
  guard test. Adds its section to `UPDATE-NOTES.md`.
- Windows only for now. The map finds secure file operations unimplemented on Linux and macOS (`sandbox.rs`),
  and sound uses Windows only. File features fail closed there. [OWNER: say so openly, or fix it first.]
- Does not include any mod, and does not let mods act on their own unless you choose it: one switch, not an API
  change. [OWNER: how much to allow in vanilla.]

### PR 5. FrontiersForge compatibility layer

[OWNER: only if asked, and only after the Forge project's licence is checked. Not checked yet.]

- Adds the adapters that let unchanged FrontiersForge addon packages run on the host.
- New files only. No-op unless a mod uses it.
- Tested with the full suite for an unchanged Forge package. Does not bundle any Forge addon.

---

## Open markers

- [OWNER] licence and scope: AGPL contribution, which PRs, no extra conditions (plan section 2 and question 1).
- [OWNER] fixes: offered or not (question 5).
- [OWNER] how much to offer: seams only, or the host too (question 4).
- [OWNER] name and sign-off; the two-line bag mod hook in PR 2; the Linux and macOS statement in PR 4; limits
  for mods in vanilla (question 7).
