# ARPG and Avoids native compatibility diagnosis (2026-10-31)

Pinned read-only reference: FrontiersForge v0.3.2 (4026d5c), UiForge 6f2d125,
sol2 ImGui ec6f9ef. Paths below are relative to run/forge-compat-reference.
The unchanged packages are the source of the API inventory, including modules they require.

## Confirmed gaps before implementation

| API | Reference contract and citation |
| --- | --- |
| DrawList:AddImageRounded | Required texture, p_min, p_max, uv_min, uv_max, U32 tint, rounding; optional corner flags; no returns. Real sol_ImGui.h:2591-2593 (under UiForge/externals/sol2_ImGui_Bindings). |
| ImGui.ColorEdit3 | label, RGB table, optional integer flags; missing components default to zero. Returns a fresh three-element RGB table, changed boolean. Same header:1338-1365,3068-3071. |
| ImGui.BeginGroup / EndGroup | No arguments or returns; native group bounds/layout. Same header:192-193,2794-2795. |
| ImGui.Indent / Unindent | Optional float width (default zero selects native style indent); no returns. Same header:188-191,2786-2793. |
| UiForge.scripts_path | String containing active script root; UiForge/src/core/core.cpp:916. resources_path already exists in the texture bridge. |
| Util.LogError | message; forwards to UiForge.Log(2,message) if available; otherwise drops silently; no returns. scripts/modules/frontiers_forge/util.lua:99-119. |
| Util.GetExpRequiredForLevel | Existing API returned the native HUD formula, which differs from the pinned table at levels 50-59. Forge table is scripts/modules/frontiers_forge/util.lua:59-73,358-360; the getter returns table[level], including nil for 0/60. The adapter now matches that table. |
| ImGui.SliderFloat literal percent format | Avoids passes string.format("%d%%", n), producing literal strings such as "25%", in modules/avoids_health_bar.lua:128-129 and avoids_mana_bar.lua:128-129. The existing bridge rejected these settings; numeric percent literals now become safe printf literals, preserving unsafe-conversion rejection. |
| frontiers_forge.quest_log | max_quests=8; GetCount() returns 0..8, invalid counts become zero; GetQuestByIndex(index) uses zero-based packed journal order and returns nil outside range or with no game context; Quests() returns an iterator yielding zero-based index, quest until nil. scripts/modules/frontiers_forge/quest_log.lua:12,21-61. |
| Quest:GetName | UTF-16 journal title converted to UTF-8; string (including empty). No IDs, descriptions, goals, or completion fields exist in the reference quest. scripts/modules/frontiers_forge/quest.lua:4-9,33-35. Native source is Play.quests.titles (questlog.rs:14-20), not the optional guide/tracker. |
| io.open; file:lines, :write, :close; os.remove | Missed by the initial assessment. Lua 5.1 standard I/O: reads return nil on missing file; lines yields lines without terminators; write accepts successive strings and returns file; close returns true; remove returns true on success or nil,error. Avoids initializer calls io.open unconditionally (avoids_modern_ui.lua:122); notes use all four methods (modules/avoids_quest_log.lua:147-171); legacy migration calls os.remove (avoids_modern_ui.lua:136). Native host previously omitted io and stripped os.remove. |

## Complete package API inventory

Each entry below was checked against bridge.rs, bridge_drawlist.rs, bridge_widgets.rs,
host.rs and the matching native frontiers_forge adapter. The APIs above were absent;
all other listed APIs were present. Lua math/string/table/os.clock come from the Lua 5.1
stdlib. Package-local functions are excluded. Dynamic pcall targets and module properties
are included explicitly where applicable. This is an API availability audit, not live-game acceptance.

### arpg_ui

Required modules: `frontiers_forge.ability_bar`, `frontiers_forge.ability_list`, `frontiers_forge.effects`, `frontiers_forge.entity`, `frontiers_forge.entity_list`, `frontiers_forge.icon`, `frontiers_forge.input`, `frontiers_forge.player`, `frontiers_forge.ui`, `frontiers_forge.util`.

