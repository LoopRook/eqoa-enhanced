//! Scoped Lua draw lists; native pointers exist only during validated calls.
use super::*;

// Conservative index-equivalent budget. Thick AA strokes use up to 18 indices
// per path point; 24 reserves join/closing overhead too. Native auto circle
// tessellation is capped at 512; allow 520 for arc end points. Rounded rectangles
// reserve that full-circle upper bound, including quarter-arc end points.
const GEOMETRY_LIMIT: usize = 196_608;
const AUTO_POINTS: usize = 520;

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

fn texture_ref(val: mlua::Value) -> Result<sys::ImTextureRef_c> {
    match val {
        mlua::Value::UserData(ud) => super::super::textures::Textures::raw(&ud),
        mlua::Value::LightUserData(lud) => Ok(sys::ImTextureRef_c {
            _TexData: lud.0.cast(),
            _TexID: lud.0 as u64,
        }),
        _ => Err(mlua::Error::runtime("expected texture handle or pointer")),
    }
}

pub(super) fn methods<M: UserDataMethods<DrawList>>(m: &mut M) {
    m.add_method(
        "AddText",
        |_lua, d, args: mlua::MultiValue| {
            let vec: Vec<mlua::Value> = args.into_iter().collect();
            match vec.len() {
                3 => {
                    let pos = point(match &vec[0] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddText arg 1 must be ImVec2")),
                    })?;
                    let col = match &vec[1] {
                        mlua::Value::Integer(i) => color(*i)?,
                        mlua::Value::Number(n) => color(*n as i64)?,
                        _ => return Err(mlua::Error::runtime("AddText arg 2 must be color")),
                    };
                    let text_str = match &vec[2] {
                        mlua::Value::String(s) => s.to_str()?.to_string(),
                        mlua::Value::Integer(i) => i.to_string(),
                        mlua::Value::Number(n) => n.to_string(),
                        other => other.to_string()?,
                    };
                    let c_text = text(&text_str)?;
                    let points = text_str.len().max(1);
                    d.emit(points, |p| unsafe {
                        sys::ImDrawList_AddText_Vec2(
                            p,
                            pos,
                            col,
                            c_text.as_ptr(),
                            std::ptr::null(),
                        )
                    })
                }
                5 => {
                    let font_ud = match &vec[0] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddText arg 1 must be font userdata")),
                    };
                    let font_size = match &vec[1] {
                        mlua::Value::Number(n) => finite(*n as f32)?,
                        mlua::Value::Integer(i) => finite(*i as f32)?,
                        _ => return Err(mlua::Error::runtime("AddText arg 2 must be font size number")),
                    };
                    let pos = point(match &vec[2] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddText arg 3 must be ImVec2")),
                    })?;
                    let col = match &vec[3] {
                        mlua::Value::Integer(i) => color(*i)?,
                        mlua::Value::Number(n) => color(*n as i64)?,
                        _ => return Err(mlua::Error::runtime("AddText arg 4 must be color")),
                    };
                    let text_str = match &vec[4] {
                        mlua::Value::String(s) => s.to_str()?.to_string(),
                        mlua::Value::Integer(i) => i.to_string(),
                        mlua::Value::Number(n) => n.to_string(),
                        other => other.to_string()?,
                    };
                    let font = font_ud.borrow::<super::super::fonts::LuaImFont>()?;
                    let raw_font = match &*font.state.borrow() {
                        super::super::fonts::FontHandleState::Ready { raw_font, .. } => *raw_font,
                        _ => std::ptr::null_mut(),
                    };
                    let c_text = text(&text_str)?;
                    let points = text_str.len().max(1);
                    d.emit(points, |p| unsafe {
                        if raw_font.is_null() {
                            sys::ImDrawList_AddText_Vec2(
                                p,
                                pos,
                                col,
                                c_text.as_ptr(),
                                std::ptr::null(),
                            )
                        } else {
                            sys::ImDrawList_AddText_FontPtr(
                                p,
                                raw_font,
                                font_size,
                                pos,
                                col,
                                c_text.as_ptr(),
                                std::ptr::null(),
                                0.0,
                                std::ptr::null(),
                            )
                        }
                    })
                }
                _ => Err(mlua::Error::runtime(
                    "invalid AddText arguments: expected (pos, col, text) or (font, font_size, pos, col, text)",
                )),
            }
        },
    );

    m.add_method(
        "AddImage",
        |_lua, d, args: mlua::MultiValue| {
            let vec: Vec<mlua::Value> = args.into_iter().collect();
            match vec.len() {
                3 => {
                    let tex = texture_ref(vec[0].clone())?;
                    let p_min = point(match &vec[1] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddImage arg 2 must be ImVec2")),
                    })?;
                    let p_max = point(match &vec[2] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddImage arg 3 must be ImVec2")),
                    })?;
                    let uv_min = sys::ImVec2_c { x: 0.0, y: 0.0 };
                    let uv_max = sys::ImVec2_c { x: 1.0, y: 1.0 };
                    let col = 0xFFFFFFFFu32;
                    d.emit(4, |p| unsafe {
                        sys::ImDrawList_AddImage(p, tex, p_min, p_max, uv_min, uv_max, col)
                    })
                }
                5 => {
                    let tex = texture_ref(vec[0].clone())?;
                    let p_min = point(match &vec[1] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddImage arg 2 must be ImVec2")),
                    })?;
                    let p_max = point(match &vec[2] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddImage arg 3 must be ImVec2")),
                    })?;
                    let uv_min = point(match &vec[3] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddImage arg 4 must be ImVec2")),
                    })?;
                    let uv_max = point(match &vec[4] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddImage arg 5 must be ImVec2")),
                    })?;
                    let col = 0xFFFFFFFFu32;
                    d.emit(4, |p| unsafe {
                        sys::ImDrawList_AddImage(p, tex, p_min, p_max, uv_min, uv_max, col)
                    })
                }
                6 => {
                    let tex = texture_ref(vec[0].clone())?;
                    let p_min = point(match &vec[1] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddImage arg 2 must be ImVec2")),
                    })?;
                    let p_max = point(match &vec[2] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddImage arg 3 must be ImVec2")),
                    })?;
                    let uv_min = point(match &vec[3] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddImage arg 4 must be ImVec2")),
                    })?;
                    let uv_max = point(match &vec[4] {
                        mlua::Value::UserData(ud) => ud.clone(),
                        _ => return Err(mlua::Error::runtime("AddImage arg 5 must be ImVec2")),
                    })?;
                    let col = match &vec[5] {
                        mlua::Value::Integer(i) => color(*i)?,
                        mlua::Value::Number(n) => color(*n as i64)?,
                        _ => return Err(mlua::Error::runtime("AddImage arg 6 must be color")),
                    };
                    d.emit(4, |p| unsafe {
                        sys::ImDrawList_AddImage(p, tex, p_min, p_max, uv_min, uv_max, col)
                    })
                }
                _ => Err(mlua::Error::runtime(
                    "invalid AddImage arguments: expected (texture, p_min, p_max, [uv_min, uv_max, [col]])",
                )),
            }
        },
    );

    m.add_method(
        "PushClipRect",
        |_, d, (min, max, intersect): (AnyUserData, AnyUserData, Option<bool>)| {
            let (min, max) = (point(min)?, point(max)?);
            let intersect = intersect.unwrap_or(false);
            d.emit(0, |p| unsafe {
                sys::ImDrawList_PushClipRect(p, min, max, intersect)
            })
        },
    );

    m.add_method("PopClipRect", |_, d, ()| {
        d.emit(0, |p| unsafe {
            sys::ImDrawList_PopClipRect(p)
        })
    });

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
            d.emit_estimate(4, if r > 0. { Some(r) } else { None }, |p| unsafe {
                sys::ImDrawList_AddRect(p, a, b, c, r, t, f)
            })
        },
    );
    m.add_method(
        "AddRectFilled",
        |_, d, (a, b, c, r, f): (AnyUserData, AnyUserData, i64, Option<f32>, Option<i32>)| {
            let (a, b, c, r, f) = (point(a)?, point(b)?, color(c)?, rounding(r)?, flags(f)?);
            d.emit_estimate(4, if r > 0. { Some(r) } else { None }, |p| unsafe {
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
