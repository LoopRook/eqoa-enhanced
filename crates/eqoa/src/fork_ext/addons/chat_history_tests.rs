//! `eqoa.ChatHistory()` and the bundled Wereoxxs chat scroller.
use super::host::{Host, Snapshot};
use std::fs;
use std::path::{Path, PathBuf};

fn chat_history(total: i32, active: bool) -> crate::fork_ext::chat_history::ChatHistorySnapshot {
    crate::fork_ext::chat_history::ChatHistorySnapshot { active, total }
}

/// The API reports the frame's active flag and the lines scrolled since the
/// addon's previous call. A stock (unclaimed) chat reports nothing; the
/// baseline still follows the count so a later claim does not invent a jump.
#[test]
fn forge_chat_history_api_reports_active_and_step_deltas() {
    crate::fork_ext::chat_history::reset();
    let host = Host::new().unwrap();
    let set = |total, active| {
        host.snapshot.set(Snapshot {
            chat_history: chat_history(total, active),
            ..Default::default()
        });
    };

    set(0, true);
    host.execute("local h=eqoa.ChatHistory(); assert(h.active==true); assert(h.steps==0)")
        .unwrap();
    set(2, true);
    host.execute("local h=eqoa.ChatHistory(); assert(h.active==true); assert(h.steps==2)")
        .unwrap();
    set(2, true);
    host.execute("local h=eqoa.ChatHistory(); assert(h.steps==0)")
        .unwrap();

    // Negative control: without an addon-owned chat the API stays inactive
    // and drops the steps it saw.
    set(9, false);
    host.execute("local h=eqoa.ChatHistory(); assert(h.active==false); assert(h.steps==0)")
        .unwrap();
    set(12, false);
    host.execute("local h=eqoa.ChatHistory(); assert(h.steps==0)")
        .unwrap();
    set(13, true);
    host.execute("local h=eqoa.ChatHistory(); assert(h.steps==1)")
        .unwrap();

    // A host that missed frames gets a bounded jump, not the whole backlog.
    // The next call sees none of the excess: it was dropped, not replayed.
    set(100, true);
    host.execute("local h=eqoa.ChatHistory(); assert(h.steps==8)")
        .unwrap();
    set(100, true);
    host.execute("local h=eqoa.ChatHistory(); assert(h.steps==0)")
        .unwrap();
    crate::fork_ext::chat_history::reset();
}

fn lua_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            lua_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "lua") {
            out.push(path);
        }
    }
}

