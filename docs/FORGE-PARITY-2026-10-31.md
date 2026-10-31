# Forge parity left after the Wereoxxs gate (assessment, 2026-10-31)

Read-only assessment by a Gemini Flash scout against develop (3e04337) and the pinned reference
(`run/forge-compat-reference/`). "Implemented" was checked by reading our code per called function, but
it is a worker's assessment: confirm each "likely works" package with the owner's live test before
advertising it. Native runtime: 14 of 25 FrontiersForge modules (ability, ability_bar, ability_list,
chat, combat, effects, entity, entity_list, group, icon, input, player, ui, util), plus UiForge, ImGui
and DrawList.

| Package | Verdict | What is missing |
|---|---|---|
| wereoxxs_ui | Works (owner live test 2026-10-31) | window positions persist (in progress) |
| screen_grid | Likely works now | needs a live check |
| uiforge_example | Likely works now | needs a live check (sound, fonts, clicks) |
| retro_health_hearts | Likely works now (worked in the early preview) | needs a live check under damage |
| arpg_ui | Small gaps | `DrawList:AddImageRounded`, `ImGui.ColorEdit3` |
| avoids_modern_ui | Small gaps | `UiForge.scripts_path`, `ImGui.BeginGroup/EndGroup/Indent/Unindent`, `util.LogError`, `frontiers_forge.quest_log` |
| world_map (optional) | Large gaps, not blocked | JPEG textures; ~13 ImGui mouse/tooltip APIs (`BeginTooltip`, `GetMousePos`, `GetMouseWheel`, drag delta, `IsMouseDown/Clicked`); `AddImageQuad`; map helpers (`GetCompassRadians`, `GetWorldId`, `ListFilesInDir`) |
| mini_map (optional) | Blocked by design | LuaJIT `ffi` + `Util.EEmem()` reading PCSX2 guest memory and a software rasterizer over it; the native minimap stays the default |
| ff_example (API harness) | Large gaps / partly blocked | `EEmem`; modules camera, inventory, item, loot, merchant, window, quest_log; `profiles_path` |

Most-shared missing pieces: `UiForge.scripts_path` (3 packages), `AddImageRounded` (2), `Indent/Unindent` (2),
`quest_log` (2), map helpers (2), tooltip/mouse APIs (2), `AddImageQuad` (2).
