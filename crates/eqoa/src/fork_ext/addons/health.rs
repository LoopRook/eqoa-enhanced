//! Native health and HUD suppression adapter slice (Stage 2E).
//!
//! Authoritative Contracts:
//! - `docs/FORGE-FIRST-NATIVE-ADAPTER-CONTRACTS.md`
//! - Upstream pinned reference: `scripts/modules/frontiers_forge/player.lua`, `ui.lua`,
//!   and `scripts/retro_health_hearts.lua`.
//!
//! Scope & Invariant Guarantees:
//! - Owns `HealthSnapshot` carrying `(available, current_hp, max_hp)`.
//! - Per-VM installer registers `frontiers_forge.player` (`GetCurrentHp`, `GetMaxHp`)
//!   and `frontiers_forge.ui` (`DisableHealthBar`, `EnableHealthBar`).
//! - Shared `HealthBarClaimsRegistry` manages multi-VM HUD health bar suppression claims
//!   keyed by `(InstanceToken, owner_label)`.
//! - Omission of owner label maps to a per-VM anonymous owner label.
//! - Bounds: max 128 bytes per owner label, max 32 distinct owner labels per VM instance.
//! - Nonpositive `max_hp` is treated as unavailable (returns 0 / 0; never fabricates maxHP=1).
//! - `DisableHealthBar` returns `false` and records no new claim while HUD is unavailable;
//!   returns `true` (idempotent) when available.
//! - `EnableHealthBar` releases only its own instance/owner claim and returns `true` even
//!   when unavailable (native cleanup adaptation).
//! - Unsupported APIs (`player.GetName`, `ui.DisablePowerBar`, etc.) remain absent.
//! - No real client fields read directly in this module (app adapter supplies snapshot).
//! - No mutation of user settings or maps.

use super::host::Host;
use mlua::Value;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

pub const MAX_OWNER_LABEL_BYTES: usize = 128;
pub const MAX_OWNERS_PER_VM: usize = 32;
pub const ANONYMOUS_OWNER: &str = "(anonymous)";

/// Owned snapshot of player health and HUD readiness for the active frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HealthSnapshot {
    pub available: bool,
    pub current_hp: i32,
    pub max_hp: i32,
}

impl HealthSnapshot {
    /// Creates a snapshot. If `max_hp <= 0`, availability is forced to false.
    pub fn new(available: bool, current_hp: i32, max_hp: i32) -> Self {
        if max_hp <= 0 {
            Self {
                available: false,
                current_hp: 0,
                max_hp: 0,
            }
        } else {
            Self {
                available,
                current_hp,
                max_hp,
            }
        }
    }

    /// Unavailable snapshot representation (logged out or unready).
    pub fn unavailable() -> Self {
        Self {
            available: false,
            current_hp: 0,
            max_hp: 0,
        }
    }

    /// True only when available and max_hp > 0.
    #[inline]
    pub fn is_available(&self) -> bool {
        self.available && self.max_hp > 0
    }

    /// Effective current HP: 0 when unavailable or max_hp <= 0.
    #[inline]
    pub fn current_hp(&self) -> i32 {
        if self.is_available() {
            self.current_hp
        } else {
            0
        }
    }

    /// Effective max HP: 0 when unavailable or max_hp <= 0.
    #[inline]
    pub fn max_hp(&self) -> i32 {
        if self.is_available() { self.max_hp } else { 0 }
    }
}

/// Unique instance token representing a single VM lifecycle.
///
/// Tokens are monotonically increasing and never reused across reloads or VM recreation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceToken(pub u64);

/// Error returned when the monotonic instance token counter is exhausted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenExhaustionError;

impl std::fmt::Display for TokenExhaustionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "instance token counter exhausted (u64::MAX reached)")
    }
}

impl std::error::Error for TokenExhaustionError {}

impl InstanceToken {
    /// Generates a strictly unique, monotonically increasing instance token.
    ///
    /// Rejects counter wrap-around at `u64::MAX` with a clear allocation error.
    pub fn next() -> Result<Self, TokenExhaustionError> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let mut current = NEXT_ID.load(Ordering::Relaxed);
        loop {
            if current == u64::MAX {
                return Err(TokenExhaustionError);
            }
            let next = current + 1;
            match NEXT_ID.compare_exchange_weak(current, next, Ordering::SeqCst, Ordering::Relaxed)
            {
                Ok(val) => return Ok(Self(val)),
                Err(actual) => current = actual,
            }
        }
    }
}

