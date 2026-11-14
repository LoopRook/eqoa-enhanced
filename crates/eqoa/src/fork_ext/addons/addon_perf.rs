// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! What each addon costs per frame, printed under `EQOA_PERF` beside upstream's own `perf:` line.
//!
//! Every timer here is behind `enabled()` (the environment, read once): with `EQOA_PERF` unset
//! nothing is timed, nothing is stored and no wrapper is installed. One line per five seconds:
//! `addons: <id> <ms> (lua <ms>, api <ms>, bridge <ms>, draw <ms>), ..., imgui <ms>`, each a mean per
//! frame. `lua` is the addon's own Lua (callbacks and render events) with the native calls it makes
//! taken out; `api` is those `eqoa.*` calls; `bridge` is the Forge ImGui translation (the `ImGui`
//! table and the draw lists); `draw` is the native HUD layer drawing the addon's queue; `imgui` is
//! the shared ImGui render. A second line, `addon-api: <id> <call> <n>x <ms>, ...`, names each
//! addon's costliest `eqoa.*` calls (the per-frame time and number of calls).
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    sync::OnceLock,
    time::{Duration, Instant},
};

const PERIOD: Duration = Duration::from_secs(5);
/// Calls named per addon in the `addon-api:` line.
const TOP_CALLS: usize = 6;

#[cfg(test)]
thread_local! {
    static FORCED: Cell<Option<bool>> = const { Cell::new(None) };
}

/// True under `EQOA_PERF` (read once).
pub(crate) fn enabled() -> bool {
    #[cfg(test)]
    if let Some(on) = FORCED.with(Cell::get) {
        return on;
    }
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("EQOA_PERF").is_some())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Api,
    Bridge,
}

#[derive(Default)]
struct Slot {
    lua: Duration,
    api: Duration,
    bridge: Duration,
    draw: Duration,
    /// per `eqoa.*` call: how many and how long
    calls: BTreeMap<String, (u32, Duration)>,
}

struct State {
    slots: BTreeMap<String, Slot>,
    /// native work shared by all addons, by name: how many times and how long (not exclusive of one another)
    sections: BTreeMap<&'static str, (u32, Duration)>,
    /// the addon whose Lua is running
    current: Option<String>,
    /// a native call or bridge call is open (the outermost one is timed)
    open: bool,
    imgui: Duration,
    frames: u32,
    since: Instant,
}

impl State {
    fn new() -> State {
        State { slots: BTreeMap::new(), sections: BTreeMap::new(), current: None, open: false, imgui: Duration::ZERO, frames: 0, since: Instant::now() }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::new());
}

fn ms(d: Duration, frames: u32) -> f64 {
    d.as_secs_f64() * 1e3 / frames.max(1) as f64
}

/// The addon's Lua is running until this drops.
pub(crate) struct LuaScope {
    id: String,
    start: Instant,
    inner_at_start: Duration,
    outer: Option<String>,
}

pub(crate) fn lua_scope(id: &str) -> Option<LuaScope> {
    if !enabled() {
        return None;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let slot = s.slots.entry(id.to_owned()).or_default();
        let inner_at_start = slot.api + slot.bridge;
        let outer = s.current.replace(id.to_owned());
        Some(LuaScope { id: id.to_owned(), start: Instant::now(), inner_at_start, outer })
    })
}

impl Drop for LuaScope {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        let _ = STATE.try_with(|s| {
            let mut s = s.borrow_mut();
            let slot = s.slots.entry(self.id.clone()).or_default();
            let inner = (slot.api + slot.bridge).saturating_sub(self.inner_at_start);
            slot.lua += elapsed.saturating_sub(inner);
            s.current = self.outer.take();
        });
    }
}

/// An `eqoa.*` call or a bridge call, timed until this drops (only the outermost of nested ones).
pub(crate) struct CallTimer {
    kind: Kind,
    name: String,
    start: Instant,
}

fn call(kind: Kind, name: &str) -> Option<CallTimer> {
    if !enabled() {
        return None;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.open || s.current.is_none() {
            return None;
        }
        s.open = true;
        Some(CallTimer { kind, name: name.to_owned(), start: Instant::now() })
    })
}

