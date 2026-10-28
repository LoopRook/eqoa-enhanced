# Native Client and EQOA Parser Integration

Status: reference proposal, October 28, 2026. Another agent now owns parser
implementation and integration. This chat maintains parser documentation only
and prioritizes Forge map parity immediately; parser completion is not a dependency.

## MVP Boundary and Priority

Repair the existing parser rather than overhaul its UI or analysis engine. The
sections below describe the longer-term integration direction; they are not all
prerequisites for returning to map work.

The MVP consists of:

1. Consistent launching from the desktop launcher, batch script, and game menu;
   explicit startup errors; correct package invocation; no user-specific paths.
2. A native-mode, non-admin parser build and a documented one-folder Windows
   package that an ordinary player can unzip and start without installing Python
   or configuring PCSX2/PINE. No automatic installer or download manager required.
3. The existing meter receiving correctly serialized native combat events, with
   player/target names, honest unknown ability labels, and accurate waiting versus
   receiving status. Keep one native-client session in scope initially.
4. A small session/roster message for player identity and group names/IDs using
   the native client's existing state, if it fits the receiver repair. Roster
   display is not a prerequisite for first verified damage totals. Native code
   already has group health state and positions; only send what is needed.
5. An end-to-end test through the current engine/UI and a real in-game fight
   checking totals and saved history. Distinguish verified features from unknown
   pet-credit or healing behavior on the tested server.

These checks describe the parser MVP for the owning agent. Forge world-map/minimap
parity proceeds independently in this chat. Defer a parser UI
redesign, general mod framework, automatic installation/update management, full
versioned-session rollout, multibox aggregation, complete group DPS, and advanced
spell attribution unless a confirmed MVP blocker requires a particular part.

Group membership is available directly from `group.rs`; complete group combat
events are not established. The parser README documents that Sandstorm sends
hits involving the local player/pet rather than every groupmate's fights. Native
integration cannot manufacture events the server does not send. Validate server
coverage before promising group DPS; sharing multiple clients' feeds is separate
future work.

## Intent and Decisions

