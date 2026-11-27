// Copyright LoopRook. SPDX-License-Identifier: AGPL-3.0-or-later
//! Headless author tools, compiled as a child of addons to use its production installers.

use mlua::Table;
use serde_json::{Value as Json, json};
use std::{
    cell::RefCell, collections::BTreeMap, fmt::Write as _, io::Write as _, path::PathBuf, rc::Rc,
};

const USAGE: &str = "usage: eqoa mod-api [--format md|json] [--out <file>]";

#[derive(Debug)]
struct GraphicsUnavailable(String);

impl std::fmt::Display for GraphicsUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mod-api graphics unavailable: {}", self.0)
    }
}

impl std::error::Error for GraphicsUnavailable {}

pub fn run(args: &[String]) -> Option<crate::Result<()>> {
    (args.first().map(String::as_str) == Some("mod-api")).then(|| command(&args[1..]))
}

fn command(args: &[String]) -> crate::Result<()> {
    let mut format = None;
    let mut out = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--format" if format.is_none() => {
                format = Some(args.next().ok_or(USAGE)?.as_str());
                if !matches!(format, Some("md" | "json")) {
                    return Err(USAGE.into());
                }
            }
            "--out" if out.is_none() => {
                out = Some(args.next().filter(|s| !s.is_empty()).ok_or(USAGE)?)
            }
            _ => return Err(USAGE.into()),
        }
    }
    let reference = reference()?;
    let text = match format.unwrap_or("md") {
        "json" => format!("{}\n", serde_json::to_string_pretty(&reference)?),
        _ => markdown(&reference),
    };
    if let Some(path) = out {
        std::fs::write(path, text)?;
    } else {
        std::io::stdout().lock().write_all(text.as_bytes())?;
    }
    Ok(())
}

/// Only a scratch package is mounted. No player config, disc or real addon is read.
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> crate::Result<Self> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        for _ in 0..100 {
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("eqoa-mod-api-{}-{n}", std::process::id()));
            match std::fs::create_dir(&dir) {
                Ok(()) => return Ok(Self(dir)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err("cannot create mod-api scratch package".into())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn reference() -> crate::Result<Json> {
    reference_with_instance(render::wgpu::Instance::default())
}

fn reference_with_instance(instance: render::wgpu::Instance) -> crate::Result<Json> {
    with_reference_host(instance, collect_reference)
}

fn with_reference_host<T>(
    instance: render::wgpu::Instance,
    inspect: impl FnOnce(&super::host::Host) -> crate::Result<T>,
) -> crate::Result<T> {
    use dear_imgui_rs::Context;
    use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
    use render::wgpu;

    let scratch = Scratch::new()?;
    let package = scratch.0.join("mod_api_reference");
    std::fs::create_dir(&package)?;
    std::fs::write(
        package.join("mod.json"),
        r#"{
        "schemaVersion":1,"runtime":"lua","api":"eqoa/1",
        "id":"mod_api_reference","name":"Mod API reference",
        "entry":"main.lua","entryMode":"once","capabilities":[]
    }"#,
    )?;
    std::fs::write(package.join("main.lua"), "return require('eqoa')")?;
    let discovered = super::discovery::discover(&scratch.0);
    if !discovered.diagnostics.is_empty() || discovered.addons.len() != 1 {
        return Err(format!(
            "mod-api package discovery failed: {:?}",
            discovered.diagnostics
        )
        .into());
    }
    let addon = &discovered.addons[0];

    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .map_err(|e| GraphicsUnavailable(format!("adapter request failed: {e}")))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))
        .map_err(|e| GraphicsUnavailable(format!("device request failed: {e}")))?;
    let mut context = Context::create();
    context.set_ini_filename::<PathBuf>(None)?;
    let renderer = Rc::new(RefCell::new(WgpuRenderer::new(
        WgpuInitInfo::new(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        ),
        &mut context,
    )?));
    // Matches Manager::prepare's service install order, including managed visibility.
    // Nothing executes the entry chunk, a frame, a game action, or a media load.
    let host = super::host::Host::for_addon(addon)?;
    super::file_io::install(&host, addon, Some(scratch.0.join("data")))?;
    let visibility = super::visibility::VisibilityLease::new(Rc::new(RefCell::new(
        super::health::HealthBarClaimsRegistry::new(),
    )))?;
    visibility.update(host.snapshot.get().health, false);
    super::visibility::install_managed_health_adapter(&host, &visibility)?;
    super::player::install_player_adapter(&host)?;
    let _entry = host.compile_addon_entry(addon)?;
    let _bridge = super::bridge::Bridge::install(&host)?;

    let textures = super::textures::Textures::new(&device, &queue, renderer);
    super::textures::Textures::install_for_addon(&textures, &host.lua, addon)?;
    let sounds = Rc::new(RefCell::new(
        super::sound::SoundBank::new().map_err(|e| e.to_string())?,
    ));
    super::sound::SoundBank::install_for_addon(&sounds, &host.lua, addon)?;
    let fonts = Rc::new(RefCell::new(super::fonts::FontBank::new(
        0,
        super::fonts::current_context_epoch(),
    )));
    super::fonts::FontBank::install_for_addon(&fonts, &host.lua, addon)?;

    inspect(&host)
}

