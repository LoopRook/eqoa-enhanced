//! Bounded native widget bindings audited against pinned sol_ImGui.h.
//! All closures own only the epoch guard; native Ui/window addresses never escape.
use super::{Frame, Stack, Vec2, finite, text};
use dear_imgui_rs::sys;
use mlua::{Lua, MultiValue, Result, Table, Value};
use std::{cell::RefCell, rc::Rc};

const WINDOWS: &[(&str, i32)] = &[
    ("None", 0),
    ("NoTitleBar", sys::ImGuiWindowFlags_NoTitleBar),
    ("NoResize", sys::ImGuiWindowFlags_NoResize),
    ("NoMove", sys::ImGuiWindowFlags_NoMove),
    ("NoScrollbar", sys::ImGuiWindowFlags_NoScrollbar),
    ("NoScrollWithMouse", sys::ImGuiWindowFlags_NoScrollWithMouse),
    ("NoCollapse", sys::ImGuiWindowFlags_NoCollapse),
    ("AlwaysAutoResize", sys::ImGuiWindowFlags_AlwaysAutoResize),
    ("NoBackground", sys::ImGuiWindowFlags_NoBackground),
    ("NoSavedSettings", sys::ImGuiWindowFlags_NoSavedSettings),
    ("NoMouseInputs", sys::ImGuiWindowFlags_NoMouseInputs),
    ("MenuBar", sys::ImGuiWindowFlags_MenuBar),
    (
        "HorizontalScrollbar",
        sys::ImGuiWindowFlags_HorizontalScrollbar,
    ),
    (
        "NoFocusOnAppearing",
        sys::ImGuiWindowFlags_NoFocusOnAppearing,
    ),
    (
        "NoBringToFrontOnFocus",
        sys::ImGuiWindowFlags_NoBringToFrontOnFocus,
    ),
    (
        "AlwaysVerticalScrollbar",
        sys::ImGuiWindowFlags_AlwaysVerticalScrollbar,
    ),
    (
        "AlwaysHorizontalScrollbar",
        sys::ImGuiWindowFlags_AlwaysHorizontalScrollbar,
    ),
    ("NoNavInputs", sys::ImGuiWindowFlags_NoNavInputs),
    ("NoNavFocus", sys::ImGuiWindowFlags_NoNavFocus),
    ("NoNav", sys::ImGuiWindowFlags_NoNav),
    ("NoDecoration", sys::ImGuiWindowFlags_NoDecoration),
    ("NoInputs", sys::ImGuiWindowFlags_NoInputs),
];
fn flags(value: i32, allowed: i32, what: &str) -> Result<i32> {
    if value & !allowed != 0 {
        Err(mlua::Error::runtime(format!("unsupported {what} flags")))
    } else {
        Ok(value)
    }
}
pub(super) fn window_flags(value: i32) -> Result<()> {
    flags(value, WINDOWS.iter().fold(0, |v, (_, f)| v | f), "window").map(|_| ())
}
fn size(x: f32, y: f32) -> Result<sys::ImVec2_c> {
    Ok(Vec2(finite(x)?, finite(y)?).raw())
}
fn returns(open: Option<bool>, draw: bool) -> MultiValue {
    MultiValue::from_vec(match open {
        Some(open) => vec![Value::Boolean(open), Value::Boolean(draw)],
        None => vec![Value::Boolean(draw)],
    })
}
fn scope_end(
    lua: &Lua,
    imgui: &Table,
    frame: &Rc<RefCell<Frame>>,
    name: &str,
    kind: Stack,
    end: unsafe extern "C" fn(),
) -> Result<()> {
    let f = frame.clone();
    imgui.set(
        name,
        lua.create_function(move |_, ()| {
            let mut state = f.borrow_mut();
            state.check()?;
            state.pop(kind)?;
            unsafe { end() };
            Ok(())
        })?,
    )
}

