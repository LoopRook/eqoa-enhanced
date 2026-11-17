// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Physical player gesture gate and synthetic report guard for game actions.
//!
//! Actions require an active, unconsumed `GestureToken` bound to the calling VM
//! during:
//! 1. Its own Controls-page key press (`eqoa.input.bind` matching non-repeat,
//!    non-synthetic key down),
//! 2. A `pointer.pressed` inside the VM's claimed pointer rectangle, or
//! 3. An interactive ImGui widget returning true/changed during that VM's
//!    `render` / `settings.render` callback.

use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GestureKind {
    ModKey,
    PointerClaim,
    ImGuiClick,
}

impl GestureKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ModKey => "mod_key",
            Self::PointerClaim => "pointer_claim",
            Self::ImGuiClick => "imgui_click",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GestureToken {
    pub serial: u64,
    pub vm: u64,
    pub addon_id: String,
    pub kind: GestureKind,
}

#[derive(Clone, Debug)]
struct ActiveGesture {
    vm: u64,
    addon_id: String,
    kind: GestureKind,
    serial: u64,
    armed: bool,
    consumed_single: bool,
    consumed_batch: bool,
}

thread_local! {
    static NEXT_SERIAL: Cell<u64> = const { Cell::new(1) };
    static ACTIVE_GESTURE: RefCell<Option<ActiveGesture>> = const { RefCell::new(None) };
}

fn next_serial() -> u64 {
    NEXT_SERIAL.with(|n| {
        let s = n.get();
        n.set(s.wrapping_add(1).max(1));
        s
    })
}

pub(crate) struct GestureScope {
    prev: Option<ActiveGesture>,
}

impl Drop for GestureScope {
    fn drop(&mut self) {
        ACTIVE_GESTURE.with(|slot| {
            *slot.borrow_mut() = self.prev.take();
        });
    }
}

/// Enters a synchronous gesture delivery scope for `vm`/`addon_id`, restoring
/// any enclosing scope on drop.
pub(crate) fn enter_gesture(
    vm: u64,
    addon_id: &str,
    kind: GestureKind,
    armed: bool,
) -> GestureScope {
    let prev = ACTIVE_GESTURE.with(|slot| {
        slot.borrow_mut().replace(ActiveGesture {
            vm,
            addon_id: addon_id.to_string(),
            kind,
            serial: next_serial(),
            armed,
            consumed_single: false,
            consumed_batch: false,
        })
    });
    GestureScope { prev }
}

/// Enters an initially unarmed ImGui render scope; `arm_imgui_click()` arms it
/// if a clickable widget reports activation during the callback.
pub(crate) fn enter_render_scope(vm: u64, addon_id: &str) -> GestureScope {
    enter_gesture(vm, addon_id, GestureKind::ImGuiClick, false)
}

/// Arms the current `ImGuiClick` scope when an interactive widget returns true.
pub(crate) fn arm_imgui_click() {
    ACTIVE_GESTURE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(active) = slot.as_mut()
            && active.kind == GestureKind::ImGuiClick
            && !active.consumed_single
            && !active.consumed_batch
        {
            active.armed = true;
        }
    });
}

/// Arms the current `ModKey` physical-event scope when a Controls bind matches.
pub(crate) fn arm_mod_key(vm: u64) {
    ACTIVE_GESTURE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(active) = slot.as_mut()
            && active.vm == vm
            && active.kind == GestureKind::ModKey
            && !active.consumed_single
            && !active.consumed_batch
        {
            active.armed = true;
        }
    });
}

/// Checks whether the current gesture scope authorizes `vm` without consuming yet.
pub(crate) fn peek_token(
    vm: u64,
    addon_id: &str,
    for_batch: bool,
) -> std::result::Result<GestureToken, (&'static str, String)> {
    let kind_label = if for_batch { "starting a batch" } else { "actions" };
    ACTIVE_GESTURE.with(|slot| {
        let slot = slot.borrow();
        let Some(active) = slot.as_ref() else {
            return Err((
                "needs_player_action",
                format!(
                    "{addon_id}: {kind_label} require a physical player gesture (Controls key, claimed pointer click, or ImGui click)"
                ),
            ));
        };
        if active.vm != vm || !active.armed {
            return Err((
                "needs_player_action",
                format!(
                    "{addon_id}: {kind_label} require an active player click or mod key gesture"
                ),
            ));
        }
        if active.consumed_single || active.consumed_batch {
            return Err((
                "needs_player_action",
                format!(
                    "{addon_id}: only one action or batch may start per player gesture"
                ),
            ));
        }
        Ok(GestureToken {
            serial: active.serial,
            vm: active.vm,
            addon_id: active.addon_id.clone(),
            kind: active.kind,
        })
    })
}

/// Marks the checked token as consumed once the action or batch is accepted.
pub(crate) fn consume_token(token: &GestureToken, for_batch: bool) {
    ACTIVE_GESTURE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(active) = slot.as_mut()
            && active.serial == token.serial
            && active.vm == token.vm
        {
            if for_batch {
                active.consumed_batch = true;
            } else {
                active.consumed_single = true;
            }
        }
    });
}

/// Consumes the single-action allowance from the current gesture scope.
pub(crate) fn take_single_token(
    vm: u64,
    addon_id: &str,
) -> std::result::Result<GestureToken, (&'static str, String)> {
    let tok = peek_token(vm, addon_id, false)?;
    consume_token(&tok, false);
    Ok(tok)
}

/// Consumes the single-batch allowance from the current gesture scope (Part C).
pub(crate) fn take_batch_token(
    vm: u64,
    addon_id: &str,
) -> std::result::Result<GestureToken, (&'static str, String)> {
    let tok = peek_token(vm, addon_id, true)?;
    consume_token(&tok, true);
    Ok(tok)
}

/// Returns true if `text` routes through `/bug`, `#bug`, `/feature`, `#feature`,
/// or wraps one inside a chat/tell/group/guild/shout/reply/petition command.
pub(crate) fn is_forbidden_report_text(text: &str) -> bool {
    for segment in text.split(['\r', '\n']) {
        if check_report_segment(segment, 0) {
            return true;
        }
    }
    false
}

fn check_report_segment(segment: &str, depth: usize) -> bool {
    if depth >= 16 {
        return true;
    }
    let trimmed = segment.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    if trimmed.is_empty() {
        return false;
    }
    let split_at = trimmed
        .find(|c: char| c.is_whitespace() || c == ':')
        .unwrap_or(trimmed.len());
    let head = trimmed[..split_at].to_ascii_lowercase();
    if matches!(head.as_str(), "/bug" | "#bug" | "/feature" | "#feature") {
        return true;
    }
    let after_head = trimmed[split_at..].trim_start_matches(|c: char| c.is_whitespace() || c == ':');
    if after_head.is_empty() {
        return false;
    }
    match head.as_str() {
        "/t" | "/tell" | "/w" | "/whisper" | "/sayto" => {
            if check_report_segment(after_head, depth + 1) {
                return true;
            }
            if let Some(idx) = after_head.find(char::is_whitespace) {
                return check_report_segment(&after_head[idx..], depth + 1);
            }
            false
        }
        "/say" | "/sa" | "/g" | "/group" | "/party" | "/p" | "/gu" | "/guild" | "/s"
        | "/shout" | "/sh" | "/r" | "/reply" | "/ooc" | "/auc" | "/auction" | "/em"
        | "/emote" | "/me" | "/chat" | "/petition" | "/feedback" | "/report" => {
            check_report_segment(after_head, depth + 1)
        }
        _ => false,
    }
}
