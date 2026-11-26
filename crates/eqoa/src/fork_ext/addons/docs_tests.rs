// LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE).
//! Round 7 Part D2: the author documentation against the running client.
//!
//! * Every fenced `lua` block in `docs/modding/GETTING-STARTED.md`, and the template mod, runs in a
//!   real `eqoa/1` host (the production install path, a world published from a `Play`, an ImGui
//!   frame, no graphics adapter) with no error and no diagnostic.
//! * Every name the generated surface file calls supported is named in the spec, and every
//!   `eqoa.x.y` the hand-written pages name exists in the surface file.
//! * Every limit the spec calls tested is tied to the line of code that holds the number and to the
//!   test that covers it, and the spec calls a number tested nowhere else.
//!
//! What these tests do not show: that a described argument, default, result or error code is right
//! (those were read from the code), or that an example's handlers work when a key or a result
//! arrives (the fixture runs three frames; handlers that never fire are checked for names only).
//!
//! The pages are read from disk when the test runs, so a wording fix needs no rebuild.
use super::bridge::Bridge;
use super::discovery::{DiscoveredAddon, discover};
use super::host::Host;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn docs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/modding")
}

fn page(name: &str) -> String {
    let path = docs().join(name);
    // a checkout may give the pages either line ending
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .replace("\r\n", "\n")
}

