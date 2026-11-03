//! `eqoa.ChatHistory()`: the stock chat-history scroll, for addons that draw
//! their own chat window (Wereoxxs UI). Enhanced-only; `eqoa` is absent under
//! FrontiersForge on PCSX2, so addons must guard the call.
use super::host::Host;
use mlua::Table;
use std::cell::Cell;
use std::rc::Rc;

pub fn install(host: &Host) -> mlua::Result<()> {
    let lua = &host.lua;
    let eqoa = match lua.globals().get::<Option<Table>>("eqoa")? {
        Some(t) => t,
        None => lua.create_table()?,
    };
    let state = host.snapshot.clone();
    // Seed from the count at load so the first call reports no invented jump.
    let last = Rc::new(Cell::new(crate::fork_ext::chat_history::total()));
    eqoa.set(
        "ChatHistory",
        lua.create_function(move |lua, ()| {
            let snap = state.get().chat_history;
            let steps = if snap.active {
                let steps = snap.total.saturating_sub(last.get());
                last.set(snap.total);
                steps
            } else {
                // Stock chat (or no chat mode): keep the baseline current and
                // report nothing, so a later claim starts from here.
                last.set(snap.total);
                0
            };
            let out = lua.create_table()?;
            out.set("active", snap.active)?;
            out.set("steps", steps)?;
            Ok(out)
        })?,
    )?;
    lua.globals().set("eqoa", eqoa)
}
