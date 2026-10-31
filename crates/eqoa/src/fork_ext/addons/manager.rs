//! Backend lifecycle coordinator. The caller supplies an active frame and owns
//! the single Context/platform/shared renderer; this module never creates them.
//!
//! The caller owns the sole Context/WinitPlatform and supplies its shared renderer.
//! Commands are drained only at frame boundaries. A retained Instance is a single
//! unit; normal disable preserves its VM/globals/callbacks. Reload prepares a
//! fresh compiled VM before disabling/replacing the old one. Failed entries do
//! not retry. DisableScript and OnEject bookkeeping remain independent.
//!
//! Every render-capable Lua invocation uses Bridge::invoke. Resource retirement
//! is queued and banks remain alive until after_render; no collect(true) during
//! frame. Shutdown is explicitly after the final pass, idempotent, and nonrendering
//! when outside a frame. Drop cannot catch native crashes or promise panic isolation.

use super::{
    bridge::Bridge,
    discovery::DiscoveredAddon,
    host::{Host, Snapshot},
    sound::SoundBank,
    storage::Storage,
    textures::Textures,
};
use dear_imgui_wgpu::WgpuRenderer;
use mlua::{Function, Value};
use render::wgpu;
use std::{cell::RefCell, collections::VecDeque, path::PathBuf, rc::Rc};

const MAX_INSTANCES: usize = 16;
const MAX_DIAGNOSTICS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Disabled,
    Enabled,
    Failed,
}

