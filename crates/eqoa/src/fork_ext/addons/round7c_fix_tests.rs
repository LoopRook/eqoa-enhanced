//! Round 7 Part C, fix round 1: each review finding reproduced by a test before its fix.
use super::discovery::{DiscoveredAddon, discover};
use super::host::{Host, Snapshot};
use serde_json::json;
use std::path::{Path, PathBuf};

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "eqoa-r7c-fix-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A native `eqoa/1` package `id` under `root` with `extra` manifest fields.
fn package(root: &Path, id: &str, extra: serde_json::Value) {
    let pkg = root.join(id);
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(pkg.join("main.lua"), "-- fixture").unwrap();
    let mut manifest = json!({
        "schemaVersion": 1, "runtime": "lua", "api": "eqoa/1", "id": id, "entry": "main.lua"
    });
    for (key, value) in extra.as_object().unwrap() {
        manifest[key] = value.clone();
    }
    std::fs::write(pkg.join("mod.json"), manifest.to_string()).unwrap();
}

fn one(root: &Path) -> DiscoveredAddon {
    let found = discover(root);
    assert!(found.diagnostics.is_empty(), "{:?}", found.diagnostics);
    assert_eq!(found.addons.len(), 1);
    found.addons.into_iter().next().unwrap()
}

fn lines(addon: &DiscoveredAddon) -> Vec<String> {
    super::runtime::disclosure_lines(addon)
}

/// The bundled mods as the client ships them.
fn bundled() -> Vec<DiscoveredAddon> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods");
    let found = discover(&root);
    assert!(found.diagnostics.is_empty(), "{:?}", found.diagnostics);
    found.addons
}

/// A snapshot in play with weapon set 2 ("Bow") ready, as the weapon-set fixture builds it.
fn weapon_set_snapshot() -> Snapshot {
    use super::weapon_sets::WeaponSetsSnapshot;
    let mut gear = crate::gear::Gear::default();
    gear.set(2, "Bow".into(), 7, u32::MAX);
    let inv = net::dump::ItemList {
        items: vec![net::dump::NetItem {
            item: net::dump::Item {
                id: 7,
                attack_type: 7,
                ..Default::default()
            },
            ..Default::default()
        }],
        ..Default::default()
    };
    let menus = crate::hudmenu::HudMenus::default();
    Snapshot {
        in_game: true,
        weapon_sets: WeaponSetsSnapshot::from_native(&gear, &inv, &menus, true, &Default::default(), 0),
        ..Default::default()
    }
}

// ---- H1 ----------------------------------------------------------------------------------------

#[test]
fn round7c_takeover_answers_unsupported_for_a_game_window_it_cannot_take_yet() {
    super::window_takeover::reset_all_for_test();
    let host = Host::with_test_capabilities(&["eqoa.windows.takeover"]).unwrap();
    host.execute(
        r#"
        for _, kind in ipairs({'inventory', 'spellbook', 'quest_log', 'response_dialog', 'map'}) do
            local ok, err = eqoa.windows.takeover(kind, {scope='all'})
            assert(ok == nil and err.code == 'unsupported', kind .. ': ' .. tostring(err and err.code))
            assert(err.message:find('window-replacement', 1, true), err.message)
            assert(eqoa.windows.taken(kind) ~= true)
        end
        for _, kind in ipairs({'hud.target', 'bags', 'journal', 'Loot', ''}) do
            local ok, err = eqoa.windows.takeover(kind)
            assert(ok == nil and err.code == 'invalid_argument', kind .. ': ' .. tostring(err and err.code))
        end
        assert(eqoa.windows.takeover('loot'))
        assert(eqoa.windows.release('loot'))
        "#,
    )
    .unwrap();
    super::window_takeover::reset_all_for_test();
}

