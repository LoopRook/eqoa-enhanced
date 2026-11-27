//! Which mod window is in front under the pointer: Dear ImGui's own display order is the one order every mod's
//! window shares, so a click goes to the front-most window and the windows behind it (other mods', the game's)
//! get nothing. A mod's custom-drawn window is an ImGui layer window with its own `claim_pointer` rectangles
//! (`pointer_api`); a mod's ImGui window is a real one ImGui hit-tests itself. This file reads the order
//! (`Windows` is back to front), lets a click raise the window it landed on, and hides the pointer from a real
//! window that a claimed window covers.
use dear_imgui_rs::sys;
use std::cell::Cell;

thread_local! {
    /// The window a press landed on, to raise at the start of the next frame (ImGui is idle between frames).
    static RAISE: Cell<Option<u32>> = const { Cell::new(None) };
}

/// Runs `f` with the live ImGui context, or returns `None` when there is none.
fn with_context<R>(f: impl FnOnce(*mut sys::ImGuiContext) -> R) -> Option<R> {
    let g = unsafe { sys::igGetCurrentContext() };
    (!g.is_null()).then(|| f(g))
}

/// The id of the top-level window being built right now (the layer a mod's `claim_pointer` was called in), or
/// `None` outside a frame, in the context's fallback window, or without a context.
pub(super) fn current_window_id() -> Option<u32> {
    with_context(|g| unsafe {
        if !(*g).WithinFrameScope {
            return None;
        }
        let w = (*g).CurrentWindow;
        if w.is_null() || (*w).IsFallbackWindow {
            return None;
        }
        let root = (*w).RootWindow;
        Some(if root.is_null() { (*w).ID } else { (*root).ID })
    })
    .flatten()
}

/// The place of a window in ImGui's display order (0 is the back), or `None` for a window it does not know.
pub(super) fn z_of(id: u32) -> Option<usize> {
    with_context(|g| unsafe {
        let w = sys::igFindWindowByID(id);
        if w.is_null() {
            return None;
        }
        index_of(g, w)
    })
    .flatten()
}

unsafe fn index_of(g: *mut sys::ImGuiContext, w: *mut sys::ImGuiWindow) -> Option<usize> {
    let list = unsafe { &(*g).Windows };
    (0..list.Size.max(0) as usize).find(|i| unsafe { *list.Data.add(*i) } == w)
}

/// The place in display order of the front-most REAL window (one that takes the mouse; a mod's layer windows are
/// `NoInputs` and never count) whose rectangle holds the point, as ImGui drew it last frame.
pub(super) fn front_real_window_z(x: f32, y: f32) -> Option<usize> {
    with_context(|g| unsafe {
        let list = &(*g).Windows;
        (0..list.Size.max(0) as usize).rev().find(|i| {
            let w = *list.Data.add(*i);
            if w.is_null() {
                return false;
            }
            let w = &*w;
            w.Active
                && !w.Hidden
                && !w.IsFallbackWindow
                && w.Flags & (sys::ImGuiWindowFlags_NoMouseInputs | sys::ImGuiWindowFlags_ChildWindow) == 0
                && x >= w.Pos.x
                && x < w.Pos.x + w.Size.x
                && y >= w.Pos.y
                && y < w.Pos.y + w.Size.y
        })
    })
    .flatten()
}

/// A press landed on this window: it comes to the front when the next frame starts.
pub(super) fn raise_next_frame(id: u32) {
    RAISE.with(|r| r.set(Some(id)));
}

#[cfg(test)]
pub(super) fn raise_pending() -> Option<u32> {
    RAISE.with(|r| r.get())
}