- `ImGui.Begin`, `ImGui.BeginCombo`, `ImGui.Button`, `ImGui.CalcTextSize`, `ImGui.Checkbox`, `ImGui.ColorEdit3`, `ImGui.Dummy`, `ImGui.End`, `ImGui.EndCombo`, `ImGui.GetColorU32`, `ImGui.GetCursorScreenPos`, `ImGui.GetTime`, `ImGui.GetWindowDrawList`, `ImGui.GetWindowPos`, `ImGui.IsMouseHoveringRect`, `ImGui.PopFont`, `ImGui.PushFont`, `ImGui.Selectable`, `ImGui.Separator`, `ImGui.SetNextWindowPos`, `ImGui.SetNextWindowSize`, `ImGui.SetTooltip`, `ImGui.SetWindowPos`, `ImGui.SliderFloat`, `ImGui.SliderInt`, `ImGui.Text`, `ImGui.TextWrapped`, `ImGui.TreeNode`, `ImGui.TreePop`.
- `UiForge.LoadFont`, `UiForge.LoadTexture`, `UiForge.RegisterCallback`, `UiForge.ReleaseTexture`.
- `ImVec2.new`.
- `AbilityBar.GetAbility`, `AbilityBar.GetItem`, `AbilityBar.GetSelectedBarIndex`, `AbilityBar.GetSelectedSlotIndex`, `AbilityBar.GetSlotCount`, `AbilityBar.GetSlotUITexId`.
- `AbilityList.Acquire`, `AbilityList.GetAbilityByDisplayIndex`, `AbilityList.Off`, `AbilityList.On`, `AbilityList.Release`, `AbilityList.Update`.
- `Effects.All`, `Effects.GetCount`.
- `EntityList.GetEntityById`.
- `Icon.Acquire`, `Icon.GetTexture`, `Icon.GetUITexture`, `Icon.Release`.
- `Input.AcquireDpadRotation`, `Input.GetDpadHookError`, `Input.IsDpadHookSuspended`, `Input.ReleaseDpadRotation`, `Input.SetDpadRotated`, `Input.Update`.
- `Player.GetCoordinates`, `Player.GetCurrentHp`, `Player.GetCurrentPwr`, `Player.GetExp`, `Player.GetLevel`, `Player.GetMaxHp`, `Player.GetMaxPwr`, `Player.GetPetEntityId`, `Player.GetTargetEntityId`.
- `UI.DisableExperienceBars`, `UI.DisableHealthBar`, `UI.DisablePowerBar`, `UI.EnableExperienceBars`, `UI.EnableHealthBar`, `UI.EnablePowerBar`.
- `Util.GetExpRequiredForLevel`, `Util.IsInGame`, `Util.IsStartMenuOpen`, `Util.IsUiBusy`.
- `ability:GetCastTime`, `ability:GetCooldownLockoutMs`, `ability:GetDescription`, `ability:GetDisplayIndex`, `ability:GetIconBackgroundRef`, `ability:GetIconForegroundRef`, `ability:GetName`, `ability:GetPwrCost`, `ability:IsInRange`, `ability:IsOnCooldown`.
- `item:GetDescription`, `item:GetIconRef`, `item:GetName`.
- `entity:GetPosition`.
- `dl:AddImage`, `dl:AddImageRounded`, `dl:AddLine`, `dl:AddRect`, `dl:AddRectFilled`, `dl:AddText`, `dl:AddTriangleFilled`, `dl:PopClipRect`, `dl:PushClipRect`.
- `os.clock`.
- `math.abs`, `math.ceil`, `math.cos`, `math.floor`, `math.max`, `math.min`, `math.sin`, `math.sqrt`.
- `string.format`.

### avoids_modern_ui

Required modules: `avoids_ability_bar`, `avoids_chat_log`, `avoids_common`, `avoids_effects`, `avoids_exp_bars`, `avoids_group_frames`, `avoids_health_bar`, `avoids_mana_bar`, `avoids_persist`, `avoids_pet_frame`, `avoids_quest_log`, `avoids_target_level`, `frontiers_forge.ability_bar`, `frontiers_forge.ability_list`, `frontiers_forge.chat`, `frontiers_forge.effects`, `frontiers_forge.entity_list`, `frontiers_forge.group`, `frontiers_forge.icon`, `frontiers_forge.input`, `frontiers_forge.player`, `frontiers_forge.quest_log`, `frontiers_forge.ui`, `frontiers_forge.util`.