#[test]
fn round7c_takeover_checks_view_scope_before_build_support() {
    super::window_takeover::reset_all_for_test();
    let host = Host::with_test_capabilities(&["eqoa.windows.takeover"]).unwrap();
    host.execute(
        r#"
        for _, opts in ipairs({{}, {scope='own'}}) do
            local ok, err = eqoa.windows.takeover('inventory', opts)
            assert(ok == nil and err.code == 'invalid_argument', 'own inventory: ' .. tostring(err and err.code))
        end
        local ok, err = eqoa.windows.takeover('inventory')
        assert(ok == nil and err.code == 'invalid_argument', 'default scope: ' .. tostring(err and err.code))
        ok, err = eqoa.windows.takeover('inventory', {scope='all'})
        assert(ok == nil and err.code == 'unsupported')
        ok, err = eqoa.windows.takeover('bogus', {scope='all'})
        assert(ok == nil and err.code == 'invalid_argument')
        ok, err = eqoa.windows.takeover('response_dialog')
        assert(ok == nil and err.code == 'unsupported', 'future server session')
        "#,
    )
    .unwrap();
}

#[test]
fn round7c_window_design_manifest_loads_and_discloses_hud_target() {
    let temp = TempDir::new("design-replaces");
    package(
        &temp.0,
        "design_example",
        json!({
            "capabilities": ["eqoa.windows.takeover"],
            "replaces": ["inventory", "spellbook", "hud.target"]
        }),
    );
    let disclosure = lines(&one(&temp.0));
    assert_eq!(disclosure, [
        "Replaces game windows: Target frame",
        "Not replaceable in this version: Inventory, Abilities",
    ]);
}

#[test]
fn round7c_replacement_disclosure_requires_the_takeover_grant() {
    let temp = TempDir::new("missing-takeover-grant");
    package(&temp.0, "ungated_bank", json!({"replaces": ["bank"]}));
    let found = discover(&temp.0);
    assert_eq!(found.addons.len(), 1);
    let disclosure = lines(&found.addons[0]);
    assert!(!disclosure.iter().any(|line| line.starts_with("Replaces game windows")), "{disclosure:?}");
    assert!(found.diagnostics.iter().any(|row| row.message.contains("eqoa.windows.takeover")));
}

#[test]
fn round7c_replaces_accepts_game_windows_and_reports_any_other_name() {
    let temp = TempDir::new("replaces");
    package(
        &temp.0,
        "later_build",
        json!({"capabilities": ["eqoa.windows.takeover"], "replaces": ["bank", "inventory", "spellbook", "loot"]}),
    );
    // a kind this build cannot replace still loads (a mod written for a later build), but is not promised
    assert_eq!(
        lines(&one(&temp.0)),
        [
            "Replaces game windows: Bank, Loot",
            "Not replaceable in this version: Inventory, Abilities",
        ]
    );
    std::fs::remove_dir_all(temp.0.join("later_build")).unwrap();
    package(&temp.0, "views_only", json!({
        "capabilities": ["eqoa.windows.takeover"], "replaces": ["inventory"]
    }));
    assert_eq!(
        lines(&one(&temp.0)),
        ["Not replaceable in this version: Inventory"]
    );
    std::fs::remove_dir_all(temp.0.join("views_only")).unwrap();
    for unknown in ["hud.bogus", "bags", "journal"] {
        package(
            &temp.0,
            "typo",
            json!({"capabilities": ["eqoa.windows.takeover"], "replaces": ["bank", unknown]}),
        );
        let found = discover(&temp.0);
        assert!(found.addons.is_empty(), "{unknown} accepted");
        assert_eq!(found.diagnostics.len(), 1);
        let message = &found.diagnostics[0].message;
        assert!(
            message.contains("replaces") && message.contains(unknown) && message.contains("not a game window"),
            "{message}"
        );
        std::fs::remove_dir_all(temp.0.join("typo")).unwrap();
    }
}

// ---- H2 ----------------------------------------------------------------------------------------

