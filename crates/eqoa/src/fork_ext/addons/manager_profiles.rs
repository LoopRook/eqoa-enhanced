// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see LICENSE). LoopRook's additions first published in 0.4.6 or later are under LICENSE-ENHANCED; upstream and third-party code stays under its own license.
//! Profile transitions execute outside NewFrame, at the manager's render boundary.
use super::super::named_profiles::{Character, CharacterSnapshot, DEFAULT, Document, Store};
use super::*;
use dear_imgui_rs::{Context, Ui};
use std::time::{Duration, Instant};

const SWITCH_RETRY_DELAY: Duration = Duration::from_secs(2);

pub(crate) enum ProfileAction {
    Load(String),
    Revert,
    CopyCharacter(String),
    SetupClose,
    SetClassDefault(String),
    Save,
    SaveAs(String),
    BindCharacter,
    BindClass,
    Delete(String),
}
pub(super) struct NamedSession {
    pub data: Document,
    pub active: String,
    pub dirty: bool,
    writable: bool,
    shutdown_save_failed: bool,
    store: Store,
    character: Option<Character>,
    retry: Option<(Character, Instant)>,
    apply_initial: bool,
    waiting_world: bool,
    actions: VecDeque<ProfileAction>,
    name_input: String,
    selected: Option<String>,
    setup: Option<Setup>,
}
struct Setup {
    preselected: String,
    selected: String,
}
impl NamedSession {
    fn current_character(&self) -> Option<&Character> {
        self.retry
            .as_ref()
            .map(|(c, _)| c)
            .or(self.character.as_ref())
    }
}
pub(crate) fn class_display(class: u8) -> String {
    match crate::fork_ext::addons::player::CLASSES
        .get(class as usize)
        .copied()
    {
        Some(name) => name.to_string(),
        None => format!("class {class}"),
    }
}

pub(crate) fn class_default_button_label(character: Option<&Character>) -> String {
    match character.and_then(|c| {
        crate::fork_ext::addons::player::CLASSES
            .get(c.class as usize)
            .copied()
    }) {
        Some(name) => format!("Use as {name} default"),
        None => "Use as class default".to_string(),
    }
}

