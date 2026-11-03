# Chat history API

`eqoa.ChatHistory()` is an Enhanced-only addon call that exposes the game's
native chat-history scroll to a Lua window that draws its own chat log
(Wereoxxs UI). Without it, a claimed `chat_window` has no way to hear R2, so
holding R2 scrolls nothing and the native chat scrolls a window nobody sees.

Stock behavior is unchanged: when no addon claims the chat window, R2 opens the
native `VIWndChat` history and the HUD carousel hides exactly as before.

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
  one unit per line, matching the native window. The first repeat arrives after
  0.45 s and then every 0.05 s while the key is held, as `WndChat::draw` does.
- The count is kept by the game and only read by the API. Repeated calls in one
  frame return 0 after the first, so a host may poll freely; calls are not
  consumed by rendering order.
- The first call after a host loads reports no scroll: the host is seeded with
  the game's current count.

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
    if steps ~= 0 then
        local per_line = ImGui.GetTextLineHeightWithSpacing()
        local y = ImGui.GetScrollY() - steps * per_line
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

Guard the call rather than requiring `eqoa`: under FrontiersForge on PCSX2 the
table does not exist, and a bare `eqoa.ChatHistory()` errors. The bundled
`wereoxxs_ui/modules/chat_window.lua` uses exactly this shape, so its log keeps
the stock stick-to-bottom behavior when the API is absent.

## Why the ability bar stays up

Mode 33 normally hides the HUD carousel, because the native chat window is the
only thing that needs the screen. When an addon draws the chat, that reasoning
does not hold, so `Play` keeps the stock ability bar drawing in mode 33 if and
only if `chat_window` is claimed. Other modes are untouched.

## Implementation

- `crates/eqoa/src/fork_ext/chat_history.rs`: held key, repeat clock, running
  count, snapshot.
- `crates/eqoa/src/fork_ext/hud.rs`: `chat_history_keeps_ability_bar`,
  `chat_history_key`, and the `tick` in `draw_chat_window`.
- `crates/eqoa/src/play.rs`: one tap in `chat_scroll_key` (mode 33's key path)
  and the ability-bar condition.
- `crates/eqoa/src/fork_ext/addons/chat_history.rs`: the `eqoa.ChatHistory`
  table, with a per-host baseline.
- `crates/eqoa/src/fork_ext/addons/host.rs`: snapshot field and managed reset.

Tests: `fork_ext/chat_history.rs` (timing), `fork_ext/addons/chat_history_tests.rs`
(API deltas, ability-bar claim, bundled Lua scroll/repeat/release and the
no-`eqoa` control), and `fork_ext/play.rs` (mode 33 wiring).