/// Lua source without comments or strings, so only real `ImGui.Name(` calls
/// are seen.
fn lua_code(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if c == '"' || c == '\'' {
            out.push(c);
            while let Some((_, d)) = chars.next() {
                out.push(d);
                if d == '\\' {
                    if let Some((_, escaped)) = chars.next() {
                        out.push(escaped);
                    }
                } else if d == c {
                    break;
                }
            }
            continue;
        }
        if c == '-' && matches!(chars.peek(), Some((_, '-'))) {
            chars.next();
            if matches!(chars.peek(), Some((_, '['))) {
                chars.next();
                let mut level = 0;
                while matches!(chars.peek(), Some((_, '='))) {
                    level += 1;
                    chars.next();
                }
                if matches!(chars.peek(), Some((_, '['))) {
                    chars.next();
                    while let Some((_, d)) = chars.next() {
                        if d == ']' {
                            let mut close = 0;
                            while matches!(chars.peek(), Some((_, '='))) {
                                close += 1;
                                chars.next();
                            }
                            if close == level && matches!(chars.peek(), Some((_, ']'))) {
                                chars.next();
                                break;
                            }
                        }
                    }
                    out.push('\n');
                    continue;
                }
            }
            while let Some((_, d)) = chars.next() {
                if d == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn imgui_calls(source: &str) -> Vec<String> {
    let code = lua_code(source);
    let mut names = Vec::new();
    let mut at = 0;
    while let Some(pos) = code[at..].find("ImGui.") {
        let start = at + pos + "ImGui.".len();
        let rest = &code[start..];
        let len = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .count();
        if len > 0 && rest[len..].trim_start().starts_with('(') {
            names.push(rest[..len].to_string());
        }
        at = start;
    }
    names.sort();
    names.dedup();
    names
}

/// The real bridge table, not a hand-made facade, must expose every ImGui
/// function the bundled Lua calls. A missing one disables the addon at runtime.
#[test]
fn bundled_lua_calls_only_imgui_symbols_the_real_bridge_exposes() {
    let host = Host::new().unwrap();
    let _bridge = super::bridge::Bridge::install(&host).unwrap();
    let imgui: mlua::Table = host.lua.globals().get("ImGui").unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods");
    let mut files = Vec::new();
    lua_files(&root, &mut files);
    files.sort();
    assert!(!files.is_empty(), "bundled-mods must contain Lua files");

    let mut missing = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).unwrap();
        for name in imgui_calls(&text) {
            if imgui
                .get::<Option<mlua::Value>>(name.as_str())
                .unwrap()
                .is_none()
            {
                missing.push(format!("{}: ImGui.{}", file.display(), name));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "bundled Lua calls ImGui symbols the bridge lacks:\n{}",
        missing.join("\n")
    );
}

#[derive(Default)]
struct BlitRecorder {
    blits: Vec<u32>,
}

impl vi::ui::Raster for BlitRecorder {
    fn blit(
        &mut self,
        id: u32,
        _flags: u32,
        _at: [i32; 2],
        _a: [i32; 2],
        _b: [i32; 2],
        _mode: i32,
        _alpha: f32,
    ) {
        self.blits.push(id);
    }
    fn stretch_blit(
        &mut self,
        id: u32,
        _flags: u32,
        _at: [i32; 2],
        _size: [i32; 2],
        _a: [i32; 2],
        _b: [i32; 2],
        _mode: i32,
        _alpha: f32,
    ) {
        self.blits.push(id);
    }
    fn fill_fan(&mut self, _color: [f32; 4], _corners: [[i32; 2]; 4]) {}
    fn blit_full(&mut self, id: u32, _flags: u32, _at: [i32; 2], _mode: i32, _alpha: f32) {
        self.blits.push(id);
    }
    fn draw_string(&mut self, _at: [i32; 2], _color: [f32; 4], _s: &[u16]) {}
    fn draw_centered_string(&mut self, _at: [i32; 2], _color: [f32; 4], _s: &[u16], _mode: u32) {}
    fn string_width(&self, _s: &[u16]) -> i32 {
        0
    }
    fn point_size(&self) -> i32 {
        12
    }
    fn viewport(&self) -> [i32; 4] {
        [0, 0, 639, 447]
    }
    fn set_viewport(&mut self, _v: [i32; 4]) {}
}

fn test_vui() -> vi::ui::Vui {
    let atlas = (0..252)
        .map(|_| vi::ui::UiTexEntry {
            uv1: [16, 16],
            ..Default::default()
        })
        .collect();
    vi::ui::Vui::new(Default::default(), Default::default(), atlas)
}

fn set_claims(elements: &[&str]) {
    use crate::fork_ext::addons::health::InstanceToken;
    use crate::fork_ext::addons::visibility::{HudClaimsRegistry, publish_claims};
    let mut reg = HudClaimsRegistry::new();
    let token = InstanceToken::next().unwrap();
    for el in elements {
        reg.disable(el, token, "test");
    }
    publish_claims(&reg);
}

fn clear_claims() {
    crate::fork_ext::addons::visibility::publish_claims(
        &crate::fork_ext::addons::visibility::HudClaimsRegistry::new(),
    );
}

/// Upstream draws the HUD carousel in modes 0/30/38/43 only. Mode 33 (R2/C
/// held to read the chat history) keeps it too, whether or not an addon owns
/// the chat window: the native history box sits below the bar.
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn forge_chat_history_keeps_ability_bar_in_mode_33_always() {
    use crate::hudmenu::HudMenus;

    let vui = test_vui();
    let mut menus = HudMenus::default();
    menus.build(
        &Default::default(),
        &Default::default(),
        &Default::default(),
        Default::default(),
    );
    menus.sel = 0;
    let play_condition = |mode: i32| {
        crate::hud::fork_hud::ability_bar_shown(matches!(mode, 0 | 30 | 38 | 43), mode)
    };
    let draw = |menus: &mut HudMenus| {
        let mut r = BlitRecorder::default();
        crate::hud::fork_hud::draw_hud_menus(menus, &vui, &mut r, 0, &|_, _| Vec::new());
        r.blits.len()
    };

    // Stock mode 33 is not in upstream's carousel mode list, so the fork adds
    // it for the native chat too.
    clear_claims();
    assert!(crate::hud::fork_hud::chat_history_keeps_ability_bar(33));
    assert!(play_condition(33), "stock mode 33 keeps the ability bar");
    assert!(
        draw(&mut menus) > 0,
        "the bar is drawable when nothing hides it"
    );

    // A claimed chat changes nothing here, and the bar's own claim still
    // hides it.
    set_claims(&["chat_window"]);
    assert!(crate::hud::fork_hud::chat_history_keeps_ability_bar(33));
    assert!(
        play_condition(33),
        "claimed chat keeps the ability bar in mode 33"
    );
    assert!(draw(&mut menus) > 0);
    set_claims(&["chat_window", "hotbar_draw_compact", "hotbar_draw_expanded"]);
    assert_eq!(
        draw(&mut menus),
        0,
        "a mod owning the bars still suppresses them"
    );

    // Negative controls: a menu (mode 1) still hides the bar, the other modes
    // are unchanged, and releasing the chat claim changes nothing.
    assert!(!crate::hud::fork_hud::chat_history_keeps_ability_bar(1));
    assert!(!play_condition(1), "a menu (mode 1) still hides the bar");
    assert!(!crate::hud::fork_hud::chat_history_keeps_ability_bar(0));
    assert!(!crate::hud::fork_hud::chat_history_keeps_ability_bar(30));
    clear_claims();
    assert!(
        play_condition(33),
        "releasing the chat claim keeps the bar up"
    );
}

// Exercise the bundled chat_window.lua with a small ImGui facade: the real
// module reads eqoa.ChatHistory, scrolls the messages child, and snaps back on
// release. Without eqoa (FrontiersForge) it keeps the stock behavior.
#[test]
fn forge_chat_history_wereoxxs_scrolls_and_snaps_to_bottom() {
    let lua = mlua::Lua::new();
    lua.load(
        r#"
        scroll_y = 200
        scroll_max = 200
        history_active = false
        history_steps = 0
        lines = 0
        Paint = { Rect = function() end }
        require = function(name) if name == 'paint' then return Paint end end
        ImGuiCol = { Text=0, Tab=1, TabDimmed=2, TabHovered=3, TabSelected=4,
                     TabDimmedSelected=5, FrameBg=6, FrameBgHovered=7, FrameBgActive=8 }
        ImGui = {
            BeginChild = function() end, EndChild = function() end,
            SetWindowFontScale = function() end,
            TextWrapped = function() lines = lines + 1 end,
            PushStyleColor = function() end, PopStyleColor = function() end,
            GetScrollY = function() return scroll_y end,
            SetScrollY = function(y) scroll_y = y end,
            GetScrollMaxY = function() return scroll_max end,
            SetScrollHereY = function(f) scroll_y = scroll_max * f end,
            GetTextLineHeightWithSpacing = function() return 20 end,
            GetWindowDrawList = function() return {} end,
            GetCursorScreenPos = function() return 0, 0 end,
            GetContentRegionAvail = function() return 100, 100 end,
            GetFrameHeight = function() return 10 end,
            SetNextItemWidth = function() end,
            InputTextWithHint = function(id, hint, text, flags) return text, false end,
            IsItemActive = function() return false end,
            SetKeyboardFocusHere = function() end,
            BeginTabBar = function() return true end, EndTabBar = function() end,
            BeginTabItem = function() return true end, EndTabItem = function() end,
        }
        "#,
    )
    .exec()
    .unwrap();
    let chat = lua
        .load(include_str!(
            "../../../../../bundled-mods/wereoxxs_ui/modules/chat_window.lua"
        ))
        .eval::<mlua::Table>()
        .unwrap();
    lua.globals().set("ChatWindow", chat).unwrap();
    lua.load(
        r#"
        state = ChatWindow.new()
        config = ChatWindow.DefaultConfig()
        state.messages = {}
        for i = 1, 40 do
            state.messages[i] = { text = "line " .. i, type_name = "Say", time = "12:00" }
        end
        state.type_order = {}
        eqoa = { ChatHistory = function() return { active = history_active, steps = history_steps } end }
        "#,
    )
    .exec()
    .unwrap();
    let draw = || {
        lua.load("ChatWindow.Draw(state, config, {})")
            .exec()
            .unwrap()
    };
    let scroll = || lua.globals().get::<f64>("scroll_y").unwrap();
    let set_history = |active: bool, steps: i64| {
        lua.globals().set("history_active", active).unwrap();
        lua.globals().set("history_steps", steps).unwrap();
    };

    // R2 up: one step is one line, from the bottom.
    set_history(true, 1);
    draw();
    assert_eq!(scroll(), 180.0, "one up step scrolls back one line");
    // Down scrolls forward again.
    set_history(true, -1);
    draw();
    assert_eq!(scroll(), 200.0, "a down step scrolls forward");
    // Several steps and the clamp.
    set_history(true, 5);
    draw();
    assert_eq!(scroll(), 100.0, "five steps scroll five lines");
    set_history(true, 1000);
    draw();
    assert_eq!(scroll(), 0.0, "clamped at the oldest line");

    // Negative control: an older bridge without these calls skips the scroll
    // instead of erroring (an error would disable the whole addon).
    lua.load(
        "ImGui.SetScrollY = nil; ImGui.GetTextLineHeightWithSpacing = nil; scroll_y = 100",
    )
    .exec()
    .unwrap();
    set_history(true, 3);
    draw();
    assert_eq!(scroll(), 100.0, "without the bridge calls the log stays put");
    lua.load(
        "ImGui.SetScrollY = function(y) scroll_y = y end; \
         ImGui.GetTextLineHeightWithSpacing = function() return 20 end",
    )
    .exec()
    .unwrap();

    // Release: jump to the newest line, once.
    set_history(false, 0);
    draw();
    assert_eq!(scroll(), 200.0, "releasing R2 snaps to the newest line");
    draw();
    assert_eq!(scroll(), 200.0, "and keeps sticking to the bottom");

    // Negative control: no eqoa (PCSX2/FrontiersForge) leaves the stock
    // behavior: no jump to the bottom while reading back, and no error.
    lua.load(
        r#"
        eqoa = nil
        old_state = ChatWindow.new()
        old_state.messages = state.messages
        old_state.type_order = {}
        scroll_y = 50
        "#,
    )
    .exec()
    .unwrap();
    lua.load("ChatWindow.Draw(old_state, config, {})")
        .exec()
        .unwrap();
    assert_eq!(scroll(), 50.0, "without eqoa the log stays where it was");
    lua.load("scroll_y = 200").exec().unwrap();
    lua.load("ChatWindow.Draw(old_state, config, {})")
        .exec()
        .unwrap();
    assert_eq!(
        scroll(),
        200.0,
        "without eqoa the stock stick-to-bottom remains"
    );
    assert!(
        lua.globals().get::<usize>("lines").unwrap() > 0,
        "the harness drew the message lines"
    );
}