- `ImGui.Begin`, `ImGui.BeginGroup`, `ImGui.Button`, `ImGui.CalcTextSize`, `ImGui.Checkbox`, `ImGui.CollapsingHeader`, `ImGui.ColorEdit4`, `ImGui.Dummy`, `ImGui.End`, `ImGui.EndGroup`, `ImGui.GetColorU32`, `ImGui.GetContentRegionAvail`, `ImGui.GetCursorPos`, `ImGui.GetCursorScreenPos`, `ImGui.GetTime`, `ImGui.GetWindowDrawList`, `ImGui.Indent`, `ImGui.InputText`, `ImGui.IsItemActive`, `ImGui.IsItemHovered`, `ImGui.PopClipRect`, `ImGui.PopStyleColor`, `ImGui.PopTextWrapPos`, `ImGui.PushClipRect`, `ImGui.PushStyleColor`, `ImGui.PushTextWrapPos`, `ImGui.RadioButton`, `ImGui.SameLine`, `ImGui.Separator`, `ImGui.SetCursorPos`, `ImGui.SetKeyboardFocusHere`, `ImGui.SetNextWindowSize`, `ImGui.SetScrollHereY`, `ImGui.SetTooltip`, `ImGui.SetWindowFontScale`, `ImGui.SliderFloat`, `ImGui.SliderInt`, `ImGui.Text`, `ImGui.TextUnformatted`, `ImGui.Unindent`.
- `UiForge.RegisterCallback`.
- `ImVec2.new`.
- `AbilityBar.GetSelectedBarIndex`, `AbilityBar.GetSelectedSlotIndex`, `AbilityBar.GetSlotCount`.
- `AbilityList.Acquire`, `AbilityList.Off`, `AbilityList.On`, `AbilityList.Release`, `AbilityList.Update`.
- `Effects.All`, `Effects.GetCount`.
- `EntityList.GetAllEntities`, `EntityList.GetEntityById`.
- `Group.GetMemberCount`, `Group.IsInGroup`, `Group.IsSelfLeader`, `Group.Members`.
- `Icon.Acquire`, `Icon.GetTexture`, `Icon.Release`.
- `Input.AcquireKey`, `Input.GetKeyOwner`, `Input.IsKeyHookInstalled`, `Input.PollKeyEvents`, `Input.ReleaseKeyHook`, `Input.SetKeySuppressed`, `Input.SetTextCaptureActive`.
- `Player.GetCurrentHp`, `Player.GetCurrentPwr`, `Player.GetExp`, `Player.GetLevel`, `Player.GetMaxHp`, `Player.GetMaxPwr`, `Player.GetTargetEntityId`.
- `UI.DisableAbilityBar`, `UI.DisableActiveEffectsDisplay`, `UI.DisableChatWindow`, `UI.DisableGroupDisplay`, `UI.DisableHealthBar`, `UI.DisableMainExpBar`, `UI.DisablePetPanel`, `UI.DisablePowerBar`, `UI.DisableSecondaryExpBar`, `UI.EnableAbilityBar`, `UI.EnableActiveEffectsDisplay`, `UI.EnableChatWindow`, `UI.EnableGroupDisplay`, `UI.EnableHealthBar`, `UI.EnableMainExpBar`, `UI.EnablePetPanel`, `UI.EnablePowerBar`, `UI.EnableSecondaryExpBar`.
- `Util.GetExpRequiredForLevel`, `Util.IsInGame`, `Util.IsStartMenuOpen`, `Util.LogError`.
- `Chat.InstallSendHook`, `Chat.Off`, `Chat.On`, `Chat.SendChatText`, `Chat.UninstallSendHook`, `Chat.Update`.
- `QuestLog.GetCount`.
- `ability:GetCooldownLockoutMs`, `ability:GetDisplayIndex`, `ability:GetId`, `ability:GetName`, `ability:IsInRange`, `ability:IsOnCooldown`.
- `member:GetCoordinates`, `member:GetHealthPercent`, `member:GetName`, `member:IsActive`.
- `quest:GetName`.
- `draw_list:AddRect`, `draw_list:AddRectFilled`, `draw_list:AddText`.
- `io.open`.
- `os.clock`.
- `file:close`, `file:lines`, `file:write`.
- `math.floor`, `math.max`, `math.min`, `math.sqrt`.
- `string.format`.
- `table.concat`, `table.insert`, `table.remove`, `table.sort`.
- Passed as a function value: `QuestLog.GetQuestByIndex`. Properties: `UiForge.scripts_path`, `UiForge.resources_path`, `UiForge.CallbackType`, `Chat.Events`, `Input.Key`, ImGui flag/color enums.