fn collect_reference(host: &super::host::Host) -> crate::Result<Json> {
    // Read the public Lua API, rather than the private seed/descriptor storage.
    let list: Table = host
        .lua
        .load("return require('eqoa').capabilities.list('')")
        .eval()?;
    let mut rows = Vec::new();
    for row in list.sequence_values::<Table>() {
        let row = row?;
        let children: Table = row.get("children")?;
        rows.push(json!({
            "path":row.get::<String>("path")?,
            "status":row.get::<String>("status")?,
            "revision":row.get::<u32>("revision")?,
            "round":row.get::<Option<u8>>("round")?,
            "reason":row.get::<Option<String>>("reason")?,
            "children":children.sequence_values::<String>().collect::<mlua::Result<Vec<_>>>()?,
        }));
    }
    rows.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let date: String = host.lua.load("return os.date('!%Y-%m-%d')").eval()?;
    Ok(json!({
        "schema_version":1,"api":"eqoa/1",
        "metadata":{
            "client_version":include_str!("../../../../VERSION").trim(),
            "commit":crate::update::COMMIT.unwrap_or("unstamped local build"),
            "platform":format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            "generated_utc":date,
        },
        "capabilities":rows,
    }))
}

fn string<'a>(row: &'a Json, key: &str) -> &'a str {
    row[key].as_str().unwrap_or("")
}

fn cell(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
}

fn table(out: &mut String, rows: &[&Json]) {
    out.push_str("| Path | Status | Since round | Reason |\n|---|---|---|---|\n");
    for row in rows {
        let round = row["round"]
            .as_u64()
            .map(|n| n.to_string())
            .unwrap_or_default();
        writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            cell(string(row, "path")),
            cell(string(row, "status")),
            round,
            cell(string(row, "reason"))
        )
        .unwrap();
    }
    out.push('\n');
}