impl Manager {
    pub fn enable_profiles(&mut self, config: Option<PathBuf>) {
        let store = Store::new(config);
        let addons = self
            .entries
            .iter()
            .map(|e| e.metadata.clone())
            .collect::<Vec<_>>();
        let (data, writable) = match store.migrate(&self.storage, &addons).and_then(|data| {
            store.seed_roles()?;
            Ok(data)
        }) {
            Ok(data) => (data, true),
            Err(e) => {
                self.diagnostic(
                    None,
                    "profiles",
                    format!("Profiles could not be loaded: {e}. The original files were kept."),
                );
                (Document::empty(), false)
            }
        };
        for entry in &mut self.entries {
            entry.enabled_intent = data.enabled.get(&entry.key).copied().unwrap_or(false);
        }
        self.named = Some(NamedSession {
            data,
            active: DEFAULT.into(),
            dirty: false,
            writable,
            shutdown_save_failed: false,
            store,
            character: None,
            retry: None,
            apply_initial: true,
            waiting_world: false,
            actions: VecDeque::new(),
            name_input: String::new(),
            selected: None,
            setup: None,
        });
    }
    pub(super) fn profile_waiting_world(&self) -> bool {
        self.named.as_ref().is_some_and(|s| s.waiting_world)
    }
    fn log_profile(s: &NamedSession, event: &str) {
        // Only validated character/server identity reaches this boundary. No
        // session, account, password, argv, or raw game snapshot is logged.
        let identity = s.retry.as_ref().map(|(c, _)| c).or(s.character.as_ref());
        let (server, character) = identity
            .map(|c| (c.server.as_str(), c.name.as_str()))
            .unwrap_or(("none", "none"));
        crate::say(&format!(
            "Mod profiles: {event}; server={server}; character={character}; active={}; enabled={}/{}",
            s.active,
            s.data.enabled.values().filter(|v| **v).count(),
            s.data.enabled.len()
        ));
    }
    fn leave_profile_world(&mut self, s: &mut NamedSession, context: &mut Context) {
        if let Err(error) = Self::accept_setup_default(s) {
            self.diagnostic(None, "profiles", error);
        }
        // Save while the old world snapshot is still available, before cleanup
        // can change addon globals. Keep the selected profile at character select.
        let saved = match self.persist_profile(s, context) {
            Ok(()) => true,
            Err(e) => {
                self.diagnostic(None, "profiles", e);
                false
            }
        };
        let mut entries = std::mem::take(&mut self.entries);
        for entry in &mut entries {
            self.disable(entry, false, false, Disposition::Retain);
        }
        self.entries = entries;
        self.commands.clear();
        s.waiting_world = true;
        Self::log_profile(
            s,
            if saved {
                "logout saved; addon frames paused"
            } else {
                "logout save failed; addon frames paused"
            },
        );
        s.character = None;
        s.retry = None;
        // Discard clicks queued by the departing character's final frame.
        s.actions.clear();
    }
    fn accept_setup_default(s: &mut NamedSession) -> Result<(), String> {
        if let Some(setup) = &s.setup {
            let character = s
                .character
                .as_ref()
                .ok_or("Character setup is unavailable")?;
            s.store.bind(character, &setup.preselected, false)?;
            s.setup = None;
        }
        Ok(())
    }
    pub fn profile_name(&self) -> Option<&str> {
        self.named.as_ref().map(|s| s.active.as_str())
    }
    pub fn queue_profile(&mut self, action: ProfileAction) {
        if let Some(s) = &mut self.named {
            if s.actions.len() < 16 {
                s.actions.push_back(action);
            }
        }
    }
    fn capture_profile_states(&self, previous: &Document) -> Result<Document, String> {
        let mut data = previous.clone();
        for entry in &self.entries {
            data.enabled.insert(entry.key.clone(), entry.enabled_intent);
            if entry.status == Status::Failed {
                continue;
            }
            if let Some(i) = entry.instance.as_ref().filter(|i| i.initialized) {
                let value = Self::invoke(i, 2, Value::Nil, false)
                    .map_err(|_| "An addon Save callback failed; profile was not saved")?;
                data.addons.insert(
                    entry.key.clone(),
                    super::super::profile::encode(value).map_err(
                        |_| "An addon returned invalid Save data; profile was not saved",
                    )?,
                );
            }
        }
        Ok(data)
    }
    fn capture_profile(
        &mut self,
        s: &mut NamedSession,
        context: &mut Context,
    ) -> Result<(), String> {
        let mut data = self.capture_profile_states(&s.data)?;
        let mut ini = String::new();
        context.save_ini_settings(&mut ini);
        data.layout = ini;
        s.data = data;
        s.dirty = true;
        Ok(())
    }
    pub(super) fn capture_shutdown_profile(&mut self) {
        let Some(mut s) = self.named.take() else {
            return;
        };
        if let Err(error) = Self::accept_setup_default(&mut s) {
            s.shutdown_save_failed = true;
            self.diagnostic(None, "profiles", format!("Shutdown: {error}"));
        }
        if s.shutdown_save_failed {
            self.named = Some(s);
            return;
        }
        match self.capture_profile_states(&s.data) {
            Ok(data) => {
                s.data = data;
                s.dirty = true;
            }
            Err(error) => {
                // Runtime force-flushes the layout after cleanup. It must not
                // publish anything, including enabled/layout edits, on failure.
                s.shutdown_save_failed = true;
                self.diagnostic(None, "profiles", format!("Shutdown: {error}"));
            }
        }
        self.named = Some(s);
    }
    fn persist_profile(
        &mut self,
        s: &mut NamedSession,
        context: &mut Context,
    ) -> Result<(), String> {
        if !s.writable {
            return Err("Original profile settings could not be read; load a valid profile or Save as a new name".into());
        }
        if s.setup.is_some() {
            return Ok(());
        } // Preview is not published before a choice.
        self.capture_profile(s, context)?;
        s.store.write(&s.active, &s.data)?;
        s.dirty = false;
        Ok(())
    }
    fn apply_profile_layout(context: &mut Context, layout: &str) {
        // No frame or borrowed Ui exists here. Remove previous saved settings.
        context.binding().with_bound_context(|| unsafe {
            use dear_imgui_rs::sys;
            sys::igClearIniSettings();
            // ClearIniSettings removes saved records but leaves live windows.
            // Reset those too before applying the destination, including Once /
            // FirstUseEver conditions, so absent windows behave like a fresh load.
            let raw = sys::igGetCurrentContext();
            sys::igClearActiveID();
            if (*raw).OpenPopupStack.Size > 0 {
                sys::igClosePopupToLevel(0, false);
            }
            let conditions = sys::ImGuiCond_Always
                | sys::ImGuiCond_Once
                | sys::ImGuiCond_FirstUseEver
                | sys::ImGuiCond_Appearing;
            for index in 0..(*raw).Windows.Size {
                let window = *(*raw).Windows.Data.add(index as usize);
                // Retained windows that this profile never opens must not be
                // serialized by ImGui's WriteAll. Begin overwrites Flags when
                // a new-profile addon actually opens its window.
                (*window).Flags |= sys::ImGuiWindowFlags_NoSavedSettings;
                sys::igSetWindowPos_WindowPtr(
                    window,
                    sys::ImVec2_c { x: 60., y: 60. },
                    sys::ImGuiCond_Always,
                );
                sys::igSetWindowSize_WindowPtr(
                    window,
                    sys::ImVec2_c { x: 0., y: 0. },
                    sys::ImGuiCond_Always,
                );
                sys::igSetWindowCollapsed_WindowPtr(window, false, sys::ImGuiCond_Always);
                (*window).set_SetWindowPosAllowFlags(conditions);
                (*window).set_SetWindowSizeAllowFlags(conditions);
                (*window).set_SetWindowCollapsedAllowFlags(conditions);
                (*window).WantCollapseToggle = false;
                (*window).Scroll = sys::ImVec2_c { x: 0., y: 0. };
                (*window).ScrollTarget = sys::ImVec2_c {
                    x: f32::MAX,
                    y: f32::MAX,
                };
                (*window).LastFrameActive = -1;
            }
        });
        // ImGui interprets size=0 as strlen(data); Rust's empty str pointer is
        // not a NUL-terminated C string. A newline is an empty, explicit-size ini.
        context.load_ini_settings(if layout.is_empty() { "\n" } else { layout });
        unsafe {
            (*context.as_raw()).IO.WantSaveIniSettings = false;
        }
    }
    fn replace_profile(
        &mut self,
        s: &mut NamedSession,
        name: String,
        data: Document,
        context: &mut Context,
    ) {
        let mut entries = std::mem::take(&mut self.entries);
        for entry in &mut entries {
            self.disable(entry, false, false, Disposition::Retire);
            if let Some(i) = entry
                .instance
                .as_mut()
                .filter(|i| i.initialized && !i.ejected)
            {
                i.ejected = true;
                if Self::invoke(i, 4, Value::Nil, false).is_err() {
                    self.diagnostic(
                        Some(&entry.metadata.id),
                        "profiles",
                        "An addon cleanup callback failed during profile switch".into(),
                    );
                }
            }
            entry.instance = None;
            entry.enabled_intent = data.enabled.get(&entry.key).copied().unwrap_or(false);
            entry.status = Status::Disabled;
            entry.not_before = self.frame_number + 1;
        }
        self.entries = entries;
        self.commands.clear();
        self.after_render();
        Self::apply_profile_layout(context, &data.layout);
        s.selected = Some(name.clone());
        s.active = name;
        s.setup = None;
        s.data = data;
        s.dirty = false;
        s.writable = true;
    }
    fn load_named(
        &mut self,
        s: &mut NamedSession,
        name: String,
        context: &mut Context,
        revert: bool,
        assign: bool,
    ) -> Result<(), String> {
        let character = if assign {
            s.current_character().cloned()
        } else {
            None
        };
        if s.writable && name.eq_ignore_ascii_case(&s.active) && !revert {
            if let Some(character) = &character {
                s.store.bind(character, &name, false)?;
                s.character = Some(character.clone());
                s.retry = None;
                s.waiting_world = false;
                s.setup = None;
            }
            return Ok(());
        }
        // Validate the entire destination before touching the outgoing session.
        let data = s.store.read(&name)?;
        if s.writable && !s.waiting_world && !name.eq_ignore_ascii_case(&s.active) {
            if let Err(error) = self.persist_profile(s, context) {
                if s.store.read(&s.active).is_ok() {
                    return Err(error);
                }
                self.diagnostic(
                    None,
                    "profiles",
                    format!("Previous profile was not saved: {error}"),
                );
            }
        }
        if let Some(character) = &character {
            s.store.bind(character, &name, false)?;
        }
        self.replace_profile(s, name, data, context);
        if let Some(character) = character {
            s.character = Some(character);
            s.retry = None;
            s.waiting_world = false;
        }
        Ok(())
    }
    pub fn profile_boundary(
        &mut self,
        context: &mut Context,
        character: Option<CharacterSnapshot>,
        server: &str,
    ) {
        if self.awaiting_render {
            self.diagnostic(
                None,
                "profiles",
                "Profile switch must wait for the render boundary".into(),
            );
            return;
        }
        let Some(mut s) = self.named.take() else {
            return;
        };
        if s.apply_initial {
            Self::apply_profile_layout(context, &s.data.layout);
            s.apply_initial = false;
        }
        let identity = character.map(|c| Character::new(server, c)).transpose();
        match identity {
            Err(e) => self.diagnostic(None, "profiles", e),
            Ok(identity) if identity != s.character => {
                if let Some(character) = &identity {
                    let now = Instant::now();
                    let retry = s.retry.as_ref().filter(|(c, _)| c == character);
                    let already_reported = retry.is_some();
                    if retry.is_none_or(|(_, deadline)| now >= *deadline) {
                        if s.character.is_some() && !s.waiting_world {
                            self.leave_profile_world(&mut s, context);
                        }
                        // A new world is a new addon session even when two
                        // characters share Default/the same assigned profile.
                        // Failed loads keep old-character addon frames paused.
                        s.waiting_world = true;
                        let result = s.store.world_choice(character).and_then(|choice| {
                            if choice.1 {
                                s.store.seed_roles()?;
                            }
                            let (name, needs_setup) = s.store.world_choice(character)?;
                            self.load_named(&mut s, name, context, true, false)?;
                            Ok(needs_setup)
                        });
                        match result {
                            Ok(needs_setup) => {
                                s.character = identity;
                                if needs_setup {
                                    s.setup = Some(Setup {
                                        preselected: s.active.clone(),
                                        selected: s.active.clone(),
                                    });
                                }
                                s.retry = None;
                                s.waiting_world = false;
                                Self::log_profile(
                                    &s,
                                    if needs_setup {
                                        "world entry preview loaded; waiting for Use or separate copy"
                                    } else {
                                        "world entry loaded fresh assigned profile"
                                    },
                                );
                            }
                            Err(e) => {
                                if !already_reported {
                                    self.diagnostic(
                                        None,
                                        "profiles",
                                        format!("Could not switch profile: {e}. Retrying shortly."),
                                    );
                                }
                                s.retry = Some((character.clone(), now + SWITCH_RETRY_DELAY));
                                if !already_reported {
                                    Self::log_profile(&s, "world entry failed; retry pending");
                                }
                            }
                        }
                    }
                } else {
                    self.leave_profile_world(&mut s, context);
                }
            }
            Ok(_) => s.retry = None,
        }
        while let Some(action) = s.actions.pop_front() {
            let result = match action {
                ProfileAction::Load(name) => self
                    .load_named(&mut s, name, context, false, true)
                    .map(|()| {
                        Self::log_profile(&s, "profile selected");
                    }),
                ProfileAction::Revert => {
                    let name = s.active.clone();
                    let setup = s.setup.take();
                    let result = self.load_named(&mut s, name, context, true, false);
                    s.setup = setup;
                    result
                }
                ProfileAction::SetupClose => Self::accept_setup_default(&mut s),
                ProfileAction::CopyCharacter(source) => (|| {
                    let character = s
                        .current_character()
                        .cloned()
                        .ok_or("Enter a character to copy a profile")?;
                    s.store.read(&source)?;
                    if s.writable && !s.waiting_world {
                        self.persist_profile(&mut s, context)?;
                    }
                    let name = if s.setup.is_some() {
                        s.store.copy_unassigned(&character, &source)?
                    } else {
                        s.store.copy_character(&character, &source)?
                    };
                    // We saved the outgoing session above. Avoid a second Save.
                    s.waiting_world = true;
                    self.load_named(&mut s, name, context, true, false)?;
                    s.character = Some(character);
                    s.retry = None;
                    s.waiting_world = false;
                    Self::log_profile(&s, "separate character profile copied and assigned");
                    Ok(())
                })(),
                ProfileAction::SetClassDefault(name) => match s.current_character() {
                    Some(character) => s.store.bind(character, &name, true),
                    None => Err("Enter a character to choose its class default".into()),
                },
                ProfileAction::Save => self.persist_profile(&mut s, context).map(|()| {
                    Self::log_profile(&s, "active profile saved");
                }),
                ProfileAction::SaveAs(name) => (|| {
                    if !s.writable && name.eq_ignore_ascii_case(DEFAULT) {
                        return Err("Choose a new profile name; Default's original settings could not be read".into());
                    }
                    if s.writable && s.setup.is_none() {
                        self.persist_profile(&mut s, context)?;
                    } else {
                        self.capture_profile(&mut s, context)?;
                    }
                    s.store.create(&name, &s.data)?;
                    let character = s.current_character().cloned();
                    if let Some(character) = &character {
                        s.store.bind(character, &name, false)?;
                    }
                    let data = s.data.clone();
                    self.replace_profile(&mut s, name, data, context);
                    if let Some(character) = character {
                        s.character = Some(character);
                        s.retry = None;
                        s.waiting_world = false;
                    }
                    Self::log_profile(&s, "new profile saved");
                    Ok(())
                })(),
                ProfileAction::BindCharacter | ProfileAction::BindClass => {
                    let class = matches!(action, ProfileAction::BindClass);
                    match &s.character {
                        _ if s.retry.is_some() => Err(
                            "Wait for this character's profile to load before assigning it".into(),
                        ),
                        Some(c) => s.store.bind(c, &s.active, class).map(|()| {
                            Self::log_profile(
                                &s,
                                if class {
                                    "class default assigned"
                                } else {
                                    "character profile assigned"
                                },
                            );
                        }),
                        None => Err("Log in to set a character or class profile".into()),
                    }
                }
                ProfileAction::Delete(name) => s.store.delete(&name, &s.active),
            };
            if let Err(e) = result {
                self.diagnostic(None, "profiles", format!("Profile action failed: {e}"));
            }
        }
        self.named = Some(s);
    }
    pub fn flush_profile(&mut self, context: &mut Context, force: bool) {
        let Some(mut s) = self.named.take() else {
            return;
        };
        if s.shutdown_save_failed {
            self.named = Some(s);
            return;
        }
        if s.setup.is_some() {
            self.named = Some(s);
            return;
        }
        if s.dirty || force || context.io().want_save_ini_settings() {
            if !s.apply_initial {
                let mut ini = String::new();
                context.save_ini_settings(&mut ini);
                s.data.layout = ini;
            }
            let result = if s.writable {
                s.store.write(&s.active, &s.data)
            } else {
                Err("Unreadable original settings were preserved; use Save as a new name to recover".into())
            };
            if let Err(e) = result {
                self.diagnostic(None, "profiles", format!("Profile was not saved: {e}"));
            }
            s.dirty = false;
            unsafe {
                (*context.as_raw()).IO.WantSaveIniSettings = false;
            }
        }
        self.named = Some(s);
    }

