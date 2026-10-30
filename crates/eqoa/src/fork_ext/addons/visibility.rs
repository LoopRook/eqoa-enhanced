//! Managed desired visibility is separate from the standalone effective registry.
use super::{
    health::{self, HealthBarClaimsRegistry, HealthSnapshot, InstanceToken},
    host::Host,
};
use mlua::Value;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

pub const ALL_UI_ELEMENTS: &[&str] = &[
    "health_bar",
    "power_bar",
    "main_exp_bar",
    "secondary_exp_bar",
    "compass_face",
    "compass_back",
    "target_nameplate",
    "group_member_panel",
    "group_compass_marks",
    "pet_panel",
    "chat_window",
    "hotbar_draw_compact",
    "hotbar_draw_expanded",
    "active_effects",
];

/// Shared registry tracking suppression claims across all HUD elements.
#[derive(Default, Debug)]
pub struct HudClaimsRegistry {
    // element_name -> (InstanceToken -> HashSet<owner_label>)
    claims: HashMap<String, HashMap<InstanceToken, HashSet<String>>>,
}

impl HudClaimsRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_hidden(&self, element: &str) -> bool {
        self.claims.get(element).map_or(false, |instances| {
            instances.values().any(|set| !set.is_empty())
        })
    }

    pub fn release_instance(&mut self, token: InstanceToken) {
        for instances in self.claims.values_mut() {
            instances.remove(&token);
        }
    }

    pub fn disable(&mut self, element: &str, token: InstanceToken, owner: &str) {
        self.claims
            .entry(element.to_string())
            .or_default()
            .entry(token)
            .or_default()
            .insert(owner.to_string());
    }

    #[allow(dead_code)]
    pub fn enable(&mut self, element: &str, token: InstanceToken, owner: &str) {
        if let Some(instances) = self.claims.get_mut(element) {
            if let Some(set) = instances.get_mut(&token) {
                set.remove(owner);
                if set.is_empty() {
                    instances.remove(&token);
                }
            }
        }
    }

    pub fn get_hidden_by(&self, element: &str) -> Vec<String> {
        let mut owners = Vec::new();
        if let Some(instances) = self.claims.get(element) {
            for set in instances.values() {
                for label in set {
                    if !owners.contains(label) {
                        owners.push(label.clone());
                    }
                }
            }
        }
        owners.sort();
        owners
    }

    #[allow(dead_code)]
    pub fn total_claims_count(&self) -> usize {
        self.claims
            .values()
            .map(|instances| instances.values().map(|set| set.len()).sum::<usize>())
            .sum()
    }
}

struct Intent {
    desired: HashMap<String, HashSet<String>>,
    active: bool,
    suspended: bool,
    token: InstanceToken,
    registry: Rc<RefCell<HealthBarClaimsRegistry>>,
    hud_claims: Rc<RefCell<HudClaimsRegistry>>,
}

impl Intent {
    fn project(&self, snapshot: HealthSnapshot) {
        let mut health_reg = self.registry.borrow_mut();
        let mut hud_reg = self.hud_claims.borrow_mut();
        health_reg.release_instance(self.token);
        hud_reg.release_instance(self.token);
        if self.active && !self.suspended && snapshot.is_available() {
            for (element, labels) in &self.desired {
                for label in labels {
                    hud_reg.disable(element, self.token, label);
                    if element == "health_bar" {
                        health_reg
                            .disable(self.token, Some(label), true)
                            .expect("bounded validated intent");
                    }
                }
            }
        }
    }
}

pub(crate) struct VisibilityLease {
    pub snapshot: Rc<Cell<HealthSnapshot>>,
    intent: Rc<RefCell<Intent>>,
}

impl VisibilityLease {
    pub fn new(
        registry: Rc<RefCell<HealthBarClaimsRegistry>>,
    ) -> Result<Self, super::health::TokenExhaustionError> {
        Self::with_hud_claims(registry, Rc::new(RefCell::new(HudClaimsRegistry::new())))
    }

