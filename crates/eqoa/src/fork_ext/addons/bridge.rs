use super::host::Host;
use dear_imgui_rs::sys;
use mlua::{AnyUserData, Result, Table, UserData, UserDataFields, UserDataMethods};
use std::{
    cell::RefCell,
    ffi::{CStr, CString},
    rc::Rc,
};

#[derive(Clone, Copy)]
struct Vec2(f32, f32);
impl UserData for Vec2 {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, v| Ok(v.0));
        fields.add_field_method_get("y", |_, v| Ok(v.1));
    }
}
impl Vec2 {
    fn raw(self) -> sys::ImVec2_c {
        sys::ImVec2_c {
            x: self.0,
            y: self.1,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Stack {
    Window,
    Color,
}

#[derive(Default)]
struct Frame {
    active: bool,
    epoch: i32,
    context: *mut sys::ImGuiContext,
    calls: usize,
    stack: Vec<Stack>,
}

impl Frame {
    fn check(&mut self) -> Result<()> {
        // All entry points run on the application thread. Lua never receives a
        // native address; the current Context and frame must still be ours.
        if !self.active || unsafe { sys::igGetCurrentContext() } != self.context {
            return Err(mlua::Error::runtime(
                "ImGui is unavailable outside the active spike frame",
            ));
        }
        self.calls += 1;
        if self.calls > 4096 {
            return Err(mlua::Error::runtime("ImGui call budget exceeded"));
        }
        Ok(())
    }
    fn pop(&mut self, kind: Stack) -> Result<()> {
        let index = self
            .stack
            .iter()
            .rposition(|k| *k == kind)
            .ok_or_else(|| mlua::Error::runtime("unbalanced ImGui pop/end"))?;
        self.stack.remove(index);
        Ok(())
    }
}

#[derive(Clone)]
struct DrawList {
    frame: Rc<RefCell<Frame>>,
    epoch: i32,
}
impl UserData for DrawList {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "AddLine",
            |_, list, (a, b, color, thickness): (AnyUserData, AnyUserData, i64, Option<f32>)| {
                let a = *a.borrow::<Vec2>()?;
                let b = *b.borrow::<Vec2>()?;
                let thickness = finite(thickness.unwrap_or(1.0))?;
                if !(0.0..=64.0).contains(&thickness) {
                    return Err(mlua::Error::runtime("line thickness outside spike bounds"));
                }
                let mut frame = list.frame.borrow_mut();
                frame.check()?;
                if frame.epoch != list.epoch {
                    return Err(mlua::Error::runtime("expired ImGui draw-list handle"));
                }
                unsafe {
                    sys::ImDrawList_AddLine(
                        sys::igGetBackgroundDrawList(std::ptr::null_mut()),
                        a.raw(),
                        b.raw(),
                        color as u32,
                        thickness,
                    );
                }
                Ok(())
            },
        );
    }
}

fn finite(v: f32) -> Result<f32> {
    if v.is_finite() && v.abs() <= 1_000_000.0 {
        Ok(v)
    } else {
        Err(mlua::Error::runtime("non-finite or oversized ImGui value"))
    }
}
fn text(v: &str) -> Result<CString> {
    if v.len() > 4096 {
        return Err(mlua::Error::runtime("ImGui text exceeds spike limit"));
    }
    CString::new(v).map_err(mlua::Error::external)
}

pub(super) struct Bridge {
    frame: Rc<RefCell<Frame>>,
}

impl Bridge {
    pub fn install(host: &Host) -> Result<Self> {
        let lua = &host.lua;
        let frame = Rc::new(RefCell::new(Frame::default()));
        let imgui = lua.create_table()?;
        let vec2 = lua.create_table()?;
        vec2.set(
            "new",
            lua.create_function(|_, (x, y): (Option<f32>, Option<f32>)| {
                Ok(Vec2(finite(x.unwrap_or(0.0))?, finite(y.unwrap_or(0.0))?))
            })?,
        )?;
        lua.globals().set("ImVec2", vec2)?;
        let f = frame.clone();
        imgui.set(
            "GetDisplaySize",
            lua.create_function(move |_, ()| {
                f.borrow_mut().check()?;
                let size = unsafe { (*sys::igGetIO_Nil()).DisplaySize };
                Ok((size.x, size.y))
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "GetBackgroundDrawList",
            lua.create_function(move |_, ()| {
                let mut state = f.borrow_mut();
                state.check()?;
                Ok(DrawList {
                    frame: f.clone(),
                    epoch: state.epoch,
                })
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "GetColorU32",
            lua.create_function(move |_, (r, g, b, a): (f32, f32, f32, f32)| {
                f.borrow_mut().check()?;
                Ok(unsafe {
                    sys::igGetColorU32_Vec4(sys::ImVec4_c {
                        x: finite(r)?,
                        y: finite(g)?,
                        z: finite(b)?,
                        w: finite(a)?,
                    })
                } as i32)
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "Begin",
            lua.create_function(
                move |_, (label, open, flags): (String, Option<bool>, Option<i32>)| {
                    let label = text(&label)?;
                    let flags = flags.unwrap_or(0);
                    let allowed = sys::ImGuiWindowFlags_NoTitleBar
                        | sys::ImGuiWindowFlags_AlwaysAutoResize
                        | sys::ImGuiWindowFlags_NoBackground;
                    if flags & !allowed != 0 {
                        return Err(mlua::Error::runtime("unsupported spike window flags"));
                    }
                    let mut state = f.borrow_mut();
                    state.check()?;
                    if state.stack.len() >= 32 {
                        return Err(mlua::Error::runtime("ImGui stack budget exceeded"));
                    }
                    let mut is_open = open.unwrap_or(true);
                    let visible = unsafe {
                        sys::igBegin(
                            label.as_ptr(),
                            if open.is_some() {
                                &mut is_open
                            } else {
                                std::ptr::null_mut()
                            },
                            flags,
                        )
                    };
                    state.stack.push(Stack::Window);
                    // Begin(name) has one return; the open overload also returns open.
                    Ok(mlua::MultiValue::from_vec(if open.is_some() {
                        vec![mlua::Value::Boolean(visible), mlua::Value::Boolean(is_open)]
                    } else {
                        vec![mlua::Value::Boolean(visible)]
                    }))
                },
            )?,
        )?;
        let f = frame.clone();
        imgui.set(
            "End",
            lua.create_function(move |_, ()| {
                let mut state = f.borrow_mut();
                state.check()?;
                state.pop(Stack::Window)?;
                unsafe {
                    sys::igEnd();
                }
                Ok(())
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "Text",
            lua.create_function(move |_, value: String| {
                let value = text(&value)?;
                f.borrow_mut().check()?;
                unsafe {
                    sys::igTextUnformatted(value.as_ptr(), std::ptr::null());
                }
                Ok(())
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "Separator",
            lua.create_function(move |_, ()| {
                f.borrow_mut().check()?;
                unsafe {
                    sys::igSeparator();
                }
                Ok(())
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "SliderInt",
            lua.create_function(
                move |_,
                      (label, mut value, min, max, format): (
                    String,
                    i32,
                    i32,
                    i32,
                    Option<String>,
                )| {
                    let label = text(&label)?;
                    let format = text(format.as_deref().unwrap_or("%d"))?;
                    if min > max || !safe_format(format.to_str().unwrap_or(""), false) {
                        return Err(mlua::Error::runtime("invalid slider range/format"));
                    }
                    f.borrow_mut().check()?;
                    let changed = unsafe {
                        sys::igSliderInt(label.as_ptr(), &mut value, min, max, format.as_ptr(), 0)
                    };
                    Ok((value, changed))
                },
            )?,
        )?;
        let f = frame.clone();
        imgui.set(
            "SliderFloat",
            lua.create_function(
                move |_,
                      (label, mut value, min, max, format): (
                    String,
                    f32,
                    f32,
                    f32,
                    Option<String>,
                )| {
                    let label = text(&label)?;
                    let format = text(format.as_deref().unwrap_or("%.3f"))?;
                    finite(value)?;
                    finite(min)?;
                    finite(max)?;
                    if min >= max || !safe_format(format.to_str().unwrap_or(""), true) {
                        return Err(mlua::Error::runtime("invalid slider range/format"));
                    }
                    f.borrow_mut().check()?;
                    let changed = unsafe {
                        sys::igSliderFloat(label.as_ptr(), &mut value, min, max, format.as_ptr(), 0)
                    };
                    Ok((value, changed))
                },
            )?,
        )?;
        let f = frame.clone();
        imgui.set(
            "ColorEdit4",
            lua.create_function(
                move |lua, (label, color, flags): (String, Table, Option<i32>)| {
                    if flags.unwrap_or(0) != 0 {
                        return Err(mlua::Error::runtime("unsupported spike color flags"));
                    }
                    let label = text(&label)?;
                    let mut rgba = [0.0; 4];
                    for (i, v) in rgba.iter_mut().enumerate() {
                        *v = finite(color.get(i + 1)?)?;
                    }
                    f.borrow_mut().check()?;
                    let changed =
                        unsafe { sys::igColorEdit4(label.as_ptr(), rgba.as_mut_ptr(), 0) };
                    Ok((lua.create_sequence_from(rgba)?, changed))
                },
            )?,
        )?;
        let f = frame.clone();
        imgui.set(
            "InputText",
            lua.create_function(
                move |_, (label, value, flags): (String, String, Option<i32>)| {
                    let label = text(&label)?;
                    let value = text(&value)?;
                    let flags = flags.unwrap_or(0);
                    if flags & !sys::ImGuiInputTextFlags_EnterReturnsTrue != 0 {
                        return Err(mlua::Error::runtime("unsupported spike input flags"));
                    }
                    let mut buffer = vec![0u8; 16385];
                    buffer[..value.as_bytes().len()].copy_from_slice(value.as_bytes());
                    f.borrow_mut().check()?;
                    let entered = unsafe {
                        sys::igInputText(
                            label.as_ptr(),
                            buffer.as_mut_ptr().cast(),
                            buffer.len(),
                            flags,
                            None,
                            std::ptr::null_mut(),
                        )
                    };
                    let value = unsafe { CStr::from_ptr(buffer.as_ptr().cast()) }
                        .to_string_lossy()
                        .into_owned();
                    Ok((value, entered))
                },
            )?,
        )?;
        let f = frame.clone();
        imgui.set(
            "Image",
            lua.create_function(move |_, (handle, width, height): (AnyUserData, f32, f32)| {
                let size = Vec2(finite(width)?, finite(height)?);
                if width <= 0.0 || height <= 0.0 {
                    return Err(mlua::Error::runtime("image dimensions must be positive"));
                }
                let texture = super::textures::Textures::raw(&handle)?;
                f.borrow_mut().check()?;
                unsafe {
                    sys::igImage(
                        texture,
                        size.raw(),
                        Vec2(0.0, 0.0).raw(),
                        Vec2(1.0, 1.0).raw(),
                    );
                }
                Ok(())
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "PushStyleColor",
            lua.create_function(move |_, (idx, r, g, b, a): (i32, f32, f32, f32, f32)| {
                if idx != sys::ImGuiCol_Text as i32 {
                    return Err(mlua::Error::runtime("unsupported spike style color"));
                }
                let col = sys::ImVec4_c {
                    x: finite(r)?,
                    y: finite(g)?,
                    z: finite(b)?,
                    w: finite(a)?,
                };
                let mut state = f.borrow_mut();
                state.check()?;
                if state.stack.len() >= 32 {
                    return Err(mlua::Error::runtime("ImGui stack budget exceeded"));
                }
                unsafe {
                    sys::igPushStyleColor_Vec4(idx, col);
                }
                state.stack.push(Stack::Color);
                Ok(())
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "PopStyleColor",
            lua.create_function(move |_, ()| {
                let mut state = f.borrow_mut();
                state.check()?;
                state.pop(Stack::Color)?;
                unsafe {
                    sys::igPopStyleColor(1);
                }
                Ok(())
            })?,
        )?;
        lua.globals().set("ImGui", imgui)?;
        let flags = lua.create_table()?;
        flags.set("NoTitleBar", sys::ImGuiWindowFlags_NoTitleBar)?;
        flags.set("AlwaysAutoResize", sys::ImGuiWindowFlags_AlwaysAutoResize)?;
        flags.set("NoBackground", sys::ImGuiWindowFlags_NoBackground)?;
        lua.globals().set("ImGuiWindowFlags", flags)?;
        let flags = lua.create_table()?;
        flags.set(
            "EnterReturnsTrue",
            sys::ImGuiInputTextFlags_EnterReturnsTrue,
        )?;
        lua.globals().set("ImGuiInputTextFlags", flags)?;
        let colors = lua.create_table()?;
        colors.set("Text", sys::ImGuiCol_Text)?;
        lua.globals().set("ImGuiCol", colors)?;
        Ok(Self { frame })
    }

    pub fn invoke(&self, call: impl FnOnce() -> Result<()>) -> Result<()> {
        {
            let mut state = self.frame.borrow_mut();
            if state.active {
                return Err(mlua::Error::runtime("recursive spike frame"));
            }
            state.context = unsafe { sys::igGetCurrentContext() };
            if state.context.is_null() {
                return Err(mlua::Error::runtime("no ImGui Context"));
            }
            state.epoch = unsafe { sys::igGetFrameCount() };
            state.calls = 0;
            state.active = true;
        }
        let result = call();
        let mut state = self.frame.borrow_mut();
        let unbalanced = !state.stack.is_empty();
        // Reverse our own operations, never pop another mod's or the host's stacks.
        while let Some(op) = state.stack.pop() {
            unsafe {
                match op {
                    Stack::Window => sys::igEnd(),
                    Stack::Color => sys::igPopStyleColor(1),
                }
            }
        }
        state.active = false;
        result?;
        if unbalanced {
            Err(mlua::Error::runtime("unbalanced ImGui stacks recovered"))
        } else {
            Ok(())
        }
    }
}

// ImGui passes formats through printf. Reject arbitrary conversion specifiers
// before entering C++; permit the literal formats used by screen_grid.
fn safe_format(value: &str, float: bool) -> bool {
    !value.contains('%')
        || if float {
            matches!(value, "%f" | "%.1f" | "%.2f" | "%.3f")
        } else {
            value == "%d"
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::{Context, sys};

    #[test]
    fn forge_bridge_renders_real_lines_preserves_returns_and_rejects_expired_handles() {
        let mut context = Context::create();
        context
            .set_ini_filename::<std::path::PathBuf>(None)
            .unwrap();
        context.io_mut().set_display_size([800.0, 600.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        let host = Host::new().unwrap();
        let bridge = Bridge::install(&host).unwrap();
        let script = host.compile(r#"
            local w,h = ImGui.GetDisplaySize(); assert(w==800 and h==600)
            dl = ImGui.GetBackgroundDrawList()
            dl:AddLine(ImVec2.new(0,0),ImVec2.new(w,h),ImGui.GetColorU32(1,0,0,1),2)
            assert(ImGui.Begin('test',true,ImGuiWindowFlags.NoTitleBar))
            local n,changed = ImGui.SliderInt('n',12,0,20); assert(n==12 and type(changed)=='boolean')
            local text,entered = ImGui.InputText('text','hello',ImGuiInputTextFlags.EnterReturnsTrue)
            assert(text=='hello' and type(entered)=='boolean')
            local color,edited = ImGui.ColorEdit4('color',{1,0,0,1}); assert(color[1]==1 and type(edited)=='boolean')
            ImGui.End()
        "#).unwrap();
        context.frame();
        bridge.invoke(|| host.run(&script)).unwrap();
        drop(context.render_legacy());
        assert!(unsafe { (*sys::igGetDrawData()).TotalVtxCount } > 0);
        let stale = host
            .compile("dl:AddLine(ImVec2.new(0,0),ImVec2.new(1,1),0,1)")
            .unwrap();
        assert!(host.run(&stale).is_err());
        context.frame();
        assert!(
            bridge
                .invoke(|| host.run(&stale))
                .unwrap_err()
                .to_string()
                .contains("expired")
        );
        let broken = host.compile("ImGui.Begin('broken'); ImGui.PushStyleColor(ImGuiCol.Text,1,0,0,1); error('controlled')").unwrap();
        assert!(bridge.invoke(|| host.run(&broken)).is_err());
        let survivor = Host::new().unwrap();
        let survivor_bridge = Bridge::install(&survivor).unwrap();
        let good = survivor
            .compile("assert(ImGui.Begin('survivor')); ImGui.Text('alive'); ImGui.End()")
            .unwrap();
        survivor_bridge.invoke(|| survivor.run(&good)).unwrap();
        drop(context.render_legacy());
    }
}