/// Shared registry tracking HUD health bar suppression claims across VM instances.
#[derive(Default, Debug)]
pub struct HealthBarClaimsRegistry {
    claims: HashMap<InstanceToken, HashSet<String>>,
}

impl HealthBarClaimsRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// True if any active VM holds at least one health bar suppression claim.
    pub fn is_hidden(&self) -> bool {
        self.claims.values().any(|set| !set.is_empty())
    }

    /// Releases all claims associated with a given VM instance (e.g. on disable, reload, eject, or fault).
    pub fn release_instance(&mut self, token: InstanceToken) {
        self.claims.remove(&token);
    }

    /// Records a health bar suppression claim.
    ///
    /// - Returns `Ok(false)` if `hud_available` is false without recording a claim.
    /// - Returns `Ok(true)` idempotently if `hud_available` is true.
    /// - Rejects oversized labels (> 128 bytes) or exceeding quota (> 32 distinct labels per VM).
    pub fn disable(
        &mut self,
        token: InstanceToken,
        owner: Option<&str>,
        hud_available: bool,
    ) -> mlua::Result<bool> {
        if !hud_available {
            return Ok(false);
        }

        let label = owner.unwrap_or(ANONYMOUS_OWNER);
        if label.len() > MAX_OWNER_LABEL_BYTES {
            return Err(mlua::Error::runtime(format!(
                "owner label exceeds {} bytes limit",
                MAX_OWNER_LABEL_BYTES
            )));
        }

        let set = self.claims.entry(token).or_default();
        if !set.contains(label) {
            if set.len() >= MAX_OWNERS_PER_VM {
                return Err(mlua::Error::runtime(format!(
                    "VM owner claims limit exceeded (max {})",
                    MAX_OWNERS_PER_VM
                )));
            }
            set.insert(label.to_string());
        }
        Ok(true)
    }

    /// Releases an owner claim for a specific VM instance.
    ///
    /// Only releases this token's own claim and returns `true` even if HUD is unavailable
    /// or no claim was previously held (native cleanup adaptation).
    pub fn enable(&mut self, token: InstanceToken, owner: Option<&str>) -> bool {
        let label = owner.unwrap_or(ANONYMOUS_OWNER);
        if let Some(set) = self.claims.get_mut(&token) {
            set.remove(label);
            if set.is_empty() {
                self.claims.remove(&token);
            }
        }
        true
    }

    /// Returns the number of distinct active claims for a given VM instance.
    pub fn claims_count_for_instance(&self, token: InstanceToken) -> usize {
        self.claims.get(&token).map_or(0, |set| set.len())
    }

    /// Returns the total number of active claims across all instances.
    pub fn total_claims_count(&self) -> usize {
        self.claims.values().map(|set| set.len()).sum()
    }
}

