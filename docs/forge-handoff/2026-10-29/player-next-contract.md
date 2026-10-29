# FrontiersForge Native Player Adapter Contract (Read-Only Prep)

**Status:** Static analysis & read-only architectural specification.
**Compilation/Execution State:** UNCOMPILED / UNRUN (strictly static analysis; compiler slot released).
**Baseline Commit:** `9afa4c2` (stdlib checkpoint accepted and verified).

---

## 1. Verified API Mapping & Primary Source Line References

Analysis verified against primary sources:
- Upstream FrontiersForge Lua: `scripts/modules/frontiers_forge/player.lua` (lines 1–253) and `util.lua` (lines 384–386, 570–600).
- Guest EE Memory Layout & Physics: `crates/vi/src/walk.rs` (`WalkPhysics` lines 40–74, `WalkControl` lines 80–138).
- Native Engine State: `crates/eqoa/src/play.rs`, `crates/net/src/dump.rs` (lines 15–32, 177–185, 319–337), and `crates/net/src/session.rs` (lines 1160–1172).

| Pinned API | FF Lua Offset & Type (`player.lua`) | Native Rust Source Field & Type | Verified Formula / Value Semantics |
| :--- | :--- | :--- | :--- |
| `GetName` | `player_offset + 0x04` (`char*`, narrow 24) (L11-14) | `Play::player.name: String` (`net::session::Player`, L222; `net::dump::Player`, L17) | Player name string (capped to 24 chars null-terminated). Return empty or `nil` if player not ready. |
| `GetLevel` | `player_offset + 0x24` (`uint32_t`) (L16-18) | `Play::dump.player.level: i64` (`net::dump::Player:20`), updated via `Event::LevelExp` (`play.rs:6451`, `net::session.rs:1161`) | Level as integer. Unavailable when `level <= 0`. |
| `GetExp` | `player_offset + 0x28` (`uint32_t`) (L20-22) | `Play::dump.player.exp: i64` (`net::dump::Player:21`), updated via `Event::LevelExp` (`play.rs:6452`) | Current experience points as integer. |
| `GetExpDebt` | `player_offset + 0x2C` (`uint32_t`) (L24-26) | `Play::dump.player.p2c: i64` (`net::dump::Player:22`), updated via `Event::PlayerP2C` (`net::session.rs:1170`) | Experience debt as integer (`p2c` matches EE offset `0x2C`). |
| `GetCurrentPwr` | `player_offset + 0x88` (`uint32_t`) (L96-98) | `Play::stats.stat[9]: i32` (`crates/net/src/dump.rs:182`) | Current power stat. 0 if unavailable or `max_pwr <= 0`. |
| `GetMaxPwr` | `player_offset + 0x8C` (`uint32_t`) (L100-102) | `Play::stats.stat[10]: i32` (`crates/net/src/dump.rs:182`) | Maximum power stat. 0 if unavailable or nonpositive. |
| `GetBasePwr` | `player_offset + 0x100` (`uint32_t`) (L104-106) | `Play::stats.base[10]: i32` (`crates/net/src/dump.rs:184`) | Base power stat (before buffs/gear). |
| `GetCoordinates` | `0x1FB65B0` (`float[3]` `{x, y, z}`) (L160-169) | `walker.cam.walk.phys.position: Vect3<Ee>` (`crates/vi/src/walk.rs:44`, offset `0xB0` of `WalkControl`) | Returns `{ x = pos.x, y = pos.y, z = pos.z }`. $X, Z$ are horizontal; $Y$ is elevation. |
| `GetVelocity` | `walk_control_offset + 0xC8 = 0x1FB65C8` (`float[3]`) (L178-181) | `walker.cam.walk.phys.velocity: Vect3<Ee>` (`crates/vi/src/walk.rs:47`, offset `0xC8` of `WalkControl`) | Returns `{ x = vel.x, y = vel.y, z = vel.z }`. |
| `GetSpeed` | Derived in Lua: $\sqrt{v_x^2 + v_y^2 + v_z^2}$ (L187-190) | Derived from `phys.velocity` | Full 3D Euclidean velocity magnitude. |
| `GetHorizontalSpeed` | Derived in Lua: $\sqrt{v_x^2 + v_z^2}$ (L194-197) | Derived from `phys.velocity` ($X, Z$ plane) | Horizontal plane Euclidean speed: $\sqrt{v_x^2 + v_z^2}$. |
| `GetHeadingRadians` | `walk_control_offset + 0xBC = 0x1FB65BC` (`float`) (L202-204) | `walker.cam.walk.phys.angles.x: F<Ee>` (`crates/vi/src/walk.rs:46`, offset `0xBC` of `WalkControl`) | Facing angle in radians wrapped to $[0, 2\pi)$. 0 is North, $\pi/2$ is West, $3\pi/2$ is East. |
| `GetHeadingDegrees` | Derived in Lua: `NormalizeDegrees(RadiansToDegrees(rad))` (L208-210) | Derived: `(rad * (180.0 / PI)).rem_euclid(360.0)` | Facing angle in degrees in $[0, 360)$. 0 North, 90 West, 180 South, 270 East. |
| `GetEntityId` | `0x1FB6928` (`uint32_t`) (L214-216) | `Play::session.self_id: Option<u64>` / `Play::self_id32() -> Option<u32>` (`play.rs:61`) | Local player entity ID (`u32`). Returns 0 or `nil` if not logged in. |
| `GetTargetEntityId` | `0x1FBB870` (`uint32_t`) (L218-221) | `Play::target: u32` (`play.rs:194-195`) | Current target entity ID. If no target selected, `Play::target` is `u32::MAX`; PS2 memory / FF returns `0` (or `u32::MAX`). *(See unresolved section below)*. |

