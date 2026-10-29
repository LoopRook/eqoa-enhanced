# Addon testing instructions

An isolated local early preview has been produced from source commit
`7bf02d39a3ce5dc209550a9d61bf344ffe625ec3`. It bundles unchanged `screen_grid`
and `retro_health_hearts` with the four original hearts PNGs, both released
native map fixes, and a separate profile. Use the preview package's launcher
and README for disc setup and live checks. This is not a public release or full
Forge compatibility; FF_Example and larger UI suites remain pending.

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

The unchanged hearts script has passed native GPU fixtures with real PNGs and
changing HP snapshots. Actual server health delivery and live interaction still
require player testing. FF_Example and many game-data/action APIs remain unsupported. Both native maps remain the defaults;
no Forge map port or PCSX2 texture-pack installation is required.

For developer fixtures only, `EQOA_FORGE_MODS` selects an explicit source root and
`EQOA_CONFIG` selects scratch configuration. `EQOA_FORGE_SPIKE` selects the earlier
single-script fixture instead of the managed runtime, with a single Context total.
Do not ship the downloaded source-reference tree or point fixtures at user profiles.

Interactive checks required with the early preview: manager
open/close in pregame and in world, keyboard/IME focus, presses held across overlay
focus changes, mouse drags/releases, losing focus, resize and monitor DPI changes,
render scale x1/x2/x4, restart/profile persistence, and launching from an unrelated
working directory. Existing app offscreen/report captures should include overlays;
standalone CLI `shot_with` is a separate path and does not run addons.
