//! Scoped Lua draw lists; native pointers exist only during validated calls.
use super::*;

// Conservative index-equivalent budget. Thick AA strokes use up to 18 indices
// per path point; 24 reserves join/closing overhead too. Native auto circle
// tessellation is capped at 512; allow 520 for arc end points. Rounded rectangles
// reserve that full-circle upper bound, including quarter-arc end points.
const GEOMETRY_LIMIT: usize = 196_608;
const AUTO_POINTS: usize = 520;
const ROUND_RECT_POINTS: usize = AUTO_POINTS;

#[derive(Clone, Copy)]
pub(super) enum Target {
    Background,
    Window(WindowToken),
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct WindowToken {
    address: usize,
    id: u32,
    begin_count: i16,
    depth: i32,
    invocation: u64,
}

fn window_token(frame: &Frame) -> Result<WindowToken> {
    // Frame::check has already validated Context and WithinFrameScope. Never
    // dereference a saved address: capture fresh native state for comparison.
    unsafe {
        let current = (*frame.context).CurrentWindow;
        if current.is_null() {
            return Err(mlua::Error::runtime("no active ImGui window"));
        }
        Ok(WindowToken {
            address: current as usize,
            id: (*current).ID,
            begin_count: (*current).BeginCount,
            depth: (*frame.context).CurrentWindowStack.Size,
            invocation: frame.invocation,
        })
    }
}

pub(super) fn install(lua: &mlua::Lua, imgui: &Table, frame: &Rc<RefCell<Frame>>) -> Result<()> {
    for (name, is_window) in [
        ("GetBackgroundDrawList", false),
        ("GetWindowDrawList", true),
    ] {
        let f = frame.clone();
        imgui.set(
            name,
            lua.create_function(move |_, ()| {
                let mut state = f.borrow_mut();
                state.check()?;
                let target = if is_window {
                    Target::Window(window_token(&state)?)
                } else {
                    Target::Background
                };
                Ok(DrawList {
                    frame: f.clone(),
                    epoch: state.epoch,
                    context: state.context as usize,
                    target,
                })
            })?,
        )?;
    }
    Ok(())
}

impl DrawList {
    fn emit(&self, points: usize, draw: impl FnOnce(*mut sys::ImDrawList)) -> Result<()> {
        self.emit_estimate(points, None, draw)
    }
    fn emit_auto_circle(&self, radius: f32, draw: impl FnOnce(*mut sys::ImDrawList)) -> Result<()> {
        self.emit_estimate(0, Some(radius), draw)
    }
    fn emit_estimate(
        &self,
        points: usize,
        auto_radius: Option<f32>,
        draw: impl FnOnce(*mut sys::ImDrawList),
    ) -> Result<()> {
        let mut frame = self.frame.borrow_mut();
        frame.check()?;
        if frame.epoch != self.epoch || frame.context as usize != self.context {
            return Err(mlua::Error::runtime("expired ImGui draw-list handle"));
        }
        if let Target::Window(token) = self.target {
            if window_token(&frame)? != token {
                return Err(mlua::Error::runtime(
                    "expired or mismatched ImGui window draw-list handle",
                ));
            }
        }
        let native = unsafe {
            match self.target {
                Target::Background => sys::igGetBackgroundDrawList(std::ptr::null_mut()),
                Target::Window(_) => sys::igGetWindowDrawList(),
            }
        };
        if native.is_null() {
            return Err(mlua::Error::runtime("unavailable ImGui draw list"));
        }
        let points = if let Some(radius) = auto_radius {
            // Fresh native pointer is obtained only after every handle guard.
            // ArcFast rounds its sample step down; its48-sample full circle can
            // exceed the auto calculator's nominal count at small radii. Reserve
            // at least50 points, otherwise the calculator plus8 endpoints. For
            // outlines use the unshrunk radius, conservatively above radius-.5.
            let count =
                unsafe { sys::ImDrawList__CalcCircleAutoSegmentCount(native, radius.max(0.5)) };
            if !(3..=512).contains(&count) {
                return Err(mlua::Error::runtime(
                    "native circle estimate outside bounds",
                ));
            }
            (count as usize + 8).max(50)
        } else {
            points
        };
        let cost = points
            .checked_mul(24)
            .ok_or_else(|| mlua::Error::runtime("geometry budget overflow"))?;
        let next = frame
            .geometry
            .checked_add(cost)
            .ok_or_else(|| mlua::Error::runtime("geometry budget overflow"))?;
        if next > GEOMETRY_LIMIT {
            return Err(mlua::Error::runtime("ImGui geometry budget exceeded"));
        }
        // Charge before calling native code, including invisible/transparent
        // primitives. A callback error cannot refund already recorded geometry.
        frame.geometry = next;
        draw(native);
        Ok(())
    }
}

fn color(v: i64) -> Result<u32> {
    if (i32::MIN as i64..=u32::MAX as i64).contains(&v) {
        Ok(v as u32)
    } else {
        Err(mlua::Error::runtime(
            "packed color outside signed/unsigned U32 bounds",
        ))
    }
}
fn positive_bound(v: f32, max: f32, label: &str) -> Result<f32> {
    let v = finite(v)?;
    if (0.0..=max).contains(&v) {
        Ok(v)
    } else {
        Err(mlua::Error::runtime(format!(
            "{label} outside draw-list bounds"
        )))
    }
}
fn thickness(v: Option<f32>) -> Result<f32> {
    positive_bound(v.unwrap_or(1.), 64., "thickness")
}
fn radius(v: f32) -> Result<f32> {
    positive_bound(v, 4096., "radius")
}
fn rounding(v: Option<f32>) -> Result<f32> {
    positive_bound(v.unwrap_or(0.), 4096., "rounding")
}
fn flags(v: Option<i32>) -> Result<i32> {
    let v = v.unwrap_or(0);
    // Only rectangle corner flags; low bits include native assertion traps.
    if v & !0x1f0 != 0 {
        Err(mlua::Error::runtime("unsupported rectangle draw flags"))
    } else {
        Ok(v)
    }
}
fn segments(v: Option<i32>, ngon: bool) -> Result<(i32, usize)> {
    let n = v.unwrap_or(0);
    if n == 0 && !ngon {
        Ok((0, AUTO_POINTS))
    } else if (3..=512).contains(&n) {
        Ok((n, n as usize + 2))
    } else {
        Err(mlua::Error::runtime(
            "segments must be 3..512 (circle alone permits auto 0)",
        ))
    }
}
fn point(v: AnyUserData) -> Result<sys::ImVec2_c> {
    let v = *v.borrow::<Vec2>()?;
    finite(v.0)?;
    finite(v.1)?;
    Ok(v.raw())
}

pub(super) fn methods<M: UserDataMethods<DrawList>>(m: &mut M) {
    m.add_method(
        "AddLine",
        |_, d, (a, b, c, t): (AnyUserData, AnyUserData, i64, Option<f32>)| {
            let (a, b, c, t) = (point(a)?, point(b)?, color(c)?, thickness(t)?);
            d.emit(2, |p| unsafe { sys::ImDrawList_AddLine(p, a, b, c, t) })
        },
    );
    // Legacy Lua order is rounding, flags, thickness. Native 1.92.8 swapped
    // thickness and flags; the adapter MUST reorder at this FFI boundary.
    m.add_method(
        "AddRect",
        |_,
         d,
         (a, b, c, r, f, t): (
            AnyUserData,
            AnyUserData,
            i64,
            Option<f32>,
            Option<i32>,
            Option<f32>,
        )| {
            let (a, b, c, r, f, t) = (
                point(a)?,
                point(b)?,
                color(c)?,
                rounding(r)?,
                flags(f)?,
                thickness(t)?,
            );
            d.emit(if r > 0. { ROUND_RECT_POINTS } else { 4 }, |p| unsafe {
                sys::ImDrawList_AddRect(p, a, b, c, r, t, f)
            })
        },
    );
    m.add_method(
        "AddRectFilled",
        |_, d, (a, b, c, r, f): (AnyUserData, AnyUserData, i64, Option<f32>, Option<i32>)| {
            let (a, b, c, r, f) = (point(a)?, point(b)?, color(c)?, rounding(r)?, flags(f)?);
            d.emit(if r > 0. { ROUND_RECT_POINTS } else { 4 }, |p| unsafe {
                sys::ImDrawList_AddRectFilled(p, a, b, c, r, f)
            })
        },
    );
    m.add_method(
        "AddTriangle",
        |_, d, (a, b, c, col, t): (AnyUserData, AnyUserData, AnyUserData, i64, Option<f32>)| {
            let (a, b, c, col, t) = (point(a)?, point(b)?, point(c)?, color(col)?, thickness(t)?);
            d.emit(3, |p| unsafe {
                sys::ImDrawList_AddTriangle(p, a, b, c, col, t)
            })
        },
    );
    m.add_method(
        "AddTriangleFilled",
        |_, d, (a, b, c, col): (AnyUserData, AnyUserData, AnyUserData, i64)| {
            let (a, b, c, col) = (point(a)?, point(b)?, point(c)?, color(col)?);
            d.emit(3, |p| unsafe {
                sys::ImDrawList_AddTriangleFilled(p, a, b, c, col)
            })
        },
    );
    for (name, ngon) in [("AddCircle", false), ("AddNgon", true)] {
        m.add_method(
            name,
            move |_, d, (a, r, c, n, t): (AnyUserData, f32, i64, Option<i32>, Option<f32>)| {
                let (a, r, c, (n, points), t) = (
                    point(a)?,
                    radius(r)?,
                    color(c)?,
                    segments(n, ngon)?,
                    thickness(t)?,
                );
                let draw = |p| unsafe {
                    if ngon {
                        sys::ImDrawList_AddNgon(p, a, r, c, n, t)
                    } else {
                        sys::ImDrawList_AddCircle(p, a, r, c, n, t)
                    }
                };
                if n == 0 && !ngon {
                    d.emit_auto_circle(r, draw)
                } else {
                    d.emit(points, draw)
                }
            },
        );
    }
    for (name, ngon) in [("AddCircleFilled", false), ("AddNgonFilled", true)] {
        m.add_method(
            name,
            move |_, d, (a, r, c, n): (AnyUserData, f32, i64, Option<i32>)| {
                let (a, r, c, (n, points)) = (point(a)?, radius(r)?, color(c)?, segments(n, ngon)?);
                let draw = |p| unsafe {
                    if ngon {
                        sys::ImDrawList_AddNgonFilled(p, a, r, c, n)
                    } else {
                        sys::ImDrawList_AddCircleFilled(p, a, r, c, n)
                    }
                };
                if n == 0 && !ngon {
                    d.emit_auto_circle(r, draw)
                } else {
                    d.emit(points, draw)
                }
            },
        );
    }
}