## Implementation and bounded native storage

The journal snapshot comes from the retail journal, independent of optional quest-guide
or server-tracker data. All reference-exposed fields are available; no invented IDs or
objectives, and no unknown-field fallback was needed.

File I/O is the text subset these packages use (r/w open, lines/write/close, remove),
not unrestricted Lua I/O. Virtual script-root resource paths read from the unchanged
source or an addon-specific config overlay. Writes and migration-removal tombstones
stay in config/addons/<identity>-files. Limits: 1 MiB per file, 16 retained file buffers,
64 stored files/markers, path depth 8; symlinks/reparse points, traversal, code paths,
other packages and alternate streams are rejected. Iterators share file data, cursor,
closed state and quota ownership. Closing frees data and quota even with retained iterators.

During validation, the new rounded-image test using legacy lightuserdata ID 1 crashed
with 0xC0000005. The cause was pre-existing texture_ref conversion: it assigned the ID
to _TexData as a pointer. The corrected bridge leaves _TexData null and sets only _TexID,
as the pinned void* texture constructor does. The same ID now emits tessellated geometry
without dereferencing it. PNG handle validation and frame/window/geometry guards remain.

## Verification record

- Baseline: 8 new tests failed (0 passed), including both unchanged package loads.
- Additional high-level XP regression failed before the formula correction.
- Avoids full settings invocation failed on literal percentage format before correction.
- Corrected focused run: 16 passed, 0 failed; no package fixture skips. Additional iterator
  budget regression is included in the final full-suite run.
- Independent read-only review found and verified fixes for iterator allocation ownership,
  close quota release and deletion/recreation at the file cap; no important findings remain.
- Live game acceptance remains pending. The pinned reference returns nil for level 60 XP;
  Avoids compares that value without a guard. That reference-package limitation is retained.

The incorrect legacy-pointer branch is present in both v0.2.0 and v0.2.1 and is
shared by the released DrawList:AddImage overloads. ImGui.Image uses the separate,
already-correct Textures::raw path. Texture/icon APIs return full Handle userdata;
no production addon API was found exporting lightuserdata. Consequently the defect
is latent in released code, but no normal Wereoxxs/addon Lua route to its required
input was demonstrated; the regression injects that value from Rust.

Final native suite (outside sandbox for Windows audio): 1373 passed, zero failed,
2 ignored. The initial sandboxed run had 1370 passed/3 MCI initialization failures;
the isolated MCI test passed outside the sandbox before the full rerun. No sound
code or sound tests were changed. All 17 new modern-addon regressions passed.

Final suite printed 106 prerequisite SKIP notices: 4 symlink-privilege cases,
2 optional pinned GPU/hearts fixture prerequisites, and 100 disc/module/save-state/
WORDS data prerequisites. The two ignored cases are fx::bench::fans_cover and
fx::bench::volumes_cost. Neither new package fixture skipped. WASM check and final
footprint policy check passed. Initial WASM OS112 disk-full failure was resolved by
removing only this worktree's rebuildable target/debug cache after saving logs.

Independent reviewer: gpt-6-astra, three review turns, about four minutes wall time;
token/runtime accounting was unavailable. No edits or cargo runs by the reviewer.

Release build passed: cargo build --release --locked -p eqoa (3m35s).
All requested checks passed on the final source. Full command logs remain in
target/local-checks (untracked build artifacts). No live-game acceptance claimed.

After restoring the original changelog heading encoding, WASM recheck passed
(4.66s) and the release executable rebuilt successfully (1m21s). Addon Rust
source is unchanged from the complete passing native test run.