    pub fn with_hud_claims(
        registry: Rc<RefCell<HealthBarClaimsRegistry>>,
        hud_claims: Rc<RefCell<HudClaimsRegistry>>,
    ) -> Result<Self, super::health::TokenExhaustionError> {
        Ok(Self {
            snapshot: Rc::new(Cell::new(HealthSnapshot::unavailable())),
            intent: Rc::new(RefCell::new(Intent {
                desired: HashMap::new(),
                active: false,
                suspended: false,
                token: InstanceToken::next()?,
                registry,
                hud_claims,
            })),
        })
    }

    pub fn update(&self, health: HealthSnapshot, active: bool) {
        self.snapshot.set(health);
        let mut intent = self.intent.borrow_mut();
        intent.active = active;
        intent.suspended = false;
        intent.project(health);
    }

    pub fn suspend(&self) {
        let mut i = self.intent.borrow_mut();
        i.suspended = true;
        i.project(self.snapshot.get());
    }

    pub fn clear(&self) {
        let mut i = self.intent.borrow_mut();
        i.active = false;
        i.desired.clear();
        i.project(self.snapshot.get());
    }
}

impl Drop for VisibilityLease {
    fn drop(&mut self) {
        self.clear();
    }
}

fn disable_element(
    intent: &mut Intent,
    snapshot: HealthSnapshot,
    element: &str,
    owner: Option<String>,
) -> mlua::Result<bool> {
    let label = owner.as_deref().unwrap_or(health::ANONYMOUS_OWNER);
    if label.len() > health::MAX_OWNER_LABEL_BYTES {
        return Err(mlua::Error::runtime("owner label exceeds 128 bytes limit"));
    }
    if !intent.active {
        return Ok(false);
    }
    let set = intent.desired.entry(element.to_string()).or_default();
    if !set.contains(label) && set.len() >= health::MAX_OWNERS_PER_VM {
        return Err(mlua::Error::runtime(
            "VM owner claims limit exceeded (max 32)",
        ));
    }
    set.insert(label.to_owned());
    intent.project(snapshot);
    Ok(snapshot.is_available() && !intent.suspended)
}

fn enable_element(
    intent: &mut Intent,
    snapshot: HealthSnapshot,
    element: &str,
    owner: Option<String>,
) -> bool {
    let label = owner.as_deref().unwrap_or(health::ANONYMOUS_OWNER);
    if let Some(set) = intent.desired.get_mut(element) {
        set.remove(label);
    }
    intent.project(snapshot);
    true
}

