// LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE).
//! Round 7 Part D2: the author documentation against the running client.
//!
//! * Every fenced `lua` block in `docs/modding/GETTING-STARTED.md`, and the template mod, runs in a
//!   real `eqoa/1` host (the production install path, a world published from a `Play`, an ImGui
//!   frame, no graphics adapter) with no error and no diagnostic.
//! * Every name the generated surface file calls supported is named in the spec, and every
//!   `eqoa.x.y` the hand-written pages name exists in the surface file.
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
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
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
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "{} name(s) the pages give that the client does not have (docs/modding/eqoa-1-surface.json):\n{}",
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