    /// Always rendered in the world, independently of the Addon Manager toggle.
    pub fn profile_setup_controls(&mut self, ui: &Ui) {
        let Some(s) = &mut self.named else {
            return;
        };
        let Some(character) = s.character.as_ref() else {
            return;
        };
        let Some(setup) = &mut s.setup else {
            return;
        };
        let names = match s.store.names() {
            Ok(names) => names,
            Err(_) => return,
        };
        let mut opened = true;
        let display_name = character
            .name
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect::<String>() + &character.name[c.len_utf8()..])
            .unwrap_or_default();
        ui.window(format!(
            "Set up mods for {display_name}###Character mod setup"
        ))
        .opened(&mut opened)
        .size([440., 290.], dear_imgui_rs::Condition::FirstUseEver)
        .build(|| {
            ui.text("Start from:");
            ui.child_window("Setup profiles")
                .size([300., 130.])
                .build(ui, || {
                    for name in &names {
                        if ui
                            .selectable_config(name)
                            .selected(setup.selected.eq_ignore_ascii_case(name))
                            .build()
                        {
                            setup.selected = name.clone();
                        }
                    }
                });
            if ui.button(format!("Use {}", setup.selected)) {
                s.actions
                    .push_back(ProfileAction::Load(setup.selected.clone()));
            }
            // Use is the default keyboard action as well as the close action.
            ui.set_item_default_focus();
            if ui.is_window_focused_with_flags(dear_imgui_rs::FocusedFlags::ROOT_AND_CHILD_WINDOWS)
                && ui.is_key_pressed(dear_imgui_rs::Key::Enter)
            {
                s.actions
                    .push_back(ProfileAction::Load(setup.selected.clone()));
            }
            ui.same_line();
            if ui.button("Make a separate copy") {
                s.actions
                    .push_back(ProfileAction::CopyCharacter(setup.selected.clone()));
            }
            ui.text_wrapped(
                "Use shares settings and layout. A separate copy belongs to this character.",
            );
        });
        if !opened {
            s.actions.push_back(ProfileAction::SetupClose);
        }
    }
    pub fn profile_controls(&mut self, ui: &Ui) {
        let Some(s) = &mut self.named else {
            return;
        };
        let rows = match s.store.catalog(&s.active, s.current_character()) {
            Ok(rows) => rows,
            Err(error) => {
                ui.text(format!("Profile: {}", s.active));
                ui.text_wrapped(error);
                return;
            }
        };
        let active = rows.iter().find(|r| r.active);
        let role = if active.is_some_and(|r| r.characters > 1) {
            format!("shared with {} characters", active.unwrap().characters)
        } else if active.is_some_and(|r| r.assigned) {
            "this character's".into()
        } else {
            "setup preview".into()
        };
        ui.text(format!("Profile: {} ({role})", s.active));
        ui.text_wrapped("Choose a shared profile for this character, or make a separate copy. Each profile saves its mods, settings and window layout.");
        ui.child_window("Profiles list")
            .size([0., 128.])
            .build(ui, || {
                for row in &rows {
                    let mut label = row.name.clone();
                    if row.active {
                        label.push_str(" [active]");
                    }
                    if row.assigned {
                        label.push_str(" [this character's]");
                    }
                    if !row.class_defaults.is_empty() {
                        label.push_str(&format!(
                            " [default: {}]",
                            row.class_defaults
                                .iter()
                                .map(|c| class_display(*c))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    if ui
                        .selectable_config(format!("{label}##profile-row-{}", row.name))
                        .selected(
                            s.selected
                                .as_deref()
                                .is_some_and(|n| n.eq_ignore_ascii_case(&row.name)),
                        )
                        .build()
                    {
                        s.selected = Some(row.name.clone());
                    }
                }
            });
        let selected = s.selected.clone();
        {
            let _disabled =
                ui.begin_disabled_with_cond(selected.is_none() || s.current_character().is_none());
            if ui.button("Use for this character")
                && let Some(name) = selected.clone()
            {
                s.actions.push_back(ProfileAction::Load(name));
            }
            if ui.button("Copy into this character's profile")
                && let Some(name) = selected.clone()
            {
                s.actions.push_back(ProfileAction::CopyCharacter(name));
            }
            if ui.button(class_default_button_label(s.current_character()))
                && let Some(name) = selected.clone()
            {
                s.actions.push_back(ProfileAction::SetClassDefault(name));
            }
        }
        {
            let _disabled = ui.begin_disabled_with_cond(selected.as_deref().is_none_or(|n| {
                n.eq_ignore_ascii_case(&s.active) || n.eq_ignore_ascii_case(DEFAULT)
            }));
            if ui.button("Delete")
                && let Some(name) = selected
            {
                s.actions.push_back(ProfileAction::Delete(name));
                s.selected = None;
            }
        }
        ui.input_text("New profile name", &mut s.name_input).build();
        s.name_input = s.name_input.chars().take(48).collect();
        if ui.button("Save current as...") {
            s.actions
                .push_back(ProfileAction::SaveAs(s.name_input.clone()));
        }
        if ui.button("Save profile") {
            s.actions.push_back(ProfileAction::Save);
        }
        ui.same_line();
        if ui.button("Revert to saved") {
            s.actions.push_back(ProfileAction::Revert);
        }
        if s.retry.is_some() {
            ui.text_wrapped(
                "Character profile load failed; retrying shortly. See diagnostics below.",
            );
        }
        ui.separator();
    }
}