pub(super) fn install(lua: &Lua, imgui: &Table, frame: &Rc<RefCell<Frame>>) -> Result<()> {
    let f = frame.clone();
    imgui.set(
        "Button",
        lua.create_function(move |lua, args: MultiValue| {
            if !matches!(args.len(), 1 | 3) {
                return Err(mlua::Error::runtime(
                    "Button expects label or label,width,height",
                ));
            }
            let (label, x, y): (String, Option<f32>, Option<f32>) = lua.unpack_multi(args)?;
            let label = text(&label)?;
            let size = size(x.unwrap_or(0.), y.unwrap_or(0.))?;
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igButton(label.as_ptr(), size) })
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "RadioButton",
        lua.create_function(move |lua, args: MultiValue| {
            f.borrow_mut().check()?;
            match (args.len(), args.get(1)) {
                (2, Some(Value::Boolean(_))) => {
                    let (label, active): (String, bool) = lua.unpack_multi(args)?;
                    let label = text(&label)?;
                    Ok(MultiValue::from_vec(vec![Value::Boolean(unsafe {
                        sys::igRadioButton_Bool(label.as_ptr(), active)
                    })]))
                }
                (3, _) => {
                    let (label, mut value, button): (String, i32, i32) = lua.unpack_multi(args)?;
                    let label = text(&label)?;
                    let changed =
                        unsafe { sys::igRadioButton_IntPtr(label.as_ptr(), &mut value, button) };
                    Ok(MultiValue::from_vec(vec![
                        Value::Integer(value as i64),
                        Value::Boolean(changed),
                    ]))
                }
                _ => Err(mlua::Error::runtime(
                    "RadioButton expects label,bool or label,int,int",
                )),
            }
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "Combo",
        lua.create_function(
            move |_,
                  (label, mut current, items, count, height): (
                String,
                i32,
                Table,
                i32,
                Option<i32>,
            )| {
                if !(0..=256).contains(&count) || !(-1..=256).contains(&height.unwrap_or(-1)) {
                    return Err(mlua::Error::runtime("Combo item/height limit exceeded"));
                }
                let label = text(&label)?;
                let mut strings = Vec::new();
                let mut bytes = 0;
                for i in 1..=count {
                    let value = items.raw_get::<Value>(i)?;
                    let s = match value {
                        Value::String(s) => text(&s.to_str()?)?,
                        _ => text("Missing")?,
                    };
                    bytes += s.as_bytes().len();
                    if bytes > 65536 {
                        return Err(mlua::Error::runtime("Combo text budget exceeded"));
                    }
                    strings.push(s);
                }
                let pointers: Vec<_> = strings.iter().map(|s| s.as_ptr()).collect();
                f.borrow_mut().check()?;
                let changed = unsafe {
                    sys::igCombo_Str_arr(
                        label.as_ptr(),
                        &mut current,
                        pointers.as_ptr(),
                        count,
                        height.unwrap_or(-1),
                    )
                };
                Ok((current, changed))
            },
        )?,
    )?;
    let f = frame.clone();
    imgui.set(
        "InputInt",
        lua.create_function(
            move |_,
                  (label, mut value, step, fast, input_flags): (
                String,
                i32,
                Option<i32>,
                Option<i32>,
                Option<i32>,
            )| {
                let label = text(&label)?;
                let step = step.unwrap_or(1);
                let fast = fast.unwrap_or(100);
                if value.unsigned_abs() > 1_000_000_000
                    || step.unsigned_abs() > 1_000_000
                    || fast.unsigned_abs() > 1_000_000
                {
                    return Err(mlua::Error::runtime("InputInt numeric bounds exceeded"));
                }
                let input_flags = flags(
                    input_flags.unwrap_or(0),
                    sys::ImGuiInputTextFlags_EnterReturnsTrue
                        | sys::ImGuiInputTextFlags_ReadOnly
                        | sys::ImGuiInputTextFlags_AutoSelectAll
                        | sys::ImGuiInputTextFlags_CharsHexadecimal,
                    "InputInt",
                )?;
                f.borrow_mut().check()?;
                let changed =
                    unsafe { sys::igInputInt(label.as_ptr(), &mut value, step, fast, input_flags) };
                Ok((value, changed))
            },
        )?,
    )?;
    // Fixed printf formats protect all user-controlled strings, including %n.
    for name in [
        "BulletText",
        "TextDisabled",
        "TextWrapped",
        "TextUnformatted",
        "SetTooltip",
    ] {
        let f = frame.clone();
        imgui.set(
            name,
            lua.create_function(move |lua, args: MultiValue| {
                if args.len()!=1 {return Err(mlua::Error::runtime("text helper requires one string; unrelated text-end pointers are unsupported"));}
                let value:String=lua.unpack_multi(args)?;
                let value = text(&value)?;
                f.borrow_mut().check()?;
                unsafe {
                    match name {
                        "BulletText" => sys::igBulletText(c"%s".as_ptr(), value.as_ptr()),
                        "TextDisabled" => sys::igTextDisabled(c"%s".as_ptr(), value.as_ptr()),
                        "TextWrapped" => sys::igTextWrapped(c"%s".as_ptr(), value.as_ptr()),
                        "TextUnformatted" => {
                            sys::igTextUnformatted(value.as_ptr(), std::ptr::null())
                        }
                        _ => sys::igSetTooltip(c"%s".as_ptr(), value.as_ptr()),
                    }
                };
                Ok(())
            })?,
        )?;
    }
    let f = frame.clone();
    imgui.set(
        "TextColored",
        lua.create_function(
            move |_, (r, g, b, a, value): (f32, f32, f32, f32, String)| {
                let color = sys::ImVec4_c {
                    x: finite(r)?,
                    y: finite(g)?,
                    z: finite(b)?,
                    w: finite(a)?,
                };
                let value = text(&value)?;
                f.borrow_mut().check()?;
                unsafe { sys::igTextColored(color, c"%s".as_ptr(), value.as_ptr()) };
                Ok(())
            },
        )?,
    )?;
    for (name, call) in [("Separator", sys::igSeparator as unsafe extern "C" fn())] {
        let f = frame.clone();
        imgui.set(
            name,
            lua.create_function(move |_, ()| {
                f.borrow_mut().check()?;
                unsafe { call() };
                Ok(())
            })?,
        )?;
    }
    let f = frame.clone();
    imgui.set(
        "GetTime",
        lua.create_function(move |_, ()| {
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igGetTime() })
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "IsItemActive",
        lua.create_function(move |_, ()| {
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igIsItemActive() })
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "IsItemHovered",
        lua.create_function(move |_, hovered: Option<i32>| {
            let hovered = flags(
                hovered.unwrap_or(0),
                sys::ImGuiHoveredFlags_AllowedMaskForIsItemHovered,
                "item hover",
            )?;
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igIsItemHovered(hovered) })
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "SetKeyboardFocusHere",
        lua.create_function(move |_, offset: Option<i32>| {
            let offset = offset.unwrap_or(0);
            if !(-1..=256).contains(&offset) {
                return Err(mlua::Error::runtime("keyboard focus offset outside bounds"));
            }
            f.borrow_mut().check()?;
            unsafe { sys::igSetKeyboardFocusHere(offset) };
            Ok(())
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "Dummy",
        lua.create_function(move |_, (x, y): (f32, f32)| {
            let size = size(x, y)?;
            if x < 0. || y < 0. {
                return Err(mlua::Error::runtime("Dummy requires nonnegative size"));
            }
            f.borrow_mut().check()?;
            unsafe { sys::igDummy(size) };
            Ok(())
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "ProgressBar",
        lua.create_function(move |lua, args: MultiValue| {
            if !matches!(args.len(), 1 | 3 | 4) {
                return Err(mlua::Error::runtime(
                    "ProgressBar expects fraction[,width,height[,overlay]]",
                ));
            }
            let (fraction, x, y, overlay): (f32, Option<f32>, Option<f32>, Option<String>) =
                lua.unpack_multi(args)?;
            let fraction = finite(fraction)?;
            let size = size(x.unwrap_or(-f32::MIN_POSITIVE), y.unwrap_or(0.))?;
            let overlay = overlay.as_deref().map(text).transpose()?;
            f.borrow_mut().check()?;
            unsafe {
                sys::igProgressBar(
                    fraction,
                    size,
                    overlay.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                )
            };
            Ok(())
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "PushTextWrapPos",
        lua.create_function(move |_, x: Option<f32>| {
            let x = finite(x.unwrap_or(0.))?;
            let mut s = f.borrow_mut();
            s.check()?;
            s.room()?;
            unsafe { sys::igPushTextWrapPos(x) };
            s.stack.push(Stack::TextWrap);
            Ok(())
        })?,
    )?;
    scope_end(
        lua,
        imgui,
        frame,
        "PopTextWrapPos",
        Stack::TextWrap,
        sys::igPopTextWrapPos,
    )?;
    let f = frame.clone();
    imgui.set(
        "TreeNode",
        lua.create_function(move |_, (label, display): (String, Option<String>)| {
            let label = text(&label)?;
            let display = display.as_deref().map(text).transpose()?;
            let mut s = f.borrow_mut();
            s.check()?;
            s.room()?;
            let open = unsafe {
                match display {
                    Some(display) => {
                        sys::igTreeNode_StrStr(label.as_ptr(), c"%s".as_ptr(), display.as_ptr())
                    }
                    None => sys::igTreeNode_Str(label.as_ptr()),
                }
            };
            if open {
                s.stack.push(Stack::Tree);
            }
            Ok(open)
        })?,
    )?;
    scope_end(lua, imgui, frame, "TreePop", Stack::Tree, sys::igTreePop)?;
    let tree_allowed = sys::ImGuiTreeNodeFlags_Selected
        | sys::ImGuiTreeNodeFlags_Framed
        | sys::ImGuiTreeNodeFlags_DefaultOpen
        | sys::ImGuiTreeNodeFlags_Leaf
        | sys::ImGuiTreeNodeFlags_NoTreePushOnOpen
        | sys::ImGuiTreeNodeFlags_OpenOnArrow
        | sys::ImGuiTreeNodeFlags_OpenOnDoubleClick
        | sys::ImGuiTreeNodeFlags_Bullet
        | sys::ImGuiTreeNodeFlags_FramePadding
        | sys::ImGuiTreeNodeFlags_SpanAvailWidth
        | sys::ImGuiTreeNodeFlags_SpanFullWidth;
    let f = frame.clone();
    imgui.set(
        "CollapsingHeader",
        lua.create_function(move |lua, args: MultiValue| {
            if !(1..=3).contains(&args.len()) {
                return Err(mlua::Error::runtime("invalid CollapsingHeader overload"));
            }
            let (label, arg, extra): (String, Option<Value>, Option<i32>) =
                lua.unpack_multi(args)?;
            let label = text(&label)?;
            f.borrow_mut().check()?;
            match arg {
                Some(Value::Boolean(mut open)) => {
                    let flags = flags(extra.unwrap_or(0), tree_allowed, "header")?;
                    let draw = unsafe {
                        sys::igCollapsingHeader_BoolPtr(label.as_ptr(), &mut open, flags)
                    };
                    Ok(returns(Some(open), draw))
                }
                None | Some(Value::Nil) => {
                    if extra.is_some() {
                        return Err(mlua::Error::runtime("header flags require open argument"));
                    }
                    Ok(returns(None, unsafe {
                        sys::igCollapsingHeader_TreeNodeFlags(label.as_ptr(), 0)
                    }))
                }
                Some(Value::Integer(value)) if extra.is_none() => {
                    let flags = flags(
                        i32::try_from(value).map_err(mlua::Error::external)?,
                        tree_allowed,
                        "header",
                    )?;
                    Ok(returns(None, unsafe {
                        sys::igCollapsingHeader_TreeNodeFlags(label.as_ptr(), flags)
                    }))
                }
                _ => Err(mlua::Error::runtime(
                    "header requires bool open or integer flags",
                )),
            }
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "BeginChild",
        lua.create_function(
            move |_,
                  (name, x, y, child, window): (
                String,
                Option<f32>,
                Option<f32>,
                Option<i32>,
                Option<i32>,
            )| {
                let name = text(&name)?;
                let size = size(x.unwrap_or(0.), y.unwrap_or(0.))?;
                let child = flags(child.unwrap_or(0), sys::ImGuiChildFlags_Borders, "child")?;
                let window = window.unwrap_or(0);
                window_flags(window)?;
                let mut s = f.borrow_mut();
                s.check()?;
                s.room()?;
                let draw = unsafe { sys::igBeginChild_Str(name.as_ptr(), size, child, window) };
                s.stack.push(Stack::Child);
                Ok(draw) // EndChild is mandatory even on false.
            },
        )?,
    )?;
    scope_end(lua, imgui, frame, "EndChild", Stack::Child, sys::igEndChild)?;
    for (name, kind, call) in [
        (
            "BeginMenuBar",
            Stack::MenuBar,
            sys::igBeginMenuBar as unsafe extern "C" fn() -> bool,
        ),
        (
            "BeginMainMenuBar",
            Stack::MainMenuBar,
            sys::igBeginMainMenuBar,
        ),
    ] {
        let f = frame.clone();
        imgui.set(
            name,
            lua.create_function(move |_, ()| {
                let mut s = f.borrow_mut();
                s.check()?;
                s.room()?;
                // Dear ImGui asserts on duplicate menu-bar entry, including
                // a bar owned by the host rather than this Lua invocation.
                let current = unsafe { sys::igGetCurrentWindow() };
                if !current.is_null() && unsafe { (*current).DC.MenuBarAppending } {
                    return Err(mlua::Error::runtime("menu bar is already active"));
                }
                let open = unsafe { call() };
                if open {
                    s.stack.push(kind);
                }
                Ok(open)
            })?,
        )?;
    }
    scope_end(
        lua,
        imgui,
        frame,
        "EndMenuBar",
        Stack::MenuBar,
        sys::igEndMenuBar,
    )?;
    scope_end(
        lua,
        imgui,
        frame,
        "EndMainMenuBar",
        Stack::MainMenuBar,
        sys::igEndMainMenuBar,
    )?;
    let f = frame.clone();
    imgui.set(
        "BeginMenu",
        lua.create_function(move |_, (label, enabled): (String, Option<bool>)| {
            let label = text(&label)?;
            let mut s = f.borrow_mut();
            s.check()?;
            s.room()?;
            let open = unsafe { sys::igBeginMenu(label.as_ptr(), enabled.unwrap_or(true)) };
            if open {
                s.stack.push(Stack::Menu);
            }
            Ok(open)
        })?,
    )?;
    scope_end(lua, imgui, frame, "EndMenu", Stack::Menu, sys::igEndMenu)?;
    let f = frame.clone();
    imgui.set(
        "MenuItem",
        lua.create_function(
            move |_,
                  (label, shortcut, selected, enabled): (
                String,
                Option<String>,
                Option<bool>,
                Option<bool>,
            )| {
                let label = text(&label)?;
                let shortcut = shortcut.as_deref().map(text).transpose()?;
                let shortcut = shortcut.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
                f.borrow_mut().check()?;
                match selected {
                    Some(mut selected) => {
                        let activated = unsafe {
                            sys::igMenuItem_BoolPtr(
                                label.as_ptr(),
                                shortcut,
                                &mut selected,
                                enabled.unwrap_or(true),
                            )
                        };
                        Ok(returns(Some(selected), activated))
                    }
                    None => {
                        if enabled.is_some() {
                            return Err(mlua::Error::runtime(
                                "MenuItem enabled requires selected overload",
                            ));
                        }
                        Ok(returns(None, unsafe {
                            sys::igMenuItem_Bool(label.as_ptr(), shortcut, false, true)
                        }))
                    }
                }
            },
        )?,
    )?;
    let tab_allowed = sys::ImGuiTabBarFlags_Reorderable
        | sys::ImGuiTabBarFlags_AutoSelectNewTabs
        | sys::ImGuiTabBarFlags_TabListPopupButton
        | sys::ImGuiTabBarFlags_NoCloseWithMiddleMouseButton
        | sys::ImGuiTabBarFlags_NoTabListScrollingButtons
        | sys::ImGuiTabBarFlags_NoTooltip
        | sys::ImGuiTabBarFlags_FittingPolicyScroll;
    let f = frame.clone();
    imgui.set(
        "BeginTabBar",
        lua.create_function(move |_, (id, tab): (String, Option<i32>)| {
            let id = text(&id)?;
            let tab = flags(tab.unwrap_or(0), tab_allowed, "tab bar")?;
            let mut s = f.borrow_mut();
            s.check()?;
            s.room()?;
            if s.tab_item_parent() {
                return Err(mlua::Error::runtime(
                    "nested TabBar requires a TabItem boundary",
                ));
            }
            let open = unsafe { sys::igBeginTabBar(id.as_ptr(), tab) };
            if open {
                s.stack.push(Stack::TabBar);
            }
            Ok(open)
        })?,
    )?;
    scope_end(
        lua,
        imgui,
        frame,
        "EndTabBar",
        Stack::TabBar,
        sys::igEndTabBar,
    )?;
    let item_allowed = sys::ImGuiTabItemFlags_UnsavedDocument
        | sys::ImGuiTabItemFlags_SetSelected
        | sys::ImGuiTabItemFlags_NoCloseWithMiddleMouseButton
        | sys::ImGuiTabItemFlags_NoPushId
        | sys::ImGuiTabItemFlags_NoTooltip
        | sys::ImGuiTabItemFlags_NoReorder
        | sys::ImGuiTabItemFlags_Leading
        | sys::ImGuiTabItemFlags_Trailing;
    let f = frame.clone();
    imgui.set(
        "BeginTabItem",
        lua.create_function(
            move |_, (label, open, item): (String, Option<bool>, Option<i32>)| {
                let label = text(&label)?;
                if open.is_none() && item.is_some() {
                    return Err(mlua::Error::runtime("tab flags require open overload"));
                }
                let item = flags(item.unwrap_or(0), item_allowed, "tab item")?;
                if item & sys::ImGuiTabItemFlags_Leading != 0
                    && item & sys::ImGuiTabItemFlags_Trailing != 0
                {
                    return Err(mlua::Error::runtime(
                        "tab cannot be both Leading and Trailing",
                    ));
                }
                let mut s = f.borrow_mut();
                s.check()?;
                s.room()?;
                if !s.tab_item_parent() {
                    return Err(mlua::Error::runtime(
                        "BeginTabItem requires own active TabBar",
                    ));
                }
                let mut value = open.unwrap_or(true);
                let draw = unsafe {
                    sys::igBeginTabItem(
                        label.as_ptr(),
                        if open.is_some() {
                            &mut value
                        } else {
                            std::ptr::null_mut()
                        },
                        item,
                    )
                };
                if draw {
                    s.stack.push(Stack::TabItem);
                }
                Ok(returns(open.map(|_| value), draw))
            },
        )?,
    )?;
    scope_end(
        lua,
        imgui,
        frame,
        "EndTabItem",
        Stack::TabItem,
        sys::igEndTabItem,
    )?;

    // Window queries and position
    let f = frame.clone();
    imgui.set(
        "GetWindowPos",
        lua.create_function(move |_, ()| {
            f.borrow_mut().check()?;
            let pos = unsafe { sys::igGetWindowPos() };
            Ok((pos.x, pos.y))
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "SetWindowPos",
        lua.create_function(move |_, (x, y, cond): (f32, f32, Option<i32>)| {
            f.borrow_mut().check()?;
            let pos = sys::ImVec2_c {
                x: finite(x)?,
                y: finite(y)?,
            };
            unsafe {
                sys::igSetWindowPos_Vec2(pos, cond.unwrap_or(0));
            }
            Ok(())
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "SetNextWindowPos",
        lua.create_function(
            move |_, (x, y, cond, px, py): (f32, f32, Option<i32>, Option<f32>, Option<f32>)| {
                f.borrow_mut().check()?;
                let pos = sys::ImVec2_c {
                    x: finite(x)?,
                    y: finite(y)?,
                };
                let pivot = sys::ImVec2_c {
                    x: finite(px.unwrap_or(0.0))?,
                    y: finite(py.unwrap_or(0.0))?,
                };
                unsafe {
                    sys::igSetNextWindowPos(pos, cond.unwrap_or(0), pivot);
                }
                Ok(())
            },
        )?,
    )?;
    let f = frame.clone();
    imgui.set(
        "SetNextWindowSize",
        lua.create_function(move |_, (w, h, cond): (f32, f32, Option<i32>)| {
            f.borrow_mut().check()?;
            let size = sys::ImVec2_c {
                x: finite(w)?,
                y: finite(h)?,
            };
            unsafe {
                sys::igSetNextWindowSize(size, cond.unwrap_or(0));
            }
            Ok(())
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "IsWindowFocused",
        lua.create_function(move |_, flags: Option<i32>| {
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igIsWindowFocused(flags.unwrap_or(0)) })
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "IsWindowHovered",
        lua.create_function(move |_, flags: Option<i32>| {
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igIsWindowHovered(flags.unwrap_or(0)) })
        })?,
    )?;

    // Cursor, layout, and queries
    let f = frame.clone();
    imgui.set(
        "SetCursorScreenPos",
        lua.create_function(move |_, (x, y): (f32, f32)| {
            f.borrow_mut().check()?;
            unsafe {
                sys::igSetCursorScreenPos(sys::ImVec2_c {
                    x: finite(x)?,
                    y: finite(y)?,
                });
            }
            Ok(())
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "SetNextItemWidth",
        lua.create_function(move |_, width: f32| {
            f.borrow_mut().check()?;
            unsafe {
                sys::igSetNextItemWidth(finite(width)?);
            }
            Ok(())
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "GetContentRegionAvail",
        lua.create_function(move |_, ()| {
            f.borrow_mut().check()?;
            let avail = unsafe { sys::igGetContentRegionAvail() };
            Ok((avail.x, avail.y))
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "GetFrameHeight",
        lua.create_function(move |_, ()| {
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igGetFrameHeight() })
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "GetTextLineHeight",
        lua.create_function(move |_, ()| {
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igGetTextLineHeight() })
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "GetScrollY",
        lua.create_function(move |_, ()| {
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igGetScrollY() })
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "GetScrollMaxY",
        lua.create_function(move |_, ()| {
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igGetScrollMaxY() })
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "SetScrollHereY",
        lua.create_function(move |_, ratio: Option<f32>| {
            f.borrow_mut().check()?;
            unsafe {
                sys::igSetScrollHereY(ratio.unwrap_or(0.5));
            }
            Ok(())
        })?,
    )?;

    // Mouse queries and invisible button
    let f = frame.clone();
    imgui.set(
        "IsMouseDragging",
        lua.create_function(move |_, (button, threshold): (Option<i32>, Option<f32>)| {
            f.borrow_mut().check()?;
            Ok(unsafe { sys::igIsMouseDragging(button.unwrap_or(0), threshold.unwrap_or(-1.0)) })
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "IsMouseHoveringRect",
        lua.create_function(
            move |_, (x1, y1, x2, y2, clip): (f32, f32, f32, f32, Option<bool>)| {
                f.borrow_mut().check()?;
                let r_min = sys::ImVec2_c {
                    x: finite(x1)?,
                    y: finite(y1)?,
                };
                let r_max = sys::ImVec2_c {
                    x: finite(x2)?,
                    y: finite(y2)?,
                };
                Ok(unsafe { sys::igIsMouseHoveringRect(r_min, r_max, clip.unwrap_or(true)) })
            },
        )?,
    )?;
    let f = frame.clone();
    imgui.set(
        "InvisibleButton",
        lua.create_function(
            move |_, (id, w, h, flags): (String, f32, f32, Option<i32>)| {
                let id = text(&id)?;
                let size = sys::ImVec2_c {
                    x: finite(w)?,
                    y: finite(h)?,
                };
                f.borrow_mut().check()?;
                Ok(unsafe { sys::igInvisibleButton(id.as_ptr(), size, flags.unwrap_or(0)) })
            },
        )?,
    )?;

    // ID stack
    let f = frame.clone();
    imgui.set(
        "PushID",
        lua.create_function(move |_, val: Value| {
            let mut state = f.borrow_mut();
            state.check()?;
            state.room()?;
            match val {
                Value::String(s) => {
                    let cs = text(&s.to_str()?)?;
                    unsafe { sys::igPushID_Str(cs.as_ptr()) };
                }
                Value::Integer(i) => {
                    unsafe { sys::igPushID_Int(i as std::os::raw::c_int) };
                }
                _ => return Err(mlua::Error::runtime("PushID requires string or integer")),
            }
            state.stack.push(Stack::Id);
            Ok(())
        })?,
    )?;
    scope_end(lua, imgui, frame, "PopID", Stack::Id, sys::igPopID)?;

    // Combos
    let f = frame.clone();
    imgui.set(
        "BeginCombo",
        lua.create_function(
            move |_, (label, preview, flags): (String, Option<String>, Option<i32>)| {
                let label = text(&label)?;
                let preview = preview.map(|p| text(&p)).transpose()?;
                let mut state = f.borrow_mut();
                state.check()?;
                state.room()?;
                let open = unsafe {
                    sys::igBeginCombo(
                        label.as_ptr(),
                        preview.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
                        flags.unwrap_or(0),
                    )
                };
                if open {
                    state.stack.push(Stack::Combo);
                }
                Ok(open)
            },
        )?,
    )?;
    scope_end(lua, imgui, frame, "EndCombo", Stack::Combo, sys::igEndCombo)?;

    // Popups
    let f = frame.clone();
    imgui.set(
        "OpenPopup",
        lua.create_function(move |_, (str_id, flags): (String, Option<i32>)| {
            let str_id = text(&str_id)?;
            f.borrow_mut().check()?;
            unsafe {
                sys::igOpenPopup_Str(str_id.as_ptr(), flags.unwrap_or(0));
            }
            Ok(())
        })?,
    )?;
    let f = frame.clone();
    imgui.set(
        "BeginPopup",
        lua.create_function(move |_, (str_id, flags): (String, Option<i32>)| {
            let str_id = text(&str_id)?;
            let mut state = f.borrow_mut();
            state.check()?;
            state.room()?;
            let open = unsafe { sys::igBeginPopup(str_id.as_ptr(), flags.unwrap_or(0)) };
            if open {
                state.stack.push(Stack::Popup);
            }
            Ok(open)
        })?,
    )?;
    scope_end(lua, imgui, frame, "EndPopup", Stack::Popup, sys::igEndPopup)?;

    // Selectable
    let f = frame.clone();
    imgui.set(
        "Selectable",
        lua.create_function(
            move |_,
                  (label, selected, flags, w, h): (
                String,
                Option<bool>,
                Option<i32>,
                Option<f32>,
                Option<f32>,
            )| {
                let label = text(&label)?;
                let size = sys::ImVec2_c {
                    x: finite(w.unwrap_or(0.0))?,
                    y: finite(h.unwrap_or(0.0))?,
                };
                f.borrow_mut().check()?;
                Ok(unsafe {
                    sys::igSelectable_Bool(
                        label.as_ptr(),
                        selected.unwrap_or(false),
                        flags.unwrap_or(0),
                        size,
                    )
                })
            },
        )?,
    )?;

    // Text inputs
    let f = frame.clone();
    imgui.set(
        "InputTextWithHint",
        lua.create_function(
            move |_,
                  (label, hint, current_text, flags): (
                String,
                String,
                Option<String>,
                Option<i32>,
            )| {
                let label = text(&label)?;
                let hint = text(&hint)?;
                let cur = current_text.unwrap_or_default();
                let mut buf = vec![0u8; 4096];
                let bytes = cur.as_bytes();
                let copy_len = bytes.len().min(4095);
                buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
                buf[copy_len] = 0;
                f.borrow_mut().check()?;
                let ret = unsafe {
                    sys::igInputTextWithHint(
                        label.as_ptr(),
                        hint.as_ptr(),
                        buf.as_mut_ptr() as *mut std::os::raw::c_char,
                        buf.len(),
                        flags.unwrap_or(0),
                        None,
                        std::ptr::null_mut(),
                    )
                };
                let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
                let out_text = String::from_utf8_lossy(&buf[..len]).to_string();
                Ok((out_text, ret))
            },
        )?,
    )?;
    let windows: Table = lua.globals().get("ImGuiWindowFlags")?;
    for &(name, value) in WINDOWS {
        windows.set(name, value)?;
    }
    let colors: Table = lua.globals().get("ImGuiCol")?;
    colors.set("PlotHistogram", sys::ImGuiCol_PlotHistogram)?;
    for (name, entries) in [
        (
            "ImGuiChildFlags",
            vec![("None", 0), ("Border", sys::ImGuiChildFlags_Borders)],
        ),
        (
            "ImGuiTreeNodeFlags",
            vec![
                ("None", 0),
                ("DefaultOpen", sys::ImGuiTreeNodeFlags_DefaultOpen),
                ("Leaf", sys::ImGuiTreeNodeFlags_Leaf),
                ("CollapsingHeader", sys::ImGuiTreeNodeFlags_CollapsingHeader),
            ],
        ),
        (
            "ImGuiTabBarFlags",
            vec![
                ("None", 0),
                ("Reorderable", sys::ImGuiTabBarFlags_Reorderable),
                ("AutoSelectNewTabs", sys::ImGuiTabBarFlags_AutoSelectNewTabs),
            ],
        ),
        (
            "ImGuiTabItemFlags",
            vec![
                ("None", 0),
                ("SetSelected", sys::ImGuiTabItemFlags_SetSelected),
            ],
        ),
        (
            "ImGuiHoveredFlags",
            vec![
                ("None", 0),
                (
                    "AllowWhenDisabled",
                    sys::ImGuiHoveredFlags_AllowWhenDisabled,
                ),
            ],
        ),
    ] {
        let table = lua.create_table()?;
        for (key, value) in entries {
            table.set(key, value)?;
        }
        lua.globals().set(name, table)?;
    }
    Ok(())
}