#[test]
fn round7c_native_weapon_set_alias_passes_the_weapon_set_gate() {
    let temp = TempDir::new("weapon_gate");
    package(&temp.0, "no_list", json!({}));
    let no_list = one(&temp.0);
    for (caps, expected) in [
        (Some(&["eqoa.actions.weapon_set"][..]), "ok"),
        (Some(&["eqoa.actions.*"][..]), "ok"),
        (Some(&["eqoa.actions.chat"][..]), "permission_denied"),
        (None, "not_declared"),
    ] {
        let _ = super::weapon_sets::take_pending();
        // `None`: a native package whose mod.json has no capabilities list
        let host = match caps {
            Some(caps) => Host::with_test_capabilities(caps).unwrap(),
            None => Host::for_addon(&no_list).unwrap(),
        };
        host.snapshot.set(weapon_set_snapshot());
        host.execute(
            "local ok, err = eqoa.WeaponSets.Use(2); result = ok and 'ok' or (err and err.code) or tostring(ok)",
        )
        .unwrap();
        let result: String = host.lua.globals().get("result").unwrap();
        assert_eq!(result, expected, "{caps:?}");
        let pending = super::weapon_sets::take_pending();
        assert_eq!(pending.is_empty(), expected != "ok", "{caps:?}: {pending:?}");
    }
}

#[test]
fn round7c_native_chat_send_passes_the_chat_and_command_gates() {
    let temp = TempDir::new("chat_gate");
    for (caps, line, expected) in [
        (json!([]), "/sit", false),
        (json!(["eqoa.actions.chat"]), "/sit", false),
        (json!(["eqoa.actions.command"]), "/sit", true),
        (json!(["eqoa.actions.command"]), "hello", false),
        (json!(["eqoa.actions.chat"]), "hello", true),
        (json!(["eqoa.actions.*"]), "/sit", true),
    ] {
        let _ = std::fs::remove_dir_all(temp.0.join("chatter"));
        package(&temp.0, "chatter", json!({"capabilities": caps}));
        let addon = one(&temp.0);
        let host = Host::for_addon(&addon).unwrap();
        host.snapshot.set(Snapshot {
            in_game: true,
            ..Default::default()
        });
        let _ = super::chat::take_outgoing_chat();
        host.lua.globals().set("line", line).unwrap();
        host.execute("sent, why = require('frontiers_forge.chat').SendChatText(line)")
            .unwrap();
        let sent: bool = host.lua.globals().get("sent").unwrap();
        assert_eq!(sent, expected, "{caps} {line}");
        let outgoing = super::chat::take_outgoing_chat();
        assert_eq!(outgoing.is_empty(), !expected, "{caps} {line}: {outgoing:?}");
        if !expected {
            let why: String = host.lua.globals().get("why").unwrap();
            assert!(why.starts_with("permission_denied"), "{why}");
        }
    }
}

#[test]
fn round7c_forge_imports_are_labelled_and_keep_their_acting_calls() {
    let addons = bundled();
    let forge: Vec<_> = addons.iter().filter(|a| a.api.is_none()).collect();
    assert!(forge.len() >= 3, "Wereoxxs, ARPG UI and Modern UI are Forge imports");
    for addon in &forge {
        assert_eq!(
            lines(addon),
            ["Actions: can act on its own (chat (ordinary lines), command (slash lines), weapon_set)"],
            "{}",
            addon.id
        );
    }
    for addon in addons.iter().filter(|a| a.api.is_some()) {
        assert!(
            !lines(addon).iter().any(|l| l.contains("chat (ordinary lines), command (slash lines), weapon_set")),
            "{} is native; its label comes from its own capabilities",
            addon.id
        );
    }
    let wereoxxs = forge.iter().find(|a| a.id.contains("wereoxxs")).expect("bundled Wereoxxs");
    let host = Host::for_addon(wereoxxs).unwrap();
    host.snapshot.set(weapon_set_snapshot());
    let _ = super::weapon_sets::take_pending();
    let _ = super::chat::take_outgoing_chat();
    let _ = host.drain_logs().unwrap();
    host.execute(
        r#"
        assert(eqoa.WeaponSets.Use(2) == true)
        local Chat = require('frontiers_forge.chat')
        assert(Chat.SendChatText('/tell Bob Greetings!') == true)
        assert(Chat.SendChatText('hello', Chat.ChatMode.Say) == true)
        "#,
    )
    .unwrap();
    assert_eq!(super::weapon_sets::take_pending(), [2]);
    assert_eq!(
        super::chat::take_outgoing_chat(),
        ["/tell Bob Greetings!", "/say hello"]
    );
    let logs = host.drain_logs().unwrap();
    assert!(logs.is_empty(), "new diagnostics for a Forge import: {logs:?}");
}