/// A native `eqoa.<module>.<name>` call (`symbol` is the full name).
pub(crate) fn api(symbol: &str) -> Option<CallTimer> {
    call(Kind::Api, symbol)
}

/// The Forge ImGui translation: an `ImGui.*` function or a draw-list call.
pub(crate) fn bridge(name: &str) -> Option<CallTimer> {
    call(Kind::Bridge, name)
}

impl Drop for CallTimer {
    fn drop(&mut self) {
        let took = self.start.elapsed();
        let _ = STATE.try_with(|s| {
            let mut s = s.borrow_mut();
            s.open = false;
            let Some(id) = s.current.clone() else { return };
            let slot = s.slots.entry(id).or_default();
            match self.kind {
                Kind::Bridge => slot.bridge += took,
                Kind::Api => {
                    slot.api += took;
                    let name = self.name.strip_prefix("eqoa.").unwrap_or(&self.name);
                    if let Some(c) = slot.calls.get_mut(name) {
                        c.0 += 1;
                        c.1 += took;
                    } else {
                        slot.calls.insert(name.to_owned(), (1, took));
                    }
                }
            }
        });
    }
}

/// A named stretch of shared native work (`core:` in the line), timed until this drops.
pub(crate) struct SectionTimer {
    name: &'static str,
    start: Instant,
}

pub(crate) fn section(name: &'static str) -> Option<SectionTimer> {
    enabled().then(|| SectionTimer { name, start: Instant::now() })
}

impl Drop for SectionTimer {
    fn drop(&mut self) {
        let took = self.start.elapsed();
        let _ = STATE.try_with(|s| {
            let mut s = s.borrow_mut();
            let e = s.sections.entry(self.name).or_default();
            e.0 += 1;
            e.1 += took;
        });
    }
}

/// The native HUD layer drew an addon's queue in `took`.
pub(crate) fn add_draw(id: &str, took: Duration) {
    if !enabled() {
        return;
    }
    STATE.with(|s| s.borrow_mut().slots.entry(id.to_owned()).or_default().draw += took);
}

/// The shared ImGui render (every addon's windows at once) took `took`.
pub(crate) fn add_imgui(took: Duration) {
    if !enabled() {
        return;
    }
    STATE.with(|s| s.borrow_mut().imgui += took);
}

/// Wraps each function of an `ImGui` table so its time counts as the bridge's.
pub(crate) fn wrap_imgui(lua: &mlua::Lua, imgui: &mlua::Table) -> mlua::Result<()> {
    if !enabled() {
        return Ok(());
    }
    let pairs: Vec<(mlua::Value, mlua::Value)> = imgui.pairs::<mlua::Value, mlua::Value>().collect::<mlua::Result<_>>()?;
    for (key, value) in pairs {
        if let mlua::Value::Function(f) = value {
            let name = match &key {
                mlua::Value::String(s) => format!("ImGui.{}", s.to_string_lossy()),
                _ => continue,
            };
            let wrapped = lua.create_function(move |_, args: mlua::MultiValue| {
                let _t = bridge(&name);
                f.call::<mlua::MultiValue>(args)
            })?;
            imgui.raw_set(key, wrapped)?;
        }
    }
    Ok(())
}

/// Wraps each function of a Forge host module (`frontiers_forge.*`, the native adapters) so its time
/// counts as a native call, named `<module>.<function>`.
pub(crate) fn wrap_module(lua: &mlua::Lua, module: &mlua::Table, name: &str) -> mlua::Result<()> {
    if !enabled() {
        return Ok(());
    }
    let pairs: Vec<(mlua::Value, mlua::Value)> = module.pairs::<mlua::Value, mlua::Value>().collect::<mlua::Result<_>>()?;
    for (key, value) in pairs {
        if let (mlua::Value::Function(f), mlua::Value::String(k)) = (value, &key) {
            let symbol = format!("eqoa.{name}.{}", k.to_string_lossy());
            let wrapped = lua.create_function(move |_, args: mlua::MultiValue| {
                let _t = api(&symbol);
                f.call::<mlua::MultiValue>(args)
            })?;
            module.raw_set(key, wrapped)?;
        }
    }
    Ok(())
}

