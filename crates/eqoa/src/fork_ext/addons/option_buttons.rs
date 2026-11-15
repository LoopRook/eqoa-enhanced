//! Native option action rows owned by a mod VM; actions run at its next boundary.
use super::{
    host::Host,
    native::{bind, error, record},
    native_events::{self, Bus},
};
use mlua::{Function, Value};
use std::{cell::RefCell, rc::Weak};
struct Button {
    addon: String,
    id: String,
    item: i32,
    label: String,
    bus: Weak<RefCell<Bus>>,
}
thread_local! {static BUTTONS:RefCell<Vec<Button>>=const{RefCell::new(Vec::new())};static NEXT:std::cell::Cell<i32>=const{std::cell::Cell::new(3000)};}
fn active(b: &Button) -> bool {
    super::options::is_addon_enabled(&b.addon)
        && b.bus.upgrade().is_some_and(|bus| bus.borrow().active)
}
pub(super) fn reset() {
    BUTTONS.with(|b| b.borrow_mut().clear());
}
pub(super) fn items() -> Vec<(i32, String)> {
    BUTTONS.with(|b| {
        b.borrow()
            .iter()
            .filter(|b| active(b))
            .map(|b| (b.item, b.label.clone()))
            .collect()
    })
}
pub(crate) fn is_button(item: i32) -> bool {
    BUTTONS.with(|b| b.borrow().iter().any(|b| b.item == item && active(b)))
}
pub(crate) fn activate_button(item: i32) -> bool {
    BUTTONS.with(|buttons| {
        let buttons = buttons.borrow();
        let Some(button) = buttons.iter().find(|b| b.item == item && active(b)) else {
            return false;
        };
        button
            .bus
            .upgrade()
            .is_some_and(|bus| native_events::activate_option(&bus, &button.id))
    })
}
pub(super) fn install(host: &Host) -> mlua::Result<()> {
    bind(
        &host.lua,
        "options",
        "button",
        host.lua
            .create_function(|lua, (spec, callback): (mlua::Table, Function)| {
                let id = spec.get::<String>("id")?;
                let label = spec.get::<String>("label")?;
                if id.is_empty()
                    || id.len() > 64
                    || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    || label.is_empty()
                    || label.len() > 128
                    || label.chars().any(char::is_control)
                {
                    return error(
                        lua,
                        "eqoa.options.button",
                        "invalid_argument",
                        "invalid option id or label",
                    );
                }
                let addon = lua
                    .app_data_ref::<super::sandbox::Mounts>()
                    .map(|m| m.addon.id.clone())
                    .unwrap_or_else(|| super::native::session(lua).borrow().addon.clone());
                if !native_events::register_option(lua, &id, callback) {
                    return error(
                        lua,
                        "eqoa.options.button",
                        "quota",
                        "32 option action rows per VM",
                    );
                }
                let bus = native_events::weak_bus(lua).expect("native event bus");
                let item = BUTTONS.with(|buttons| {
                    let mut buttons = buttons.borrow_mut();
                    buttons.retain(|b| b.bus.upgrade().is_some());
                    if let Some(b) = buttons.iter_mut().find(|b| b.addon == addon && b.id == id) {
                        b.bus = bus;
                        b.label = label.clone();
                        return b.item;
                    }
                    let item = NEXT.with(|n| {
                        let v = n.get();
                        n.set(v + 1);
                        v
                    });
                    buttons.push(Button {
                        addon,
                        id: id.clone(),
                        item,
                        label: label.clone(),
                        bus,
                    });
                    item
                });
                lua.pack_multi((
                    record(
                        lua,
                        "eqoa.options.Button",
                        &serde_json::json!({"id":id,"label":label,"item_id":item}),
                    )?,
                    Value::Nil,
                ))
            })?,
    )
}
