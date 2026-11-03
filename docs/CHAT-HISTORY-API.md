# Chat history API

`eqoa.ChatHistory()` is an Enhanced-only addon call that exposes the game's
native chat-history scroll to a Lua window that draws its own chat log
(Wereoxxs UI). Without it, a claimed `chat_window` has no way to hear R2, so
holding R2 scrolls nothing and the native chat scrolls a window nobody sees.

Stock behavior is unchanged except for the HUD while reading: when no addon
claims the chat window, R2 opens the native `VIWndChat` history exactly as
before, and the ability bar and minimap stay up (see below).

## Contract

```lua
local h = eqoa.ChatHistory()
-- h.active  boolean: the game is scrolling an addon-owned chat right now
-- h.steps   integer: lines scrolled since this host's previous call
```

- `active` is true while the player is in game, the game is in chat mode 33
  (R2 held, or the keyboard's C), and an addon claims `chat_window`. In every
  other case it is false and `steps` is 0.
- `steps` is positive for back/up (older lines) and negative for forward/down,
  one unit per visual (wrapped) line, matching the native window. The first
  repeat arrives after 0.45 s and then every 0.05 s while the key is held, as
  `WndChat::draw` does.
- The count is kept by the game and only read by the API. Each addon host keeps
  its own baseline and gets its own delta; repeated calls in one frame return 0
  after the first for that host. A host that polls rarely gets the accumulated
  delta, capped at 8 lines per call, so a missed frame cannot jump the log far.
  Calls are not consumed by rendering order.
- The first call after a host loads reports no scroll: the host is seeded with
  the game's current count.
- Leaving mode 33 without a key-up (a dialog, zoning, death) stops the repeat;
  the API reports `active == false` on the next frame.

An addon should treat `active` turning false as "the player let go": snap the
log back to the newest line, then resume its own stick-to-bottom behavior.

## Using it

```lua
local function ChatHistory()
    if eqoa == nil or eqoa.ChatHistory == nil then
        return nil          -- PCSX2/FrontiersForge, or an older Enhanced build
    end
    local ok, value = pcall(eqoa.ChatHistory)
    if ok and type(value) == "table" then
        return value
    end
    return nil
end
```

Then, in the message child window:

```lua
local history = ChatHistory()
local active = history ~= nil and history.active == true
if active then
    local steps = tonumber(history.steps) or 0
    if steps ~= 0
        and type(ImGui.GetTextLineHeightWithSpacing) == "function"
        and type(ImGui.SetScrollY) == "function" then
        local y = ImGui.GetScrollY() - steps * ImGui.GetTextLineHeightWithSpacing()
        local max_y = ImGui.GetScrollMaxY()
        if y < 0 then y = 0 elseif y > max_y then y = max_y end
        ImGui.SetScrollY(y)
    end
elseif was_active then
    ImGui.SetScrollHereY(1.0)   -- released: newest line
elseif ImGui.GetScrollY() >= ImGui.GetScrollMaxY() - 4 then
    ImGui.SetScrollHereY(1.0)   -- stock stick-to-bottom
end
was_active = active
```

Guard the API call rather than requiring `eqoa`: under FrontiersForge on PCSX2
the table does not exist, and a bare `eqoa.ChatHistory()` errors. Guard the two
scroll calls as well: an older bridge may not expose them, and calling a nil
field disables the whole addon. The bundled
`wereoxxs_ui/modules/chat_window.lua` uses exactly this shape, so its log keeps
the stock stick-to-bottom behavior when either is absent.

## Why the ability bar stays up

The game normally hides the HUD carousel and the right side of the HUD (the
minimap, quest tracker and Stats Box) in mode 33, because the native chat
window was the only thing that needed the screen. The owner has asked for them
to stay up while R2 (or the keyboard's C) reads the history, so `Play` keeps
both drawing in mode 33 whether or not `chat_window` is claimed. They do not
overlap: in the HUD's 640x448 frame the native history box sits at about
y 336..444, under the minimap (y 50..162) and under the ability bar's menus
(which end by y ~300). Other modes are untouched, R2 still scrolls the native
chat when no addon claims it, and the d-pad still belongs to the chat while
R2 is held.

## Implementation

- `crates/eqoa/src/fork_ext/chat_history.rs`: held key, repeat clock, mode-leave
  clear, running count, snapshot.
- `crates/eqoa/src/fork_ext/hud.rs`: `chat_history_keeps_ability_bar`,
  `chat_history_keeps_right_hud`, `chat_history_key`, and the `tick` in
  `draw_chat_window`.
- `crates/eqoa/src/play.rs`: one tap in `chat_scroll_key` (mode 33's key path),
  the ability-bar condition and the right-HUD condition.
- `crates/eqoa/src/fork_ext/addons/chat_history.rs`: the `eqoa.ChatHistory`
  table, with a per-host baseline and the per-call cap.
- `crates/eqoa/src/fork_ext/addons/bridge_widgets.rs`: the real bridge's
  `GetTextLineHeightWithSpacing` and `SetScrollY`.
- `crates/eqoa/src/fork_ext/addons/host.rs`: snapshot field and managed reset.

Tests: `fork_ext/chat_history.rs` (timing and mode-leave clear),
`fork_ext/addons/chat_history_tests.rs` (API deltas and cap, ability-bar claim,
bundled Lua scroll/repeat/release, the no-`eqoa` and no-bridge-call controls,
and a scan of every bundled Lua file against the real bridge table), and
`fork_ext/play.rs` (mode 33 wiring).