pub(crate) fn install_managed_health_adapter(
    host: &Host,
    lease: &VisibilityLease,
) -> mlua::Result<()> {
    let token = lease.intent.borrow().token;
    let registry = lease.intent.borrow().registry.clone();
    if !host.has_host_module("frontiers_forge.player") {
        health::install_health_adapter(host, token, lease.snapshot.clone(), registry)?;
    }

    let ui = host.lua.create_table()?;
    let intent = lease.intent.clone();
    let snapshot = lease.snapshot.clone();

    // 1. Single element functions
    macro_rules! bind_element {
        ($disable_name:literal, $enable_name:literal, $elem_key:literal) => {{
            let intent_d = intent.clone();
            let snapshot_d = snapshot.clone();
            ui.set(
                $disable_name,
                host.lua.create_function(move |_, owner: Option<String>| {
                    let mut i = intent_d.borrow_mut();
                    let s = snapshot_d.get();
                    disable_element(&mut i, s, $elem_key, owner)
                })?,
            )?;
            let intent_e = intent.clone();
            let snapshot_e = snapshot.clone();
            ui.set(
                $enable_name,
                host.lua.create_function(move |_, owner: Option<String>| {
                    let mut i = intent_e.borrow_mut();
                    let s = snapshot_e.get();
                    Ok(enable_element(&mut i, s, $elem_key, owner))
                })?,
            )?;
        }};
    }

    bind_element!("DisableHealthBar", "EnableHealthBar", "health_bar");
    bind_element!("DisablePowerBar", "EnablePowerBar", "power_bar");
    bind_element!("DisableMainExpBar", "EnableMainExpBar", "main_exp_bar");
    bind_element!(
        "DisableSecondaryExpBar",
        "EnableSecondaryExpBar",
        "secondary_exp_bar"
    );
    bind_element!(
        "DisableTargetNameplate",
        "EnableTargetNameplate",
        "target_nameplate"
    );
    bind_element!(
        "DisableGroupMemberPanel",
        "EnableGroupMemberPanel",
        "group_member_panel"
    );
    bind_element!(
        "DisableGroupCompassMarkers",
        "EnableGroupCompassMarkers",
        "group_compass_marks"
    );
    bind_element!("DisablePetPanel", "EnablePetPanel", "pet_panel");
    bind_element!(
        "DisableActiveEffectsDisplay",
        "EnableActiveEffectsDisplay",
        "active_effects"
    );
    bind_element!("DisableChatWindow", "EnableChatWindow", "chat_window");

    // 2. Compound element functions
    // Experience bars
    let intent_d = intent.clone();
    let snapshot_d = snapshot.clone();
    ui.set(
        "DisableExperienceBars",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent_d.borrow_mut();
            let s = snapshot_d.get();
            let m = disable_element(&mut i, s, "main_exp_bar", owner.clone())?;
            let sec = disable_element(&mut i, s, "secondary_exp_bar", owner)?;
            Ok(m && sec)
        })?,
    )?;
    let intent_e = intent.clone();
    let snapshot_e = snapshot.clone();
    ui.set(
        "EnableExperienceBars",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent_e.borrow_mut();
            let s = snapshot_e.get();
            let m = enable_element(&mut i, s, "main_exp_bar", owner.clone());
            let sec = enable_element(&mut i, s, "secondary_exp_bar", owner);
            Ok(m && sec)
        })?,
    )?;

    // Compass
    let intent_d = intent.clone();
    let snapshot_d = snapshot.clone();
    ui.set(
        "DisableCompass",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent_d.borrow_mut();
            let s = snapshot_d.get();
            let face = disable_element(&mut i, s, "compass_face", owner.clone())?;
            let back = disable_element(&mut i, s, "compass_back", owner)?;
            Ok(face && back)
        })?,
    )?;
    let intent_e = intent.clone();
    let snapshot_e = snapshot.clone();
    ui.set(
        "EnableCompass",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent_e.borrow_mut();
            let s = snapshot_e.get();
            let face = enable_element(&mut i, s, "compass_face", owner.clone());
            let back = enable_element(&mut i, s, "compass_back", owner);
            Ok(face && back)
        })?,
    )?;

    // Ability Bar
    let intent_d = intent.clone();
    let snapshot_d = snapshot.clone();
    ui.set(
        "DisableAbilityBar",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent_d.borrow_mut();
            let s = snapshot_d.get();
            let c = disable_element(&mut i, s, "hotbar_draw_compact", owner.clone())?;
            let e = disable_element(&mut i, s, "hotbar_draw_expanded", owner)?;
            Ok(c && e)
        })?,
    )?;
    let intent_e = intent.clone();
    let snapshot_e = snapshot.clone();
    ui.set(
        "EnableAbilityBar",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent_e.borrow_mut();
            let s = snapshot_e.get();
            let c = enable_element(&mut i, s, "hotbar_draw_compact", owner.clone());
            let e = enable_element(&mut i, s, "hotbar_draw_expanded", owner);
            Ok(c && e)
        })?,
    )?;

    // Group Display
    let intent_d = intent.clone();
    let snapshot_d = snapshot.clone();
    ui.set(
        "DisableGroupDisplay",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent_d.borrow_mut();
            let s = snapshot_d.get();
            let p = disable_element(&mut i, s, "group_member_panel", owner.clone())?;
            let m = disable_element(&mut i, s, "group_compass_marks", owner)?;
            Ok(p && m)
        })?,
    )?;
    let intent_e = intent.clone();
    let snapshot_e = snapshot.clone();
    ui.set(
        "EnableGroupDisplay",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent_e.borrow_mut();
            let s = snapshot_e.get();
            let p = enable_element(&mut i, s, "group_member_panel", owner.clone());
            let m = enable_element(&mut i, s, "group_compass_marks", owner);
            Ok(p && m)
        })?,
    )?;

    // Global UI
    let intent_d = intent.clone();
    let snapshot_d = snapshot.clone();
    ui.set(
        "DisableUI",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent_d.borrow_mut();
            let s = snapshot_d.get();
            let mut all = true;
            for el in ALL_UI_ELEMENTS {
                all = disable_element(&mut i, s, el, owner.clone())? && all;
            }
            Ok(all)
        })?,
    )?;
    let intent_e = intent.clone();
    let snapshot_e = snapshot.clone();
    ui.set(
        "EnableUI",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent_e.borrow_mut();
            let s = snapshot_e.get();
            let mut all = true;
            for el in ALL_UI_ELEMENTS {
                all = enable_element(&mut i, s, el, owner.clone()) && all;
            }
            Ok(all)
        })?,
    )?;

    // 3. Query function
    let hud_claims_q = lease.intent.borrow().hud_claims.clone();
    ui.set(
        "GetHiddenBy",
        host.lua.create_function(move |lua, element_name: String| {
            let owners = hud_claims_q.borrow().get_hidden_by(&element_name);
            let tbl = lua.create_table()?;
            for (idx, owner) in owners.into_iter().enumerate() {
                tbl.set(idx + 1, owner)?;
            }
            Ok(tbl)
        })?,
    )?;

    // 4. Disposition and Class Icon helpers
    ui.set(
        "GetDispositionFaceTexId",
        host.lua.create_function(|_, val: Option<Value>| {
            let disp_opt: Option<i64> = match val {
                Some(Value::Integer(i)) => Some(i),
                Some(Value::Number(n)) => Some(n as i64),
                Some(Value::String(s)) => s
                    .to_str()
                    .ok()
                    .and_then(|str_val| str_val.parse::<i64>().ok()),
                _ => None,
            };
            match disp_opt {
                Some(d) if d <= 1 => Ok(Some(0x73u32)),
                Some(d) if (4..=6).contains(&d) => Ok(Some(0x75u32)),
                Some(_) => Ok(Some(0x74u32)),
                None => Ok(None),
            }
        })?,
    )?;

    ui.set(
        "GetClassIconTexId",
        host.lua.create_function(|_, val: Option<Value>| {
            let class_opt: Option<u32> = match val {
                Some(Value::Integer(i)) if i >= 0 => Some(i as u32),
                Some(Value::Number(n)) if n >= 0.0 => Some(n as u32),
                Some(Value::String(s)) => s
                    .to_str()
                    .ok()
                    .and_then(|str_val| str_val.parse::<u32>().ok()),
                _ => None,
            };
            const CLASS_ICON_TEX_IDS: [u32; 15] = [
                0x17, // 0: Warrior
                0x13, // 1: Ranger
                0x12, // 2: Paladin
                0x15, // 3: Shadowknight
                0x10, // 4: Monk
                0x0B, // 5: Bard
                0x14, // 6: Rogue
                0x0D, // 7: Druid
                0x16, // 8: Shaman
                0x0C, // 9: Cleric
                0x0F, // 10: Magician
                0x11, // 11: Necromancer
                0x0E, // 12: Enchanter
                0x18, // 13: Wizard
                0xFB, // 14: Alchemist
            ];
            match class_opt {
                Some(c) if (c as usize) < CLASS_ICON_TEX_IDS.len() => {
                    Ok(Some(CLASS_ICON_TEX_IDS[c as usize]))
                }
                _ => Ok(None),
            }
        })?,
    )?;

    ui.set(
        "GetClassIconTexture",
        host.lua
            .create_function(|_, _: Option<u32>| Ok((Value::Nil, Value::Nil, Value::Nil)))?,
    )?;
    ui.set(
        "DrawDispositionIcon",
        host.lua
            .create_function(|_, _: mlua::MultiValue| Ok(false))?,
    )?;
    ui.set(
        "DrawClassIcon",
        host.lua
            .create_function(|_, _: mlua::MultiValue| Ok(false))?,
    )?;

    host.register_host_module("frontiers_forge.ui", Value::Table(ui))
}

