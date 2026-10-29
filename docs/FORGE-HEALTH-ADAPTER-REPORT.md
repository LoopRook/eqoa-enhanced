# Native Health & UI Adapter Slice (Stage 2E) — Implementation Report

**Date**: 2026-10-29
**Crate**: `crates/eqoa`
**Module**: `crates/eqoa/src/fork_ext/addons/health.rs`
**Activation**: Activated in `crates/eqoa/src/fork_ext/addons/mod.rs` via `#[cfg(not(target_arch = "wasm32"))] pub(crate) mod health;`.
**Scope Boundary**: Zero edits to manager, runtime, viewer, HUD, host, dependencies, policy, or maps. Zero mock data invented.

---

## 1. Interface & Architecture

The health adapter slice provides the first native bridge for `frontiers_forge.player` and `frontiers_forge.ui` required by `retro_health_hearts.lua`.

```rust
pub const MAX_OWNER_LABEL_BYTES: usize = 128;
pub const MAX_OWNERS_PER_VM: usize = 32;
pub const ANONYMOUS_OWNER: &str = "(anonymous)";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HealthSnapshot {
    pub available: bool,
    pub current_hp: i32,
    pub max_hp: i32,
}

impl HealthSnapshot {
    pub fn new(available: bool, current_hp: i32, max_hp: i32) -> Self;
    pub fn unavailable() -> Self;
    pub fn is_available(&self) -> bool;
    pub fn current_hp(&self) -> i32;
    pub fn max_hp(&self) -> i32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceToken(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenExhaustionError;

impl InstanceToken {
    pub fn next() -> Result<Self, TokenExhaustionError>;
}

#[derive(Default, Debug)]
pub struct HealthBarClaimsRegistry {
    claims: HashMap<InstanceToken, HashSet<String>>,
}

impl HealthBarClaimsRegistry {
    pub fn new() -> Self;
    pub fn is_hidden(&self) -> bool;
    pub fn release_instance(&mut self, token: InstanceToken);
    pub fn disable(&mut self, token: InstanceToken, owner: Option<&str>, hud_available: bool) -> mlua::Result<bool>;
    pub fn enable(&mut self, token: InstanceToken, owner: Option<&str>) -> bool;
    pub fn claims_count_for_instance(&self, token: InstanceToken) -> usize;
    pub fn total_claims_count(&self) -> usize;
}

pub fn install_health_adapter(
    host: &Host,
    token: InstanceToken,
    snapshot: Rc<Cell<HealthSnapshot>>,
    registry: Rc<RefCell<HealthBarClaimsRegistry>>,
) -> mlua::Result<()>;
```

---

## 2. Invariants & Implementation Details

1. **Player Health Semantics & Absent Data**:
   - `HealthSnapshot` is populated by the app layer (from `Stats.stat[7]` and `stat[8]`).
   - Nonpositive max HP (`max_hp <= 0`) is treated as unavailable, returning `0` / `0`. No synthetic fallback (such as `maxHP = 1`) is fabricated.
   - Unavailable snapshots return `0` / `0`.
   - Dead player state (`current_hp = 0, max_hp > 0`) is valid and returns `0` and `max_hp`.
   - The app integration gates `util.IsInGame` before unchanged hearts render to avoid division by zero.

2. **Checked Monotonic Token Allocation**:
   - `InstanceToken::next()` advances an `AtomicU64` with `compare_exchange_weak`.
   - Monotonic issuance is test-verified (`forge_health_adapter_token_monotonic`).
   - At `u64::MAX`, it returns `Err(TokenExhaustionError)` rather than wrapping, guaranteeing uniqueness. This wrap rejection is code-reviewed, but its exhaustion branch is not exercised by tests.
   - Tokens are never reused across VM reloads or reinstantiations.

3. **Multi-Owner HUD Claims & Isolation**:
   - `DisableHealthBar(owner)` returns `false` without recording a claim if HUD is unavailable. When available, returns `true` idempotently.
   - `EnableHealthBar(owner)` releases only the caller's specific token/owner claim and returns `true` unconditionally (native cleanup adaptation).
   - Omitted or `nil` owner parameter maps to a per-VM anonymous owner (`ANONYMOUS_OWNER = "(anonymous)"`).
   - Two VMs using the same owner string (e.g. `"retro_health_hearts"`) maintain isolated claims; one VM releasing its claim leaves the HUD hidden until the other releases as well.
   - Quotas enforce bounded memory: owner labels cannot exceed 128 bytes; a VM instance cannot exceed 32 distinct active owner labels.
   - `release_instance(token)` clears all claims registered by an instance upon exit, disable, reload, or fault.

4. **API Conservatism**:
   - `frontiers_forge.player` exports ONLY `GetCurrentHp` and `GetMaxHp`.
   - `frontiers_forge.ui` exports ONLY `DisableHealthBar` and `EnableHealthBar`.
   - All other functions (`GetName`, `GetLevel`, `DisablePowerBar`, etc.) remain absent (`nil`), preventing any fabricated or dummy module state.

---

## 3. Test Verification & Evidence

PowerShell PATH was prepended with `C:\Users\hocka\.cargo\bin`.

### A. RED Phase (Stubs)
Command:
```powershell
cargo test --locked --manifest-path Cargo.toml -p eqoa forge_health_adapter_ -- --nocapture
```
Output:
- 5 tests run, 5 failed, 0 passed in 0.01s:
  - `forge_health_adapter_player_hp_shape_and_snapshot_transitions` (FAILED)
  - `forge_health_adapter_ui_claims_multi_vm_and_anonymous` (FAILED)
  - `forge_health_adapter_owner_quota_and_label_bounds` (FAILED)
  - `forge_health_adapter_forced_release_on_instance_exit` (FAILED)
  - `forge_health_adapter_unsupported_apis_absent` (FAILED)

### B. GREEN Phase (Focused Implementation)
Command:
```powershell
cargo test --locked --manifest-path Cargo.toml -p eqoa forge_health_adapter_ -- --nocapture
```
Output:
- **6 passed; 0 failed; 0 ignored** in 0.01s.
- Verified:
  - `forge_health_adapter_token_monotonic`: ok
  - `forge_health_adapter_unsupported_apis_absent`: ok
  - `forge_health_adapter_player_hp_shape_and_snapshot_transitions`: ok
  - `forge_health_adapter_forced_release_on_instance_exit`: ok
  - `forge_health_adapter_ui_claims_multi_vm_and_anonymous`: ok
  - `forge_health_adapter_owner_quota_and_label_bounds`: ok

### C. Full Serial Addon Suite with Pinned Fixture
Command:
```powershell
$env:EQOA_FORGE_SPIKE = "C:\Users\hocka\Github Projects\EQOA Native Rust\run\forge-compat-reference\scripts\screen_grid.lua"
cargo test --locked --manifest-path Cargo.toml -p eqoa fork_ext::addons:: -- --nocapture --test-threads=1
```
Output:
- **67 passed; 0 failed; 0 ignored** in 10.80s.
- Pinned GPU fixture executed and captured: `target/forge-spike-grid.png`.
- Skips: Exactly 4 Windows symlink privilege skips (`os error 1314`) in discovery and package suites; **0 fixture skips**.

### D. Formatting & Diff Check
- `rustfmt --edition 2024` formatted touched files (`health.rs` and `addons/mod.rs`).

---

## 4. Status

- **Implementation**: COMPLETE and verified.
- **Compiler Slot**: RELEASED / FREE (now idling).