Keep [LoopRook/EQOA-Parser](https://github.com/LoopRook/EQOA-Parser) as an external
application. The user approved that direction and wants better integration with
the native Rust client's data. Preserve the parser's existing PCSX2 workflows.

Two approaches were considered: integrate the existing external meter, or rebuild
it as an in-game addon. External integration retains the existing meter, history,
and analysis without rebuilding its UI and engine. In-game rendering would be a
separate feature. The selected approach is external integration.

The client and launcher must call it **Combat Parser** under a **Companion Tools**
category. Built-in HUD settings and future executable game addons are distinct
capabilities. Remove the parser's pretend mod manifest and auto-generated manifest
template after replacing their launch behavior; preserve unrelated user mods.

## Scope and Order

1. The parser's owning agent may use the bounded MVP above, preserving PCSX2/replay
   compatibility; implementation decisions belong to that agent and the user.
2. This chat addresses Forge map parity independently, following `MAP-PARITY.md`.
3. The broader companion architecture below remains a reference proposal, not an
   instruction for this chat to implement parser or integration changes.

The transport and rollout details below are the implementer's proposals, not
additional user requirements. Windows is the initial acceptance platform because
the existing launcher and reported workflow are Windows-based. Native game changes
must continue to compile for supported desktop targets and isolate browser code.

## Discovery, Launch, and Lifecycle

Implement the canonical launcher in the Rust client's companion module, also
exposed through an `eqoa companion parser` command before game/disc startup. The
desktop launcher and batch script call this command; the in-game menu calls the
same module. This avoids requiring Python just to launch a packaged companion.
Resolve the game installation from the executable/script
location, independently of the current directory.

Prefer a packaged parser under `companions/eqoa-parser/`. Allow an explicit parser
location setting. In developer checkouts, support a sibling `EQOA Parser` checkout
and an explicitly selected Python interpreter. Never depend on a particular user
name or Python installation directory. Validate required files before spawning.
Source launches use `python -m eqoa_parser --client`; packaged launches use the
parser executable with `--client`.

Keep launch diagnostics in the user's configuration/log directory. A detached
process must redirect output to a log and report early exit, missing dependencies,
missing files, and port conflicts in the launcher or in-game notice. Avoid console
flashes. Launch work and startup checks must not block the game thread.

Enforce a single native-mode parser instance with a per-user instance lock and an
activation mechanism; launching again focuses the existing meter. Show process
state separately from data connection state. Closing the game leaves the parser
open to finish/persist its fight and show disconnected status. Closing the parser
must not affect gameplay. Builder may choose the lock/activation implementation.

## Native Telemetry Contract

First preserve localhost UDP at `127.0.0.1:28015` to repair the existing path without
a transport migration. It remains best effort: count malformed messages, missing
sequence numbers, and send/receive errors; display degraded data rather than
claiming complete capture. All game sends remain nonblocking. Reliable transport
can replace this later if loss measurements justify it.

Define schema v1 envelopes containing schema version, message type, a random
process-instance ID, character-session ID, a per-session sequence number, and event time. Send a session
snapshot/heartbeat once per second and after identity changes, plus combat events
as they occur. Use structured JSON serialization. Keep the legacy flat event
decoder temporarily for existing clients, visibly identifying its limited mode.

Snapshots include server/world identity, player ID/name, pet ID/name when known,
group IDs/names, and selected target identity. Include only decoded runtime data;
never credentials, tokens, or machine identifiers. Each client window has its own
process instance, allowing the meter to distinguish multiple games. Login creates
a new character-session ID; logout, disconnect, character change, or server switch
ends that session and closes its fight. A reconnect creates a fresh character
session even in the same process. Scope entity IDs to the server and character
session so reused IDs cannot carry identity or totals across logins.

Combat events include source and target entity IDs, known names, amount, observed
event type, and ability ID/name only when the event establishes them. Unknown
abilities are null, not `melee`. Display damage/healing as supported by the decoded
message; do not infer a specific spell from the most recent cast without evidence.
Cast, death, XP, and resource updates may be separate typed events once their
existing native hooks are verified; missing capabilities must be reported.

The initial meter counts damage involving the selected session's player/pet.
Self-damage is incoming damage. Unrelated encounters do not extend its fights.
Multiple sessions remain separate by default. Group roster metadata does not prove
complete group DPS; do not aggregate group or multibox feeds until duplicate-event
ownership is tested. Preserve raw IDs so name changes cannot split actor identity.
If a server credits pet damage to its owner without a proven discriminator, show
combined player/pet damage with an explicit attribution warning. Keep the split
unavailable for that capability; never guess it or block the entire meter. Live
acceptance can establish combined totals while leaving pet separation explicitly
unverified.

The parser displays Waiting, Receiving, Idle, Client Lost, Unsupported Schema, or
Degraded as appropriate. Receiving means a valid heartbeat/event was received,
not merely that a UDP socket bound successfully. After five seconds without a
heartbeat, report Client Lost; use a monotonic local clock for timeout decisions.

## Packaging and Compatibility

Provide a windowed native-client parser build without a UAC/admin manifest. Keep
the separate PCSX2 packet-capture executable and its elevation behavior explicit.
Bundle runtime dependencies, assets, default configuration, and licenses. Native
mode stores writable state under the user's application-data directory, supporting
installation in a read-only program directory. Source-development runs may retain
their existing paths when explicitly selected.

An enhanced release includes the game, launcher/helper, companion build, and clear
startup instructions. Preserve third-party license obligations in distribution.
Building or staging artifacts is authorized; publishing a release is outside this
design's implementation scope.

## Acceptance Evidence

- Reproduce the current direct-file import failure, then verify every launch entry
  uses the shared supported path. Test missing parser/runtime, immediate crash,
  spaces in paths, arbitrary working directories, duplicate launch, and port use.
- Native schema fixtures cover identity updates, escaping, malformed/unsupported
  envelopes, idle heartbeats, stale sessions, and packet loss indicators.
- Send native-produced event fixtures over real localhost UDP into the parser
  engine and assert fight totals, attribution, idle closure, and saved history.
  Test the Rust serializer and Python decoder against the same fixtures.
- Validate player, pet, incoming damage, healing, unknown ability labels, and two
  separate client sessions without mixing or double-counting. Validate observed
  pet-credit semantics against live server behavior before claiming pet accuracy.
- Run parser regression tests, relevant Rust tests, and a Windows release build.
  Check the packaged native parser has no elevation requirement.
- Perform a real in-game fight and compare observed damage and saved meter totals;
  restart both applications and confirm recovery. Synthetic tests alone do not
  establish live integration success.

## Forge Follow-up

The map requirement includes both the world map and minimap: presentation, creature
search, journal, custom locations, shared tracking, and indoor height-aware views.
Pin the Forge source revision, inventory reference settings, and produce comparison
captures before claiming aesthetic parity. The current terrain-tile implementation
is a starting point, not an accepted equivalent.