pub(crate) fn install_standalone_ui_adapter(host: &Host) -> mlua::Result<()> {
    let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));
    let lease = VisibilityLease::new(registry).map_err(|e| mlua::Error::runtime(e.to_string()))?;
    lease.update(host.snapshot.get().health, true);
    install_managed_health_adapter(host, &lease)
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct PublishedVisibility {
    pub health_bar: bool,
    pub power_bar: bool,
    pub main_exp_bar: bool,
    pub secondary_exp_bar: bool,
    pub compass: bool,
    pub target: bool,
    pub group_panel: bool,
    pub group_compass_marks: bool,
    pub pet: bool,
    pub chat: bool,
    pub ability_bar: bool,
    pub active_effects: bool,
}

#[allow(dead_code)]
impl PublishedVisibility {
    pub fn any_hidden(&self) -> bool {
        self.health_bar
            || self.power_bar
            || self.main_exp_bar
            || self.secondary_exp_bar
            || self.compass
            || self.target
            || self.group_panel
            || self.group_compass_marks
            || self.pet
            || self.chat
            || self.ability_bar
            || self.active_effects
    }
}

thread_local! {
    static PUBLISHED_VIS: Cell<PublishedVisibility> = const { Cell::new(PublishedVisibility {
        health_bar: false,
        power_bar: false,
        main_exp_bar: false,
        secondary_exp_bar: false,
        compass: false,
        target: false,
        group_panel: false,
        group_compass_marks: false,
        pet: false,
        chat: false,
        ability_bar: false,
        active_effects: false,
    }) };
}

