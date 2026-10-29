# Addon testing instructions

This source work has not been packaged as a player preview. Before packaging,
the integration lead must bring in the already-published native map fixes and
complete the preview checks. These instructions describe the intended reviewed
native build; they are not a release announcement.

1. Open **Options → Mods & Addons → Open Mods Folder**. This folder is beside
   the client executable, regardless of the directory used to launch it.
2. Add a legacy `.lua` file or a complete addon folder. Native packages include
   a `mod.json` manifest. Preserve private `modules` and `resources` folders.
3. Open **Addon Manager** in the same options category and choose **Rescan folder**.
4. Enable the addon. Use **Settings**, **Save profile**, **Load profile** or **Reload**
   as needed. Read the displayed failure/resource/capability explanation when a
   mod fails. Ordinary use does not require environment variables.

Legacy addons default to disabled. Native manifests can request initial enable,
and saved explicit enabled/disabled choices take precedence. Profiles are stored
under the client configuration directory, separate from addon source files.
Moving an addon to a different source path gives it a new profile identity.

A small installation-control example, saved as `example.lua`, uses only the
currently implemented window/text bridge:

```lua
if ImGui.Begin("Example addon") then
    ImGui.Text("Addon loaded")
end
ImGui.End()
```

This is not evidence that FF_Example or health hearts already work. Many Forge
game-data/action APIs are still unsupported. Both native maps remain the defaults;
no Forge map port or PCSX2 texture-pack installation is required.

For developer fixtures only, `EQOA_FORGE_MODS` selects an explicit source root and
`EQOA_CONFIG` selects scratch configuration. `EQOA_FORGE_SPIKE` selects the earlier
single-script fixture instead of the managed runtime, with a single Context total.
Do not ship the downloaded source-reference tree or point fixtures at user profiles.

Interactive checks still required once a concrete preview is packaged: manager
open/close in pregame and in world, keyboard/IME focus, presses held across overlay
focus changes, mouse drags/releases, losing focus, resize and monitor DPI changes,
render scale x1/x2/x4, restart/profile persistence, and launching from an unrelated
working directory. Existing app offscreen/report captures should include overlays;
standalone CLI `shot_with` is a separate path and does not run addons.