---

## 2. Incomplete / Unresolved Items

1. **Target Sentinel Value Conversion:**
   - In PS2 memory (`0x1FBB870`), when no entity is targeted, the value is typically `0` (or `0xFFFFFFFF`). In native Rust `Play::target`, `u32::MAX` represents no selection.
   - *Status:* Unresolved whether FrontiersForge scripts expect `0` or `nil` or `0xFFFFFFFF` when no target is held. Needs confirmation against Lua caller scripts (e.g., target frames or combat addons) before implementing.

2. **Snapshot Ownership & Walker Coupling:**
   - `Viewer` holds `walker: Option<Walker>` and `play: Option<Play>` separately (`crates/eqoa/src/viewer.rs:1820`).
   - `Play::forge_snapshot(&self)` currently does not receive `&Walker`.
   - To provide `position`, `velocity`, and `heading` to `Snapshot`:
     - Option A: Pass `Option<&Walker>` into `Play::forge_snapshot(&self, walker: Option<&Walker>)`.
     - Option B: Cache `last_walk_physics` inside `Play` during `Play::frame` or `Play::hud`.
   - *Status:* Incomplete design. Option A is cleaner and avoids duplicate state, but changes `forge_snapshot` signature.

3. **`Snapshot` Copy + `Rc<Cell<Snapshot>>` vs Non-Copy:**
   - Currently `Snapshot` is `#[derive(Clone, Copy, Default)]` stored in `Rc<Cell<Snapshot>>`.
   - If player name is included as `[u8; 24]` + `len: u8` and numeric values are stored directly, `Snapshot` easily remains `Copy`.
   - If `String` or `Arc<str>` were used, `Snapshot` would lose `Copy`, forcing `Rc<RefCell<Snapshot>>` across `Host`, `BaseHost`, and `Manager`.
   - *Recommendation:* Keep `Snapshot` `Copy` by storing player name as fixed array `[u8; 24]` and length `u8`.

4. **Stats vs Session Freshness during Zoning/Warping:**
   - On world change or teleport (`Event::Teleport` / `0x00B2`), `Play` reinitializes or warps the walker. Power stats from `net::dump::Stats` persist until channel `0x42` updates.
   - *Status:* Need to ensure availability flag forces power to 0 if HUD/player is loading or disconnected.

---

## 3. Proposed Offline Fixture Test Oracles (Unrun)

The following test oracles are proposed for future verification (marked UNRUN):

1. **Player Stat Receipt & Power Zero-Handling:**
   - Fixture provides `available = true`, `stat[9] = 250`, `stat[10] = 500`, `base[10] = 400`. Verify `GetCurrentPwr() == 250`, `GetMaxPwr() == 500`, `GetBasePwr() == 400`.
   - Fixture provides `max_pwr = 0` (or `available = false`). Verify `GetCurrentPwr() == 0`, `GetMaxPwr() == 0`.

2. **Level, Exp, and Exp Debt Reflection:**
   - Fixture with `level = 15`, `exp = 1822720`, `p2c = 4500`. Verify `GetLevel() == 15`, `GetExp() == 1822720`, `GetExpDebt() == 4500`.

3. **Coordinates 3D Vector Structure:**
   - Fixture with position `(1234.5, -67.25, 8910.125)`. Verify `GetCoordinates()` returns Lua table `{ x = 1234.5, y = -67.25, z = 8910.125 }`.

4. **Horizontal Speed vs Full 3D Speed:**
   - Fixture with velocity `(3.0, 4.0, 4.0)`.
   - Verify `GetHorizontalSpeed()` returns $\sqrt{3^2 + 4^2} = 5.0$.
   - Verify `GetSpeed()` returns $\sqrt{3^2 + 4^2 + 4^2} = \sqrt{41} \approx 6.4031$.

5. **Heading Radians to Normalized Degrees:**
   - Fixture with heading `0.0` radians (North) -> degrees `0.0`.
   - Fixture with heading $\pi / 2$ radians (West) -> degrees `90.0`.
   - Fixture with heading $\pi$ radians (South) -> degrees `180.0`.
   - Fixture with heading $3\pi / 2$ radians (East) -> degrees `270.0`.
   - Fixture with negative heading $-\pi / 2$ -> degrees `270.0`.

6. **Target Sentinel & Disconnected Player:**
   - When `self_id` is none or `in_game = false`, verify `GetEntityId() == 0` (or `nil`).
   - When `target == u32::MAX` (no target), verify `GetTargetEntityId()` returns agreed sentinel (`0`).
