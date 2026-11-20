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
}