/// Called right after ImGui's new frame, before any mod draws. Raises the window the last press landed on, then,
/// when a claimed (custom-drawn) window is in front of the real window ImGui found under the pointer, takes the
/// pointer from that real window for the frame (no hover, no click, no tooltip): the window in front owns it.
pub(super) fn begin_frame() {
    if super::pointer_api::legacy() {
        return;
    }
    if let Some(id) = RAISE.with(|r| r.take()) {
        bring_to_front(id);
    }
    let Some(at) = super::pointer_api::pointer_at() else {
        return;
    };
    let Some(front) = super::pointer_api::front_claim_z(at[0], at[1]) else {
        return;
    };
    with_context(|g| unsafe {
        let hovered = (*g).HoveredWindow;
        if hovered.is_null() || !(*g).WithinFrameScope {
            return;
        }
        // a drag that began on a real window (a slider, a moving window) keeps it
        if (*g).ActiveId != 0 || !(*g).MovingWindow.is_null() {
            return;
        }
        let root = if (*hovered).RootWindow.is_null() { hovered } else { (*hovered).RootWindow };
        let Some(real) = index_of(g, root) else {
            return;
        };
        if front > real {
            (*g).HoveredWindow = std::ptr::null_mut();
            (*g).HoveredWindowUnderMovingWindow = std::ptr::null_mut();
            (*g).IO.WantCaptureMouse = false;
        }
    });
}

fn bring_to_front(id: u32) {
    with_context(|_| unsafe {
        let w = sys::igFindWindowByID(id);
        if !w.is_null() {
            sys::igBringWindowToDisplayFront(w);
        }
    });
}

// ---- which mod a window is: a click on any mod window gives that mod Escape (2026-11-27) -------------------------------
//
// A mod draws its windows inside its own `render` (`native_events::render`), so a top-level window that first becomes active
// this frame while a mod draws is that mod's. A click on a real ImGui window (a mod's `ImGui.Begin` window with no
// `claim_pointer`, such as Social's, or a third-party mod's plain window) then finds its mod here, as a click on a claimed
// window finds it through its claim.

thread_local! {
    /// Each mod window's id with its mod's VM, as last drawn.
    static OWNERS: std::cell::RefCell<std::collections::HashMap<u32, u64>> = std::cell::RefCell::new(std::collections::HashMap::new());
    /// The top-level windows already active this frame when a mod began to draw (they are not that mod's).
    static BEFORE: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The top-level windows begun so far this frame.
fn active_roots() -> Vec<u32> {
    with_context(|g| unsafe {
        let list = &(*g).Windows;
        let frame = (*g).FrameCount;
        (0..list.Size.max(0) as usize)
            .filter_map(|i| {
                let w = *list.Data.add(i);
                (!w.is_null()
                    && (*w).LastFrameActive == frame
                    && !(*w).IsFallbackWindow
                    && (*w).Flags & sys::ImGuiWindowFlags_ChildWindow == 0)
                    .then(|| (*w).ID)
            })
            .collect()
    })
    .unwrap_or_default()
}

/// A mod is about to draw.
pub(super) fn begin_vm_draw() {
    let roots = active_roots();
    BEFORE.with(|b| *b.borrow_mut() = roots);
}

/// Mod `vm` drew: the top-level windows that became active while it drew are its own.
pub(super) fn end_vm_draw(vm: u64) {
    let before = BEFORE.with(|b| std::mem::take(&mut *b.borrow_mut()));
    let mine: Vec<u32> = active_roots().into_iter().filter(|id| !before.contains(id)).collect();
    if mine.is_empty() {
        return;
    }
    OWNERS.with(|o| {
        let mut o = o.borrow_mut();
        if o.len() > 4096 {
            o.clear();
        }
        for id in mine {
            o.insert(id, vm);
        }
    });
}

/// The mod whose real (input-taking) window is in front at the point, as ImGui drew it last frame.
pub(super) fn real_window_vm_at(x: f32, y: f32) -> Option<u64> {
    let z = front_real_window_z(x, y)?;
    let id = with_context(|g| unsafe {
        let w = *(*g).Windows.Data.add(z);
        let root = if (*w).RootWindow.is_null() { w } else { (*w).RootWindow };
        (*root).ID
    })?;
    OWNERS.with(|o| o.borrow().get(&id).copied())
}