// ---- M3 ----------------------------------------------------------------------------------------

#[test]
fn round7c_dialog_label_follows_every_action_that_can_answer_a_dialog() {
    let temp = TempDir::new("dialog_label");
    for (caps, labelled) in [
        (json!(["eqoa.actions.activate_row"]), true),
        (json!(["eqoa.actions.dialog_answer"]), true),
        (json!(["eqoa.actions.*"]), true),
        (json!(["eqoa.actions.select_row", "eqoa.actions.menu_command"]), false),
        (json!(["eqoa.actions.talk"]), false),
    ] {
        let _ = std::fs::remove_dir_all(temp.0.join("answers"));
        package(&temp.0, "answers", json!({"capabilities": caps}));
        let lines = lines(&one(&temp.0));
        assert_eq!(
            lines.iter().any(|l| l == "Can answer NPC dialogs"),
            labelled,
            "{caps}: {lines:?}"
        );
    }
}

// ---- M5 and L5 ---------------------------------------------------------------------------------

/// A native mod's host with the ImGui bridge installed, so every refusal the client registers is listed.
fn reference_host(body: impl FnOnce(&Host)) {
    let temp = TempDir::new("reference");
    package(&temp.0, "reference", json!({"capabilities": []}));
    let addon = one(&temp.0);
    let mut context = dear_imgui_rs::Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    let host = Host::for_addon(&addon).unwrap();
    let _bridge = super::bridge::Bridge::install(&host).unwrap();
    body(&host);
}