/// The generated surface file: path -> status.
fn surface() -> BTreeMap<String, String> {
    let doc: serde_json::Value = serde_json::from_str(&page("eqoa-1-surface.json")).unwrap();
    doc["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["path"].as_str().unwrap().to_owned(),
                c["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The fenced blocks of `language` in a Markdown page, with the line each starts on.
fn fenced(text: &str, language: &str) -> Vec<(usize, String)> {
    let mut blocks = Vec::new();
    let mut open: Option<(usize, String)> = None;
    let mut wanted = false;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        match &mut open {
            None => {
                if let Some(tag) = trimmed.strip_prefix("```") {
                    wanted = tag.trim() == language;
                    open = Some((index + 2, String::new()));
                }
            }
            Some((_, body)) => {
                if trimmed.starts_with("```") {
                    let block = open.take().unwrap();
                    if wanted {
                        blocks.push(block);
                    }
                } else {
                    body.push_str(line);
                    body.push('\n');
                }
            }
        }
    }
    assert!(open.is_none(), "an unclosed fenced block");
    blocks
}

/// Every `eqoa.a.b...` path written in `text` (identifiers joined by single dots).
fn eqoa_paths(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut found = BTreeSet::new();
    let mut at = 0;
    while let Some(offset) = text[at..].find("eqoa.") {
        let start = at + offset;
        at = start + 5;
        // not the tail of a longer word (`my_eqoa.x`) or of a file name (`foo-eqoa.log`)
        if start > 0 && (word(bytes[start - 1]) || matches!(bytes[start - 1], b'-' | b'/' | b'\\')) {
            continue;
        }
        let mut end = start + 4;
        // each further segment: a dot, then an identifier that starts with a letter or `_`
        while end + 1 < bytes.len()
            && bytes[end] == b'.'
            && (bytes[end + 1].is_ascii_alphabetic() || bytes[end + 1] == b'_')
        {
            end += 1;
            while end < bytes.len() && word(bytes[end]) {
                end += 1;
            }
        }
        if end > start + 4 {
            found.insert(text[start..end].to_owned());
        }
    }
    found
}

fn has_word(text: &str, name: &str) -> bool {
    let bytes = text.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut at = 0;
    while let Some(offset) = text[at..].find(name) {
        let start = at + offset;
        let end = start + name.len();
        if (start == 0 || !word(bytes[start - 1])) && (end == bytes.len() || !word(bytes[end])) {
            return true;
        }
        at = start + 1;
    }
    false
}

struct Scratch(PathBuf);
impl Scratch {
    fn new(tag: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "eqoa-docs-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn only_package(root: &Path) -> DiscoveredAddon {
    let found = discover(root);
    assert!(found.diagnostics.is_empty(), "{:?}", found.diagnostics);
    assert_eq!(found.addons.len(), 1, "one package under {}", root.display());
    found.addons.into_iter().next().unwrap()
}

fn imgui_context() -> dear_imgui_rs::Context {
    let mut context = dear_imgui_rs::Context::create();
    context.set_ini_filename::<PathBuf>(None).unwrap();
    context.io_mut().set_display_size([640.0, 480.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.font_atlas().try_claim_legacy_renderer().unwrap().build();
    context
}

/// Loads `addon` as the manager does (`Manager::prepare`'s install order, without the services that need a
/// graphics device or a sound device), in a world published from a `Play`, and runs it for three frames:
/// the entry, `enabled`, `world.entered`, `state.reset`, `render` and whatever it queued. Returns the first
/// Lua error, or every diagnostic the frames left.
fn run_package(addon: &DiscoveredAddon, data: PathBuf) -> Result<Vec<String>, String> {
    let play = crate::play::Play::bundled_ui_fixture();
    let walker = crate::play::Play::bundled_ui_walker();
    super::hud_layout::set_window_size(640, 448);

    let host = Host::for_addon(addon).map_err(|e| e.to_string())?;
    super::file_io::install(&host, addon, Some(data)).map_err(|e| e.to_string())?;
    let visibility = super::visibility::VisibilityLease::with_hud_claims(
        std::rc::Rc::new(std::cell::RefCell::new(super::health::HealthBarClaimsRegistry::new())),
        std::rc::Rc::new(std::cell::RefCell::new(super::visibility::HudClaimsRegistry::new())),
    )
    .map_err(|e| e.to_string())?;
    super::visibility::install_managed_health_adapter(&host, &visibility).map_err(|e| e.to_string())?;
    super::player::install_player_adapter(&host).map_err(|e| e.to_string())?;
    let entry = host.compile_addon_entry(addon).map_err(|e| e.to_string())?;
    let bridge = Bridge::install(&host).map_err(|e| e.to_string())?;
    super::native_events::clear(&host);

    let mut context = imgui_context();
    let mut diagnostics = Vec::new();
    for frame in 0..3 {
        let snapshot = play.forge_snapshot(Some(&walker)).managed();
        assert!(snapshot.in_game, "the fixture is in the world");
        host.begin_iteration();
        host.snapshot.set(snapshot);
        visibility.update(snapshot.health, true);
        context.frame();
        let ran = bridge
            .invoke(|| host.frame_entry(&entry))
            .and_then(|()| bridge.invoke(|| host.finish_frame(frame == 0)));
        drop(context.render_legacy());
        host.end_iteration();
        diagnostics.extend(bridge.drain_diagnostics());
        // the same queue carries the mod's own `eqoa.log.write` lines; as in Mods & Addons, its "info" and
        // "debug" lines are not diagnostics ("warn" and "error" are)
        diagnostics.extend(
            host.drain_logs()
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter(|line| !line.starts_with("eqoa/1 [info] ") && !line.starts_with("eqoa/1 [debug] ")),
        );
        ran.map_err(|e| format!("frame {frame}: {e}"))?;
    }
    Ok(diagnostics)
}

/// A package whose entry is `source`, with the grants a documentation example may use.
fn example_package(root: &Path, source: &str) -> DiscoveredAddon {
    let package = root.join("docs_example");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("main.lua"), source).unwrap();
    std::fs::write(
        package.join("mod.json"),
        serde_json::json!({
            "schemaVersion": 1, "runtime": "lua", "api": "eqoa/1",
            "id": "docs_example", "name": "Docs example", "entry": "main.lua", "entryMode": "once",
            "capabilities": ["eqoa.actions.*", "eqoa.windows.takeover", "eqoa.map.waypoint"]
        })
        .to_string(),
    )
    .unwrap();
    only_package(root)
}

/// The names a Lua example calls must be supported ones, also inside handlers the fixture never fires.
fn check_names(label: &str, source: &str, surface: &BTreeMap<String, String>, problems: &mut Vec<String>) {
    for path in eqoa_paths(source) {
        match surface.get(&path).map(String::as_str) {
            Some("supported" | "partial") => {}
            // a name that only refuses may be written (asked about with `capabilities.get`), not called
            Some(status) if source.contains(&format!("{path}(")) => {
                problems.push(format!("{label}: {path} is called and is {status}"))
            }
            Some(_) => {}
            None => problems.push(format!("{label}: {path} is not in the surface file")),
        }
    }
    // eqoa.events.on("name", ...) / once("name", ...)
    for call in ["eqoa.events.on(\"", "eqoa.events.once(\""] {
        let mut rest = source;
        while let Some(at) = rest.find(call) {
            rest = &rest[at + call.len()..];
            let name = &rest[..rest.find('"').unwrap_or(0)];
            if surface.get(&format!("eqoa.events.{name}")).map(String::as_str) != Some("supported") {
                problems.push(format!("{label}: event {name:?} is not a supported event"));
            }
        }
    }
}

#[test]
fn docs_getting_started_and_template_lua_runs_clean() {
    super::big_stack(|| {
        let surface = surface();
        let mut problems = Vec::new();
        let mut ran = 0;

        let guide = page("GETTING-STARTED.md");
        let blocks = fenced(&guide, "lua");
        assert!(blocks.len() >= 3, "GETTING-STARTED.md has its Lua examples ({} found)", blocks.len());
        for (line, source) in &blocks {
            let label = format!("GETTING-STARTED.md:{line}");
            check_names(&label, source, &surface, &mut problems);
            let scratch = Scratch::new("guide");
            let addon = example_package(&scratch.0, source);
            match run_package(&addon, scratch.0.join("data")) {
                Err(error) => problems.push(format!("{label}: Lua error: {error}")),
                Ok(diagnostics) => {
                    problems.extend(diagnostics.into_iter().map(|d| format!("{label}: diagnostic: {d}")))
                }
            }
            ran += 1;
        }

        // every manifest the guide shows is one discovery accepts
        let manifests: Vec<_> = fenced(&guide, "json")
            .into_iter()
            .filter(|(_, text)| text.contains("\"schemaVersion\""))
            .collect();
        assert!(!manifests.is_empty(), "GETTING-STARTED.md shows a mod.json");
        for (line, manifest) in manifests {
            let scratch = Scratch::new("manifest");
            let package = scratch.0.join("shown");
            std::fs::create_dir_all(&package).unwrap();
            std::fs::write(package.join("mod.json"), &manifest).unwrap();
            let entry = serde_json::from_str::<serde_json::Value>(&manifest)
                .unwrap_or_else(|e| panic!("GETTING-STARTED.md:{line}: {e}"))["entry"]
                .as_str()
                .unwrap_or("main.lua")
                .to_owned();
            std::fs::write(package.join(entry), "-- shown manifest").unwrap();
            let found = discover(&scratch.0);
            if !found.diagnostics.is_empty() || found.addons.len() != 1 {
                problems.push(format!("GETTING-STARTED.md:{line}: manifest refused: {:?}", found.diagnostics));
            }
        }

        // the template, as a copy of its folder
        let scratch = Scratch::new("template");
        copy_tree(&docs().join("template"), &scratch.0.join("template"));
        let template = only_package(&scratch.0);
        assert_eq!(template.api.as_deref(), Some("eqoa/1"));
        let source = std::fs::read_to_string(&template.entry).unwrap();
        check_names("template/main.lua", &source, &surface, &mut problems);
        for cap in template.declared_capabilities().unwrap_or(&[]) {
            if cap.starts_with("eqoa.") && !surface.contains_key(cap) {
                problems.push(format!("template/mod.json: capability {cap} is not in the surface file"));
            }
        }
        match run_package(&template, scratch.0.join("data")) {
            Err(error) => problems.push(format!("template/main.lua: Lua error: {error}")),
            Ok(diagnostics) => {
                problems.extend(diagnostics.into_iter().map(|d| format!("template/main.lua: diagnostic: {d}")))
            }
        }
        ran += 1;

        assert!(problems.is_empty(), "{} problem(s) in {ran} example(s):\n{}", problems.len(), problems.join("\n"));
    });
}

/// The negative control of the test above, kept as a test: a misspelt name in an example is caught twice,
/// by the name check and by the running client's `unknown_symbol` diagnostic.
#[test]
fn docs_a_misspelt_name_in_an_example_is_caught() {
    super::big_stack(|| {
        let source = "local p = eqoa.player.gett and eqoa.player.gett()\n";
        let mut problems = Vec::new();
        check_names("example", source, &surface(), &mut problems);
        assert_eq!(problems, ["example: eqoa.player.gett is not in the surface file"]);
        let scratch = Scratch::new("misspelt");
        let addon = example_package(&scratch.0, source);
        let diagnostics = run_package(&addon, scratch.0.join("data")).unwrap();
        assert!(
            diagnostics.iter().any(|d| d.contains("unknown_symbol") && d.contains("eqoa.player.gett")),
            "{diagnostics:?}"
        );
    });
}

const HAND_WRITTEN: [&str; 6] = [
    "EQOA-API-SPEC.md",
    "GETTING-STARTED.md",
    "README.md",
    "VERSIONING.md",
    "EXAMPLES.md",
    "AGENT-GUIDE.md",
];

#[test]
fn docs_every_eqoa_name_in_the_pages_exists_in_the_surface_file() {
    let surface = surface();
    let mut unknown = Vec::new();
    for name in HAND_WRITTEN {
        let text = page(name);
        for (index, line) in text.lines().enumerate() {
            for path in eqoa_paths(line) {
                // a name the client has, or a prefix of names it has
                let below = format!("{path}.");
                let known = surface.contains_key(&path)
                    || surface.range(below.clone()..).next().is_some_and(|(next, _)| next.starts_with(&below));
                if !known {
                    unknown.push(format!("{name}:{}: {path}", index + 1));
                }
                // a name that only refuses is never written as if it worked: its line says so (or asks
                // the client about it)
                let refuses = matches!(surface.get(&path).map(String::as_str), Some("unsupported" | "not_yet"));
                if refuses && !["unsupported", "not_yet", "eqoa.capabilities."].iter().any(|word| line.contains(word)) {
                    unknown.push(format!(
                        "{name}:{}: {path} only refuses, and the line does not say `unsupported` or `not_yet`",
                        index + 1
                    ));
                }
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "{} name(s) the pages give that the client does not have, or does not support and the page does not say \
         (docs/modding/eqoa-1-surface.json):\n{}",
        unknown.len(),
        unknown.join("\n")
    );
}

/// The part of `text` under the heading that contains `title`, up to the next heading of the same or a
/// higher level.
fn section<'a>(text: &'a str, title: &str) -> &'a str {
    let mut start = None;
    let mut level = 0;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let hashes = line.bytes().take_while(|b| *b == b'#').count();
        if hashes > 0 && line[hashes..].starts_with(' ') {
            match start {
                None if line.contains(title) => {
                    start = Some(offset + line.len());
                    level = hashes;
                }
                Some(from) if hashes <= level => return &text[from..offset],
                _ => {}
            }
        }
        offset += line.len();
    }
    &text[start.unwrap_or_else(|| panic!("the spec has no heading containing {title:?}"))..]
}

#[test]
fn docs_spec_names_everything_the_surface_file_calls_supported() {
    let surface = surface();
    let spec = page("EQOA-API-SPEC.md");
    let imgui = section(&spec, "ImGui");
    let lower = |s: &str| s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    let mut missing = Vec::new();
    let (mut functions, mut events, mut fields, mut imgui_names) = (0, 0, 0, 0);
    for (path, status) in &surface {
        if status != "supported" && status != "partial" {
            continue;
        }
        let Some(rest) = path.strip_prefix("eqoa.") else {
            continue;
        };
        let parts: Vec<&str> = rest.split('.').collect();
        let named = match parts.as_slice() {
            // a module, a root value, a shared record
            [_] => has_word(&spec, path),
            // an event: by its name in backticks
            ["events", name @ ..] if !matches!(name, ["on"] | ["off"] | ["once"] | ["after"] | ["Event", ..]) => {
                if name.contains(&"data") {
                    continue;
                }
                events += 1;
                spec.contains(&format!("`{}`", name.join(".")))
            }
            // a field of a shared record (`eqoa.Vec3.x`)
            [record, field] if record.starts_with(|c: char| c.is_ascii_uppercase()) && *record != "WeaponSets" => {
                fields += 1;
                has_word(&spec, field)
            }
            // the ImGui bridge: by its ImGui name, in the spec's ImGui section
            ["imgui", name] if !lower(name) => {
                imgui_names += 1;
                has_word(imgui, name)
            }
            // a function, constant table or value of a module, and the legacy alias's members
            [module, name] if lower(name) || *module == "WeaponSets" => {
                functions += 1;
                has_word(&spec, path)
            }
            // a constant: its table is named (checked above); the keys are data
            [_, table, _] if lower(table) => continue,
            // a record type: by its name
            [_, record] => has_word(&spec, record),
            // a record field: by its name, anywhere in the spec
            [_, _record, field] => {
                fields += 1;
                has_word(&spec, field)
            }
            _ => continue,
        };
        if !named {
            missing.push(path.clone());
        }
    }
    assert!(
        functions > 190 && events > 40 && fields > 400 && imgui_names > 90,
        "the surface file was read: {functions} functions, {events} events, {fields} fields, {imgui_names} ImGui names"
    );
    assert!(
        missing.is_empty(),
        "{} supported name(s) the spec does not name:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

#[test]
fn docs_readers_find_names_and_blocks() {
    let text = "Call `eqoa.player.get()`, then eqoa.hud.layout().minimap; see eqoa.actions.* and\n\
                `eqoa.events.cooldown.started.data.lockout`. The file mods/eqoa.log is not a name here,\n\
                but eqoa.log.write is. eqoa-1-surface.json and eqoa/1 are not paths.";
    let paths: Vec<String> = eqoa_paths(text).into_iter().collect();
    assert_eq!(
        paths,
        [
            "eqoa.actions",
            "eqoa.events.cooldown.started.data.lockout",
            "eqoa.hud.layout",
            "eqoa.log.write",
            "eqoa.player.get",
        ]
    );
    assert!(has_word("uses Begin and End", "End") && !has_word("EndChild", "End"));
    let blocks = fenced("a\n```lua\nlocal x = 1\n```\n```json\n{}\n```\n  ```lua\n  y()\n  ```\n", "lua");
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0], (3, "local x = 1\n".to_owned()));
    assert_eq!(section("# A\nx\n## ImGui is here\nBegin\n### sub\nEnd\n## Next\nno\n", "ImGui"), "Begin\n### sub\nEnd\n");
}

// ---- limits (review round 1) --------------------------------------------------------------------

fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// One number of a row in the spec's table of tested limits: what the row says, the line of code that
/// holds the number, and the test that covers it.
struct Pin {
    /// the row's first cell, exactly
    row: &'static str,
    /// the number as the row's second cell writes it
    says: &'static str,
    /// (file under `crates/eqoa/src`, text of the code that holds the number)
    code: (&'static str, &'static str),
    /// (file under `crates/eqoa/src`, test function)
    test: (&'static str, &'static str),
}

macro_rules! pin {
    ($row:expr, $says:expr, $code_file:expr, $code:expr, $test_file:expr, $test:expr) => {
        Pin { row: $row, says: $says, code: ($code_file, $code), test: ($test_file, $test) }
    };
}

/// Every row of the spec's "tested" table. A row with no entry here, an entry with no row, a number the row
/// does not state, a constant that is no longer in the code and a test that no longer exists all fail
/// `docs_every_tested_limit_names_its_constant_and_its_test`.
fn pins() -> Vec<Pin> {
    let actions = "Actions waiting per mod; dispatched per mod per game step; for all mods per step";
    let waits = "An action waiting for its turn; an action waiting for the server's answer";
    let events = "Undelivered events per mod; events delivered per frame";
    let json = "JSON load: file size, depth, values, loads at once";
    let files = "A private file; all private data of a mod, settings included";
    let commands = "Commands waiting per sender (`eqoa.mods.send`); a command's `args`";
    let broker = "fork_ext/addons/action_broker.rs";
    let rates = "round6b_broker_rate_limits_fairness_retention_cancel_and_reset";
    vec![
        pin!("Lua memory per mod", "8 MiB", "fork_ext/addons/host.rs", "set_memory_limit(8 * 1024 * 1024)",
            "fork_ext/addons/file_io.rs", "round4_json_calls_keep_ordinary_heap_and_manifest_opt_in"),
        pin!("Lua memory per mod", "256 MiB", "fork_ext/addons/async_json.rs", "JSON_HEAP: usize = 256 * 1024 * 1024",
            "fork_ext/addons/file_io.rs", "round4_json_calls_keep_ordinary_heap_and_manifest_opt_in"),
        pin!("Lua instructions per mod per frame, all handlers together", "4 million",
            "fork_ext/addons/host.rs", "INSTRUCTION_BUDGET: u32 = 4_000_000",
            "fork_ext/addons/native_tests.rs", "native_iteration_shares_instruction_budget_across_entry_and_callbacks"),
        pin!(files, "1 MiB", "fork_ext/addons/file_io.rs", "MAX_BYTES: usize = 1024 * 1024",
            "fork_ext/addons/file_io.rs", "forge_modern_io_rejects_traversal_code_other_addons_and_oversized_files"),
        pin!(files, "16 MiB", "fork_ext/addons/sandbox.rs", "size > 16 * 1024 * 1024",
            "fork_ext/addons/native_tests.rs", "native_storage_quota_combines_private_files_default_settings_and_named_profiles"),
        pin!("Files open at once through `io.open`", "16", "fork_ext/addons/file_io.rs", "registry.files.len() >= 16",
            "fork_ext/addons/file_io.rs", "forge_modern_io_iterator_gc_keeps_memory_budget_and_close_releases_it"),
        pin!("HUD draw calls per mod per frame (`eqoa.hud.text` and the like)", "256",
            "fork_ext/addons/hud_layer.rs", "MAX_COMMANDS_PER_MOD_PER_FRAME: usize = 256",
            "fork_ext/addons/hud_layer.rs", "text_and_utf16_storage_survive_clear_and_command_limit_rejection"),
        pin!("ImGui calls in one run of a mod's handlers", "131,072", "fork_ext/addons/bridge.rs", "CALL_BUDGET: usize = 131_072",
            "fork_ext/addons/docs_tests.rs", "docs_limit_imgui_calls_are_131072_a_run"),
        pin!("Drawn ImGui geometry per mod per frame", "524,288", "fork_ext/addons/bridge_drawlist.rs", "GEOMETRY_LIMIT: usize = 524_288",
            "fork_ext/addons/bridge_drawlist_tests.rs", "forge_drawlist_geometry_budget_is_shared_across_invocations_in_one_frame"),
        pin!("ImGui scopes open at once (`Begin`, `BeginChild`, pushes)", "64", "fork_ext/addons/bridge.rs", "self.stack.len() >= 64",
            "fork_ext/addons/docs_tests.rs", "docs_limit_imgui_scopes_are_64"),
        pin!(actions, "32", broker, "MAX_QUEUED_PER_VM: usize = 32", broker, rates),
        pin!(actions, "8", broker, "MAX_DISPATCH_PER_TICK_PER_VM: usize = 8", broker, rates),
        pin!(actions, "one a step", broker, "TOKEN_BUCKET_REFILL_PER_TICK: u32 = 1", broker, rates),
        pin!(actions, "64", broker, "MAX_DISPATCH_PER_TICK_GLOBAL: usize = 64", broker, rates),
        pin!(waits, "1 second", broker, "RATE_LIMIT_WAIT_TICKS: u64 = 30", broker, rates),
        pin!(waits, "10 seconds", broker, "SERVER_TIMEOUT_TICKS: u64 = 300", broker, "round6b_broker_server_timeout_and_subsystem_locks"),
        pin!("Action results kept per mod", "256", broker, "MAX_RETAINED_RESULTS_PER_VM: usize = 256", broker, rates),
        pin!("A chat or command line", "128 UTF-16 units", broker, "encode_utf16().count() >= 128",
            "fork_ext/r6d_tests.rs", "round7c_typed_chat_rejects_the_native_utf16_cutoff"),
        pin!("A waypoint label", "128 bytes", "fork_ext/addons/map_views.rs", "s.as_bytes().len() <= 128",
            "fork_ext/api_capture_tests.rs", "round7c_mod_moves_and_clears_the_players_own_waypoint"),
        pin!(events, "1,024", "fork_ext/addons/native_events.rs", "b.pending.len() > 1024",
            "fork_ext/addons/native_tests.rs", "native_events_share_one_retention_limit_for_sources_and_resource_completions"),
        pin!(events, "2 MiB", "fork_ext/addons/native_events.rs", "b.pending_bytes > 2 * 1024 * 1024",
            "fork_ext/addons/native_tests.rs", "native_events_enforce_two_mebibyte_payload_retention_without_losing_dispatch_backlog"),
        pin!(events, "256", "fork_ext/addons/native_events.rs", "256usize.saturating_sub(b.delivered",
            "fork_ext/addons/perf_delta_event_tests.rs", "an_event_nobody_listens_for_still_spends_the_delivery_budget"),
        pin!(json, "16 MiB", "fork_ext/addons/async_json.rs", "READ_CAP: usize = 16 * 1024 * 1024",
            "fork_ext/addons/file_io.rs", "round4_json_read_cap_is_enforced"),
        pin!(json, "32", "fork_ext/addons/async_json.rs", "depth > 32", "fork_ext/addons/file_io.rs", "round4_json_depth_is_enforced"),
        pin!(json, "two million", "fork_ext/addons/async_json.rs", "NODES: usize = 2_000_000",
            "fork_ext/addons/file_io.rs", "round4_json_nodes_are_enforced"),
        pin!(json, ", 2", "fork_ext/addons/async_json.rs", "s.pending >= 2",
            "fork_ext/addons/file_io.rs", "round4_json_shared_mount_and_inflight_quota"),
        pin!("JSON save: size", "14 MiB", "fork_ext/addons/json_save.rs", "CAP: usize = 14 * 1024 * 1024",
            "fork_ext/addons/file_io.rs", "round4_json_private_journal_flushes_on_drop_and_reloads"),
        pin!(commands, "4;", "fork_ext/addons/native_events.rs", ".count()>=4",
            "fork_ext/addons/file_io.rs", "round4b_mod_commands_are_declared_bounded_and_delayed"),
        pin!(commands, "4 KiB", "fork_ext/addons/exchange.rs", "&mut 256, &mut 4096",
            "fork_ext/addons/file_io.rs", "round4b_review_commands_preserve_other_events_and_sender_fairness"),
        pin!("Fonts and sounds held", "16 each", "fork_ext/addons/fonts.rs", "MAX_FONT_HANDLES: usize = 16",
            "fork_ext/addons/fonts.rs", "forge_fonts_handle_and_size_limits"),
        pin!("Fonts and sounds held", "16 each", "fork_ext/addons/sound.rs", "MAX_SOUND_HANDLES: usize = 16",
            "fork_ext/addons/sound.rs", "forge_sound_handle_and_file_size_limits"),
        pin!("Imported copies kept", "3", "fork_ext/addons/player_files.rs", ".skip(3)",
            "fork_ext/addons/file_io.rs", "round4b_review_import_ids_survive_enable_and_staging_cannot_starve_saves"),
    ]
}

/// The rows of the spec's limits table that end in "tested": (first cell, second cell).
fn tested_rows(limits: &str) -> Vec<(String, String)> {
    limits
        .lines()
        .filter(|line| line.starts_with("| ") && line.trim_end().ends_with("| tested |"))
        .map(|line| {
            let cells: Vec<&str> = line.trim().trim_matches('|').split(" | ").map(str::trim).collect();
            assert_eq!(cells.len(), 3, "a limits row has three cells: {line}");
            (cells[0].to_owned(), cells[1].to_owned())
        })
        .collect()
}

/// The check behind `docs_every_tested_limit_names_its_constant_and_its_test`, over a given spec text.
fn limit_problems(spec: &str) -> Vec<String> {
    let limits = section(spec, "The sandbox and its limits");
    let rows = tested_rows(limits);
    let pins = pins();
    let mut problems = Vec::new();
    if rows.len() < 15 {
        problems.push(format!("only {} tested rows were found in the limits table", rows.len()));
    }
    for (row, value) in &rows {
        let mine: Vec<&Pin> = pins.iter().filter(|pin| pin.row == row).collect();
        if mine.is_empty() {
            problems.push(format!("the spec calls {row:?} tested, and the docs test names no constant and no test for it"));
        }
        for pin in mine {
            if !value.contains(pin.says) {
                problems.push(format!("{row:?}: the spec says {value:?}, the code's number is written {:?}", pin.says));
            }
        }
    }
    for pin in &pins {
        if !rows.iter().any(|(row, _)| row == pin.row) {
            problems.push(format!("{:?} is not a row of the spec's tested limits any more", pin.row));
        }
        if !source(pin.code.0).contains(pin.code.1) {
            problems.push(format!("{:?} ({}): the code no longer has {:?} in {}", pin.row, pin.says, pin.code.1, pin.code.0));
        }
        if !source(pin.test.0).contains(&format!("fn {}(", pin.test.1)) {
            problems.push(format!("{:?} ({}): no test {} in {}", pin.row, pin.says, pin.test.1, pin.test.0));
        }
    }
    // only that table may call a number tested
    let from = limits.as_ptr() as usize - spec.as_ptr() as usize;
    let outside = [&spec[..from], &spec[from + limits.len()..]];
    for (index, line) in outside.iter().flat_map(|part| part.lines()).enumerate() {
        if has_word(line, "tested") {
            problems.push(format!("\"tested\" outside the limits table (line {} of the text around it): {line}", index + 1));
        }
    }
    problems
}

#[test]
fn docs_every_tested_limit_names_its_constant_and_its_test() {
    let problems = limit_problems(&page("EQOA-API-SPEC.md"));
    assert!(problems.is_empty(), "{} problem(s):\n{}", problems.len(), problems.join("\n"));
}

/// The negative controls of the check above, kept as a test: a changed number, a row with nothing behind it
/// and a "tested" claim outside the table are each caught.
#[test]
fn docs_a_wrong_or_unbacked_tested_limit_is_caught() {
    let spec = page("EQOA-API-SPEC.md");
    let wrong = spec.replacen("| Action results kept per mod | 256 | tested |", "| Action results kept per mod | 512 | tested |", 1);
    assert_ne!(wrong, spec, "the row is in the spec");
    assert!(limit_problems(&wrong).iter().any(|p| p.contains("Action results kept per mod") && p.contains("512")));
    let unbacked = spec.replacen("| Imported copies kept | 3 | tested |", "| Imported copies kept | 3 | tested |\n| Option buttons per mod | 32 | tested |", 1);
    assert!(limit_problems(&unbacked).iter().any(|p| p.contains("Option buttons per mod") && p.contains("no constant")));
    let stray = spec.replacen("## 10. Keys\n", "## 10. Keys\n\nA mod may hold 128 subscriptions (tested).\n", 1);
    assert!(limit_problems(&stray).iter().any(|p| p.contains("outside the limits table")));
}

/// Integration review P1 (2026-11-25): every item-definition and ability id is the spec's `Id`, a decimal string, in
/// every round. Each field of the surface file named `id` or `*_id` is classified here, once: as an `Id` (with the
/// words the spec must use for it) or as a whole number, with the kind of number it is. A new id field fails the
/// check until someone decides which it is, so a round cannot hand an item or ability id over as a Lua number
/// again without the spec and this list saying so. The values themselves are pinned by the rounds' own tests
/// (`a8_the_tray_keeps_the_games_limits_and_a_fill_is_all_or_nothing`, the A9 trade and smith reads, the A11 merc
/// read, `a3` buffs, `api_codes_tests::codes_bad_argument_*`).
const ID_STRINGS: &[(&str, &str)] = &[
    ("eqoa.abilities.Ability.id", "`Ability = {ref:AbilityRef, id:Id,"),
    ("eqoa.events.cooldown.started.data.ability_id", "ability_id:Id?"),
    ("eqoa.events.cooldown.updated.data.ability_id", "ability_id:Id?"),
    ("eqoa.events.cooldown.ended.data.ability_id", "ability_id:Id?"),
    ("eqoa.items.Item.id", "| `id:Id`, `name:string`"),
    ("eqoa.raid.LootRollItem.item_id", "item_name:string, item_id:Id"),
    ("eqoa.effects.Buff.spell_id", "spell_id:Id?, description"),
    ("eqoa.craft.Item.id", "`eqoa.craft.Item = {slot:int, id:Id"),
    ("eqoa.craft.TrayItem.id", "`eqoa.craft.TrayItem = {slot:int, id:Id"),
    ("eqoa.trade.Item.id", "`eqoa.trade.Item = {slot:int, id:Id"),
    ("eqoa.trade.Bag.id", "`eqoa.trade.Bag = {slot:int, id:Id"),
    ("eqoa.blacksmith.Item.id", "eqoa.blacksmith.Item = {slot:int, id:Id"),
    ("eqoa.mail.PackItem.id", "`eqoa.mail.PackItem = {slot:int, id:Id"),
    ("eqoa.auction.Sellable.id", "`eqoa.auction.Sellable = {slot:int, id:Id"),
    ("eqoa.merc.Record.item_id", "item_id:Id, name, quantity:int, equip_slot:int}"),
    ("eqoa.merc.Spell.id", "`Spell = {id:Id, name"),
];

/// Ids that are rightly whole numbers, by what they number. None of them is an item definition or an ability.
const ID_NUMBERS: &[(&str, &[&str])] = &[
    (
        "an entity, player or character id (the server's, as `Entity.id`)",
        &[
            "eqoa.entities.Entity.id", "eqoa.entities.Entity.target_id", "eqoa.player.Player.id",
            "eqoa.player.Player.pet_id", "eqoa.player.Player.target_id", "eqoa.target.TargetInfo.id",
            "eqoa.casts.Cast.caster_id", "eqoa.casts.Cast.target_id", "eqoa.events.cast.started.data.caster_id",
            "eqoa.events.cast.started.data.target_id", "eqoa.events.cast.ended.data.caster_id",
            "eqoa.events.cast.ended.data.target_id", "eqoa.combat.CombatHit.attacker_id",
            "eqoa.combat.CombatHit.defender_id", "eqoa.events.combat.hit.data.attacker_id",
            "eqoa.events.combat.hit.data.defender_id", "eqoa.events.guide.lost.data.entity_id",
            "eqoa.items.LootSource.entity_id", "eqoa.items.MerchantState.entity_id", "eqoa.items.MasterLootMember.id",
            "eqoa.group.Invite.id", "eqoa.group.Member.id", "eqoa.group.RaidMember.id", "eqoa.group.RaidRole.id",
            "eqoa.raid.RaidMember.id", "eqoa.raid.RaidState.assist_id", "eqoa.raid.RaidState.helper_id",
            "eqoa.raid.RaidState.leader_id", "eqoa.trade.Partner.id", "eqoa.events.trade.request.data.id",
            "eqoa.duel.State.opponent_id", "eqoa.duel.Invite.id", "eqoa.guild.GuildMember.id",
            "eqoa.guild.GuildInvite.inviter_id", "eqoa.social.Friend.id", "eqoa.social.FriendRequest.id",
            "eqoa.social.Ignored.id", "eqoa.who.WhoEntry.id", "eqoa.lfg.LfgEntry.id", "eqoa.windows.TalkedTo.id",
        ],
    ),
    (
        "a world or place number",
        &[
            "eqoa.client.Client.world_id", "eqoa.entities.Entity.world_id", "eqoa.events.guide.lost.data.world_id",
            "eqoa.events.world.entered.data.world_id", "eqoa.events.world.leaving.data.world_id",
            "eqoa.group.Member.world_id", "eqoa.lfg.LfgEntry.world_id", "eqoa.map.MapState.world_id",
            "eqoa.map.Waypoint.world_id", "eqoa.player.Location.world_id", "eqoa.player.Location.place_id",
            "eqoa.player.Location.compass_id",
        ],
    ),
    (
        "one of the game's enum numbers (class, race, stat, slot, skill, recipe, box style)",
        &[
            "eqoa.player.Player.class_id", "eqoa.player.Player.race_id", "eqoa.duel.Invite.class_id",
            "eqoa.duel.Invite.race_id", "eqoa.group.RaidRole.class_id", "eqoa.guild.GuildMember.class_id",
            "eqoa.guild.GuildMember.race_id", "eqoa.lfg.LfgEntry.class_id", "eqoa.lfg.LfgEntry.race_id",
            "eqoa.social.Friend.class_id", "eqoa.social.Friend.race_id", "eqoa.who.WhoEntry.class_id",
            "eqoa.who.WhoEntry.race_id", "eqoa.items.ItemStat.id", "eqoa.items.SlotInfo.id",
            "eqoa.player.StatItem.stat_id", "eqoa.mastery.MasteryNode.id", "eqoa.mastery.MasteryNode.skill_id",
            "eqoa.mastery.MasterySkill.id", "eqoa.craft.Skill.id", "eqoa.craft.Recipe.id",
            "eqoa.windows.Dialog.style_id",
        ],
    ),
    (
        "the key of an entry of a server's or the client's list (a mail, a mercenary's record, a roll, a ready check, a link, a question)",
        &[
            "eqoa.mail.Entry.id", "eqoa.mail.Message.id", "eqoa.merc.Record.id", "eqoa.raid.LootRollItem.id",
            "eqoa.raid.ReadyCheckData.id", "eqoa.chat.ChatLink.id", "eqoa.windows.Confirm.id",
            "eqoa.windows.SessionData.id",
        ],
    ),
    (
        "a handle or name of the mod's own (a request, a batch, an option row)",
        &[
            "eqoa.actions.ActionResult.request_id", "eqoa.events.action.result.data.request_id",
            "eqoa.actions.BatchResult.batch_id", "eqoa.events.batch.progress.data.batch_id",
            "eqoa.events.batch.result.data.batch_id", "eqoa.options.Button.id", "eqoa.options.Button.item_id",
            "eqoa.options.Toggle.id", "eqoa.options.Toggle.item_id",
        ],
    ),
];

/// What is wrong with the id fields of `paths` against `spec`: a field in neither list, a field in both, an `Id`
/// field the spec does not type as `Id`.
fn id_problems(paths: &BTreeSet<String>, spec: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let numbers: BTreeSet<&str> = ID_NUMBERS.iter().flat_map(|(_, list)| list.iter().copied()).collect();
    let strings: BTreeMap<&str, &str> = ID_STRINGS.iter().copied().collect();
    for path in paths.iter().filter(|p| p.starts_with("eqoa.")) {
        let field = path.rsplit('.').next().unwrap_or("");
        if field != "id" && !field.ends_with("_id") {
            continue;
        }
        match (strings.get(path.as_str()), numbers.contains(path.as_str())) {
            (Some(_), true) => problems.push(format!("{path} is listed both as an Id string and as a number")),
            (None, false) => problems.push(format!(
                "{path} is a new id field: if it holds an item-definition or ability id it is the spec's `Id` (a \
                 decimal string; add it to ID_STRINGS), otherwise add it to ID_NUMBERS with what it numbers"
            )),
            (Some(needle), false) if !spec.contains(needle) => {
                problems.push(format!("{path} is an Id, but the spec does not say so (expected the words {needle:?})"))
            }
            _ => {}
        }
    }
    for (path, _) in ID_STRINGS {
        if !paths.contains(*path) {
            problems.push(format!("{path} is listed as an Id but is not in the surface file"));
        }
    }
    problems
}

#[test]
fn docs_every_item_and_ability_id_is_an_id_string() {
    let paths: BTreeSet<String> = surface().into_keys().collect();
    let problems = id_problems(&paths, &page("EQOA-API-SPEC.md"));
    assert!(problems.is_empty(), "{} problem(s):\n{}", problems.len(), problems.join("\n"));
}

/// The negative controls of the check above: the spec typing a row's id `int` again (as A8 to A11 did before the
/// integration fix), and a new `*_id` field nobody classified, are each caught.
#[test]
fn docs_a_numeric_item_id_or_an_unclassified_id_is_caught() {
    let spec = page("EQOA-API-SPEC.md");
    let paths: BTreeSet<String> = surface().into_keys().collect();
    let numeric = spec.replacen("`eqoa.craft.Item = {slot:int, id:Id", "`eqoa.craft.Item = {slot:int, id:int", 1);
    assert_ne!(numeric, spec, "the craft row is in the spec");
    assert!(id_problems(&paths, &numeric).iter().any(|p| p.starts_with("eqoa.craft.Item.id is an Id")));
    let mut grown = paths.clone();
    grown.insert("eqoa.trade.Bag.item_id".into());
    assert!(id_problems(&grown, &spec).iter().any(|p| p.starts_with("eqoa.trade.Bag.item_id is a new id field")));
}

/// Runs `source` in one bridge invocation inside an ImGui frame, as a handler runs.
fn in_frame(source: &str) -> mlua::Result<()> {
    let mut context = imgui_context();
    let host = Host::new().unwrap();
    let bridge = Bridge::install(&host).unwrap();
    context.frame();
    let result = bridge.invoke(|| host.execute(source));
    drop(context.render_legacy());
    result
}

/// 64 ImGui scopes may be open at once (a window and 63 pushes inside it); the 65th is refused.
#[test]
fn docs_limit_imgui_scopes_are_64() {
    let nested = |pushes: usize| {
        in_frame(&format!(
            "ImGui.Begin('scopes') for i = 1, {pushes} do ImGui.PushTextWrapPos() end \
             for i = 1, {pushes} do ImGui.PopTextWrapPos() end ImGui.End()"
        ))
    };
    nested(63).expect("a window and 63 pushes are 64 open scopes");
    let error = nested(64).expect_err("the 65th open scope is refused").to_string();
    assert!(error.contains("ImGui stack budget exceeded"), "{error}");
}

/// One run of a mod's handlers may make 131,072 ImGui calls; the next call is refused.
#[test]
fn docs_limit_imgui_calls_are_131072_a_run() {
    in_frame("for i = 1, 131072 do ImGui.GetTime() end").expect("131,072 calls fit");
    let error = in_frame("for i = 1, 131073 do ImGui.GetTime() end").expect_err("one call more is refused").to_string();
    assert!(error.contains("ImGui call budget exceeded: 131073 calls (limit: 131072)"), "{error}");
}
