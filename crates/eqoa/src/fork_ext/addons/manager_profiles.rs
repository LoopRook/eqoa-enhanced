//! Profile transitions execute outside NewFrame, at the manager's render boundary.
use super::super::named_profiles::{Character, CharacterSnapshot, DEFAULT, Document, Store};
use super::*;
use dear_imgui_rs::{Context, Ui};
use std::time::{Duration, Instant};

const SWITCH_RETRY_DELAY: Duration = Duration::from_secs(2);

pub(crate) enum ProfileAction {
    Load(String),
    Revert,
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
    unassigned: bool,
    apply_initial: bool,
    actions: VecDeque<ProfileAction>,
    name_input: String,
}
impl Manager {
    pub fn enable_profiles(&mut self, config: Option<PathBuf>) {
        let store = Store::new(config);
        let addons = self
            .entries
            .iter()
            .map(|e| e.metadata.clone())
            .collect::<Vec<_>>();
        let (data, writable) = match store.migrate(&self.storage, &addons) {
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
            unassigned: false,
            apply_initial: true,
            actions: VecDeque::new(),
            name_input: String::new(),
        });
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
        s.active = name;
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
    ) -> Result<(), String> {
        if s.writable && name.eq_ignore_ascii_case(&s.active) && !revert {
            return Ok(());
        }
        // Validate the entire destination before touching the outgoing session.
        let data = s.store.read(&name)?;
        if s.writable && !name.eq_ignore_ascii_case(&s.active) {
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
        self.replace_profile(s, name, data, context);
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
                        let result = s.store.resolve(character).and_then(|(name, unassigned)| {
                            self.load_named(&mut s, name, context, false)?;
                            Ok(unassigned)
                        });
                        match result {
                            Ok(unassigned) => {
                                s.character = identity;
                                s.unassigned = unassigned;
                                s.retry = None;
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
                            }
                        }
                    }
                } else {
                    if let Err(e) = self.persist_profile(&mut s, context) {
                        self.diagnostic(None, "profiles", e);
                    }
                    s.unassigned = false;
                    s.character = None;
                    s.retry = None;
                }
            }
            Ok(_) => s.retry = None,
        }
        while let Some(action) = s.actions.pop_front() {
            let result = match action {
                ProfileAction::Load(name) => self.load_named(&mut s, name, context, false),
                ProfileAction::Revert => {
                    let name = s.active.clone();
                    self.load_named(&mut s, name, context, true)
                }
                ProfileAction::Save => self.persist_profile(&mut s, context),
                ProfileAction::SaveAs(name) => (|| {
                    if !s.writable && name.eq_ignore_ascii_case(DEFAULT) {
                        return Err("Choose a new profile name; Default's original settings could not be read".into());
                    }
                    self.capture_profile(&mut s, context)?;
                    s.store.create(&name, &s.data)?;
                    s.active = name;
                    s.dirty = false;
                    s.writable = true;
                    Ok(())
                })(),
                ProfileAction::BindCharacter | ProfileAction::BindClass => {
                    let class = matches!(action, ProfileAction::BindClass);
                    match &s.character {
                        _ if s.retry.is_some() => Err(
                            "Wait for this character's profile to load before assigning it".into(),
                        ),
                        Some(c) => s.store.bind(c, &s.active, class).map(|()| {
                            s.unassigned = false;
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
    pub fn profile_controls(&mut self, ui: &Ui) {
        let Some(s) = &mut self.named else {
            return;
        };
        ui.text(format!("Active profile: {}", s.active));
        ui.text_wrapped(
            "Each client uses its own selection. Clients sharing this profile use the last save.",
        );
        if s.unassigned {
            ui.text_wrapped("This character has no profile assignment. Pick a profile or save a new one below, then use it for this character.");
        }
        if let Some(c) = &s.character {
            ui.text(format!("Character: {} (class {})", c.name, c.class));
        }
        if s.retry.is_some() {
            ui.text_wrapped("This character's profile could not be loaded. Retrying shortly; see the diagnostic below.");
        }
        ui.input_text("New profile name", &mut s.name_input).build();
        if s.name_input.len() > 48 {
            s.name_input = s.name_input.chars().take(48).collect();
        }
        if ui.button("Save active profile") {
            s.actions.push_back(ProfileAction::Save);
        }
        if ui.button("Revert active profile to saved") {
            s.actions.push_back(ProfileAction::Revert);
        }
        ui.same_line();
        if ui.button("Save as new profile") {
            s.actions
                .push_back(ProfileAction::SaveAs(s.name_input.clone()));
        }
        if ui.button("Use for this character") {
            s.actions.push_back(ProfileAction::BindCharacter);
        }
        ui.same_line();
        if ui.button("Use as class default") {
            s.actions.push_back(ProfileAction::BindClass);
        }
        match s.store.names() {
            Ok(names) => {
                for name in names {
                    if ui.button(format!("Switch / load {name}##profile-{name}")) {
                        s.actions.push_back(ProfileAction::Load(name.clone()));
                    }
                    if !name.eq_ignore_ascii_case(&s.active) && !name.eq_ignore_ascii_case(DEFAULT)
                    {
                        ui.same_line();
                        if ui.button(format!("Delete##profile-{name}")) {
                            s.actions.push_back(ProfileAction::Delete(name));
                        }
                    }
                }
            }
            Err(_) => ui.text_wrapped("Profile folder is unavailable; see the diagnostics below."),
        }
        ui.separator();
    }
}
