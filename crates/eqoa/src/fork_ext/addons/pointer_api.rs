//! Window pointer observations and frame-lived addon rectangle claims.
use super::{
    host::Host,
    native::{bind, error},
    native_core::{self, CORE},
};
use mlua::{Result, Table, Value};
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap};
#[derive(Default)]
struct Pointer {
    at: [f32; 2],
    inside: bool,
    claims: BTreeMap<u64, (u64, Vec<[f32; 4]>)>,
    wheels: BTreeMap<u64, f32>,
}
thread_local! {static POINTER:RefCell<Pointer>=RefCell::new(Pointer::default());}
pub(crate) fn position(at: [f32; 2], inside: bool, size: (u32, u32)) {
    POINTER.with(|p| {
        let mut p = p.borrow_mut();
        p.at = render::ui::frame_from_window(size, at);
        p.inside = inside;
    });
}
pub(crate) fn release_vm(vm: u64) {
    POINTER.with(|p| {
        let mut p = p.borrow_mut();
        p.claims.remove(&vm);
        p.wheels.remove(&vm);
    });
}
pub(crate) fn take_wheel() -> f32 {
    POINTER.with(|p| {
        let mut p = p.borrow_mut();
        let sum = p.wheels.values().sum::<f32>().clamp(-20.0, 20.0);
        p.wheels.clear();
        sum
    })
}
fn claimed() -> bool {
    let frame = CORE.with(|c| c.borrow().frame);
    POINTER.with(|p| {
        let p = p.borrow();
        p.inside
            && p.claims.values().any(|(f, rs)| {
                frame.saturating_sub(*f) <= 1
                    && rs.iter().any(|r| {
                        p.at[0] >= r[0]
                            && p.at[0] <= r[0] + r[2]
                            && p.at[1] >= r[1]
                            && p.at[1] <= r[1] + r[3]
                    })
            })
    })
}
fn emit(name: &str, button: Option<&str>, steps: Option<f32>) {
    if !native_core::capture_requested() {
        return;
    }
    let data = POINTER.with(|p| {
        let p = p.borrow();
        json!({"x":p.at[0],"y":p.at[1],"inside_window":p.inside,"button":button,"steps":steps})
    });
    native_core::emit(name, data);
}
pub(crate) fn observe(event: &winit::event::WindowEvent, size: (u32, u32)) -> bool {
    use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent as E};
    match event {
        E::CursorMoved { position: p, .. } => position([p.x as f32, p.y as f32], true, size),
        E::CursorLeft { .. } | E::Focused(false) => {
            POINTER.with(|p| {
                let mut p = p.borrow_mut();
                p.inside = false;
                p.claims.clear();
                p.wheels.clear();
            });
        }
        E::MouseInput {
            state: ElementState::Pressed,
            button,
            ..
        } => emit(
            "pointer.pressed",
            Some(match button {
                MouseButton::Left => "left",
                MouseButton::Right => "right",
                MouseButton::Middle => "middle",
                _ => "other",
            }),
            None,
        ),
        E::MouseWheel { delta, .. } => {
            let n = match delta {
                MouseScrollDelta::LineDelta(_, y) => *y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            };
            emit("pointer.wheel", None, Some(n));
        }
        _ => {}
    }
    !crate::hudlayout::unlocked() && claimed()
}
pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    bind(
        lua,
        "input",
        "pointer",
        lua.create_function(|lua, ()| {
            let data = POINTER.with(|p| {
                let p = p.borrow();
                json!({"x":p.at[0],"y":p.at[1],"inside_window":p.inside})
            });
            lua.pack_multi((
                super::native::record_cached(
                    lua,
                    "eqoa.input.pointer",
                    "eqoa.input.Pointer",
                    &data,
                )?,
                Value::Nil,
            ))
        })?,
    )?;
    bind(
        lua,
        "input",
        "claim_pointer",
        lua.create_function(|lua, t: Table| {
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(
                    lua,
                    "eqoa.input.claim_pointer",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let mut r = [
                t.get::<f32>("x")?,
                t.get("y")?,
                t.get::<Option<f32>>("width")?
                    .or(t.get("w")?)
                    .unwrap_or(0.0),
                t.get::<Option<f32>>("height")?
                    .or(t.get("h")?)
                    .unwrap_or(0.0),
            ];
            if !t.get::<Value>("frame_x")?.is_nil() {
                let a = render::ui::frame_from_window(
                    super::hud_layout::get_window_size(),
                    [r[0], r[1]],
                );
                let m = super::hud_layout::frame_metrics(super::hud_layout::get_window_size());
                r = [a[0], a[1], r[2] / m.2, r[3] / m.3];
            }
            if !r.iter().all(|v| v.is_finite() && v.abs() <= 32768.0) || r[2] <= 0.0 || r[3] <= 0.0
            {
                return error(
                    lua,
                    "eqoa.input.claim_pointer",
                    "invalid_argument",
                    "finite positive rectangle required",
                );
            }
            let vm = super::native_events::bus(lua).borrow().vm;
            let frame = CORE.with(|c| c.borrow().frame);
            let ok = POINTER.with(|p| {
                let mut p = p.borrow_mut();
                let (f, rs) = p.claims.entry(vm).or_insert_with(|| (frame, Vec::new()));
                if *f != frame {
                    *f = frame;
                    rs.clear();
                }
                if rs.len() >= 32 {
                    return false;
                }
                rs.push(r);
                true
            });
            if !ok {
                return error(
                    lua,
                    "eqoa.input.claim_pointer",
                    "quota",
                    "at most 32 rectangles per render frame",
                );
            }
            lua.pack_multi((true, Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "input",
        "minimap_wheel",
        lua.create_function(|lua, n: f32| {
            if !n.is_finite() || n.abs() > 20.0 {
                return error(
                    lua,
                    "eqoa.input.minimap_wheel",
                    "invalid_argument",
                    "wheel steps must be finite and at most 20",
                );
            }
            if !CORE.with(|c| c.borrow().view.in_world) {
                return error(
                    lua,
                    "eqoa.input.minimap_wheel",
                    "not_in_world",
                    "enter a loaded world first",
                );
            }
            let vm = super::native_events::bus(lua).borrow().vm;
            POINTER.with(|p| {
                let mut p = p.borrow_mut();
                let v = p.wheels.entry(vm).or_default();
                *v = (*v + n).clamp(-20.0, 20.0);
            });
            lua.pack_multi((true, Value::Nil))
        })?,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round4_pointer_claim_routes_window_coordinates_and_expires() {
        CORE.with(|c| {
            *c.borrow_mut() = super::super::native_core::Core::default();
            c.borrow_mut().view.in_world = true;
        });
        let h = Host::new().unwrap();
        position(render::ui::window_from_frame((640,448),[50.0,60.0]), true, (640, 448));
        h.execute("local p=eqoa.input.pointer();assert(math.abs(p.x-50)<0.001 and math.abs(p.y-60)<0.001 and p.inside_window);assert(eqoa.input.claim_pointer{x=40,y=50,width=30,height=30})").unwrap();
        assert!(claimed());
        position([10.0, 10.0], true, (640, 448));
        assert!(!claimed());
        position(render::ui::window_from_frame((640,448),[50.0,60.0]), true, (640, 448));
        CORE.with(|c| c.borrow_mut().frame += 2);
        assert!(!claimed());
        h.execute("assert(eqoa.input.minimap_wheel(1))").unwrap();
        assert_eq!(take_wheel(), 1.0);
        assert_eq!(take_wheel(), 0.0);
        release_vm(super::super::native_events::bus(&h.lua).borrow().vm);
    }
}