#[derive(Clone, Debug)]
pub(crate) struct EntryView {
    pub metadata: DiscoveredAddon,
    pub status: Status,
    pub enabled_intent: bool,
    pub initialized: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Diagnostic {
    pub id: Option<String>,
    pub phase: &'static str,
    pub message: String,
}

pub(crate) enum Command {
    Enable(String, bool),
    Reload(String),
    Rescan,
    Settings(String),
    Save(String),
    Load(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Disposition {
    Retain,
    Retire,
}

struct Instance {
    // Function and Lua closures drop before the bank's final renderer reference.
    function: Function,
    host: Host,
    bridge: Bridge,
    textures: Rc<RefCell<Textures>>,
    sound_bank: Rc<RefCell<SoundBank>>,
    font_bank: Rc<RefCell<super::fonts::FontBank>>,
    visibility: super::visibility::VisibilityLease,
    initialized: bool,
    active: bool,
    ejected: bool,
    resources_retired: bool,
}

struct Entry {
    metadata: DiscoveredAddon,
    key: String,
    status: Status,
    enabled_intent: bool,
    instance: Option<Instance>,
    not_before: u64,
}

pub(crate) struct Manager {
    root: PathBuf,
    storage: Storage,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Rc<RefCell<WgpuRenderer>>,
    entries: Vec<Entry>,
    commands: VecDeque<Command>,
    diagnostics: Vec<Diagnostic>,
    retired: Vec<Rc<RefCell<Textures>>>,
    shut_down: bool,
    frame_number: u64,
    awaiting_render: bool,
    snapshot: Snapshot,
    health_waiting: bool,
    health_claims: Rc<RefCell<super::health::HealthBarClaimsRegistry>>,
    hud_claims: Rc<RefCell<super::visibility::HudClaimsRegistry>>,
}

impl Manager {
    pub fn new(
        root: PathBuf,
        storage_root: Option<PathBuf>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: Rc<RefCell<WgpuRenderer>>,
    ) -> Self {
        let mut manager = Self {
            root,
            storage: Storage::new(storage_root),
            device: device.clone(),
            queue: queue.clone(),
            renderer,
            entries: Vec::new(),
            commands: VecDeque::new(),
            diagnostics: Vec::new(),
            retired: Vec::new(),
            shut_down: false,
            frame_number: 0,
            awaiting_render: false,
            snapshot: Snapshot::default(),
            health_waiting: false,
            health_claims: Rc::new(RefCell::new(super::health::HealthBarClaimsRegistry::new())),
            hud_claims: Rc::new(RefCell::new(super::visibility::HudClaimsRegistry::new())),
        };
        manager.rescan(false);
        manager
    }
    pub fn entries(&self) -> Vec<EntryView> {
        self.entries
            .iter()
            .map(|e| EntryView {
                metadata: e.metadata.clone(),
                status: e.status.clone(),
                enabled_intent: e.enabled_intent,
                initialized: e.instance.as_ref().is_some_and(|i| i.initialized),
            })
            .collect()
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn health_hidden(&self) -> bool {
        self.health_claims.borrow().is_hidden()
    }
    #[allow(dead_code)]
    pub fn is_hud_element_hidden(&self, element: &str) -> bool {
        self.hud_claims.borrow().is_hidden(element)
    }
    #[allow(dead_code)]
    pub fn hud_claims(&self) -> Rc<RefCell<super::visibility::HudClaimsRegistry>> {
        self.hud_claims.clone()
    }
    pub fn suspend_health(&self) {
        for e in &self.entries {
            if let Some(i) = &e.instance {
                i.visibility.suspend();
            }
        }
        super::visibility::publish(false);
    }
    pub fn suspend_sound(&self) {
        for e in &self.entries {
            if let Some(i) = &e.instance {
                i.sound_bank.borrow_mut().set_active(false);
            }
        }
    }
    pub fn health_readiness(&self) -> Option<&'static str> {
        if self.health_waiting {
            Some(
                "Waiting for current-world health stats. Health addons will appear when a valid update arrives.",
            )
        } else if !self.snapshot.health.is_available() {
            Some("Health data unavailable: log in and wait for the player HUD.")
        } else {
            None
        }
    }
    pub fn queue(&mut self, command: Command) {
        if self.shut_down {
            return;
        }
        if self.commands.len() >= 1024 {
            self.diagnostic(None, "command", "addon command queue limit exceeded".into());
            return;
        }
        self.commands.push_back(command);
    }
    pub fn frame(&mut self, snapshot: Snapshot) {
        if self.shut_down {
            return;
        }
        if self.awaiting_render {
            self.diagnostic(
                None,
                "frame",
                "after_render is required before the next addon frame".into(),
            );
            return;
        }
        self.frame_number = self.frame_number.saturating_add(1);
        self.health_waiting = snapshot.in_game && !snapshot.health.is_available();
        let snapshot = snapshot.managed();
        self.snapshot = snapshot;
        self.awaiting_render = true;
        for e in &self.entries {
            if let Some(i) = &e.instance {
                i.host.snapshot.set(snapshot);
                i.visibility.update(snapshot.health, i.active);
            }
        }
        while let Some(command) = self.commands.pop_front() {
            self.command(command);
        }
        let mut entries = std::mem::take(&mut self.entries);
        let mut count = entries.iter().filter(|e| e.instance.is_some()).count();
        for entry in &mut entries {
            if entry.enabled_intent
                && entry.status != Status::Failed
                && entry.not_before <= self.frame_number
            {
                if entry.instance.is_none() {
                    if count >= MAX_INSTANCES {
                        entry.status = Status::Failed;
                        self.diagnostic(Some(&entry.metadata.id),"initialize","16 addon VM limit reached; remove an instantiated addon before reloading".into());
                        continue;
                    }
                    match self.prepare(&entry.metadata) {
                        Ok(i) => {
                            entry.instance = Some(i);
                            count += 1;
                        }
                        Err(e) => {
                            entry.status = Status::Failed;
                            self.diagnostic(Some(&entry.metadata.id), "initialize", e);
                            continue;
                        }
                    }
                }
                let instance = entry.instance.as_mut().unwrap();
                instance.host.snapshot.set(snapshot);
                instance.active = true;
                instance.sound_bank.borrow_mut().set_active(true);
                instance.visibility.update(snapshot.health, true);
                instance.resources_retired = false;
                let first = !instance.initialized;
                // An entry that throws can still have registered cleanup callbacks.
                instance.initialized = true;
                match instance
                    .bridge
                    .invoke(|| instance.host.run(&instance.function))
                {
                    Ok(()) => {
                        entry.status = Status::Enabled;
                        if first {
                            self.load(entry, true);
                        }
                    }
                    Err(e) => self.fail(entry, "frame", e.to_string(), true),
                }
            }
            if let Some(i) = entry.instance.as_ref() {
                let messages = i.textures.borrow_mut().drain_diagnostics();
                for message in messages {
                    self.diagnostic(Some(&entry.metadata.id), "texture", message);
                }
                let messages = i.sound_bank.borrow_mut().drain_diagnostics();
                for message in messages {
                    self.diagnostic(Some(&entry.metadata.id), "sound", message);
                }
                let messages = i.font_bank.borrow_mut().drain_diagnostics();
                for message in messages {
                    self.diagnostic(Some(&entry.metadata.id), "font", message);
                }
            }
        }
        self.entries = entries;
    }
    pub fn flush_pending_fonts(&mut self, context: &mut dear_imgui_rs::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let epoch = super::fonts::current_context_epoch();
            let mut diags = Vec::new();
            for entry in &mut self.entries {
                if let Some(instance) = &mut entry.instance {
                    let mut bank = instance.font_bank.borrow_mut();
                    bank.set_context_epoch(epoch);
                    bank.flush_pending(context);
                    for diag in bank.drain_diagnostics() {
                        diags.push((entry.metadata.id.clone(), diag));
                    }
                }
            }
            for (id, diag) in diags {
                self.diagnostic(Some(&id), "font", diag);
            }
        }
    }

    pub fn after_render(&mut self) {
        super::icons::cleanup_active_stale();
        for bank in self.retired.drain(..) {
            bank.borrow_mut().collect(true);
        }
        for entry in &self.entries {
            if let Some(instance) = &entry.instance {
                instance.textures.borrow_mut().collect(false);
            }
        }
        self.awaiting_render = false;
        super::visibility::publish_claims(&self.hud_claims.borrow());
    }
    pub fn shutdown(&mut self) {
        super::visibility::publish(false);
        self.shutdown_inner(true);
    }

    fn diagnostic(&mut self, id: Option<&str>, phase: &'static str, message: String) {
        let message: String = message.chars().take(2048).collect();
        if self
            .diagnostics
            .iter()
            .any(|d| d.id.as_deref() == id && d.phase == phase && d.message == message)
        {
            return;
        }
        if self.diagnostics.len() == MAX_DIAGNOSTICS {
            self.diagnostics.remove(0);
        }
        self.diagnostics.push(Diagnostic {
            id: id.map(str::to_owned),
            phase,
            message: message.clone(),
        });
        let log_line = format!(
            "[{:?}] [{}] {}\n",
            phase,
            id.unwrap_or("general"),
            message
        );
        let log_path = self.root.join("addons_error.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            use std::io::Write;
            let _ = f.write_all(log_line.as_bytes());
        }
    }

    fn prepare(&self, metadata: &DiscoveredAddon) -> Result<Instance, String> {
        let host = Host::for_addon(metadata).map_err(|e| e.to_string())?;
        host.snapshot.set(self.snapshot);
        let visibility = super::visibility::VisibilityLease::with_hud_claims(
            self.health_claims.clone(),
            self.hud_claims.clone(),
        )
        .map_err(|e| e.to_string())?;
        visibility.update(self.snapshot.health, false);
        super::visibility::install_managed_health_adapter(&host, &visibility)
            .map_err(|e| e.to_string())?;
        super::player::install_player_adapter(&host).map_err(|e| e.to_string())?;
        let function = host
            .compile_addon_entry(metadata)
            .map_err(|e| e.to_string())?;
        let bridge = Bridge::install(&host).map_err(|e| e.to_string())?;
        let textures = Textures::new(&self.device, &self.queue, self.renderer.clone());
        Textures::install_for_addon(&textures, &host.lua, metadata).map_err(|e| e.to_string())?;
        let sound_bank = Rc::new(RefCell::new(SoundBank::new().map_err(|e| e.to_string())?));
        SoundBank::install_for_addon(&sound_bank, &host.lua, metadata)
            .map_err(|e| e.to_string())?;
        let font_bank = Rc::new(RefCell::new(super::fonts::FontBank::new(
            0,
            super::fonts::current_context_epoch(),
        )));
        super::fonts::FontBank::install_for_addon(&font_bank, &host.lua, metadata)
            .map_err(|e| e.to_string())?;
        Ok(Instance {
            host,
            function,
            bridge,
            textures,
            sound_bank,
            font_bank,
            visibility,
            initialized: false,
            active: false,
            ejected: false,
            resources_retired: false,
        })
    }

    fn invoke(
        instance: &Instance,
        kind: i32,
        value: Value,
        rendering: bool,
    ) -> mlua::Result<Value> {
        if !rendering {
            return instance.host.callback(kind, value);
        }
        let mut result = Value::Nil;
        instance.bridge.invoke(|| {
            result = instance.host.callback(kind, value)?;
            Ok(())
        })?;
        Ok(result)
    }

    fn retire(&mut self, entry: &mut Entry) {
        if let Some(i) = entry.instance.as_mut()
            && !i.resources_retired
        {
            self.retired.push(i.textures.clone());
            i.resources_retired = true;
        }
    }

    fn save(&mut self, entry: &mut Entry, rendering: bool) {
        if entry.status == Status::Failed {
            return;
        }
        let Some(i) = entry.instance.as_mut().filter(|i| i.initialized) else {
            return;
        };
        i.resources_retired = false;
        match Self::invoke(i, 2, Value::Nil, rendering) {
            Ok(value) => {
                if let Err(e) = self.storage.save_profile(&entry.key, value) {
                    self.diagnostic(Some(&entry.metadata.id), "storage", e);
                }
            }
            Err(e) => self.fail(entry, "save", e.to_string(), rendering),
        }
        if let Some(i) = entry.instance.as_ref().filter(|i| !i.active) {
            i.visibility.clear();
        }
    }

    fn load(&mut self, entry: &mut Entry, rendering: bool) {
        if entry.status == Status::Failed {
            return;
        }
        let Some(i) = entry.instance.as_mut().filter(|i| i.initialized) else {
            return;
        };
        i.resources_retired = false;
        match self.storage.load_profile(&entry.key, &i.host.lua) {
            Ok(Some(value)) => {
                if let Err(e) = Self::invoke(i, 3, value, rendering) {
                    self.fail(entry, "load", e.to_string(), rendering);
                }
            }
            Ok(None) => {}
            Err(e) => self.diagnostic(Some(&entry.metadata.id), "storage", e),
        }
        if let Some(i) = entry.instance.as_ref().filter(|i| !i.active) {
            i.visibility.clear();
        }
    }

    fn disable(
        &mut self,
        entry: &mut Entry,
        rendering: bool,
        save: bool,
        disposition: Disposition,
    ) {
        let active = entry.instance.as_ref().is_some_and(|i| i.active);
        if active && save {
            self.save(entry, rendering);
        }
        if let Some(i) = entry.instance.as_mut()
            && i.active
        {
            // Clear before invoking user code so a failing cleanup cannot repeat.
            i.active = false;
            i.sound_bank.borrow_mut().set_active(false);
            if let Err(e) = Self::invoke(i, 1, Value::Nil, rendering) {
                entry.status = Status::Failed;
                self.diagnostic(Some(&entry.metadata.id), "cleanup", e.to_string());
            }
        }
        // Ordinary disable retains the environment and all live cached handles.
        if let Some(i) = &entry.instance {
            i.visibility.clear();
        }
        // A callback failure is terminal even during an ordinary transition.
        if disposition == Disposition::Retire || entry.status == Status::Failed {
            if let Some(i) = entry.instance.as_mut() {
                i.sound_bank.borrow_mut().close_all();
            }
            self.retire(entry);
        }
        if entry.status != Status::Failed {
            entry.status = Status::Disabled;
        }
    }

    fn fail(&mut self, entry: &mut Entry, phase: &'static str, error: String, rendering: bool) {
        self.diagnostic(Some(&entry.metadata.id), phase, error);
        entry.status = Status::Failed;
        self.disable(entry, rendering, false, Disposition::Retire);
    }

    fn command(&mut self, command: Command) {
        if matches!(command, Command::Rescan) {
            self.rescan(true);
            return;
        }
        let id = match &command {
            Command::Enable(id, _)
            | Command::Reload(id)
            | Command::Settings(id)
            | Command::Save(id)
            | Command::Load(id) => id,
            Command::Rescan => unreachable!(),
        };
        let Some(index) = self.entries.iter().position(|e| e.metadata.id == *id) else {
            self.diagnostic(Some(id), "command", "addon is not discovered".into());
            return;
        };
        let mut entry = self.entries.remove(index);
        match command {
            Command::Enable(_, enabled) => {
                if enabled && entry.status == Status::Failed {
                    self.diagnostic(
                        Some(&entry.metadata.id),
                        "command",
                        "reload a failed addon before enabling it".into(),
                    );
                } else {
                    entry.enabled_intent = enabled;
                    if let Err(e) = self.storage.set_enabled(&entry.key, enabled) {
                        self.diagnostic(Some(&entry.metadata.id), "storage", e);
                    }
                    if !enabled {
                        self.disable(&mut entry, true, true, Disposition::Retain);
                    } else if entry.instance.as_ref().is_some_and(|i| {
                        self.retired
                            .iter()
                            .any(|bank| Rc::ptr_eq(bank, &i.textures))
                    }) {
                        entry.not_before = self.frame_number + 1;
                    }
                }
            }
            Command::Reload(_) => {
                let count = self.entries.iter().filter(|e| e.instance.is_some()).count()
                    + usize::from(entry.instance.is_some());
                if count >= MAX_INSTANCES {
                    self.diagnostic(
                        Some(&entry.metadata.id),
                        "reload",
                        "16 VM limit prevents staging a fresh replacement; remove an addon first"
                            .into(),
                    );
                } else {
                    match self.prepare(&entry.metadata) {
                        Err(e) => self.diagnostic(Some(&entry.metadata.id), "reload", e),
                        Ok(new) => {
                            self.disable(&mut entry, true, true, Disposition::Retire);
                            entry.instance = Some(new);
                            entry.status = Status::Disabled;
                            entry.not_before = self.frame_number + 1;
                        }
                    }
                }
            }
            Command::Save(_) => self.save(&mut entry, true),
            Command::Load(_) => self.load(&mut entry, true),
            Command::Settings(_) => {
                if entry.status != Status::Failed
                    && let Some(i) = entry.instance.as_mut().filter(|i| i.initialized)
                {
                    i.resources_retired = false;
                    if let Err(e) = Self::invoke(i, 0, Value::Nil, true) {
                        self.fail(&mut entry, "settings", e.to_string(), true);
                    }
                    if let Some(i) = entry.instance.as_ref().filter(|i| !i.active) {
                        i.visibility.clear();
                    }
                }
            }
            Command::Rescan => unreachable!(),
        }
        self.entries.insert(index, entry);
    }

    fn rescan(&mut self, rendering: bool) {
        let discovery = super::discovery::discover(&self.root);
        for d in discovery.diagnostics {
            self.diagnostic(
                None,
                "discovery",
                format!("{}: {}", d.path.display(), d.message),
            );
        }
        let mut old = std::mem::take(&mut self.entries);
        for metadata in discovery.addons {
            let found = old.iter().position(|e| e.metadata.id == metadata.id);
            if let Some(index) = found
                && old[index].metadata == metadata
            {
                self.entries.push(old.remove(index));
                continue;
            }
            let intent = found.map(|index| {
                let mut entry = old.remove(index);
                let intent = entry.enabled_intent;
                self.disable(&mut entry, rendering, true, Disposition::Retire);
                intent
            });
            let key = match Storage::key(&metadata) {
                Ok(key) => key,
                Err(e) => {
                    self.diagnostic(Some(&metadata.id), "storage", e);
                    continue;
                }
            };
            let persisted = match self.storage.enabled(&key) {
                Ok(v) => v,
                Err(e) => {
                    self.diagnostic(Some(&metadata.id), "storage", e);
                    None
                }
            };
            let enabled_intent = intent.or(persisted).unwrap_or(metadata.initially_enabled);
            self.entries.push(Entry {
                metadata,
                key,
                enabled_intent,
                status: Status::Disabled,
                instance: None,
                not_before: self.frame_number,
            });
        }
        for mut removed in old {
            self.disable(&mut removed, rendering, true, Disposition::Retire);
        }
        self.entries.sort_by(|a, b| {
            a.metadata
                .entry
                .cmp(&b.metadata.entry)
                .then(a.metadata.id.cmp(&b.metadata.id))
        });
    }

    fn shutdown_inner(&mut self, collect: bool) {
        if self.shut_down {
            return;
        }
        self.shut_down = true;
        self.commands.clear();
        let mut entries = std::mem::take(&mut self.entries);
        for entry in &mut entries {
            if entry.status != Status::Failed {
                self.save(entry, false);
            }
            self.disable(entry, false, false, Disposition::Retire);
            if let Some(i) = entry
                .instance
                .as_mut()
                .filter(|i| i.initialized && !i.ejected)
            {
                i.ejected = true;
                if let Err(e) = Self::invoke(i, 4, Value::Nil, false) {
                    self.diagnostic(Some(&entry.metadata.id), "eject", e.to_string());
                }
            }
            // Final callbacks can allocate resources even outside a UI frame.
            if let Some(i) = &entry.instance {
                i.visibility.clear();
            }
            if let Some(i) = entry.instance.as_mut() {
                i.sound_bank.borrow_mut().close_all();
                i.resources_retired = false;
            }
            self.retire(entry);
        }
        self.entries = entries;
        if collect {
            self.after_render();
        }
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        super::visibility::publish(false);
        // If the caller omitted the render boundary, do not unregister handles
        // possibly present in outstanding draw data. Renderer teardown owns them.
        self.shutdown_inner(!self.awaiting_render);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::Context;
    use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo};
    use mlua::{AnyUserData, Lua, Table};
    use std::{
        fs,
        path::Path,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "eqoa-forge-manager-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn package(&self, name: &str, enabled: bool, source: &str) {
            let root = self.0.join(name);
            fs::create_dir_all(root.join("modules")).unwrap();
            fs::write(root.join("main.lua"), source).unwrap();
            fs::write(root.join("modules/private.lua"), "return {value=0}").unwrap();
            fs::write(root.join("mod.json"), serde_json::to_vec(&serde_json::json!({
                "schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":name,"name":name,"entry":"main.lua","enabled":enabled
            })).unwrap()).unwrap();
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
            assert!(
                self.0
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("eqoa-forge-manager-")
            );
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct Gpu {
        renderer: Rc<RefCell<WgpuRenderer>>,
        context: Context,
        device: wgpu::Device,
        queue: wgpu::Queue,
    }
    impl Gpu {
        fn new() -> Option<Self> {
            let instance = wgpu::Instance::default();
            let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("SKIP forge_manager GPU prerequisite: {e}");
                    return None;
                }
            };
            let (device, queue) =
                pollster::block_on(adapter.request_device(&Default::default())).unwrap();
            let mut context = Context::create();
            context.set_ini_filename::<PathBuf>(None).unwrap();
            let renderer = Rc::new(RefCell::new(
                WgpuRenderer::new(
                    WgpuInitInfo::new(
                        device.clone(),
                        queue.clone(),
                        wgpu::TextureFormat::Rgba8Unorm,
                    ),
                    &mut context,
                )
                .unwrap(),
            ));
            Some(Self {
                renderer,
                context,
                device,
                queue,
            })
        }
        fn manager(&self, root: &Path, config: Option<PathBuf>) -> Manager {
            Manager::new(
                root.to_owned(),
                config,
                &self.device,
                &self.queue,
                self.renderer.clone(),
            )
        }
        fn begin(&mut self) {
            self.context.io_mut().set_display_size([256.0, 192.0]);
            self.context
                .io_mut()
                .set_display_framebuffer_scale([1.0, 1.0]);
            self.context.io_mut().set_delta_time(1.0 / 60.0);
            self.context.frame();
        }
        // Records/submits actual shared-backend draw data, intentionally leaves
        // retirement to the caller so pre/post after_render handles are testable.
        fn render(&mut self) -> wgpu::Texture {
            let target = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("manager isolation fixture"),
                size: wgpu::Extent3d {
                    width: 256,
                    height: 192,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = target.create_view(&Default::default());
            let mut encoder = self.device.create_command_encoder(&Default::default());
            {
                let mut renderer = self.renderer.borrow_mut();
                let frame = self.context.render(renderer.renderer_consumer().unwrap());
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                renderer
                    .render(frame, &mut pass, FramebufferExtent::new(256, 192))
                    .unwrap();
            }
            self.queue.submit([encoder.finish()]);
            target
        }
        fn frame(&mut self, manager: &mut Manager) {
            self.begin();
            manager.frame(Snapshot::default());
            self.render();
            manager.after_render();
        }
        fn pixels(&self, target: &wgpu::Texture) -> Vec<u8> {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 256 * 192 * 4,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = self.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256 * 4),
                        rows_per_image: Some(192),
                    },
                },
                target.size(),
            );
            self.queue.submit([encoder.finish()]);
            let (send, recv) = std::sync::mpsc::channel();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
            self.device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            recv.recv().unwrap().unwrap();
            buffer.slice(..).get_mapped_range().unwrap().to_vec()
        }
    }
    fn lua(manager: &Manager, id: &str) -> Lua {
        manager
            .entries
            .iter()
            .find(|e| e.metadata.id == id)
            .unwrap()
            .instance
            .as_ref()
            .unwrap()
            .host
            .lua
            .clone()
    }
    fn count(manager: &Manager, id: &str) -> i32 {
        lua(manager, id).globals().get("count").unwrap()
    }
    fn transcript(lua: &Lua) -> Vec<String> {
        lua.globals()
            .get::<Table>("events")
            .unwrap()
            .sequence_values()
            .collect::<mlua::Result<_>>()
            .unwrap()
    }
    const SCRIPT: &str = r#"
        local module=require('private')
        module.value=module.value+1
        module_value=module.value
        count=(count or 0)+1
        events=events or {}
        table.insert(events,'entry:'..count)
        if not registered then
            registered=true
            UiForge.RegisterCallback(UiForge.CallbackType.Load,function(saved) table.insert(events,'load:'..saved[3]) end)
            UiForge.RegisterCallback(UiForge.CallbackType.Save,function() return {[3]=count} end)
            UiForge.RegisterCallback(UiForge.CallbackType.DisableScript,function() table.insert(events,'wrong-disable') end)
            UiForge.RegisterCallback(UiForge.CallbackType.DisableScript,function() table.insert(events,'disable') end)
            UiForge.RegisterCallback(UiForge.CallbackType.OnEject,function() table.insert(events,'eject') end)
        end
    "#;

    #[test]
    fn forge_manager_callbacks_retain_disabled_resources_and_retire_eject_resources() {
        let Some(mut gpu) = Gpu::new() else {
            return;
        };
        let scripts = Scratch::new();
        let config = Scratch::new();
        scripts.package("a",true,r#"
            if not registered then
                registered=true
                local util=require('frontiers_forge.util')
                local function texture() return UiForge.CreateTextureFromMemory(string.char(255,0,0,255),1,1) end
                UiForge.RegisterCallback(UiForge.CallbackType.Settings,function() settings_seen=util.IsStartMenuOpen();settings_texture=texture();ImGui.Text('settings') end)
                UiForge.RegisterCallback(UiForge.CallbackType.Save,function() save_seen=util.IsStartMenuOpen();save_texture=texture();return {[3]=7} end)
                UiForge.RegisterCallback(UiForge.CallbackType.Load,function(saved) loaded=saved[3];load_seen=util.IsStartMenuOpen();load_texture=texture() end)
                UiForge.RegisterCallback(UiForge.CallbackType.OnEject,function() eject_count=(eject_count or 0)+1;eject_texture=texture();ImGui.Text('not allowed outside frame') end)
            end
        "#);
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        gpu.frame(&mut manager);
        let retained = lua(&manager, "a");
        manager.queue(Command::Save("a".into()));
        manager.queue(Command::Load("a".into()));
        manager.queue(Command::Settings("a".into()));
        assert!(
            retained
                .globals()
                .get::<Option<i32>>("settings_seen")
                .unwrap()
                .is_none()
        );
        gpu.begin();
        manager.frame(Snapshot {
            in_game: false,
            start_menu: true,
            ..Snapshot::default()
        });
        gpu.render();
        manager.after_render();
        for key in ["save_seen", "load_seen", "settings_seen"] {
            assert_eq!(retained.globals().get::<i32>(key).unwrap(), 1);
        }
        assert_eq!(retained.globals().get::<i32>("loaded").unwrap(), 7);
        manager.queue(Command::Enable("a".into(), false));
        gpu.frame(&mut manager);
        manager.queue(Command::Save("a".into()));
        manager.queue(Command::Load("a".into()));
        manager.queue(Command::Settings("a".into()));
        gpu.frame(&mut manager);
        for key in ["save_texture", "load_texture", "settings_texture"] {
            let handle = retained.globals().get::<AnyUserData>(key).unwrap();
            assert!(
                Textures::raw(&handle).is_ok(),
                "successful disabled {key} callback retains its cached allocation"
            );
        }
        let released = retained
            .globals()
            .get::<AnyUserData>("save_texture")
            .unwrap();
        retained
            .load("UiForge.ReleaseTexture(save_texture)")
            .exec()
            .unwrap();
        gpu.frame(&mut manager);
        assert!(Textures::raw(&released).is_err());
        assert!(
            Textures::raw(
                &retained
                    .globals()
                    .get::<AnyUserData>("load_texture")
                    .unwrap()
            )
            .is_ok(),
            "explicit release must preserve other retained allocations"
        );
        manager.shutdown();
        manager.shutdown();
        assert_eq!(retained.globals().get::<i32>("eject_count").unwrap(), 1);
        let handle = retained
            .globals()
            .get::<AnyUserData>("eject_texture")
            .unwrap();
        assert!(
            Textures::raw(&handle).is_err(),
            "OnEject error must not skip final resource cleanup"
        );
        assert!(manager.diagnostics().iter().any(|d| d.phase == "eject"));
    }

    #[test]
    fn forge_manager_order_private_modules_toggle_globals_callbacks_and_persisted_false() {
        let Some(mut gpu) = Gpu::new() else {
            return;
        };
        let scripts = Scratch::new();
        let config = Scratch::new();
        scripts.package("a", true, SCRIPT);
        scripts.package("b", true, SCRIPT);
        let addon = super::super::discovery::discover(&scripts.0)
            .addons
            .into_iter()
            .find(|a| a.id == "a")
            .unwrap();
        let storage = Storage::new(Some(config.0.clone()));
        let key = Storage::key(&addon).unwrap();
        storage
            .save_profile(&key, Lua::new().load("return {[3]=41}").eval().unwrap())
            .unwrap();
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        assert!(manager.entries.iter().all(|e| e.instance.is_none()));
        gpu.frame(&mut manager);
        assert_eq!(transcript(&lua(&manager, "a")), ["entry:1", "load:41"]);
        gpu.frame(&mut manager);
        assert_eq!(count(&manager, "a"), 2);
        assert_eq!(count(&manager, "b"), 2);
        assert_eq!(
            lua(&manager, "a")
                .globals()
                .get::<i32>("module_value")
                .unwrap(),
            2
        );
        manager.queue(Command::Enable("a".into(), false));
        assert_eq!(
            count(&manager, "a"),
            2,
            "queue must not execute Lua immediately"
        );
        gpu.frame(&mut manager);
        manager.queue(Command::Enable("a".into(), false));
        gpu.frame(&mut manager);
        assert_eq!(
            transcript(&lua(&manager, "a")),
            ["entry:1", "load:41", "entry:2", "disable"]
        );
        assert_eq!(count(&manager, "b"), 4);
        manager.queue(Command::Enable("a".into(), true));
        gpu.frame(&mut manager);
        assert_eq!(
            count(&manager, "a"),
            3,
            "normal re-enable must preserve globals"
        );
        manager.queue(Command::Enable("a".into(), false));
        gpu.frame(&mut manager);
        let retained = lua(&manager, "a");
        manager.shutdown();
        manager.shutdown();
        assert_eq!(
            transcript(&retained)
                .iter()
                .filter(|s| s.as_str() == "eject")
                .count(),
            1
        );
        let restored = gpu.manager(&scripts.0, Some(config.0.clone()));
        assert!(
            !restored
                .entries()
                .into_iter()
                .find(|e| e.metadata.id == "a")
                .unwrap()
                .enabled_intent
        );
        assert!(restored.entries.iter().all(|e| e.instance.is_none()));
    }

    #[test]
    fn forge_manager_gpu_failure_isolation_and_after_render_retirement() {
        let Some(mut gpu) = Gpu::new() else {
            return;
        };
        let scripts = Scratch::new();
        let config = Scratch::new();
        scripts.package("a",true,r#"
            count=(count or 0)+1
            if not texture then texture=UiForge.CreateTextureFromMemory(string.char(255,0,255,255),1,1) end
            if count==1 then
                UiForge.RegisterCallback(UiForge.CallbackType.DisableScript,function() error('cleanup-controlled') end)
            end
            ImGui.Begin('victim')
            ImGui.Image(texture,64,64)
            if count==2 then error('primary-controlled') end
            ImGui.End()
        "#);
        scripts.package("b",true,r#"
            count=(count or 0)+1
            if not texture then texture=UiForge.CreateTextureFromMemory(string.char(0,255,0,255),1,1) end
            if count>=4 then
                ImGui.Begin('cached healthy texture')
                ImGui.Image(texture,64,64)
                ImGui.End()
            else
                ImGui.GetBackgroundDrawList():AddLine(ImVec2.new(0,16),ImVec2.new(256,16),ImGui.GetColorU32(0,1,0,1),8)
            end
        "#);
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        gpu.frame(&mut manager);
        let failed_lua = lua(&manager, "a");
        let handle = failed_lua.globals().get::<AnyUserData>("texture").unwrap();
        gpu.begin();
        manager.frame(Snapshot::default());
        assert!(
            Textures::raw(&handle).is_ok(),
            "cleanup must not unregister before the pass"
        );
        let target = gpu.render();
        let pixels = gpu.pixels(&target);
        assert!(
            pixels
                .chunks_exact(4)
                .any(|p| p[0] > 220 && p[1] < 30 && p[2] > 220),
            "texture emitted by failed A must survive current pass"
        );
        assert!(
            pixels
                .chunks_exact(4)
                .any(|p| p[0] < 30 && p[1] > 220 && p[2] < 30),
            "healthy B must render despite A's Begin/error"
        );
        manager.after_render();
        assert!(Textures::raw(&handle).is_err());
        assert_eq!(
            manager
                .entries()
                .into_iter()
                .find(|e| e.metadata.id == "a")
                .unwrap()
                .status,
            Status::Failed
        );
        assert!(
            manager
                .diagnostics()
                .iter()
                .any(|d| d.message.contains("primary-controlled"))
        );
        assert!(
            manager
                .diagnostics()
                .iter()
                .any(|d| d.message.contains("cleanup-controlled") && d.phase == "cleanup")
        );
        gpu.frame(&mut manager);
        assert_eq!(count(&manager, "a"), 2);
        assert_eq!(count(&manager, "b"), 3);
        let healthy_handle = lua(&manager, "b")
            .globals()
            .get::<AnyUserData>("texture")
            .unwrap();
        manager.queue(Command::Enable("b".into(), false));
        gpu.begin();
        manager.frame(Snapshot::default());
        assert!(
            Textures::raw(&healthy_handle).is_ok(),
            "normal disable retains its cached texture"
        );
        gpu.render();
        manager.after_render();
        assert!(Textures::raw(&healthy_handle).is_ok());
        let old_texture_id = Textures::raw(&healthy_handle).unwrap()._TexID;
        manager.queue(Command::Enable("b".into(), true));
        // Dear ImGui sizes a newly appearing window for one frame before
        // emitting its contents; warm it without replacing the cached handle.
        gpu.frame(&mut manager);
        gpu.begin();
        manager.frame(Snapshot::default());
        let target = gpu.render();
        let pixels = gpu.pixels(&target);
        manager.after_render();
        let resumed = lua(&manager, "b")
            .globals()
            .get::<AnyUserData>("texture")
            .unwrap();
        assert_eq!(
            resumed.to_pointer(),
            healthy_handle.to_pointer(),
            "reenable reuses same cached Lua userdata"
        );
        assert_eq!(Textures::raw(&resumed).unwrap()._TexID, old_texture_id);
        assert_eq!(count(&manager, "b"), 5);
        assert_eq!(
            manager
                .entries
                .iter()
                .find(|e| e.metadata.id == "b")
                .unwrap()
                .status,
            Status::Enabled
        );
        assert!(
            pixels
                .chunks_exact(4)
                .any(|p| p[0] < 30 && p[1] > 220 && p[2] < 30),
            "cached texture must render after reenable"
        );
        manager.shutdown();
        assert!(Textures::raw(&healthy_handle).is_err());
    }

    #[test]
    fn forge_manager_unchanged_pinned_hearts_real_png_lifecycle_and_hp_pixels() {
        use sha2::{Digest, Sha256};
        let Some(grid) = std::env::var_os("EQOA_FORGE_SPIKE") else {
            eprintln!("SKIP pinned hearts: EQOA_FORGE_SPIKE prerequisite missing");
            return;
        };
        let scripts_dir = Path::new(&grid).parent().unwrap();
        let source_bytes =
            fs::read(scripts_dir.join("retro_health_hearts.lua")).expect("pinned hearts source");
        let source_hash = format!("{:x}", Sha256::digest(&source_bytes));
        assert_eq!(
            source_hash, "ac5c5b7ab8a245dfedfa7414d8938ef0608546c7618db17b44425c2467fafe37",
            "unchanged audited source required"
        );
        let Some(mut gpu) = Gpu::new() else { return };
        let scripts = Scratch::new();
        let config = Scratch::new();
        scripts.package("a", true, std::str::from_utf8(&source_bytes).unwrap());
        // Unchanged legacy script uses UiForge.resources_path (shared root),
        // not a package-relative resource lookup.
        let dest = scripts.0.join("resources/retro_health_hearts");
        fs::create_dir_all(&dest).unwrap();
        for name in [
            "heart_accent.png",
            "heart_border.png",
            "heart_fill.png",
            "heart_highlight.png",
        ] {
            let bytes =
                fs::read(scripts_dir.join("resources/retro_health_hearts").join(name)).unwrap();
            eprintln!("pinned hearts {name} SHA256 {:x}", Sha256::digest(&bytes));
            fs::write(dest.join(name), bytes).unwrap();
        }
        assert_eq!(
            fs::read(scripts.0.join("a/main.lua")).unwrap(),
            source_bytes
        );
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        let frame = |gpu: &mut Gpu, m: &mut Manager, hp: Option<i32>| {
            let s = Snapshot {
                in_game: true,
                health: hp.map_or_else(super::super::health::HealthSnapshot::unavailable, |hp| {
                    super::super::health::HealthSnapshot::new(true, hp, 100)
                }),
                ..Default::default()
            };
            gpu.begin();
            m.frame(s);
            let target = gpu.render();
            let pixels = gpu.pixels(&target);
            m.after_render();
            assert_eq!(
                m.entries[0].status,
                Status::Enabled,
                "unchanged hearts failed: {:?}",
                m.diagnostics()
            );
            if let Some(hp) = hp {
                let name = match hp {
                    100 => "full",
                    50 => "partial",
                    0 => "death",
                    _ => "other",
                };
                let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../../target/forge-hearts-{name}.png"));
                let mut encoder = png::Encoder::new(fs::File::create(path).unwrap(), 256, 192);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .unwrap()
                    .write_image_data(&pixels)
                    .unwrap();
            }
            pixels
                .chunks_exact(4)
                .filter(|p| p[0] > 150 && p[1] < 80 && p[2] < 80)
                .count()
        };
        frame(&mut gpu, &mut manager, None);
        assert!(
            !manager.diagnostics().iter().any(|d| d.phase == "texture"),
            "real PNGs must load: {:?}",
            manager.diagnostics()
        );
        assert!(!manager.health_hidden());
        let retained = lua(&manager, "a");
        let state = retained
            .globals()
            .get::<Table>("retro_health_hearts_state")
            .unwrap();
        assert!(state.get::<bool>("initialized").unwrap());
        let handles: Vec<_> = [
            "heart_accent_texture",
            "heart_border_texture",
            "heart_fill_texture",
            "heart_highlight_texture",
        ]
        .into_iter()
        .map(|k| state.get::<AnyUserData>(k).unwrap())
        .collect();
        let ids: Vec<_> = handles
            .iter()
            .map(|h| Textures::raw(h).unwrap()._TexID)
            .collect();
        frame(&mut gpu, &mut manager, Some(100)); // warm up appearing window
        let full = frame(&mut gpu, &mut manager, Some(100));
        assert!(
            full > 100,
            "actual full-health fill pixels required: {full}"
        );
        assert!(
            manager.health_hidden(),
            "prelogin Initialize desired suppression promotes on readiness"
        );
        let partial = frame(&mut gpu, &mut manager, Some(50));
        let dead = frame(&mut gpu, &mut manager, Some(0));
        assert!(
            dead < partial && partial < full,
            "actual HP fill ordering: dead={dead} partial={partial} full={full}"
        );
        eprintln!(
            "pinned hearts pixels: full={full} partial={partial} death={dead}, source={source_hash}"
        );
        manager.queue(Command::Enable("a".into(), false));
        gpu.begin();
        manager.frame(Snapshot::default());
        gpu.render();
        manager.after_render();
        assert!(!manager.health_hidden());
        for (h, id) in handles.iter().zip(&ids) {
            assert_eq!(Textures::raw(h).unwrap()._TexID, *id);
        }
        manager.queue(Command::Enable("a".into(), true));
        frame(&mut gpu, &mut manager, Some(100));
        let resumed = frame(&mut gpu, &mut manager, Some(100));
        assert_eq!(
            resumed, full,
            "same cached four PNG handles render after reenable"
        );
        assert!(
            !manager.health_hidden(),
            "upstream OnDisable released label; ordinary Enable doesn't call Load"
        );
        let resumed_state = lua(&manager, "a")
            .globals()
            .get::<Table>("retro_health_hearts_state")
            .unwrap();
        assert_eq!(state.to_pointer(), resumed_state.to_pointer());
        manager.queue(Command::Load("a".into()));
        frame(&mut gpu, &mut manager, Some(100));
        assert!(
            manager.health_hidden(),
            "explicit profile Load reasserts upstream setting"
        );
        manager.shutdown();
        assert!(!manager.health_hidden());
        for h in &handles {
            assert!(Textures::raw(h).is_err());
        }
    }

    #[test]
    fn forge_manager_health_pending_cleanup_pause_reload_and_coherent_snapshot() {
        let Some(mut gpu) = Gpu::new() else { return };
        let scripts = Scratch::new();
        let config = Scratch::new();
        let source = r#"
            ui=require('frontiers_forge.ui');player=require('frontiers_forge.player');util=require('frontiers_forge.util')
            assert(util.IsInGame()==0 or player.GetMaxHp()>0)
            if not initialized then
                initialized=true;ui.DisableHealthBar('owner')
                UiForge.RegisterCallback(UiForge.CallbackType.DisableScript,function() ui.DisableHealthBar('cleanup');ui.EnableHealthBar('owner');if cleanup_throw then error('health cleanup fault') end end)
                UiForge.RegisterCallback(UiForge.CallbackType.Load,function() ui.DisableHealthBar('owner') end)
                UiForge.RegisterCallback(UiForge.CallbackType.Settings,function() ui.DisableHealthBar('inactive') end)
                UiForge.RegisterCallback(UiForge.CallbackType.Save,function() return {[1]=true} end)
                UiForge.RegisterCallback(UiForge.CallbackType.OnEject,function() ui.DisableHealthBar('eject');error('eject fault') end)
            end
            if throw then ui.DisableHealthBar('fault');error('health controlled') end
        "#;
        scripts.package("a", true, source);
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        let waiting = Snapshot {
            in_game: true,
            ..Default::default()
        };
        gpu.begin();
        manager.frame(waiting);
        gpu.render();
        manager.after_render();
        assert_eq!(
            manager.entries[0].status,
            Status::Enabled,
            "prelogin Initialize must execute successfully"
        );
        assert!(!manager.health_hidden());
        assert!(
            manager
                .health_readiness()
                .unwrap()
                .contains("current-world")
        );
        let ready = Snapshot {
            health: super::super::health::HealthSnapshot::new(true, 0, 100),
            ..waiting
        };
        gpu.begin();
        manager.frame(ready);
        gpu.render();
        manager.after_render();
        assert!(
            manager.health_hidden(),
            "pending Initialize promotes when ready"
        );
        manager.suspend_health();
        assert!(!manager.health_hidden());
        gpu.begin();
        manager.frame(ready);
        gpu.render();
        manager.after_render();
        assert!(manager.health_hidden());
        fs::write(scripts.0.join("a/main.lua"), "invalid Lua !!!").unwrap();
        manager.queue(Command::Reload("a".into()));
        gpu.begin();
        manager.frame(ready);
        gpu.render();
        manager.after_render();
        assert!(
            manager.health_hidden(),
            "failed compile keeps original claim"
        );
        fs::write(scripts.0.join("a/main.lua"), source).unwrap();
        manager.queue(Command::Enable("a".into(), false));
        gpu.begin();
        manager.frame(ready);
        gpu.render();
        manager.after_render();
        assert!(!manager.health_hidden());
        manager.queue(Command::Settings("a".into()));
        manager.queue(Command::Load("a".into()));
        gpu.begin();
        manager.frame(ready);
        gpu.render();
        manager.after_render();
        assert!(!manager.health_hidden());
        manager.queue(Command::Enable("a".into(), true));
        gpu.begin();
        manager.frame(ready);
        gpu.render();
        manager.after_render();
        assert!(
            !manager.health_hidden(),
            "plain reenable must not invent claim restoration"
        );
        manager.queue(Command::Load("a".into()));
        gpu.begin();
        manager.frame(ready);
        gpu.render();
        manager.after_render();
        assert!(manager.health_hidden());
        manager.queue(Command::Reload("a".into()));
        gpu.begin();
        manager.frame(ready);
        gpu.render();
        manager.after_render();
        assert!(!manager.health_hidden(), "fresh staged VM isn't active yet");
        gpu.begin();
        manager.frame(ready);
        gpu.render();
        manager.after_render();
        assert!(manager.health_hidden());
        lua(&manager, "a").globals().set("throw", true).unwrap();
        lua(&manager, "a")
            .globals()
            .set("cleanup_throw", true)
            .unwrap();
        gpu.begin();
        manager.frame(ready);
        gpu.render();
        manager.after_render();
        assert!(!manager.health_hidden());
        assert_eq!(manager.entries[0].status, Status::Failed);
        assert!(
            manager
                .diagnostics()
                .iter()
                .any(|d| d.phase == "cleanup" && d.message.contains("health cleanup fault"))
        );
        manager.shutdown();
        assert!(
            !manager.health_hidden(),
            "eject exception cannot retain claim"
        );
    }

    #[test]
    fn forge_manager_cleanup_and_inactive_callback_errors_force_retirement() {
        let Some(mut gpu) = Gpu::new() else { return };
        for phase in ["cleanup", "save", "load", "settings"] {
            let scripts = Scratch::new();
            let config = Scratch::new();
            scripts.package("a", true, r#"
                count=(count or 0)+1
                local function texture() return UiForge.CreateTextureFromMemory(string.char(255,0,0,255),1,1) end
                local function maybe_fail(phase)
                    if fail_phase==phase then bad_texture=texture();error('controlled-'..phase) end
                end
                if not texture_cache then
                    texture_cache=texture()
                    UiForge.RegisterCallback(UiForge.CallbackType.Save,function() maybe_fail('save');return {[1]=true} end)
                    UiForge.RegisterCallback(UiForge.CallbackType.Load,function() maybe_fail('load') end)
                    UiForge.RegisterCallback(UiForge.CallbackType.Settings,function() maybe_fail('settings') end)
                    UiForge.RegisterCallback(UiForge.CallbackType.DisableScript,function() maybe_fail('cleanup') end)
                end
            "#);
            let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
            gpu.frame(&mut manager);
            let retained = lua(&manager, "a");
            let cached = retained
                .globals()
                .get::<AnyUserData>("texture_cache")
                .unwrap();
            if phase == "cleanup" {
                retained.globals().set("fail_phase", phase).unwrap();
            }
            manager.queue(Command::Enable("a".into(), false));
            if phase != "cleanup" {
                gpu.frame(&mut manager);
                assert!(
                    Textures::raw(&cached).is_ok(),
                    "ordinary disable must retain before {phase} error"
                );
                retained.globals().set("fail_phase", phase).unwrap();
                manager.queue(match phase {
                    "save" => Command::Save("a".into()),
                    "load" => Command::Load("a".into()),
                    _ => Command::Settings("a".into()),
                });
            }
            gpu.begin();
            manager.frame(Snapshot::default());
            assert_eq!(
                manager.entries[0].status,
                Status::Failed,
                "{phase} failure requires reload"
            );
            let bad = retained
                .globals()
                .get::<AnyUserData>("bad_texture")
                .unwrap();
            assert!(Textures::raw(&cached).is_ok());
            assert!(
                Textures::raw(&bad).is_ok(),
                "terminal allocations survive recorded pass"
            );
            gpu.render();
            manager.after_render();
            assert!(Textures::raw(&cached).is_err());
            assert!(Textures::raw(&bad).is_err());
            manager.queue(Command::Enable("a".into(), true));
            gpu.frame(&mut manager);
            assert_eq!(manager.entries[0].status, Status::Failed);
            assert_eq!(
                count(&manager, "a"),
                1,
                "failed retained VM cannot be resumed"
            );
            manager.shutdown();
        }
    }

    #[test]
    fn forge_manager_reload_compile_failure_keeps_old_vm_and_rescan_retires_only_removed() {
        let Some(mut gpu) = Gpu::new() else {
            return;
        };
        let scripts = Scratch::new();
        let config = Scratch::new();
        scripts.package("a", true, SCRIPT);
        scripts.package("b", true, SCRIPT);
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        gpu.frame(&mut manager);
        lua(&manager, "a")
            .load("cached=UiForge.CreateTextureFromMemory(string.char(255,0,0,255),1,1)")
            .exec()
            .unwrap();
        // mlua userdata references are weak with respect to their Lua owner.
        // Keep the old VM alive in the test to inspect deferred GPU expiry.
        let old_lua = lua(&manager, "a");
        let old_handle = old_lua.globals().get::<AnyUserData>("cached").unwrap();
        fs::write(scripts.0.join("a/main.lua"), "this is not valid Lua !!!").unwrap();
        manager.queue(Command::Reload("a".into()));
        gpu.frame(&mut manager);
        assert_eq!(count(&manager, "a"), 2);
        assert_eq!(count(&manager, "b"), 2);
        assert!(
            Textures::raw(&old_handle).is_ok(),
            "syntax failure preserves old texture"
        );
        fs::write(scripts.0.join("a/main.lua"), SCRIPT).unwrap();
        fs::write(scripts.0.join("a/modules/private.lua"), "return {value=80}").unwrap();
        manager.queue(Command::Reload("a".into()));
        gpu.frame(&mut manager);
        gpu.frame(&mut manager);
        assert!(
            Textures::raw(&old_handle).is_err(),
            "successful replacement retires old bank"
        );
        assert_eq!(
            count(&manager, "a"),
            1,
            "replacement initializes on a subsequent frame"
        );
        assert_eq!(
            lua(&manager, "a")
                .globals()
                .get::<i32>("module_value")
                .unwrap(),
            81
        );
        manager.queue(Command::Enable("a".into(), false));
        gpu.frame(&mut manager);
        let disabled_lua = lua(&manager, "a");
        disabled_lua
            .load("cached=UiForge.CreateTextureFromMemory(string.char(255,0,0,255),1,1)")
            .exec()
            .unwrap();
        let disabled_handle = disabled_lua.globals().get::<AnyUserData>("cached").unwrap();
        fs::write(scripts.0.join("a/main.lua"), "invalid Lua !!!").unwrap();
        manager.queue(Command::Reload("a".into()));
        gpu.frame(&mut manager);
        assert!(
            Textures::raw(&disabled_handle).is_ok(),
            "syntax failure preserves disabled bank too"
        );
        fs::write(scripts.0.join("a/main.lua"), SCRIPT).unwrap();
        manager.queue(Command::Reload("a".into()));
        gpu.frame(&mut manager);
        assert!(
            Textures::raw(&disabled_handle).is_err(),
            "replacement must retire even an already-disabled bank"
        );
        assert!(
            !manager
                .entries()
                .into_iter()
                .find(|e| e.metadata.id == "a")
                .unwrap()
                .enabled_intent
        );
        fs::rename(scripts.0.join("a"), scripts.0.join(".removed-a")).unwrap();
        manager.queue(Command::Rescan);
        gpu.frame(&mut manager);
        assert!(manager.entries().iter().all(|e| e.metadata.id != "a"));
        assert_eq!(count(&manager, "b"), 8);
        scripts.package("c", true, "count=(count or 0)+1");
        manager.queue(Command::Rescan);
        gpu.frame(&mut manager);
        scripts.package("duplicate", true, "error('duplicate must not execute')");
        fs::write(scripts.0.join("duplicate/mod.json"),r#"{"schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":"b","entry":"main.lua","enabled":true}"#).unwrap();
        manager.queue(Command::Rescan);
        gpu.frame(&mut manager);
        assert!(manager.entries().iter().all(|e| e.metadata.id != "b"));
        assert_eq!(
            count(&manager, "c"),
            2,
            "duplicate invalidation must preserve the neighbor VM"
        );
        manager.shutdown();
    }

    #[test]
    fn forge_manager_disabled_defaults_missing_root_and_sixteen_vm_cap() {
        let Some(mut gpu) = Gpu::new() else {
            return;
        };
        let scripts = Scratch::new();
        let config = Scratch::new();
        let missing = gpu.manager(&scripts.0.join("absent"), Some(config.0.clone()));
        assert!(missing.entries().is_empty());
        fs::write(
            scripts.0.join("legacy.lua"),
            "error('must remain disabled')",
        )
        .unwrap();
        for n in 0..17 {
            scripts.package(&format!("n{n:02}"), true, "count=(count or 0)+1");
        }
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        assert_eq!(manager.entries().len(), 18);
        assert!(manager.entries.iter().all(|e| e.instance.is_none()));
        gpu.frame(&mut manager);
        assert_eq!(
            manager
                .entries
                .iter()
                .filter(|e| e.instance.is_some())
                .count(),
            MAX_INSTANCES
        );
        assert!(
            manager
                .diagnostics()
                .iter()
                .any(|d| d.message.contains("16"))
        );
        assert!(manager.diagnostics().len() <= MAX_DIAGNOSTICS);
        assert_eq!(
            manager
                .entries()
                .into_iter()
                .find(|e| e.metadata.id == "legacy")
                .unwrap()
                .status,
            Status::Disabled
        );
        manager.shutdown();
    }

    fn generate_pcm_wav(samples: usize) -> Vec<u8> {
        let data_size = (samples * 4) as u32;
        let file_size = 36 + data_size;
        let mut buf = Vec::with_capacity(44 + data_size as usize);
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&file_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes());
        buf.extend_from_slice(&44100u32.to_le_bytes());
        buf.extend_from_slice(&176400u32.to_le_bytes());
        buf.extend_from_slice(&4u16.to_le_bytes());
        buf.extend_from_slice(&16u16.to_le_bytes());
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        buf.resize(44 + data_size as usize, 0);
        buf
    }

    #[test]
    fn forge_manager_sound_lifecycle_disable_retains_handles_reenable_plays_and_terminal_closes() {
        let Some(mut gpu) = Gpu::new() else {
            return;
        };
        let scripts = Scratch::new();
        let config = Scratch::new();
        let res = scripts.0.join("a/resources");
        fs::create_dir_all(&res).unwrap();
        fs::write(res.join("sound.wav"), generate_pcm_wav(8)).unwrap();
        scripts.package(
            "a",
            true,
            r#"
            if not snd then
                snd = UiForge.LoadSound("sound.wav")
                assert(snd, "sound must load")
            end
            UiForge.PlaySound(snd)
        "#,
        );
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        gpu.frame(&mut manager);
        assert_eq!(manager.entries[0].status, Status::Enabled);
        let bank = manager.entries[0]
            .instance
            .as_ref()
            .unwrap()
            .sound_bank
            .clone();
        assert_eq!(bank.borrow().len(), 1);
        assert!(bank.borrow().is_active());
        let handles = bank.borrow().handles();

        manager.queue(Command::Enable("a".into(), false));
        gpu.frame(&mut manager);
        assert!(!bank.borrow().is_active(), "disable must deactivate bank");
        assert_eq!(bank.borrow().len(), 1, "disable must retain handles");

        manager.queue(Command::Enable("a".into(), true));
        gpu.frame(&mut manager);
        assert!(bank.borrow().is_active(), "re-enable must activate bank");
        assert_eq!(
            bank.borrow().handles(),
            handles,
            "re-enable must reuse handles"
        );

        manager.shutdown();
        assert_eq!(bank.borrow().len(), 0, "shutdown must close aliases");
    }

    #[test]
    fn forge_manager_sound_inactive_callback_playback_blocked() {
        let Some(mut gpu) = Gpu::new() else {
            return;
        };
        let scripts = Scratch::new();
        let config = Scratch::new();
        let res = scripts.0.join("a/resources");
        fs::create_dir_all(&res).unwrap();
        fs::write(res.join("sound.wav"), generate_pcm_wav(8)).unwrap();
        scripts.package(
            "a",
            true,
            r#"
            if not snd then
                snd = UiForge.LoadSound("sound.wav")
                assert(snd)
            end
            UiForge.RegisterCallback(UiForge.CallbackType.DisableScript, function()
                played_while_disabled = UiForge.PlaySound(snd)
            end)
        "#,
        );
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        gpu.frame(&mut manager);
        manager.queue(Command::Enable("a".into(), false));
        gpu.frame(&mut manager);
        let retained = lua(&manager, "a");
        assert!(
            !retained
                .globals()
                .get::<bool>("played_while_disabled")
                .unwrap(),
            "playback must be rejected while disabled"
        );
    }

    #[test]
    fn forge_manager_sound_failed_reload_preserves_old_bank() {
        let Some(mut gpu) = Gpu::new() else {
            return;
        };
        let scripts = Scratch::new();
        let config = Scratch::new();
        let res = scripts.0.join("a/resources");
        fs::create_dir_all(&res).unwrap();
        fs::write(res.join("sound.wav"), generate_pcm_wav(8)).unwrap();
        scripts.package(
            "a",
            true,
            r#"
            if not snd then
                snd = UiForge.LoadSound("sound.wav")
                assert(snd)
            end
        "#,
        );
        let mut manager = gpu.manager(&scripts.0, Some(config.0.clone()));
        gpu.frame(&mut manager);
        let bank = manager.entries[0]
            .instance
            .as_ref()
            .unwrap()
            .sound_bank
            .clone();
        let old_handles = bank.borrow().handles();

        fs::write(scripts.0.join("a/main.lua"), "this is not valid Lua !!!").unwrap();
        manager.queue(Command::Reload("a".into()));
        gpu.frame(&mut manager);
        assert_eq!(
            bank.borrow().handles(),
            old_handles,
            "failed reload must preserve old bank"
        );

        fs::write(scripts.0.join("a/main.lua"), "count=(count or 0)+1").unwrap();
        manager.queue(Command::Reload("a".into()));
        gpu.frame(&mut manager);
        gpu.frame(&mut manager);
        assert!(
            manager.entries[0]
                .instance
                .as_ref()
                .map_or(true, |i| { i.sound_bank.borrow().handles() != old_handles }),
            "successful replacement must use a new bank"
        );
        assert_eq!(
            bank.borrow().len(),
            0,
            "successful replacement must close aliases in the old bank"
        );
        assert!(
            bank.borrow().is_empty(),
            "old bank must report empty after replacement closes aliases"
        );
    }
}