fn markdown(doc: &Json) -> String {
    let metadata = &doc["metadata"];
    let mut out = String::from("# Mod API support table\n\nGenerated, do not edit.\n\n");
    for (label, key) in [
        ("Client", "client_version"),
        ("Commit", "commit"),
        ("Platform", "platform"),
        ("Date (UTC)", "generated_utc"),
    ] {
        writeln!(out, "- {label}: {}", cell(string(metadata, key))).unwrap();
    }
    out.push_str("\nGenerated by `eqoa mod-api`. The committed reference is generated on Windows, the published platform; sound differs elsewhere.\n\nStatuses and round labels are the running client's registry claims. Forge and ImGui parity also rely on bridge tests and bundled UI tests. JSON includes every path, revision and child list. Regeneration: [Developer tools](../DEV-TOOLS.md).\n\n");
    let mut modules: BTreeMap<String, Vec<&Json>> = BTreeMap::new();
    let mut forge: BTreeMap<String, Vec<&Json>> = BTreeMap::new();
    let (mut events, mut fields, mut imgui, mut aliases) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for row in doc["capabilities"].as_array().unwrap() {
        let path = string(row, "path");
        let parts: Vec<_> = path.split('.').collect();
        if path.starts_with("frontiers_forge.") {
            forge
                .entry(parts[..parts.len().min(2)].join("."))
                .or_default()
                .push(row);
        } else if path.starts_with("eqoa.imgui")
            || path.starts_with("ImGui.")
            || path.starts_with("ImGuiWindowFlags.")
            || path.starts_with("ImGuiCond.")
            || path.contains("DrawList.")
        {
            imgui.push(row);
        } else if path.starts_with("eqoa.WeaponSets")
            || path.starts_with("eqoa.ChatHistory")
            || !path.starts_with("eqoa.")
        {
            aliases.push(row);
        } else if path.contains(".data.")
            || parts
                .iter()
                .skip(1)
                .any(|s| s.starts_with(char::is_uppercase))
        {
            fields.push(row);
        } else if path.starts_with("eqoa.events.")
            && !matches!(parts[2], "on" | "off" | "once" | "after")
        {
            events.push(row);
        } else {
            modules.entry(parts[..2].join(".")).or_default().push(row);
        }
    }
    for (name, rows) in modules {
        writeln!(out, "## {name}\n").unwrap();
        table(&mut out, &rows);
    }
    for (name, rows) in [
        ("Events", events),
        ("Record fields", fields),
        ("ImGui / DrawList", imgui),
        ("Legacy aliases", aliases),
    ] {
        writeln!(out, "## {name}\n").unwrap();
        table(&mut out, &rows);
    }
    out.push_str("## Forge compatibility\n\nForge compatibility is listed by presence, not verified per function; it is not part of the `eqoa/1` promise. Each module lists its member count and every entry whose status is not `supported`.\n\n");
    for (name, rows) in forge {
        let members: Vec<_> = rows
            .iter()
            .copied()
            .filter(|r| string(r, "path") != name)
            .collect();
        let supported = members
            .iter()
            .filter(|r| string(r, "status") == "supported")
            .count();
        writeln!(
            out,
            "### {name}\n\n{} members; {supported} supported.\n",
            members.len()
        )
        .unwrap();
        let exceptions = rows
            .into_iter()
            .filter(|r| string(r, "status") != "supported")
            .collect::<Vec<_>>();
        table(&mut out, &exceptions);
    }
    format!("{}\n", out.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    fn with_graphics<T>(result: crate::Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) if error.is::<GraphicsUnavailable>() => {
                eprintln!("SKIP round7b mod-api reference: {error}");
                None
            }
            Err(error) => panic!("mod-api install failed: {error}"),
        }
    }

    /// The media records that need an ImGui context but no GPU: checked on every machine, CI included.
    #[test]
    fn round7c_media_records_without_a_gpu_match_their_fields() {
        super::super::big_stack(|| {
            let scratch = Scratch::new().unwrap();
            let package = scratch.0.join("media_fields");
            std::fs::create_dir(&package).unwrap();
            std::fs::write(
                package.join("mod.json"),
                r#"{"schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":"media_fields","entry":"main.lua","capabilities":[]}"#,
            )
            .unwrap();
            std::fs::write(package.join("main.lua"), "-- fixture").unwrap();
            let addon = super::super::discovery::discover(&scratch.0).addons.remove(0);
            let mut context = dear_imgui_rs::Context::create();
            context.set_ini_filename::<PathBuf>(None).unwrap();
            let host = super::super::host::Host::for_addon(&addon).unwrap();
            let _bridge = super::super::bridge::Bridge::install(&host).unwrap();
            let fonts = Rc::new(RefCell::new(super::super::fonts::FontBank::new(
                0,
                super::super::fonts::current_context_epoch(),
            )));
            super::super::fonts::FontBank::install_for_addon(&fonts, &host.lua, &addon).unwrap();
            super::super::native_tests::install_field_check(&host).unwrap();
            host.execute(
                r#"
                fields('client.DisplayMetrics', eqoa.client.display())
                -- reason: only a font that fell back to the default
                fields('fonts.FontInfo', eqoa.fonts.default(), 'reason')
            "#,
            )
            .unwrap();
        });
    }

    /// The media record that needs a GPU texture. Without a GPU adapter it prints a SKIP the gate counts.
    #[test]
    fn round7c_texture_record_matches_its_fields() {
        super::super::big_stack(|| {
            match with_reference_host(render::wgpu::Instance::default(), |host| {
                super::super::native_tests::install_field_check(host)?;
                host.execute(
                    "fields('textures.TextureInfo', eqoa.textures.rgba(string.char(255, 0, 0, 255), 1, 1))",
                )?;
                Ok(())
            }) {
                Ok(()) => {}
                Err(error) if error.is::<GraphicsUnavailable>() => {
                    eprintln!("SKIP round7c_texture_record_matches_its_fields: {error}");
                }
                Err(error) => panic!("{error}"),
            }
        });
    }

    /// Round 7H fix 3: `eqoa.textures.formats.png` and `.rgba` are capability flags a mod probes with
    /// `eqoa.capabilities.has`; there is no `eqoa.textures.formats` table, and the descriptor says so.
    #[test]
    fn round7h_fix3_texture_formats_are_described_as_capability_flags() {
        super::super::big_stack(|| {
            match with_reference_host(render::wgpu::Instance::default(), |host| {
                host.execute(
                    r#"
                    for _, f in ipairs({'png', 'rgba'}) do
                        local c = eqoa.capabilities.get('eqoa.textures.formats.' .. f)
                        assert(c.status == 'supported' and eqoa.capabilities.has(c.path), f)
                        assert(c.reason and c.reason:find('eqoa.capabilities.has', 1, true), f .. ': reason ' .. tostring(c.reason))
                    end
                    assert(eqoa.capabilities.get('eqoa.textures.formats').status == 'unknown', 'no formats table is described')
                "#,
                )?;
                Ok(())
            }) {
                Ok(()) => {}
                Err(error) if error.is::<GraphicsUnavailable>() => {
                    eprintln!("SKIP round7h_fix3_texture_formats_are_described_as_capability_flags: {error}");
                }
                Err(error) => panic!("{error}"),
            }
        });
    }

    #[test]
    fn round7c_forge_section_says_presence_is_not_verification() {
        let doc = json!({"metadata": {}, "capabilities": [{
            "path": "frontiers_forge.chat.SendChatText", "status": "supported", "revision": 1,
            "round": 2, "reason": null, "children": []
        }]});
        let text = markdown(&doc);
        let forge = text.split("## Forge compatibility\n\n").nth(1).unwrap();
        assert!(
            forge.starts_with(
                "Forge compatibility is listed by presence, not verified per function; it is not part of the `eqoa/1` promise."
            ),
            "{forge}"
        );
    }

    #[test]
    fn round7b_without_adapter_is_an_explicit_skip() {
        let mut descriptor =
            render::wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        descriptor.backends = render::wgpu::Backends::empty();
        let instance = render::wgpu::Instance::new(descriptor);
        assert!(with_graphics(reference_with_instance(instance)).is_none());
    }

    #[test]
    fn round7b_scratch_cleans_during_unwind() {
        let scratch = Scratch::new().unwrap();
        let path = scratch.0.clone();
        let caught = std::panic::catch_unwind(move || {
            std::fs::write(scratch.0.join("partial-output"), "partial").unwrap();
            panic!("exercise scratch cleanup on a failed assertion");
        });
        assert!(caught.is_err());
        let survived = path.exists();
        if survived {
            std::fs::remove_dir_all(&path).unwrap();
        }
        assert!(!survived, "scratch directory survived unwinding");
    }

    #[test]
    fn round7b_command_dispatch_and_output() {
        assert!(run(&args(&[])).is_none());
        assert!(run(&args(&["boot"])).is_none());
        let dir = Scratch::new().unwrap();
        let out = dir.0.join("surface.json");
        let Some(()) = with_graphics(crate::run(&args(&[
            "mod-api",
            "--format",
            "json",
            "--out",
            out.to_str().unwrap(),
        ]))) else {
            return;
        };
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
        assert!(
            json["capabilities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["path"] == "eqoa.player.get")
        );
        let md = dir.0.join("surface.md");
        let Some(()) =
            with_graphics(run(&args(&["mod-api", "--out", md.to_str().unwrap()])).unwrap())
        else {
            return;
        };
        let text = std::fs::read_to_string(md).unwrap();
        for section in [
            "Generated, do not edit",
            "## eqoa.player",
            "## Events",
            "## Record fields",
            "## ImGui / DrawList",
            "## Legacy aliases",
            "## Forge compatibility",
        ] {
            assert!(text.contains(section), "missing {section}");
        }
        let fields = text.split("## Record fields\n\n").nth(1).unwrap();
        assert!(
            fields.contains("`eqoa.Color.a`"),
            "root record fields must be in the record section"
        );
    }

    #[test]
    fn round7b_command_refuses_invalid_options() {
        for options in [
            vec!["mod-api", "--format", "xml"],
            vec!["mod-api", "--out"],
            vec!["mod-api", "--format"],
            vec!["mod-api", "extra"],
        ] {
            let error = run(&args(&options)).unwrap().unwrap_err().to_string();
            assert!(
                error.contains("usage: eqoa mod-api"),
                "{options:?}: {error}"
            );
        }
    }

    #[test]
    #[cfg(windows)]
    fn round7b_support_table_matches_the_running_client() {
        const REGENERATE: &str = "Rebuild eqoa, then run: eqoa mod-api --format md --out docs/modding/SUPPORT-TABLE.md AND eqoa mod-api --format json --out docs/modding/eqoa-1-surface.json";
        let Some(mut current) = with_graphics(reference()) else {
            return;
        };
        let docs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/modding");
        let saved: serde_json::Value = serde_json::from_slice(
            &std::fs::read(docs.join("eqoa-1-surface.json")).expect(REGENERATE),
        )
        .expect(REGENERATE);
        // Provenance belongs to the generation run, not the API contract.
        current["metadata"] = saved["metadata"].clone();
        assert_eq!(current, saved, "{REGENERATE}");
        let saved_md = std::fs::read_to_string(docs.join("SUPPORT-TABLE.md")).expect(REGENERATE);
        assert_eq!(
            markdown(&current),
            saved_md.replace("\r\n", "\n"),
            "{REGENERATE}"
        );
    }

    const FROZEN_POLICY: &str = "Policy (docs/modding/VERSIONING.md, the versioning policy): eqoa/1 is additive only. A downgrade or removal of a supported name needs the owner's approval and a player note; a breaking change is eqoa/2. To record an approved addition run: python tools/eqoa1_freeze.py update";

    /// One line per frozen path that is gone from `current` or no longer `supported`.
    ///
    /// Covers NAMES and their `supported` status only. Not covered here: a supported function whose body
    /// starts refusing while its registration stays; a record field no longer projected; an event no longer
    /// emitted; a changed argument or return shape; names whose status comes from presence rather than a
    /// registration (Forge and UiForge layers, `ImGui.*`/`eqoa.imgui.*`, `bit.*`, `annotate_host` in
    /// `native.rs`). Fields and events are covered by `round7c_play_records_match_declared_fields`,
    /// `round7c_supported_event_inventory_has_fixture_coverage`,
    /// `round7c_host_service_events_reach_lua_from_their_producers`, `round7c_world_events_come_from_play_changes`
    /// and `round7c_item_records_and_window_events_use_projected_native_state`; shapes and meanings rest on
    /// the spec and reviews.
    fn frozen_violations(frozen: &Json, current: &Json) -> Vec<String> {
        let rows: std::collections::HashMap<&str, &Json> = current["capabilities"]
            .as_array()
            .expect("capabilities")
            .iter()
            .map(|row| (row["path"].as_str().expect("path"), row))
            .collect();
        let mut out = Vec::new();
        for path in frozen["supported"].as_array().expect("supported") {
            let path = path.as_str().expect("path");
            match rows.get(path) {
                None => out.push(format!("{path}: supported -> removed")),
                Some(row) if row["status"] != "supported" => out.push(format!(
                    "{path}: supported -> {}",
                    row["status"].as_str().unwrap_or("?")
                )),
                _ => {}
            }
        }
        out
    }

    fn frozen_record() -> Json {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/modding/eqoa-1-frozen.json");
        serde_json::from_slice(&std::fs::read(path).expect("docs/modding/eqoa-1-frozen.json"))
            .expect("frozen record is JSON")
    }

    /// The frozen record, turned into a surface that has exactly its names (no GPU needed).
    fn surface_of_frozen(frozen: &Json) -> Json {
        let rows: Vec<Json> = frozen["supported"]
            .as_array()
            .unwrap()
            .iter()
            .map(|path| json!({"path":path,"status":"supported","children":[]}))
            .collect();
        json!({"capabilities":rows})
    }

    /// The executed negative and positive controls of the check itself, on every machine.
    #[test]
    fn eqoa1_frozen_surface_controls() {
        fn rows(s: &mut Json) -> &mut Vec<Json> {
            s["capabilities"].as_array_mut().unwrap()
        }
        let frozen = frozen_record();
        let base = surface_of_frozen(&frozen);
        assert!(frozen_violations(&frozen, &base).is_empty());
        let mut gone = base.clone();
        let first = rows(&mut gone).remove(0);
        assert_eq!(
            frozen_violations(&frozen, &gone),
            vec![format!("{}: supported -> removed", first["path"].as_str().unwrap())]
        );
        let mut down = base.clone();
        rows(&mut down)[0]["status"] = json!("unsupported");
        assert!(frozen_violations(&frozen, &down)[0].ends_with("supported -> unsupported"));
        let mut grown = base.clone();
        rows(&mut grown).push(json!({"path":"eqoa.new_name","status":"supported","children":[]}));
        assert!(frozen_violations(&frozen, &grown).is_empty());
        // A frozen leaf may gain a supported child: additions never fail the guard.
        let mut child = base.clone();
        let leaf = rows(&mut child)[0]["path"].as_str().unwrap().to_string();
        rows(&mut child)[0]["children"] = json!([format!("{leaf}.new_child")]);
        rows(&mut child).push(json!({"path":format!("{leaf}.new_child"),"status":"supported","children":[]}));
        assert!(frozen_violations(&frozen, &child).is_empty());
        assert!(!frozen["supported"].as_array().unwrap().iter().any(|p| p == "eqoa.new_name"));
    }

    // ---- The final-freeze rule: proven and provisional names (docs/modding/VERSIONING.md).
    //
    // Not live yet. During the candidate phase `frozen_violations` above is the guard, unchanged. These
    // functions are the rule that replaces it at the final freeze: a provisional name may then be
    // corrected, but only when `docs/modding/eqoa-1-corrections.json` lists it and FORK-UPDATE-NOTES.md
    // names it. With an empty corrections list the rule gives exactly `frozen_violations`' answer
    // (`eqoa1_final_rule_equals_the_live_guard_without_corrections`).

    /// The tier file `docs/modding/eqoa-1-tiers.json`: the provisional names and the FrontiersForge
    /// compatibility names (`compat`, which keep today's rule). Every other frozen name is proven.
    struct TierRecord {
        provisional: BTreeSet<String>,
        compat: BTreeSet<String>,
    }

    /// The names the FrontiersForge compatibility layer owns (`frontiers_forge`, `UiForge`, `bit`).
    fn is_compat_root(name: &str) -> bool {
        ["frontiers_forge", "UiForge", "bit"]
            .iter()
            .any(|root| name == *root || name.strip_prefix(root).is_some_and(|rest| rest.starts_with('.')))
    }

    fn read_tier_names(
        tiers: &Json,
        key: &str,
        known: &BTreeSet<&str>,
        problems: &mut Vec<String>,
    ) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let Some(list) = tiers[key].as_array() else {
            problems.push(format!("tiers file: `{key}` is not a list"));
            return out;
        };
        for item in list {
            let Some(name) = item.as_str() else {
                problems.push(format!("tiers file: `{key}` holds something that is not a string"));
                continue;
            };
            if !known.contains(name) {
                problems.push(format!("tiers file: `{key}` lists {name}, which is not in the frozen record"));
            }
            if !out.insert(name.to_string()) {
                problems.push(format!("tiers file: `{key}` lists {name} twice"));
            }
        }
        out
    }

    /// Reads a tiers file against the frozen record. `Err` has one line per way it is malformed.
    fn parse_tiers(frozen: &Json, tiers: &Json) -> Result<TierRecord, Vec<String>> {
        let known: BTreeSet<&str> = frozen["supported"]
            .as_array()
            .expect("supported")
            .iter()
            .map(|p| p.as_str().expect("path"))
            .collect();
        let mut problems = Vec::new();
        if tiers["api"] != "eqoa/1" {
            problems.push("tiers file: `api` is not \"eqoa/1\"".to_string());
        }
        if tiers["schema_version"] != 1 {
            problems.push("tiers file: `schema_version` is not 1".to_string());
        }
        let provisional = read_tier_names(tiers, "provisional", &known, &mut problems);
        let compat = read_tier_names(tiers, "compat", &known, &mut problems);
        for name in provisional.intersection(&compat) {
            problems.push(format!("tiers file: {name} is both provisional and compat"));
        }
        for name in &known {
            if is_compat_root(name) != compat.contains(*name) {
                problems.push(format!(
                    "tiers file: {name} {} in `compat`, but {}",
                    if compat.contains(*name) { "is" } else { "is not" },
                    if is_compat_root(name) {
                        "it belongs to the FrontiersForge layer"
                    } else {
                        "it is not part of the FrontiersForge layer"
                    }
                ));
            }
        }
        if problems.is_empty() {
            Ok(TierRecord { provisional, compat })
        } else {
            Err(problems)
        }
    }

    fn non_empty<'a>(entry: &'a Json, key: &str) -> Option<&'a str> {
        entry[key].as_str().filter(|s| !s.trim().is_empty())
    }

    /// Reads `docs/modding/eqoa-1-corrections.json` (`[{ "name", "what", "notes_version" }]`): the set of
    /// names whose correction is declared. Only a provisional name can be corrected.
    fn parse_corrections(corrections: &Json, tiers: &TierRecord) -> Result<BTreeSet<String>, Vec<String>> {
        let mut problems = Vec::new();
        let mut out = BTreeSet::new();
        let Some(list) = corrections.as_array() else {
            return Err(vec!["corrections file: not a list".to_string()]);
        };
        for entry in list {
            let (Some(name), Some(_), Some(_)) = (
                non_empty(entry, "name"),
                non_empty(entry, "what"),
                non_empty(entry, "notes_version"),
            ) else {
                problems.push(format!(
                    "corrections file: an entry needs a non-empty `name`, `what` and `notes_version`: {entry}"
                ));
                continue;
            };
            if !tiers.provisional.contains(name) {
                problems.push(format!(
                    "corrections file: {name} is not a provisional name; a proven or compat name cannot be corrected"
                ));
            }
            if !out.insert(name.to_string()) {
                problems.push(format!("corrections file: {name} is listed twice"));
            }
        }
        if problems.is_empty() { Ok(out) } else { Err(problems) }
    }

    /// The final-freeze guard. A proven or compat name that changed status or disappeared is a violation
    /// (as in `frozen_violations`); so is a provisional one, unless `corrected` lists it and `notes` names
    /// it in backticks. The lines for violations that remain are exactly `frozen_violations`' lines.
    fn final_freeze_violations(
        frozen: &Json,
        current: &Json,
        tiers: &TierRecord,
        corrected: &BTreeSet<String>,
        notes: &str,
    ) -> Vec<String> {
        frozen_violations(frozen, current)
            .into_iter()
            .filter_map(|line| {
                let name = line.split(": ").next().unwrap_or("");
                if !tiers.provisional.contains(name) || !corrected.contains(name) {
                    return Some(line);
                }
                if notes.contains(&format!("`{name}`")) {
                    None
                } else {
                    Some(format!(
                        "{line} (listed in eqoa-1-corrections.json, but FORK-UPDATE-NOTES.md does not name `{name}`)"
                    ))
                }
            })
            .collect()
    }

    fn doc_json(file: &str) -> Json {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/modding").join(file);
        serde_json::from_slice(&std::fs::read(&path).unwrap_or_else(|e| panic!("{file}: {e}")))
            .unwrap_or_else(|e| panic!("{file} is not JSON: {e}"))
    }

    fn update_notes() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../FORK-UPDATE-NOTES.md");
        std::fs::read_to_string(path).expect("FORK-UPDATE-NOTES.md")
    }

    fn committed_tiers() -> TierRecord {
        match parse_tiers(&frozen_record(), &doc_json("eqoa-1-tiers.json")) {
            Ok(tiers) => tiers,
            Err(problems) => panic!("docs/modding/eqoa-1-tiers.json is malformed:\n  {}", problems.join("\n  ")),
        }
    }

    /// A tiny frozen record, tiers and surface for the controls: `a.proven`, `a.prov`, `a.prov2`, `bit.band`.
    fn tiny() -> (Json, Json, Json) {
        let frozen = json!({"supported":["a.proven","a.prov","a.prov2","bit.band"]});
        let tiers = json!({"api":"eqoa/1","schema_version":1,"provisional":["a.prov","a.prov2"],"compat":["bit.band"]});
        (frozen.clone(), tiers, surface_of_frozen(&frozen))
    }

    fn without(surface: &Json, path: &str) -> Json {
        let rows: Vec<Json> = surface["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["path"] != path)
            .cloned()
            .collect();
        json!({"capabilities":rows})
    }

    fn downgraded(surface: &Json, path: &str) -> Json {
        let mut out = surface.clone();
        for row in out["capabilities"].as_array_mut().unwrap() {
            if row["path"] == path {
                row["status"] = json!("unsupported");
            }
        }
        out
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    /// The committed tiers file is well formed against the frozen record, the corrections file reads, and
    /// the final rule passes on today's surface. (Needs no GPU.)
    #[test]
    fn eqoa1_tiers_files_are_well_formed_today() {
        let frozen = frozen_record();
        let tiers = committed_tiers();
        assert!(!tiers.provisional.is_empty() && !tiers.compat.is_empty());
        let corrected = parse_corrections(&doc_json("eqoa-1-corrections.json"), &tiers)
            .unwrap_or_else(|p| panic!("corrections file is malformed:\n  {}", p.join("\n  ")));
        let bad = final_freeze_violations(&frozen, &surface_of_frozen(&frozen), &tiers, &corrected, &update_notes());
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Positive and negative controls of the final-freeze rule, on synthetic input.
    #[test]
    fn eqoa1_final_rule_controls() {
        let (frozen, tiers_json, base) = tiny();
        let tiers = parse_tiers(&frozen, &tiers_json).unwrap();
        let ok = |current: &Json, corrected: &[&str], notes: &str| {
            final_freeze_violations(&frozen, current, &tiers, &set(corrected), notes)
        };
        assert!(ok(&base, &[], "").is_empty());
        // A provisional name that disappears is a violation unless it is corrected AND named in the notes.
        let gone = without(&base, "a.prov");
        assert_eq!(ok(&gone, &[], "`a.prov`"), vec!["a.prov: supported -> removed".to_string()]);
        assert!(ok(&gone, &["a.prov"], "`a.prov` was renamed").is_empty());
        // Negative controls: the notes do not name it (or name another, or name it without backticks).
        assert_eq!(ok(&gone, &["a.prov"], "").len(), 1);
        assert_eq!(ok(&gone, &["a.prov"], "`a.prov2` was renamed").len(), 1);
        assert_eq!(ok(&gone, &["a.prov"], "a.prov was renamed").len(), 1);
        assert!(ok(&gone, &["a.prov"], "").remove(0).contains("does not name"));
        // Negative control: named in the notes but not in the corrections file.
        assert_eq!(ok(&gone, &[], "`a.prov` was renamed").len(), 1);
        // A correction covers only its own name.
        let both = without(&gone, "a.prov2");
        assert_eq!(ok(&both, &["a.prov"], "`a.prov` `a.prov2`").len(), 1);
        assert!(ok(&both, &["a.prov", "a.prov2"], "`a.prov` `a.prov2`").is_empty());
        // A provisional name that loses its status is treated the same way.
        let down = downgraded(&base, "a.prov");
        assert_eq!(ok(&down, &[], "`a.prov`"), vec!["a.prov: supported -> unsupported".to_string()]);
        assert!(ok(&down, &["a.prov"], "`a.prov`").is_empty());
        // A proven name is never excused, even with a correction and a note.
        for current in [without(&base, "a.proven"), downgraded(&base, "a.proven")] {
            assert_eq!(ok(&current, &["a.proven"], "`a.proven`").len(), 1);
            assert_eq!(ok(&current, &[], "").len(), 1);
        }
        // A compat name follows today's rule: no correction loosens it.
        for current in [without(&base, "bit.band"), downgraded(&base, "bit.band")] {
            assert_eq!(ok(&current, &["bit.band"], "`bit.band`").len(), 1);
        }
        // Additions never fail the guard.
        let mut grown = base.clone();
        grown["capabilities"].as_array_mut().unwrap().push(json!({"path":"a.new","status":"supported","children":[]}));
        assert!(ok(&grown, &[], "").is_empty());
    }

    /// While the corrections file is empty the final rule must give the live guard's answer (candidate
    /// phase: the guard must not become looser). Checked for proven, provisional and compat names, for a
    /// removal and for a status change, on the real frozen record and tiers file.
    #[test]
    fn eqoa1_final_rule_equals_the_live_guard_without_corrections() {
        let frozen = frozen_record();
        let tiers = committed_tiers();
        let base = surface_of_frozen(&frozen);
        let names: Vec<&str> = frozen["supported"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
        let pick = |wanted: &dyn Fn(&str) -> bool| names.iter().copied().filter(|n| wanted(*n)).take(4).collect::<Vec<_>>();
        let proven = pick(&|n| !tiers.provisional.contains(n) && !tiers.compat.contains(n));
        let provisional = pick(&|n| tiers.provisional.contains(n));
        let compat = pick(&|n| tiers.compat.contains(n));
        assert!(proven.len() == 4 && provisional.len() == 4 && compat.len() == 4);
        let empty = BTreeSet::new();
        for name in proven.iter().chain(&provisional).chain(&compat) {
            for current in [without(&base, name), downgraded(&base, name)] {
                let live = frozen_violations(&frozen, &current);
                assert_eq!(live.len(), 1, "{name}");
                assert_eq!(final_freeze_violations(&frozen, &current, &tiers, &empty, &update_notes()), live, "{name}");
            }
        }
        assert!(final_freeze_violations(&frozen, &base, &tiers, &empty, "").is_empty());
        // Negative control of this comparison: with a correction the two rules differ for a provisional name.
        let name = provisional[0];
        let gone = without(&base, name);
        let corrected = set(&[name]);
        let notes = format!("`{name}` was corrected");
        assert!(final_freeze_violations(&frozen, &gone, &tiers, &corrected, &notes).is_empty());
        assert_eq!(frozen_violations(&frozen, &gone).len(), 1);
    }

    /// Malformed tiers and corrections files are refused, each way.
    #[test]
    fn eqoa1_tier_file_malformations_are_caught() {
        let (frozen, good, _) = tiny();
        assert!(parse_tiers(&frozen, &good).is_ok());
        let with = |key: &str, value: Json| {
            let mut t = good.clone();
            t[key] = value;
            parse_tiers(&frozen, &t).err().map(|p| p.join("\n")).unwrap_or_default()
        };
        // A provisional name the frozen record does not have.
        assert!(with("provisional", json!(["a.prov", "a.ghost"])).contains("a.ghost, which is not in the frozen record"));
        // Listed twice.
        assert!(with("provisional", json!(["a.prov", "a.prov"])).contains("a.prov twice"));
        // Both provisional and compat.
        assert!(with("compat", json!(["bit.band", "a.prov"])).contains("a.prov is both provisional and compat"));
        // A FrontiersForge name left out of compat, or an ordinary name put in.
        assert!(with("compat", json!([])).contains("bit.band is not in `compat`"));
        assert!(with("compat", json!(["bit.band", "a.proven"])).contains("a.proven is in `compat`"));
        // Wrong shape and wrong api.
        assert!(with("provisional", json!("a.prov")).contains("not a list"));
        assert!(with("provisional", json!([7])).contains("not a string"));
        assert!(with("api", json!("eqoa/2")).contains("`api`"));
        assert!(with("schema_version", json!(2)).contains("schema_version"));

        let tiers = parse_tiers(&frozen, &good).unwrap();
        let entry = |name: &str| json!({"name":name,"what":"renamed","notes_version":"0.9.0"});
        assert_eq!(parse_corrections(&json!([]), &tiers).unwrap(), BTreeSet::new());
        assert_eq!(parse_corrections(&json!([entry("a.prov")]), &tiers).unwrap(), set(&["a.prov"]));
        let err = |value: Json| parse_corrections(&value, &tiers).err().map(|p| p.join("\n")).unwrap_or_default();
        assert!(err(json!({})).contains("not a list"));
        assert!(err(json!([entry("a.proven")])).contains("not a provisional name"));
        assert!(err(json!([entry("bit.band")])).contains("not a provisional name"));
        assert!(err(json!([entry("a.prov"), entry("a.prov")])).contains("a.prov is listed twice"));
        assert!(err(json!([{"name":"a.prov","what":"renamed"}])).contains("needs a non-empty"));
        assert!(err(json!([{"name":"a.prov","what":" ","notes_version":"0.9.0"}])).contains("needs a non-empty"));
    }

    /// The same final rule against the live surface (needs a GPU, like `eqoa1_frozen_surface_only_grows`):
    /// with no corrections it reports exactly what the live guard reports, which is nothing today.
    #[test]
    #[cfg(windows)]
    fn eqoa1_final_rule_agrees_with_the_live_surface() {
        let Some(current) = with_graphics(reference()) else {
            return;
        };
        let frozen = frozen_record();
        let live = frozen_violations(&frozen, &current);
        let tiers = committed_tiers();
        let rule = final_freeze_violations(&frozen, &current, &tiers, &BTreeSet::new(), &update_notes());
        assert_eq!(rule, live);
        assert!(rule.is_empty(), "eqoa/1 frozen surface changed:\n  {}\n{FROZEN_POLICY}", rule.join("\n  "));
    }

    #[test]
    #[cfg(windows)]
    fn eqoa1_frozen_surface_only_grows() {
        let Some(current) = with_graphics(reference()) else {
            return;
        };
        let bad = frozen_violations(&frozen_record(), &current);
        assert!(
            bad.is_empty(),
            "eqoa/1 frozen surface changed:
  {}
{FROZEN_POLICY}",
            bad.join("\n  ")
        );
    }
}