/// One manager frame ended: counts it, and prints once the period is over.
pub(crate) fn tick() {
    if !enabled() {
        return;
    }
    let due = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.frames += 1;
        s.since.elapsed() >= PERIOD
    });
    if due {
        for line in take_report() {
            crate::say(&line);
        }
    }
}

/// The lines for the frames since the last report, and a fresh period begun.
pub(crate) fn take_report() -> Vec<String> {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let lines = format_lines(&s);
        let current = s.current.take();
        let open = s.open;
        *s = State::new();
        s.current = current;
        s.open = open;
        lines
    })
}

fn format_lines(s: &State) -> Vec<String> {
    let n = s.frames;
    let mut rows: Vec<(&String, &Slot)> = s.slots.iter().filter(|(_, v)| v.lua + v.api + v.bridge + v.draw > Duration::ZERO).collect();
    rows.sort_by_key(|(_, v)| std::cmp::Reverse(v.lua + v.api + v.bridge + v.draw));
    if rows.is_empty() && s.imgui == Duration::ZERO {
        return Vec::new();
    }
    let mut parts: Vec<String> = rows
        .iter()
        .map(|(id, v)| {
            let mut detail = format!("lua {:.2}", ms(v.lua, n));
            if v.api > Duration::ZERO {
                detail += &format!(", api {:.2}", ms(v.api, n));
            }
            if v.bridge > Duration::ZERO {
                detail += &format!(", bridge {:.2}", ms(v.bridge, n));
            }
            detail += &format!(", draw {:.2}", ms(v.draw, n));
            format!("{id} {:.2} ({detail})", ms(v.lua + v.api + v.bridge + v.draw, n))
        })
        .collect();
    parts.push(format!("imgui {:.2}", ms(s.imgui, n)));
    let mut lines = vec![format!("addons: {}", parts.join(", "))];
    if !s.sections.is_empty() {
        let mut shared: Vec<(&&str, &(u32, Duration))> = s.sections.iter().collect();
        shared.sort_by_key(|(_, (_, d))| std::cmp::Reverse(*d));
        let parts: Vec<String> = shared.iter().map(|(name, (count, d))| format!("{name} {:.1}x {:.3}", *count as f64 / n.max(1) as f64, ms(*d, n))).collect();
        lines.push(format!("core: {}", parts.join(", ")));
    }
    for (id, v) in rows {
        let mut calls: Vec<(&String, &(u32, Duration))> = v.calls.iter().collect();
        calls.sort_by_key(|(_, (_, d))| std::cmp::Reverse(*d));
        if calls.is_empty() {
            continue;
        }
        let top: Vec<String> = calls
            .iter()
            .take(TOP_CALLS)
            .map(|(name, (count, d))| format!("{name} {:.1}x {:.3}", *count as f64 / n.max(1) as f64, ms(*d, n)))
            .collect();
        lines.push(format!("addon-api: {id} {}", top.join(", ")));
    }
    lines
}

/// Test hook: turns the timers on for this thread (the environment is read once, process-wide).
#[cfg(test)]
pub(crate) fn force_for_test(on: bool) {
    FORCED.with(|f| f.set(Some(on)));
    STATE.with(|s| *s.borrow_mut() = State::new());
}

/// Test hook: whether anything was stored.
#[cfg(test)]
pub(crate) fn is_empty_for_test() -> bool {
    STATE.with(|s| {
        let s = s.borrow();
        s.slots.is_empty() && s.sections.is_empty() && s.imgui == Duration::ZERO && s.current.is_none()
    })
}

/// Test hook: an addon's mean ms a frame in the period so far (all of lua, api, bridge and draw).
#[cfg(test)]
pub(crate) fn peek_addon_ms(id: &str) -> f64 {
    STATE.with(|s| {
        let s = s.borrow();
        s.slots.get(id).map_or(0.0, |v| ms(v.lua + v.api + v.bridge + v.draw, s.frames))
    })
}

/// Test hook: a shared section's mean ms a frame in the period so far.
#[cfg(test)]
pub(crate) fn peek_section_ms(name: &str) -> f64 {
    STATE.with(|s| {
        let s = s.borrow();
        s.sections.get(name).map_or(0.0, |v| ms(v.1, s.frames))
    })
}
