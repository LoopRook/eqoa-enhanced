# Common UI bridge slice

Added real native widgets/scopes for the ff_example/uiforge_example surface. This
is a bounded bridge slice, not acceptance of either complete unchanged addon.

## Source contract and changes

Authority is the locally pinned UiForge binding submodule ec6f9ef7,
`UiForge/externals/sol2_ImGui_Bindings/sol_ImGui.h` (not IDE stubs).

- **Begin:** lines49-69 return one visibility value for Begin(name), or
  `(open, shouldDraw)` for the bool overload. Corrected the reversed tuple. End
  remains mandatory even when Begin returns false. The collapsed-window test
  warms native auto-fit frames before asserting different tuple values; the old
  code then actually returned `false/true` instead of `true/false`.
- **Widgets:** sized/default Button; bool RadioButton activation and integer
  RadioButton `(value, changed)`; table/count Combo with zero-based current index,
  default popup height -1 and `Missing` entries; InputInt `(value, changed)` with
  default steps1/100 and flags0. Existing Checkbox, sliders and both Image overloads
  remain intact.
- **Text/layout:** BulletText, TextColored(r,g,b,a,text), TextDisabled, TextWrapped,
  one-string TextUnformatted, Separator, Dummy and ProgressBar. ProgressBar's
  default size is native `(-FLT_MIN,0)`; negative fractions preserve native
  indeterminate progress. GetTime, IsItemHovered/Active, tooltip, keyboard focus
  (default offset0), Push/PopTextWrapPos (default position0) use actual ImGui.
- **Scopes:** TreeNode/TreePop, CollapsingHeader (including open/draw tuple), menu
  bars/main menu bar/menus/MenuItem, tab bars/items, BeginChild/EndChild. Child and
  window ends are unconditional; tree/menu/tab ends are owed only after true Begin.
  MenuItem selected overload returns `(selected, activated)`; tab bool overload
  returns `(open, selected)`, matching pinned source.
- Exported all window flags used by the pinned UIForge sample, including correct
  NoNav, NoDecoration and NoInputs composites. Added ImGuiChildFlags.Border mapped
  to current native Borders, common tree/tab/hover constants and
  **ImGuiCol.PlotHistogram** style support. Pinned source contains no Lua
  PlotHistogram widget registration; the sample uses that color for ProgressBar.
  No unpinned plot function was invented.

Implementation is confined to bridge.rs and child bridge_widgets.rs, with tests in
child bridge_next_tests.rs. No Host/package/runtime/manager/health/shared/native
maps/dependency/policy changes, source-reference changes, agent, commit or push.

## Safety and bounded differences

Typed scopes are tracked in strict reverse acquisition order, including style,
clip and wrap operations. Mismatched or unclosed ends fail **before native End**;
error recovery unwinds only this invocation's scopes in reverse order. Child
boundaries cannot pop parent resources; orphan tab items and duplicate menu-bar
entry are rejected before native assertions. Nested tab bars require a tab-item
boundary. Scope errors cannot consume a host or neighbor's stacks.

Every call validates current Context/frame epoch. Native window addresses used for
the menu-bar precondition are ephemeral; no native Ui/window pointer is retained
in Lua. Context guards are cleared after invocation, including no-active-frame
failure. Existing budgets remain4096 UI calls and32 stack operations; strings are
bounded to4096 bytes. Combo has256 entries/height maximum and64KiB total text.
Floating arguments must be finite with magnitude at most1,000,000; InputInt values
are bounded to +/-1,000,000,000 and steps to +/-1,000,000. Callback-bearing input
flags, unsupported/internal flags and contradictory tab item flags fail clearly.

All printf-style functions use constant `%s` formats, including text containing
literal `%n`. The pinned two-string TextUnformatted overload passes unrelated C++
string pointers as a begin/end range; that unsafe form is explicitly rejected.
The one-string form is implemented. Combo's NUL-separated-string overload, wider
child flags, fonts/audio and draw-list shape expansion are outside this slice.
Strict scope order is a deliberate bounded host contract; it may reject some
cross-scope interleavings allowed by native ImGui. No fake success is returned.

## Verification

Meaningful RED: the warmed collapsed Begin regression observed the reversed
`false/true` tuple; the common widget regression failed at missing Button. Initial
unwarmed collapse assertions were corrected because ImGui auto-fits new windows
for their initial frames; that fixture failure was not counted as tuple proof.

`cargo test --locked -p eqoa forge_bridge_ -- --nocapture --test-threads=1`:
**8 passed, 0 failed**, no prerequisite skips. This includes all existing Image,
clip and hearts bridge tests plus five new tests:

- Differing collapsed/open tuples, one-return Begin and mandatory End.
- Real native defaults/multireturns, safe literal formats, flags and balanced scopes.
- Closed/mismatched/unclosed child, tree, menu, tab, clip/style/wrap boundaries,
  unsupported input flags, numeric/text/table limits, depth/call quota recovery,
  duplicate menu-bar guard and post-frame pointer revocation.
- Synthetic **Dear ImGui IO** mouse position/down/up events activate Button and
  integer RadioButton; native focus/text/Enter events change InputInt to42 and
  return changed=true. These are actual native widget interactions, not stubbed
  return values or a real OS-window input/DPI playtest.
- Four bad invocation scenarios unwind before a second isolated Host/Bridge runs
  in the **same Context/frame**. Actual WGPU readback verifies the healthy neighbor's
  green line at physical(128,40). This is bridge/VM isolation, not a new Manager
  implementation or complete addon acceptance.

Final pinned serial addon suite: **85 passed, 0 failed**. Four Windows symlink
cases explicitly skipped privileged operations with error1314 (reported as passing
tests, not ignored). No GPU/pinned-source/PNG prerequisite skips. Pinned grid,
unchanged hearts, Image coordinate/tint/background oracles, scope/neighbor GPU
readback and real post-upload allocator rollback all executed. `git diff --check`
passed with line-ending notices only.

```powershell
$env:EQOA_FORGE_SPIKE='C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/scripts/screen_grid.lua'
cargo test --locked -p eqoa fork_ext::addons:: -- --nocapture --test-threads=1
git diff --check
```

No separate native/WASM/full-workspace/release build: module visibility and cfg
remain inside the existing native-only bridge, and the native test binary links
the actual backend. No packaging/publication or live account/server operations.
Full unchanged uiforge_example/ff_example, missing APIs, draw-list geometry and
packaged input/layout/DPI acceptance remain separate gates.
