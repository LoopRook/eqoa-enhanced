//! Managed desired visibility is separate from the standalone effective registry.
use super::{
    health::{self, HealthBarClaimsRegistry, HealthSnapshot, InstanceToken},
    host::Host,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
};

struct Intent {
    desired: HashSet<String>,
    active: bool,
    suspended: bool,
    token: InstanceToken,
    registry: Rc<RefCell<HealthBarClaimsRegistry>>,
}
impl Intent {
    fn project(&self, snapshot: HealthSnapshot) {
        let mut registry = self.registry.borrow_mut();
        registry.release_instance(self.token);
        if self.active && !self.suspended && snapshot.is_available() {
            for label in &self.desired {
                registry
                    .disable(self.token, Some(label), true)
                    .expect("bounded validated intent");
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
        Ok(Self {
            snapshot: Rc::new(Cell::new(HealthSnapshot::unavailable())),
            intent: Rc::new(RefCell::new(Intent {
                desired: HashSet::new(),
                active: false,
                suspended: false,
                token: InstanceToken::next()?,
                registry,
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
pub(crate) fn install_managed_health_adapter(
    host: &Host,
    lease: &VisibilityLease,
) -> mlua::Result<()> {
    let token = lease.intent.borrow().token;
    let registry = lease.intent.borrow().registry.clone();
    health::install_health_adapter(host, token, lease.snapshot.clone(), registry)?;
    let ui = host.lua.create_table()?;
    let intent = lease.intent.clone();
    let snapshot = lease.snapshot.clone();
    ui.set(
        "DisableHealthBar",
        host.lua.create_function(move |_, owner: Option<String>| {
            let label = owner.as_deref().unwrap_or(health::ANONYMOUS_OWNER);
            if label.len() > health::MAX_OWNER_LABEL_BYTES {
                return Err(mlua::Error::runtime("owner label exceeds 128 bytes limit"));
            }
            let mut i = intent.borrow_mut();
            if !i.active {
                return Ok(false);
            }
            if !i.desired.contains(label) && i.desired.len() >= health::MAX_OWNERS_PER_VM {
                return Err(mlua::Error::runtime(
                    "VM owner claims limit exceeded (max 32)",
                ));
            }
            i.desired.insert(label.to_owned());
            i.project(snapshot.get());
            Ok(snapshot.get().is_available() && !i.suspended)
        })?,
    )?;
    let intent = lease.intent.clone();
    let snapshot = lease.snapshot.clone();
    ui.set(
        "EnableHealthBar",
        host.lua.create_function(move |_, owner: Option<String>| {
            let mut i = intent.borrow_mut();
            i.desired
                .remove(owner.as_deref().unwrap_or(health::ANONYMOUS_OWNER));
            i.project(snapshot.get());
            Ok(true)
        })?,
    )?;
    host.register_host_module("frontiers_forge.ui", mlua::Value::Table(ui))
}
thread_local! { static PUBLISHED: Cell<bool> = const { Cell::new(false) }; }
pub(crate) fn publish(hidden: bool) {
    PUBLISHED.set(hidden);
}
pub(crate) fn published_health_hidden() -> bool {
    PUBLISHED.get()
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
}