/// Installs the `frontiers_forge.player` and `frontiers_forge.ui` adapter modules for a VM.
pub fn install_health_adapter(
    host: &Host,
    token: InstanceToken,
    snapshot: Rc<Cell<HealthSnapshot>>,
    registry: Rc<RefCell<HealthBarClaimsRegistry>>,
) -> mlua::Result<()> {
    let lua = &host.lua;

    // 1. Module `frontiers_forge.player`
    let player = lua.create_table()?;
    let s = snapshot.clone();
    player.set(
        "GetCurrentHp",
        lua.create_function(move |_, ()| Ok(s.get().current_hp()))?,
    )?;
    let s = snapshot.clone();
    player.set(
        "GetMaxHp",
        lua.create_function(move |_, ()| Ok(s.get().max_hp()))?,
    )?;
    host.register_host_module("frontiers_forge.player", Value::Table(player))?;

    // 2. Module `frontiers_forge.ui`
    let ui = lua.create_table()?;
    let reg = registry.clone();
    let s = snapshot.clone();
    ui.set(
        "DisableHealthBar",
        lua.create_function(move |_, owner: Option<String>| {
            let hud_available = s.get().is_available();
            reg.borrow_mut()
                .disable(token, owner.as_deref(), hud_available)
        })?,
    )?;
    let reg = registry.clone();
    ui.set(
        "EnableHealthBar",
        lua.create_function(move |_, owner: Option<String>| {
            Ok(reg.borrow_mut().enable(token, owner.as_deref()))
        })?,
    )?;
    host.register_host_module("frontiers_forge.ui", Value::Table(ui))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::discovery::DiscoveredAddon;
    use std::path::PathBuf;

    fn test_host(name: &str) -> Host {
        let addon = DiscoveredAddon {
            id: name.to_string(),
            name: name.to_string(),
            entry: PathBuf::from("main.lua"),
            script_root: PathBuf::from("."),
            package_root: PathBuf::from("."),
            module_dirs: Vec::new(),
            resource_dirs: Vec::new(),
            api: None,
            requested_capabilities: Vec::new(),
            initially_enabled: true,
        };
        Host::for_addon(&addon).unwrap()
    }

    #[test]
    fn forge_health_adapter_player_hp_shape_and_snapshot_transitions() {
        let host = test_host("hp_shape_test");
        let token = InstanceToken::next().unwrap();
        let snapshot = Rc::new(Cell::new(HealthSnapshot::new(true, 450, 1000)));
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));

        install_health_adapter(&host, token, snapshot.clone(), registry).unwrap();

        let script = host
            .compile(
                r#"
            local player = require("frontiers_forge.player")
            local cur = player.GetCurrentHp()
            local max = player.GetMaxHp()
            assert(type(cur) == "number", "current HP must be number")
            assert(type(max) == "number", "max HP must be number")
            assert(cur == 450 and max == 1000, "HP values must match snapshot")
        "#,
            )
            .unwrap();
        host.run(&script).unwrap();

        // 1. Healing / damage transition
        snapshot.set(HealthSnapshot::new(true, 750, 1000));
        let check_updated = host
            .compile(
                r#"
            local player = require("frontiers_forge.player")
            assert(player.GetCurrentHp() == 750)
            assert(player.GetMaxHp() == 1000)
        "#,
            )
            .unwrap();
        host.run(&check_updated).unwrap();

        // 2. Dead player transition: current_hp = 0, max_hp > 0
        snapshot.set(HealthSnapshot::new(true, 0, 1000));
        let check_dead = host
            .compile(
                r#"
            local player = require("frontiers_forge.player")
            assert(player.GetCurrentHp() == 0, "dead player HP must be 0")
            assert(player.GetMaxHp() == 1000, "dead player max HP remains 1000")
        "#,
            )
            .unwrap();
        host.run(&check_dead).unwrap();

        // 3. Unavailable / logged out transition
        snapshot.set(HealthSnapshot::unavailable());
        let check_unavail = host
            .compile(
                r#"
            local player = require("frontiers_forge.player")
            assert(player.GetCurrentHp() == 0, "unavailable HP must be 0")
            assert(player.GetMaxHp() == 0, "unavailable max HP must be 0")
        "#,
            )
            .unwrap();
        host.run(&check_unavail).unwrap();

        // 4. Invalid nonpositive max: max_hp <= 0 means unavailable (never fabricate maxHP=1)
        snapshot.set(HealthSnapshot::new(true, 250, 0));
        let check_zero_max = host
            .compile(
                r#"
            local player = require("frontiers_forge.player")
            assert(player.GetCurrentHp() == 0)
            assert(player.GetMaxHp() == 0)
        "#,
            )
            .unwrap();
        host.run(&check_zero_max).unwrap();

        snapshot.set(HealthSnapshot::new(true, 250, -100));
        let check_neg_max = host
            .compile(
                r#"
            local player = require("frontiers_forge.player")
            assert(player.GetCurrentHp() == 0)
            assert(player.GetMaxHp() == 0)
        "#,
            )
            .unwrap();
        host.run(&check_neg_max).unwrap();
    }

    #[test]
    fn forge_health_adapter_ui_claims_multi_vm_and_anonymous() {
        let host_a = test_host("ui_claims_a");
        let host_b = test_host("ui_claims_b");
        let token_a = InstanceToken::next().unwrap();
        let token_b = InstanceToken::next().unwrap();
        assert_ne!(token_a, token_b, "VM tokens must not collide");

        let snapshot = Rc::new(Cell::new(HealthSnapshot::unavailable()));
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));

        install_health_adapter(&host_a, token_a, snapshot.clone(), registry.clone()).unwrap();
        install_health_adapter(&host_b, token_b, snapshot.clone(), registry.clone()).unwrap();

        // While HUD unavailable: DisableHealthBar returns false and records no claim
        let script_dis_unavail = host_a
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            local res = ui.DisableHealthBar("retro_health_hearts")
            assert(res == false, "DisableHealthBar must return false when HUD unavailable")
        "#,
            )
            .unwrap();
        host_a.run(&script_dis_unavail).unwrap();
        assert!(!registry.borrow().is_hidden());
        assert_eq!(registry.borrow().total_claims_count(), 0);

        // Snapshot becomes available
        snapshot.set(HealthSnapshot::new(true, 500, 1000));

        // VM A disables with owner "retro_health_hearts"
        let script_a_dis = host_a
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            local res = ui.DisableHealthBar("retro_health_hearts")
            assert(res == true, "DisableHealthBar must return true when available")
        "#,
            )
            .unwrap();
        host_a.run(&script_a_dis).unwrap();
        assert!(registry.borrow().is_hidden());
        assert_eq!(registry.borrow().claims_count_for_instance(token_a), 1);

        // Idempotent disable: VM A disables again with same owner
        host_a.run(&script_a_dis).unwrap();
        assert_eq!(registry.borrow().claims_count_for_instance(token_a), 1);

        // VM B disables with SAME owner label "retro_health_hearts"
        let script_b_dis = host_b
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            local res = ui.DisableHealthBar("retro_health_hearts")
            assert(res == true, "DisableHealthBar must return true for second VM")
        "#,
            )
            .unwrap();
        host_b.run(&script_b_dis).unwrap();
        assert_eq!(registry.borrow().claims_count_for_instance(token_b), 1);
        assert_eq!(registry.borrow().total_claims_count(), 2);

        // VM A enables: releases only VM A's claim
        let script_a_en = host_a
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            local res = ui.EnableHealthBar("retro_health_hearts")
            assert(res == true, "EnableHealthBar must return true")
        "#,
            )
            .unwrap();
        host_a.run(&script_a_en).unwrap();
        assert_eq!(registry.borrow().claims_count_for_instance(token_a), 0);
        assert_eq!(registry.borrow().claims_count_for_instance(token_b), 1);
        assert!(
            registry.borrow().is_hidden(),
            "HUD must remain hidden while VM B still holds its claim"
        );

        // VM A enables again: no-op release of others, returns true
        host_a.run(&script_a_en).unwrap();
        assert_eq!(registry.borrow().claims_count_for_instance(token_b), 1);
        assert!(registry.borrow().is_hidden());

        // Anonymous owner test: nil owner argument
        let script_a_anon = host_a
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            local res = ui.DisableHealthBar()
            assert(res == true)
        "#,
            )
            .unwrap();
        host_a.run(&script_a_anon).unwrap();
        assert_eq!(registry.borrow().claims_count_for_instance(token_a), 1);

        let script_b_anon = host_b
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            local res = ui.DisableHealthBar(nil)
            assert(res == true)
        "#,
            )
            .unwrap();
        host_b.run(&script_b_anon).unwrap();
        assert_eq!(registry.borrow().claims_count_for_instance(token_b), 2); // "retro_health_hearts" + anonymous

        // VM A enables anonymous
        let script_a_en_anon = host_a
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            local res = ui.EnableHealthBar()
            assert(res == true)
        "#,
            )
            .unwrap();
        host_a.run(&script_a_en_anon).unwrap();
        assert_eq!(registry.borrow().claims_count_for_instance(token_a), 0);
        assert_eq!(registry.borrow().claims_count_for_instance(token_b), 2);
    }

    #[test]
    fn forge_health_adapter_owner_quota_and_label_bounds() {
        let host = test_host("quota_test");
        let token = InstanceToken::next().unwrap();
        let snapshot = Rc::new(Cell::new(HealthSnapshot::new(true, 100, 100)));
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));

        install_health_adapter(&host, token, snapshot, registry).unwrap();

        // 1. Oversized label (> 128 bytes)
        let script_oversized = host
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            local long_label = string.rep("x", 129)
            ui.DisableHealthBar(long_label)
        "#,
            )
            .unwrap();
        let err = host.run(&script_oversized).unwrap_err();
        assert!(err.to_string().contains("exceeds 128 bytes limit"));

        // Exactly 128 bytes succeeds
        let script_exact_128 = host
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            local exact_label = string.rep("y", 128)
            assert(ui.DisableHealthBar(exact_label) == true)
        "#,
            )
            .unwrap();
        host.run(&script_exact_128).unwrap();

        // 2. Claims count quota (max 32 distinct labels per VM)
        let script_fill_quota = host
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            for i = 2, 32 do
                assert(ui.DisableHealthBar("owner_" .. i) == true)
            end
        "#,
            )
            .unwrap();
        host.run(&script_fill_quota).unwrap();

        // 33rd distinct claim errors
        let script_exceed_quota = host
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            ui.DisableHealthBar("overflow_owner")
        "#,
            )
            .unwrap();
        let err = host.run(&script_exceed_quota).unwrap_err();
        assert!(err.to_string().contains("VM owner claims limit exceeded"));

        // Idempotent disable of existing owner within quota still succeeds
        let script_reassert = host
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            assert(ui.DisableHealthBar("owner_2") == true)
        "#,
            )
            .unwrap();
        host.run(&script_reassert).unwrap();
    }

    #[test]
    fn forge_health_adapter_forced_release_on_instance_exit() {
        let host_a = test_host("forced_release_a");
        let host_b = test_host("forced_release_b");
        let token_a = InstanceToken::next().unwrap();
        let token_b = InstanceToken::next().unwrap();

        let snapshot = Rc::new(Cell::new(HealthSnapshot::new(true, 500, 500)));
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));

        install_health_adapter(&host_a, token_a, snapshot.clone(), registry.clone()).unwrap();
        install_health_adapter(&host_b, token_b, snapshot.clone(), registry.clone()).unwrap();

        let script_a = host_a
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            ui.DisableHealthBar("mod_a_1")
            ui.DisableHealthBar("mod_a_2")
        "#,
            )
            .unwrap();
        host_a.run(&script_a).unwrap();

        let script_b = host_b
            .compile(
                r#"
            local ui = require("frontiers_forge.ui")
            ui.DisableHealthBar("mod_b")
        "#,
            )
            .unwrap();
        host_b.run(&script_b).unwrap();

        assert_eq!(registry.borrow().total_claims_count(), 3);
        assert!(registry.borrow().is_hidden());

        // Simulate VM A exiting / crashing / reloading / failing
        registry.borrow_mut().release_instance(token_a);
        assert_eq!(registry.borrow().claims_count_for_instance(token_a), 0);
        assert_eq!(registry.borrow().claims_count_for_instance(token_b), 1);
        assert!(
            registry.borrow().is_hidden(),
            "VM B's claim must survive VM A's forced release"
        );

        // Simulate VM B exiting
        registry.borrow_mut().release_instance(token_b);
        assert_eq!(registry.borrow().total_claims_count(), 0);
        assert!(
            !registry.borrow().is_hidden(),
            "HUD must be visible when all instances released"
        );
    }

    #[test]
    fn forge_health_adapter_unsupported_apis_absent() {
        let host = test_host("unsupported_absent_test");
        let token = InstanceToken::next().unwrap();
        let snapshot = Rc::new(Cell::new(HealthSnapshot::new(true, 100, 100)));
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));

        install_health_adapter(&host, token, snapshot, registry).unwrap();

        let script = host
            .compile(
                r#"
            local player = require("frontiers_forge.player")
            local ui = require("frontiers_forge.ui")

            assert(type(player.GetCurrentHp) == "function")
            assert(type(player.GetMaxHp) == "function")
            assert(player.GetName == nil, "unsupported Player APIs must be absent")
            assert(player.GetLevel == nil, "unsupported Player APIs must be absent")
            assert(player.GetTotalStr == nil, "unsupported Player APIs must be absent")

            assert(type(ui.DisableHealthBar) == "function")
            assert(type(ui.EnableHealthBar) == "function")
            assert(ui.DisablePowerBar == nil, "unsupported UI APIs must be absent")
            assert(ui.EnablePowerBar == nil, "unsupported UI APIs must be absent")
            assert(ui.GetHiddenBy == nil, "unsupported UI APIs must be absent")
        "#,
            )
            .unwrap();
        host.run(&script).unwrap();
    }

    #[test]
    fn forge_health_adapter_token_monotonic() {
        let t1 = InstanceToken::next().unwrap();
        let t2 = InstanceToken::next().unwrap();
        assert!(t2.0 > t1.0, "tokens must be monotonically increasing");
    }
}
