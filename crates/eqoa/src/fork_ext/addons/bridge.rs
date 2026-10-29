use super::host::Host;
use dear_imgui_rs::sys;
use mlua::{AnyUserData, Result, Table, UserData, UserDataFields, UserDataMethods};
use std::{
    cell::RefCell,
    ffi::{CStr, CString},
    rc::Rc,
};
#[cfg(test)]
#[path = "bridge_next_tests.rs"]
mod next_tests;
#[path = "bridge_widgets.rs"]
mod widgets;

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

#[derive(Clone, Copy)]
struct Vec4(f32, f32, f32, f32);
impl UserData for Vec4 {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, v| Ok(v.0));
        fields.add_field_method_set("x", |_, v, val: f32| {
            v.0 = finite(val)?;
            Ok(())
        });
        fields.add_field_method_get("y", |_, v| Ok(v.1));
        fields.add_field_method_set("y", |_, v, val: f32| {
            v.1 = finite(val)?;
            Ok(())
        });
        fields.add_field_method_get("z", |_, v| Ok(v.2));
        fields.add_field_method_set("z", |_, v, val: f32| {
            v.2 = finite(val)?;
            Ok(())
        });
        fields.add_field_method_get("w", |_, v| Ok(v.3));
        fields.add_field_method_set("w", |_, v, val: f32| {
            v.3 = finite(val)?;
            Ok(())
        });
    }
}
impl Vec4 {
    fn raw(self) -> sys::ImVec4_c {
        sys::ImVec4_c {
            x: self.0,
            y: self.1,
            z: self.2,
            w: self.3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stack {
    Window,
    Child,
    Tree,
    MenuBar,
    MainMenuBar,
    Menu,
    TabBar,
    TabItem,
    TextWrap,
    Color,
    ClipRect,
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
        if unsafe { !(*self.context).WithinFrameScope || sys::igGetFrameCount() != self.epoch } {
            return Err(mlua::Error::runtime("expired ImGui frame"));
        }
        self.calls += 1;
        if self.calls > 4096 {
            return Err(mlua::Error::runtime("ImGui call budget exceeded"));
        }
        Ok(())
    }
    fn pop(&mut self, kind: Stack) -> Result<()> {
        match self.stack.last() {
            Some(top) if *top == kind => {}
            Some(top) => {
                return Err(mlua::Error::runtime(format!(
                    "unbalanced ImGui scopes: expected {kind:?}, unclosed {top:?}"
                )));
            }
            None => {
                return Err(mlua::Error::runtime(format!(
                    "unbalanced ImGui pop/end: no {kind:?}"
                )));
            }
        }
        self.stack.pop();
        Ok(())
    }
    fn pop_clip_rect(&mut self) -> Result<()> {
        self.pop(Stack::ClipRect)
    }
    fn room(&self) -> Result<()> {
        if self.stack.len() >= 32 {
            Err(mlua::Error::runtime("ImGui stack budget exceeded"))
        } else {
            Ok(())
        }
    }
    fn tab_item_parent(&self) -> bool {
        self.stack.iter().rev().find(|k| {
            matches!(
                k,
                Stack::Window
                    | Stack::Child
                    | Stack::Menu
                    | Stack::MainMenuBar
                    | Stack::TabBar
                    | Stack::TabItem
            )
        }) == Some(&Stack::TabBar)
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
        let vec4 = lua.create_table()?;
        vec4.set(
            "new",
            lua.create_function(|lua, args: mlua::MultiValue| match args.len() {
                0 => Ok(Vec4(0.0, 0.0, 0.0, 0.0)),
                4 => {
                    let (x, y, z, w): (f32, f32, f32, f32) = lua.unpack_multi(args)?;
                    Ok(Vec4(finite(x)?, finite(y)?, finite(z)?, finite(w)?))
                }
                _ => Err(mlua::Error::runtime("ImVec4.new requires 0 or 4 arguments")),
            })?,
        )?;
        lua.globals().set("ImVec4", vec4)?;
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
                    widgets::window_flags(flags)?;
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
                        vec![mlua::Value::Boolean(is_open), mlua::Value::Boolean(visible)]
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
            "Checkbox",
            lua.create_function(move |_, (label, mut value): (String, bool)| {
                let label = text(&label)?;
                f.borrow_mut().check()?;
                let pressed = unsafe { sys::igCheckbox(label.as_ptr(), &mut value) };
                Ok((value, pressed))
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "GetCursorPos",
            lua.create_function(move |_, ()| {
                f.borrow_mut().check()?;
                let pos = unsafe { sys::igGetCursorPos() };
                Ok((pos.x, pos.y))
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "GetCursorScreenPos",
            lua.create_function(move |_, ()| {
                f.borrow_mut().check()?;
                let pos = unsafe { sys::igGetCursorScreenPos() };
                Ok((pos.x, pos.y))
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "SetCursorPos",
            lua.create_function(move |_, (x, y): (f32, f32)| {
                let x = finite(x)?;
                let y = finite(y)?;
                f.borrow_mut().check()?;
                unsafe {
                    sys::igSetCursorPos(sys::ImVec2_c { x, y });
                }
                Ok(())
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "SameLine",
            lua.create_function(move |_, (offset, spacing): (Option<f32>, Option<f32>)| {
                let offset = match offset {
                    Some(o) => finite(o)?,
                    None => 0.0,
                };
                let spacing = match spacing {
                    Some(s) => finite(s)?,
                    None => -1.0,
                };
                f.borrow_mut().check()?;
                unsafe {
                    sys::igSameLine(offset, spacing);
                }
                Ok(())
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "PushClipRect",
            lua.create_function(
                move |_,
                      (min_x, min_y, max_x, max_y, intersect): (
                    f32,
                    f32,
                    f32,
                    f32,
                    Option<bool>,
                )| {
                    let min_x = finite(min_x)?;
                    let min_y = finite(min_y)?;
                    let max_x = finite(max_x)?;
                    let max_y = finite(max_y)?;
                    let intersect = intersect.unwrap_or(true);
                    let mut state = f.borrow_mut();
                    state.check()?;
                    if !state.stack.iter().any(|k| {
                        matches!(
                            k,
                            Stack::Window | Stack::Child | Stack::Menu | Stack::MainMenuBar
                        )
                    }) {
                        return Err(mlua::Error::runtime("PushClipRect called outside window"));
                    }
                    if state.stack.len() >= 32 {
                        return Err(mlua::Error::runtime("ImGui stack budget exceeded"));
                    }
                    unsafe {
                        sys::igPushClipRect(
                            sys::ImVec2_c { x: min_x, y: min_y },
                            sys::ImVec2_c { x: max_x, y: max_y },
                            intersect,
                        );
                    }
                    state.stack.push(Stack::ClipRect);
                    Ok(())
                },
            )?,
        )?;
        let f = frame.clone();
        imgui.set(
            "PopClipRect",
            lua.create_function(move |_, ()| {
                let mut state = f.borrow_mut();
                state.check()?;
                state.pop_clip_rect()?;
                unsafe {
                    sys::igPopClipRect();
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
            lua.create_function(move |lua, args: mlua::MultiValue| {
                f.borrow_mut().check()?;
                match args.len() {
                    3 => {
                        let (handle, width, height): (AnyUserData, f32, f32) =
                            lua.unpack_multi(args)?;
                        let width = finite(width)?;
                        let height = finite(height)?;
                        if width <= 0.0 || height <= 0.0 {
                            return Err(mlua::Error::runtime(
                                "image dimensions must be positive",
                            ));
                        }
                        let texture = super::textures::Textures::raw(&handle)?;
                        unsafe {
                            sys::igImage(
                                texture,
                                Vec2(width, height).raw(),
                                Vec2(0.0, 0.0).raw(),
                                Vec2(1.0, 1.0).raw(),
                            );
                        }
                        Ok(())
                    }
                    6 => {
                        let (handle, size_ud, uv0_ud, uv1_ud, tint_ud, border_ud): (
                            AnyUserData,
                            AnyUserData,
                            AnyUserData,
                            AnyUserData,
                            AnyUserData,
                            AnyUserData,
                        ) = lua.unpack_multi(args)?;
                        let size = *size_ud.borrow::<Vec2>()?;
                        let uv0 = *uv0_ud.borrow::<Vec2>()?;
                        let uv1 = *uv1_ud.borrow::<Vec2>()?;
                        let tint = *tint_ud.borrow::<Vec4>()?;
                        let border = *border_ud.borrow::<Vec4>()?;
                        if size.0 <= 0.0 || size.1 <= 0.0 {
                            return Err(mlua::Error::runtime(
                                "image dimensions must be positive",
                            ));
                        }
                        let texture = super::textures::Textures::raw(&handle)?;
                        unsafe {
                            sys::igImageWithBg(
                                texture,
                                size.raw(),
                                uv0.raw(),
                                uv1.raw(),
                                border.raw(),
                                tint.raw(),
                            );
                        }
                        Ok(())
                    }
                    _ => Err(mlua::Error::runtime(
                        "invalid Image arguments: expected 3 args (handle, w, h) or 6 args (handle, size, uv0, uv1, tint, border)",
                    )),
                }
            })?,
        )?;
        let f = frame.clone();
        imgui.set(
            "PushStyleColor",
            lua.create_function(move |_, (idx, r, g, b, a): (i32, f32, f32, f32, f32)| {
                if idx != sys::ImGuiCol_Text as i32 && idx != sys::ImGuiCol_PlotHistogram as i32 {
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
        lua.globals().set("ImGui", imgui.clone())?;
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
        widgets::install(lua, &imgui, &frame)?;
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
            if unsafe { !(*state.context).WithinFrameScope } {
                state.context = std::ptr::null_mut();
                return Err(mlua::Error::runtime("no active ImGui frame"));
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
                    Stack::Child => sys::igEndChild(),
                    Stack::Tree => sys::igTreePop(),
                    Stack::MenuBar => sys::igEndMenuBar(),
                    Stack::MainMenuBar => sys::igEndMainMenuBar(),
                    Stack::Menu => sys::igEndMenu(),
                    Stack::TabBar => sys::igEndTabBar(),
                    Stack::TabItem => sys::igEndTabItem(),
                    Stack::TextWrap => sys::igPopTextWrapPos(),
                    Stack::Color => sys::igPopStyleColor(1),
                    Stack::ClipRect => sys::igPopClipRect(),
                }
            }
        }
        state.active = false;
        state.context = std::ptr::null_mut();
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
    use crate::fork_ext::addons::textures::Textures;
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

    #[test]
    fn forge_bridge_retro_health_hearts_slice_contracts() {
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

        // 1. ImVec4 userdata: 0-arg and 4-arg constructors, dyadic exact fraction values, setters
        let script = host
            .compile(
                r#"
            local v0 = ImVec4.new()
            assert(v0.x == 0.0 and v0.y == 0.0 and v0.z == 0.0 and v0.w == 0.0)
            -- Use exact dyadic fractions (powers of two) to prevent f32/f64 rounding disparity
            local v4 = ImVec4.new(0.125, 0.25, 0.375, 0.5)
            assert(v4.x == 0.125 and v4.y == 0.25 and v4.z == 0.375 and v4.w == 0.5)
            v4.x = 0.875
            assert(v4.x == 0.875)
        "#,
            )
            .unwrap();
        context.frame();
        bridge.invoke(|| host.run(&script)).unwrap();
        drop(context.render_legacy());

        // Negative control: ImVec4 partial argument constructor (2 args) rejected by Host/Rust without pcall
        let bad_partial = host.compile("ImVec4.new(1.0, 2.0)").unwrap();
        context.frame();
        let err = bridge.invoke(|| host.run(&bad_partial)).unwrap_err();
        assert!(err.to_string().contains("0 or 4 arguments"));

        // Frame recovery: verify active frame still functions after partial-constructor error
        let recover1 = host
            .compile("assert(ImVec4.new(1, 2, 3, 4).w == 4)")
            .unwrap();
        bridge.invoke(|| host.run(&recover1)).unwrap();
        drop(context.render_legacy());

        // Negative control: Non-finite setter rejected by Host/Rust without pcall
        let bad_nan = host.compile("local v = ImVec4.new(); v.y = 0/0").unwrap();
        context.frame();
        let err = bridge.invoke(|| host.run(&bad_nan)).unwrap_err();
        assert!(err.to_string().contains("non-finite"));

        // Frame recovery
        let recover2 = host.compile("assert(ImVec4.new().x == 0)").unwrap();
        bridge.invoke(|| host.run(&recover2)).unwrap();
        drop(context.render_legacy());

        // 2. Checkbox multireturn order: (value, pressed)
        context.frame();
        let script = host
            .compile(
                r#"
            assert(ImGui.Begin('chk_test', true, ImGuiWindowFlags.NoTitleBar))
            local val, pressed = ImGui.Checkbox('test_box', false)
            assert(type(val) == 'boolean' and val == false)
            assert(type(pressed) == 'boolean')
            -- Single-return compatibility as used in retro_health_hearts line 79
            local single = ImGui.Checkbox('test_box2', true)
            assert(single == true)
            ImGui.End()
        "#,
            )
            .unwrap();
        bridge.invoke(|| host.run(&script)).unwrap();
        drop(context.render_legacy());

        // 3. Cursor positions (2 scalar numbers) and SetCursorPos, SameLine optional args
        context.frame();
        let script = host
            .compile(
                r#"
            assert(ImGui.Begin('cursor_test', true, ImGuiWindowFlags.NoTitleBar))
            ImGui.Text('header')
            local cx, cy = ImGui.GetCursorPos()
            assert(type(cx) == 'number' and type(cy) == 'number')
            local sx, sy = ImGui.GetCursorScreenPos()
            assert(type(sx) == 'number' and type(sy) == 'number')
            ImGui.SetCursorPos(cx, cy)
            local nx, ny = ImGui.GetCursorPos()
            assert(nx == cx and ny == cy)
            ImGui.Text('content')
            ImGui.SameLine()
            ImGui.Text('inline')
            ImGui.SameLine(50.0)
            ImGui.Text('spaced')
            ImGui.SameLine(50.0, 5.0)
            ImGui.Text('spaced2')
            ImGui.End()
        "#,
            )
            .unwrap();
        bridge.invoke(|| host.run(&script)).unwrap();
        drop(context.render_legacy());

        // 4. ClipRect lifecycle: normal Push/Pop balanced and nested clip rects
        context.frame();
        let script = host
            .compile(
                r#"
            assert(ImGui.Begin('clip_test', true, ImGuiWindowFlags.NoTitleBar))
            ImGui.PushClipRect(10, 10, 100, 100, true)
            ImGui.PushClipRect(20, 20, 80, 80, true)
            ImGui.Text('inside nested clip')
            ImGui.PopClipRect()
            ImGui.PopClipRect()
            ImGui.End()
        "#,
            )
            .unwrap();
        bridge.invoke(|| host.run(&script)).unwrap();
        drop(context.render_legacy());

        // 5. Critical Guard: unclosed nested ClipRect before End becomes Lua error BEFORE native End,
        // and Bridge::invoke unwinds ClipRects before igEnd, preserving ImGui health
        context.frame();
        let bad_clip = host
            .compile(
                r#"
            assert(ImGui.Begin('bad_clip', true, ImGuiWindowFlags.NoTitleBar))
            ImGui.PushClipRect(5, 5, 50, 50, true)
            ImGui.PushClipRect(10, 10, 40, 40, true)
            ImGui.End() -- Must report Lua error BEFORE native End
        "#,
            )
            .unwrap();
        let err = bridge.invoke(|| host.run(&bad_clip)).unwrap_err();
        assert!(err.to_string().contains("unclosed ClipRect"));

        // Healthy second mod renders cleanly in the same frame count after unwind
        let good_mod = host
            .compile(
                r#"
            assert(ImGui.Begin('healthy_mod', true, ImGuiWindowFlags.NoTitleBar))
            ImGui.Text('recovered cleanly')
            ImGui.End()
        "#,
            )
            .unwrap();
        bridge.invoke(|| host.run(&good_mod)).unwrap();
        drop(context.render_legacy());

        // 6. Child window cannot pop parent window's clip rect (separate negative control without pcall)
        context.frame();
        let cross_clip = host
            .compile(
                r#"
            assert(ImGui.Begin('parent_win', true, ImGuiWindowFlags.NoTitleBar))
            ImGui.PushClipRect(1, 1, 20, 20, true)
            assert(ImGui.Begin('child_win', true, ImGuiWindowFlags.NoTitleBar))
            ImGui.PopClipRect() -- Must fail: cannot pop parent window clip rect
        "#,
            )
            .unwrap();
        let err = bridge.invoke(|| host.run(&cross_clip)).unwrap_err();
        assert!(err.to_string().contains("parent window") || err.to_string().contains("ClipRect"));

        // Validate recovery after cross_clip error
        let recover_win = host
            .compile(
                r#"
            assert(ImGui.Begin('recovered_win', true, ImGuiWindowFlags.NoTitleBar))
            ImGui.Text('recovered after child pop error')
            ImGui.End()
        "#,
            )
            .unwrap();
        bridge.invoke(|| host.run(&recover_win)).unwrap();
        drop(context.render_legacy());
    }

    #[test]
    fn forge_bridge_image_overloads_and_wgpu_readback() {
        use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo, WgpuRenderer};
        use render::wgpu;

        let instance = wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
            Ok(adapter) => adapter,
            Err(e) => {
                eprintln!("SKIP forge_bridge_image: no GPU adapter: {e}");
                return;
            }
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut context = Context::create();
        context
            .set_ini_filename::<std::path::PathBuf>(None)
            .unwrap();
        let renderer = Rc::new(RefCell::new(
            WgpuRenderer::new(
                WgpuInitInfo::new(
                    device.clone(),
                    queue.clone(),
                    wgpu::TextureFormat::Rgba8Unorm,
                ),
                &mut context,
            )
            .unwrap(),
        ));
        let host = Host::new().unwrap();
        let bridge = Bridge::install(&host).unwrap();
        let textures = Textures::new(&device, &queue, renderer.clone());
        Textures::install(&textures, &host.lua).unwrap();

        // Create a 4x4 test texture with 4 quadrants:
        // Top-Left: Red (255, 0, 0, 255)
        // Top-Right: Green (0, 255, 0, 255)
        // Bottom-Left: Blue (0, 0, 255, 255)
        // Bottom-Right: FULLY TRANSPARENT (0, 0, 0, 0) for background-fill visibility
        let mut raw = Vec::with_capacity(4 * 4 * 4);
        for y in 0..4 {
            for x in 0..4 {
                if y < 2 && x < 2 {
                    raw.extend_from_slice(&[255, 0, 0, 255]); // Top-Left Red
                } else if y < 2 && x >= 2 {
                    raw.extend_from_slice(&[0, 255, 0, 255]); // Top-Right Green
                } else if y >= 2 && x < 2 {
                    raw.extend_from_slice(&[0, 0, 255, 255]); // Bottom-Left Blue
                } else {
                    raw.extend_from_slice(&[0, 0, 0, 0]); // Bottom-Right Fully Transparent
                }
            }
        }

        host.lua
            .globals()
            .set("raw_tex_bytes", host.lua.create_string(&raw).unwrap())
            .unwrap();
        let create_script = host
            .compile("tex = UiForge.CreateTextureFromMemory(raw_tex_bytes, 4, 4); assert(tex)")
            .unwrap();
        host.run(&create_script).unwrap();

        let (target_w, target_h) = (320, 240);
        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bridge image readback target"),
            size: wgpu::Extent3d {
                width: target_w,
                height: target_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = render_target.create_view(&Default::default());

        // Strengthened fixture: Exports exact screen coordinates of each image to Lua globals.
        // Image A: 3-arg full texture (32x32) -> Top-Left RED, Top-Right GREEN
        // Image B: 6-arg top-right UV crop (32x32) with non-white tint [1.0, 0.5, 1.0, 1.0] -> interior green ~ 128
        // Image C: 6-arg bottom-right transparent UV crop (32x32), white tint, opaque blue background -> interior BLUE
        let render_script = host.compile(r#"
            assert(ImGui.Begin('img_test', true, ImGuiWindowFlags.NoTitleBar + ImGuiWindowFlags.NoBackground))
            -- Image A: 3 arguments (full texture)
            img_a_x, img_a_y = ImGui.GetCursorScreenPos()
            ImGui.Image(tex, 32, 32)

            -- Image B: 6 arguments (top-right UV crop with 0.5 green tint)
            img_b_x, img_b_y = ImGui.GetCursorScreenPos()
            local size = ImVec2.new(32, 32)
            local uv0_b = ImVec2.new(0.5, 0.0)
            local uv1_b = ImVec2.new(1.0, 0.5)
            local tint_b = ImVec4.new(1.0, 0.5, 1.0, 1.0)
            local bg_b = ImVec4.new(0.0, 0.0, 0.0, 0.0)
            ImGui.Image(tex, size, uv0_b, uv1_b, tint_b, bg_b)

            -- Image C: 6 arguments (bottom-right transparent UV crop with opaque blue border/background)
            img_c_x, img_c_y = ImGui.GetCursorScreenPos()
            local uv0_c = ImVec2.new(0.5, 0.5)
            local uv1_c = ImVec2.new(1.0, 1.0)
            local tint_c = ImVec4.new(1.0, 1.0, 1.0, 1.0)
            local bg_c = ImVec4.new(0.0, 0.0, 1.0, 1.0)
            ImGui.Image(tex, size, uv0_c, uv1_c, tint_c, bg_c)
            ImGui.End()
        "#).unwrap();

        for pass_index in 0..2 {
            context
                .io_mut()
                .set_display_size([target_w as f32, target_h as f32]);
            context.io_mut().set_display_framebuffer_scale([1.0, 1.0]);
            context.io_mut().set_delta_time(1.0 / 60.0);
            context.frame();
            bridge.invoke(|| host.run(&render_script)).unwrap();

            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut r = renderer.borrow_mut();
                let frame = context.render(r.renderer_consumer().unwrap());
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Bridge image render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                r.render(frame, &mut pass, FramebufferExtent::new(target_w, target_h))
                    .unwrap();
            }

            if pass_index == 0 {
                queue.submit([encoder.finish()]);
                continue;
            }

            let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Bridge image readback buffer"),
                size: (target_w * target_h * 4) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            encoder.copy_texture_to_buffer(
                render_target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback_buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(target_w * 4),
                        rows_per_image: Some(target_h),
                    },
                },
                render_target.size(),
            );
            queue.submit([encoder.finish()]);

            let (send, recv) = std::sync::mpsc::channel();
            readback_buf
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |res| send.send(res).unwrap());
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            recv.recv().unwrap().unwrap();

            let pixels = readback_buf.slice(..).get_mapped_range().unwrap();

            // Extract exported screen coordinates of Image A, B, and C
            let (ax, ay): (f32, f32) = (
                host.lua.globals().get("img_a_x").unwrap(),
                host.lua.globals().get("img_a_y").unwrap(),
            );
            let (bx, by): (f32, f32) = (
                host.lua.globals().get("img_b_x").unwrap(),
                host.lua.globals().get("img_b_y").unwrap(),
            );
            let (cx, cy): (f32, f32) = (
                host.lua.globals().get("img_c_x").unwrap(),
                host.lua.globals().get("img_c_y").unwrap(),
            );

            let get_pixel = |x: usize, y: usize| -> [u8; 4] {
                let offset = (y * target_w as usize + x) * 4;
                [
                    pixels[offset],
                    pixels[offset + 1],
                    pixels[offset + 2],
                    pixels[offset + 3],
                ]
            };

            // 1. Image A (3-arg full texture):
            // Top-left interior (+8, +8) must be RED
            let p_red = get_pixel(ax as usize + 8, ay as usize + 8);
            assert!(
                p_red[0] > 200 && p_red[1] < 40 && p_red[2] < 40,
                "Image A top-left interior must be red, got RGBA({:?})",
                p_red
            );
            // Top-right interior (+24, +8) must be GREEN
            let p_green = get_pixel(ax as usize + 24, ay as usize + 8);
            assert!(
                p_green[0] < 40 && p_green[1] > 200 && p_green[2] < 40,
                "Image A top-right interior must be green, got RGBA({:?})",
                p_green
            );

            // 2. Image B (6-arg top-right UV crop with 0.5 green tint):
            // Center interior (+16, +16) must be green tinted by 0.5 (~128)
            let p_tinted = get_pixel(bx as usize + 16, by as usize + 16);
            assert!(
                p_tinted[0] < 40 && p_tinted[1] >= 110 && p_tinted[1] <= 145 && p_tinted[2] < 40,
                "Image B interior must be tinted green (~128), got RGBA({:?})",
                p_tinted
            );

            // 3. Image C (6-arg bottom-right transparent UV crop with opaque blue border/background):
            // Center interior (+16, +16) must show opaque BLUE background fill
            let p_bg = get_pixel(cx as usize + 16, cy as usize + 16);
            assert!(
                p_bg[0] < 40 && p_bg[1] < 40 && p_bg[2] > 200,
                "Image C interior must be opaque blue background fill, got RGBA({:?})",
                p_bg
            );

            drop(pixels);
            readback_buf.unmap();
        }

        // Frame 2: Negative controls on valid handles and argument counts
        context.frame();

        // 1. Invalid argument counts on valid texture handle (e.g. 2 args or 4 args)
        let invalid_argc = host.compile("ImGui.Image(tex, 16)").unwrap();
        let err = bridge.invoke(|| host.run(&invalid_argc)).unwrap_err();
        assert!(err.to_string().contains("invalid Image arguments"));

        // 2. Non-positive dimensions on valid texture handle
        let non_pos = host.compile("ImGui.Image(tex, -10, 20)").unwrap();
        let err = bridge.invoke(|| host.run(&non_pos)).unwrap_err();
        assert!(err.to_string().contains("positive"));

        // 3. Invalid handle type (passing ImVec2 instead of texture Handle)
        let invalid_type = host
            .compile("ImGui.Image(ImVec2.new(10, 10), 16, 16)")
            .unwrap();
        let err = bridge.invoke(|| host.run(&invalid_type)).unwrap_err();
        assert!(err.to_string().contains("userdata") || err.to_string().contains("Handle"));

        let _ = context.render(renderer.borrow_mut().renderer_consumer().unwrap());

        // Frame 3: Texture release and deferred retirement
        context.frame();
        // Emit valid draw call first
        let draw_before_release = host
            .compile(
                r#"
            assert(ImGui.Begin('retire_test', true, ImGuiWindowFlags.NoTitleBar))
            ImGui.Image(tex, 16, 16)
            ImGui.End()
        "#,
            )
            .unwrap();
        bridge.invoke(|| host.run(&draw_before_release)).unwrap();

        // Now release handle
        host.run(&host.compile("UiForge.ReleaseTexture(tex)").unwrap())
            .unwrap();

        // Next call with released handle must fail with 'expired'
        let stale_call = host.compile("ImGui.Image(tex, 16, 16)").unwrap();
        let err = bridge.invoke(|| host.run(&stale_call)).unwrap_err();
        assert!(err.to_string().contains("expired"));

        // Already emitted draw calls still survive render pass (deferred retirement)
        let _ = context.render(renderer.borrow_mut().renderer_consumer().unwrap());

        // Real ReleaseTexture foreign-bank rejection test using two banks installed on one Lua VM
        let bank2 = Textures::new(&device, &queue, renderer.clone());
        let t2_handle = host
            .lua
            .load("return UiForge.CreateTextureFromMemory(raw_tex_bytes, 4, 4)")
            .eval::<mlua::AnyUserData>()
            .unwrap();
        // Reinstall bank2 onto UiForge
        Textures::install(&bank2, &host.lua).unwrap();
        host.lua.globals().set("t2_handle", t2_handle).unwrap();
        // Attempting to release a handle belonging to bank 1 using bank 2's ReleaseTexture must fail with 'foreign'
        let err = host
            .run(&host.compile("UiForge.ReleaseTexture(t2_handle)").unwrap())
            .unwrap_err();
        assert!(err.to_string().contains("foreign"));

        textures.borrow_mut().collect(true);
        bank2.borrow_mut().collect(true);
    }
}