pub(crate) fn publish(hidden: bool) {
    PUBLISHED_VIS.set(PublishedVisibility {
        health_bar: hidden,
        ..PublishedVisibility::default()
    });
}

pub(crate) fn publish_claims(claims: &HudClaimsRegistry) {
    PUBLISHED_VIS.set(PublishedVisibility {
        health_bar: claims.is_hidden("health_bar"),
        power_bar: claims.is_hidden("power_bar"),
        main_exp_bar: claims.is_hidden("main_exp_bar"),
        secondary_exp_bar: claims.is_hidden("secondary_exp_bar"),
        compass: claims.is_hidden("compass_face") || claims.is_hidden("compass_back"),
        target: claims.is_hidden("target_nameplate"),
        group_panel: claims.is_hidden("group_member_panel"),
        group_compass_marks: claims.is_hidden("group_compass_marks"),
        pet: claims.is_hidden("pet_panel"),
        chat: claims.is_hidden("chat_window"),
        ability_bar: claims.is_hidden("hotbar_draw_compact") || claims.is_hidden("hotbar_draw_expanded"),
        active_effects: claims.is_hidden("active_effects"),
    });
}

pub(crate) fn published_visibility() -> PublishedVisibility {
    PUBLISHED_VIS.get()
}

pub(crate) fn published_health_hidden() -> bool {
    PUBLISHED_VIS.get().health_bar
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Host {
        use super::super::discovery::DiscoveredAddon;
        Host::for_addon(&DiscoveredAddon {
            id: "visibility".into(),
            name: "visibility".into(),
            entry: "main.lua".into(),
            script_root: ".".into(),
            package_root: ".".into(),
            module_dirs: vec![],
            resource_dirs: vec![],
            api: None,
            requested_capabilities: vec![],
            initially_enabled: true,
        })
        .unwrap()
    }

    #[test]
    fn forge_visibility_pending_ready_two_owners_suspend_disable_and_explicit_enable() {
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));
        let a = VisibilityLease::new(registry.clone()).unwrap();
        let b = VisibilityLease::new(registry.clone()).unwrap();
        let host_a = host();
        let host_b = host();
        install_managed_health_adapter(&host_a, &a).unwrap();
        install_managed_health_adapter(&host_b, &b).unwrap();
        a.update(HealthSnapshot::unavailable(), true);
        host_a
            .execute(
                "ui=require('frontiers_forge.ui');assert(ui.DisableHealthBar('hearts')==false)",
            )
            .unwrap();
        assert!(!registry.borrow().is_hidden());
        let ready = HealthSnapshot::new(true, 0, 100);
        a.update(ready, true);
        assert!(
            registry.borrow().is_hidden(),
            "pending Initialize intent promotes without another callback"
        );
        b.update(ready, true);
        host_b
            .execute("ui=require('frontiers_forge.ui');assert(ui.DisableHealthBar('hearts'))")
            .unwrap();
        a.clear();
        assert!(registry.borrow().is_hidden(), "neighbor keeps its claim");
        b.suspend();
        assert!(!registry.borrow().is_hidden());
        b.update(ready, true);
        assert!(registry.borrow().is_hidden());
        b.clear();
        b.update(ready, true);
        assert!(
            !registry.borrow().is_hidden(),
            "disable does not resurrect intent"
        );
        a.update(HealthSnapshot::unavailable(), true);
        host_a
            .execute("ui.DisableHealthBar('hearts');ui.EnableHealthBar('hearts')")
            .unwrap();
        a.update(ready, true);
        assert!(
            !registry.borrow().is_hidden(),
            "explicit Enable removes pending too"
        );
        a.clear();
        host_a
            .execute("assert(ui.DisableHealthBar('inactive')==false)")
            .unwrap();
        a.update(ready, true);
        assert!(
            !registry.borrow().is_hidden(),
            "inactive callback cannot leave intent"
        );
    }

    #[test]
    fn forge_visibility_pending_labels_bounded_and_drop_clears_effective_claims() {
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));
        let lease = VisibilityLease::new(registry.clone()).unwrap();
        let host = host();
        install_managed_health_adapter(&host, &lease).unwrap();
        lease.update(HealthSnapshot::unavailable(), true);
        assert!(
            host.execute("require('frontiers_forge.ui').DisableHealthBar(string.rep('x',129))")
                .is_err()
        );
        host.execute(
            "ui=require('frontiers_forge.ui');for i=1,32 do ui.DisableHealthBar(tostring(i)) end",
        )
        .unwrap();
        assert!(host.execute("ui.DisableHealthBar('overflow')").is_err());
        lease.update(HealthSnapshot::new(true, 1, 100), true);
        assert_eq!(registry.borrow().total_claims_count(), 32);
        drop(lease);
        assert!(
            !registry.borrow().is_hidden(),
            "Lua closure references must not prevent owner cleanup"
        );
    }

    #[test]
    fn forge_ui_adapter_all_methods_exist_and_contract_shape() {
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));
        let hud_claims = Rc::new(RefCell::new(HudClaimsRegistry::new()));
        let lease = VisibilityLease::with_hud_claims(registry.clone(), hud_claims.clone()).unwrap();
        let host = host();
        install_managed_health_adapter(&host, &lease).unwrap();
        lease.update(HealthSnapshot::new(true, 100, 100), true);

        let script = r#"
            local UI = require("frontiers_forge.ui")
            assert(type(UI.DisableHealthBar) == "function")
            assert(type(UI.EnableHealthBar) == "function")
            assert(type(UI.DisablePowerBar) == "function")
            assert(type(UI.EnablePowerBar) == "function")
            assert(type(UI.DisableMainExpBar) == "function")
            assert(type(UI.EnableMainExpBar) == "function")
            assert(type(UI.DisableSecondaryExpBar) == "function")
            assert(type(UI.EnableSecondaryExpBar) == "function")
            assert(type(UI.DisableExperienceBars) == "function")
            assert(type(UI.EnableExperienceBars) == "function")
            assert(type(UI.DisableCompass) == "function")
            assert(type(UI.EnableCompass) == "function")
            assert(type(UI.DisableTargetNameplate) == "function")
            assert(type(UI.EnableTargetNameplate) == "function")
            assert(type(UI.DisableGroupMemberPanel) == "function")
            assert(type(UI.EnableGroupMemberPanel) == "function")
            assert(type(UI.DisableGroupCompassMarkers) == "function")
            assert(type(UI.EnableGroupCompassMarkers) == "function")
            assert(type(UI.DisableGroupDisplay) == "function")
            assert(type(UI.EnableGroupDisplay) == "function")
            assert(type(UI.DisablePetPanel) == "function")
            assert(type(UI.EnablePetPanel) == "function")
            assert(type(UI.DisableActiveEffectsDisplay) == "function")
            assert(type(UI.EnableActiveEffectsDisplay) == "function")
            assert(type(UI.DisableAbilityBar) == "function")
            assert(type(UI.EnableAbilityBar) == "function")
            assert(type(UI.DisableChatWindow) == "function")
            assert(type(UI.EnableChatWindow) == "function")
            assert(type(UI.DisableUI) == "function")
            assert(type(UI.EnableUI) == "function")
            assert(type(UI.GetHiddenBy) == "function")
            assert(type(UI.GetDispositionFaceTexId) == "function")
            assert(type(UI.GetClassIconTexId) == "function")
            assert(type(UI.GetClassIconTexture) == "function")
            assert(type(UI.DrawDispositionIcon) == "function")
            assert(type(UI.DrawClassIcon) == "function")
        "#;
        host.execute(script).unwrap();
    }

    #[test]
    fn forge_ui_adapter_single_and_compound_suppression_and_get_hidden_by() {
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));
        let hud_claims = Rc::new(RefCell::new(HudClaimsRegistry::new()));
        let lease_a =
            VisibilityLease::with_hud_claims(registry.clone(), hud_claims.clone()).unwrap();
        let lease_b =
            VisibilityLease::with_hud_claims(registry.clone(), hud_claims.clone()).unwrap();
        let host_a = host();
        let host_b = host();
        install_managed_health_adapter(&host_a, &lease_a).unwrap();
        install_managed_health_adapter(&host_b, &lease_b).unwrap();

        lease_a.update(HealthSnapshot::new(true, 100, 100), true);
        lease_b.update(HealthSnapshot::new(true, 100, 100), true);

        // PowerBar
        host_a.execute("local UI = require('frontiers_forge.ui'); assert(UI.DisablePowerBar('mod_a') == true)").unwrap();
        assert!(hud_claims.borrow().is_hidden("power_bar"));
        let owners_1: Vec<String> = host_a
            .lua
            .load("return require('frontiers_forge.ui').GetHiddenBy('power_bar')")
            .eval()
            .unwrap();
        assert_eq!(owners_1, vec!["mod_a"]);

        host_b.execute("local UI = require('frontiers_forge.ui'); assert(UI.DisablePowerBar('mod_b') == true)").unwrap();
        let owners_2: Vec<String> = host_a
            .lua
            .load("return require('frontiers_forge.ui').GetHiddenBy('power_bar')")
            .eval()
            .unwrap();
        assert_eq!(owners_2, vec!["mod_a", "mod_b"]);

        // Compound Compass
        host_a.execute("local UI = require('frontiers_forge.ui'); assert(UI.DisableCompass('tape_compass') == true)").unwrap();
        assert!(hud_claims.borrow().is_hidden("compass_face"));
        assert!(hud_claims.borrow().is_hidden("compass_back"));

        // Compound AbilityBar
        host_a.execute("local UI = require('frontiers_forge.ui'); assert(UI.DisableAbilityBar('wereoxxs_bars') == true)").unwrap();
        assert!(hud_claims.borrow().is_hidden("hotbar_draw_compact"));
        assert!(hud_claims.borrow().is_hidden("hotbar_draw_expanded"));

        // Enable one owner on power bar
        host_a.execute("local UI = require('frontiers_forge.ui'); assert(UI.EnablePowerBar('mod_a') == true)").unwrap();
        assert!(
            hud_claims.borrow().is_hidden("power_bar"),
            "mod_b still holds power_bar"
        );
        let owners_3: Vec<String> = host_a
            .lua
            .load("return require('frontiers_forge.ui').GetHiddenBy('power_bar')")
            .eval()
            .unwrap();
        assert_eq!(owners_3, vec!["mod_b"]);

        // Drop lease_b: mod_b claims must vanish
        drop(lease_b);
        assert!(!hud_claims.borrow().is_hidden("power_bar"));
        let owners_4: Vec<String> = host_a
            .lua
            .load("return require('frontiers_forge.ui').GetHiddenBy('power_bar')")
            .eval()
            .unwrap();
        assert!(owners_4.is_empty());

        // Restore compass and ability bar
        host_a.execute("local UI = require('frontiers_forge.ui'); assert(UI.EnableCompass('tape_compass') == true)").unwrap();
        assert!(!hud_claims.borrow().is_hidden("compass_face"));
        assert!(!hud_claims.borrow().is_hidden("compass_back"));

        host_a.execute("local UI = require('frontiers_forge.ui'); assert(UI.EnableAbilityBar('wereoxxs_bars') == true)").unwrap();
        assert!(!hud_claims.borrow().is_hidden("hotbar_draw_compact"));
        assert!(!hud_claims.borrow().is_hidden("hotbar_draw_expanded"));
    }

    #[test]
    fn forge_ui_adapter_global_disable_ui_and_enable_ui() {
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));
        let hud_claims = Rc::new(RefCell::new(HudClaimsRegistry::new()));
        let lease = VisibilityLease::with_hud_claims(registry.clone(), hud_claims.clone()).unwrap();
        let host = host();
        install_managed_health_adapter(&host, &lease).unwrap();
        lease.update(HealthSnapshot::new(true, 100, 100), true);

        host.execute(
            "local UI = require('frontiers_forge.ui'); assert(UI.DisableUI('all_mod') == true)",
        )
        .unwrap();
        for el in ALL_UI_ELEMENTS {
            assert!(
                hud_claims.borrow().is_hidden(el),
                "element {el} must be hidden by DisableUI"
            );
            let owners: Vec<String> = host
                .lua
                .load(&format!(
                    "return require('frontiers_forge.ui').GetHiddenBy('{el}')"
                ))
                .eval()
                .unwrap();
            assert_eq!(owners, vec!["all_mod"]);
        }
        assert!(
            registry.borrow().is_hidden(),
            "health bar must also be registered in HealthBarClaimsRegistry"
        );

        host.execute(
            "local UI = require('frontiers_forge.ui'); assert(UI.EnableUI('all_mod') == true)",
        )
        .unwrap();
        for el in ALL_UI_ELEMENTS {
            assert!(
                !hud_claims.borrow().is_hidden(el),
                "element {el} must be restored by EnableUI"
            );
        }
        assert!(!registry.borrow().is_hidden());
    }

    #[test]
    fn forge_ui_adapter_disposition_and_class_icon_lookups() {
        let registry = Rc::new(RefCell::new(HealthBarClaimsRegistry::new()));
        let lease = VisibilityLease::new(registry).unwrap();
        let host = host();
        install_managed_health_adapter(&host, &lease).unwrap();

        let script = r#"
            local UI = require("frontiers_forge.ui")
            -- Dispositions: <=1 hostile (0x73), 4..6 friendly (0x75), else neutral (0x74), nil -> nil
            assert(UI.GetDispositionFaceTexId(nil) == nil)
            assert(UI.GetDispositionFaceTexId(0) == 0x73)
            assert(UI.GetDispositionFaceTexId(1) == 0x73)
            assert(UI.GetDispositionFaceTexId(2) == 0x74)
            assert(UI.GetDispositionFaceTexId(3) == 0x74)
            assert(UI.GetDispositionFaceTexId(4) == 0x75)
            assert(UI.GetDispositionFaceTexId(5) == 0x75)
            assert(UI.GetDispositionFaceTexId(6) == 0x75)
            assert(UI.GetDispositionFaceTexId(7) == 0x74)
            assert(UI.GetDispositionFaceTexId("0") == 0x73)

            -- Classes: 0..14 pinned IDs
            assert(UI.GetClassIconTexId(0) == 0x17)   -- Warrior
            assert(UI.GetClassIconTexId(1) == 0x13)   -- Ranger
            assert(UI.GetClassIconTexId(2) == 0x12)   -- Paladin
            assert(UI.GetClassIconTexId(3) == 0x15)   -- Shadowknight
            assert(UI.GetClassIconTexId(4) == 0x10)   -- Monk
            assert(UI.GetClassIconTexId(5) == 0x0B)   -- Bard
            assert(UI.GetClassIconTexId(6) == 0x14)   -- Rogue
            assert(UI.GetClassIconTexId(7) == 0x0D)   -- Druid
            assert(UI.GetClassIconTexId(8) == 0x16)   -- Shaman
            assert(UI.GetClassIconTexId(9) == 0x0C)   -- Cleric
            assert(UI.GetClassIconTexId(10) == 0x0F)  -- Magician
            assert(UI.GetClassIconTexId(11) == 0x11)  -- Necromancer
            assert(UI.GetClassIconTexId(12) == 0x0E)  -- Enchanter
            assert(UI.GetClassIconTexId(13) == 0x18)  -- Wizard
            assert(UI.GetClassIconTexId(14) == 0xFB)  -- Alchemist
            assert(UI.GetClassIconTexId(15) == nil)
            assert(UI.GetClassIconTexId("13") == 0x18)
            assert(UI.GetClassIconTexId(nil) == nil)

            -- Textures and drawing stubs
            local tex, w, h = UI.GetClassIconTexture(0)
            assert(tex == nil and w == nil and h == nil)
            assert(UI.DrawDispositionIcon(0) == false)
            assert(UI.DrawClassIcon(0) == false)
        "#;
        host.execute(script).unwrap();
    }
}