#[test]
fn round7c_no_planned_row_names_a_closed_round_or_part() {
    super::big_stack(|| {
        reference_host(|host| {
            host.execute(
                r#"
                local caps = eqoa.capabilities
                assert(caps.get('ImGui.BeginTooltip').status ~= 'unknown', 'bridge refusals are listed')
                local stale = {}
                for _, row in ipairs(caps.list('')) do
                    if row.status == 'not_yet' then
                        local r = row.reason or ''
                        local closed = false
                        for n = 1, 7 do closed = closed or r:find('round ' .. n, 1, true) ~= nil end
                        if closed or r:find('Part %u') or not (r:find('plan', 1, true) or r:find('upstream PR', 1, true)) then
                            stale[#stale + 1] = row.path .. ': ' .. r
                        end
                    end
                end
                assert(#stale == 0, #stale .. ' stale planned rows:\n' .. table.concat(stale, '\n'))
                "#,
            )
            .unwrap();
        });
    });
}

const INSPECT_REFUSALS: &str = r#"
                local function member(path)
                    local module, name = path:match('^(frontiers_forge%.[%w_]+)%.([%w_]+)$')
                    if module then
                        local ok, t = pcall(require, module)
                        return ok and type(t) == 'table' and rawget(t, name) or nil
                    end
                    -- eqoa's modules are reached through its own lookup; ImGui and the rest are plain tables
                    local get = path:find('^eqoa%.') and function(t, k) return t[k] end or rawget
                    local v = _G
                    for part in path:gmatch('[^.]+') do
                        if type(v) ~= 'table' then return nil end
                        local ok, next_v = pcall(get, v, part)
                        if not ok then return nil end
                        v = next_v
                    end
                    return v
                end
                local function pack(...) return {n = select('#', ...), ...} end
                -- Every refusing callable's return against the exact shape it was declared with: `native` is
                -- nil plus an Error whose code is the declared code and whose message is the described reason;
                -- the Forge shapes are pinned value by value (nil; nil, reason; nil, reason, nil).
                function inspect_refusals(invoke)
                local issues, checked, names = {}, 0, {}
                for _, row in ipairs(eqoa.capabilities.list('')) do
                    local fn = (row.status == 'not_yet' or row.status == 'unsupported')
                        and not row.path:find(':', 1, true) and member(row.path)
                    if type(fn) == 'function' then
                        checked = checked + 1
                        names[#names + 1] = row.path
                        local code, shape = round7c_declared_refusal(row.path)
                        local r = pack(invoke(row.path, fn))
                        local n, value, second, third = r.n - 1, r[2], r[3], r[4]
                        local function bad(what) issues[#issues + 1] = row.path .. ': ' .. what end
                        if not r[1] then
                            bad('raised ' .. tostring(value))
                        elseif not code then
                            bad('has no declared refusal')
                        elseif (row.status == 'not_yet') ~= (code == 'not_yet') then
                            bad('declared code ' .. code .. ' but status ' .. row.status)
                        elseif shape == 'native' then
                            if n ~= 2 or value ~= nil or type(second) ~= 'table' then
                                bad(n .. ' values, ' .. type(value) .. ', ' .. type(second) .. '; want nil and an Error')
                            elseif second.code ~= code then
                                bad('code ' .. tostring(second.code) .. ' but declared ' .. code)
                            elseif second.message ~= row.reason then
                                bad('message ' .. tostring(second.message) .. ' but reason ' .. tostring(row.reason))
                            end
                        elseif shape == 'forge_nil' then
                            if n ~= 1 or value ~= nil then bad(n .. ' values; want one nil') end
                        elseif shape == 'forge_reason' then
                            if n ~= 2 or value ~= nil or second ~= row.reason then
                                bad(n .. ' values, reason ' .. tostring(second) .. '; want nil, ' .. tostring(row.reason))
                            end
                        elseif shape == 'forge_reason_and_nil' then
                            if n ~= 3 or value ~= nil or second ~= row.reason or third ~= nil then
                                bad(n .. ' values, reason ' .. tostring(second) .. '; want nil, reason, nil')
                            end
                        else
                            bad('unknown shape ' .. tostring(shape))
                        end
                    end
                end
                return issues, checked, names
                end
"#;

/// Every callable that refuses on the reference host, by name: a refusal added, removed or turned into a real
/// binding changes this list on purpose.
const EXPECTED_REFUSING_CALLABLES: &str = "ImGui.BeginItemTooltip ImGui.BeginTooltip ImGui.EndTooltip \
ImGui.GetForegroundDrawList ImGui.GetMouseDragDelta ImGui.GetMousePos ImGui.GetMousePosOnOpeningCurrentPopup \
ImGui.GetMouseWheel ImGui.IsMouseClicked ImGui.IsMouseDoubleClicked ImGui.IsMouseDown ImGui.IsMousePosValid \
ImGui.IsMouseReleased ImGui.PopItemWidth ImGui.PopStyleVar ImGui.PushItemWidth ImGui.PushStyleVar \
ImGui.ResetMouseDragDelta ImGui.SetMouseCursor ImGui.SetMousePos ImGui.SetNextWindowFocus ImGui.SetWindowFocus \
eqoa.actions.buttons eqoa.actions.look eqoa.actions.mastery_buy eqoa.actions.mastery_refund eqoa.actions.move eqoa.fonts.default eqoa.fonts.game \
eqoa.fonts.load eqoa.fonts.release eqoa.hud.hidden_by eqoa.hud.hide eqoa.hud.show eqoa.icons.class \
eqoa.icons.disposition eqoa.icons.info eqoa.icons.resource eqoa.icons.ui eqoa.imgui.BeginItemTooltip \
eqoa.imgui.BeginTooltip eqoa.imgui.EndTooltip eqoa.imgui.GetForegroundDrawList eqoa.imgui.GetMouseDragDelta \
eqoa.imgui.GetMousePos eqoa.imgui.GetMousePosOnOpeningCurrentPopup eqoa.imgui.GetMouseWheel \
eqoa.imgui.IsMouseClicked eqoa.imgui.IsMouseDoubleClicked eqoa.imgui.IsMouseDown eqoa.imgui.IsMousePosValid \
eqoa.imgui.IsMouseReleased eqoa.imgui.PopItemWidth eqoa.imgui.PopStyleVar eqoa.imgui.PushItemWidth \
eqoa.imgui.PushStyleVar eqoa.imgui.ResetMouseDragDelta eqoa.imgui.SetMouseCursor eqoa.imgui.SetMousePos \
eqoa.imgui.SetNextWindowFocus eqoa.imgui.SetWindowFocus eqoa.map.images eqoa.map.load_image \
eqoa.sound.is_playing eqoa.sound.load eqoa.sound.play eqoa.sound.release eqoa.sound.set_volume eqoa.sound.stop \
eqoa.textures.load eqoa.textures.release eqoa.textures.rgba frontiers_forge.cave.Alloc frontiers_forge.cave.Free \
frontiers_forge.cave.Get frontiers_forge.cave.Info frontiers_forge.cave.IsRegionZero frontiers_forge.cave.Release \
frontiers_forge.cave.Reserve frontiers_forge.chat.CaptureChannelTypeFromGameWindow \
frontiers_forge.chat.DebugDumpFocusChain frontiers_forge.chat.GetChannelTableInfo \
frontiers_forge.combat.ReadForeignCave frontiers_forge.icon.GetTexture frontiers_forge.icon.GetUITexture \
frontiers_forge.quest.GetName frontiers_forge.quest.new frontiers_forge.util.utf16_to_utf8 \
frontiers_forge.util.utf8_to_utf16";

fn install_refusal_inspector(host: &Host) {
    let declared = host
        .lua
        .create_function(|_, path: String| {
            let declared = super::native::declared_refusal(&path);
            Ok((declared.map(|d| d.0), declared.map(|d| d.1)))
        })
        .unwrap();
    host.lua.globals().set("round7c_declared_refusal", declared).unwrap();
    host.execute(INSPECT_REFUSALS).unwrap();
}

#[test]
fn round7c_every_refusing_member_reports_its_status_through_its_call() {
    super::big_stack(|| {
        reference_host(|host| {
            install_refusal_inspector(host);
            host.execute(
                r#"
                local issues, checked, names = inspect_refusals(function(_, fn) return pcall(fn) end)
                table.sort(names)
                round7c_refusing_paths = table.concat(names, ' ')
                assert(#issues == 0, #issues .. ' refusal shape issues:\n' .. table.concat(issues, '\n'))
                round7c_checked = checked
                "#,
            ).unwrap();
            let checked: u32 = host.lua.globals().get("round7c_checked").unwrap();
            let paths: String = host.lua.globals().get("round7c_refusing_paths").unwrap();
            eprintln!("round7c refusing members checked through their call: {checked}");
            let expected: Vec<&str> = EXPECTED_REFUSING_CALLABLES.split_whitespace().collect();
            assert_eq!(expected.len(), 91, "the pinned list holds every refusing callable");
            assert_eq!(paths.split_whitespace().collect::<Vec<_>>(), expected, "refusing callables by name");
            assert_eq!(checked as usize, expected.len(), "refusing callables by count");
        });
    });
}

#[test]
fn round7c_refusal_checker_detects_wrong_codes_and_missing_values() {
    super::big_stack(|| {
        reference_host(|host| {
            install_refusal_inspector(host);
            host.execute(
                r#"
                local issues = inspect_refusals(function(path, fn)
                    if path == 'eqoa.actions.destroy' then
                        local _, second = fn()
                        return true, nil, {code='permission_denied', message=second.message}
                    end
                    if path == 'eqoa.map.images' then return true, nil end
                    if path == 'frontiers_forge.chat.CaptureChannelTypeFromGameWindow' then
                        local _, reason = fn()
                        return true, nil, reason
                    end
                    return pcall(fn)
                end)
                assert(#issues == 3, 'expected three malformed returns to be caught; found ' .. #issues)
                "#,
            ).unwrap();
        });
    });
}

/// Fix round 4 item 3: `eqoa.actions.input_text` is disclosed as what it is, on its own line.
#[test]
fn round7c_text_boxes_label_says_what_input_text_grants() {
    let temp = TempDir::new("text-boxes");
    package(&temp.0, "typist", json!({"capabilities": ["eqoa.actions.input_text", "eqoa.actions.chat"]}));
    assert_eq!(
        lines(&one(&temp.0)),
        [
            "Actions: can act on its own (chat (ordinary lines))",
            "Text boxes: can type into and submit the game's open text boxes",
        ]
    );
}
