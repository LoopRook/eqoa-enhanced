use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;
use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

const KEY_RING_SIZE: usize = 128;
const TEXT_CAPTURE_LAPSE_SECS: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawKeyEvent {
    pub key: u8,
    pub is_down: bool,
    pub seq: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct InputSnapshot {
    pub button_bitfield: u16,
    pub right_analog: (u8, u8),
    pub left_analog: (u8, u8),
}

impl Default for InputSnapshot {
    fn default() -> Self {
        Self {
            button_bitfield: 0xFFFF, // active-low: 0xFFFF means unpressed
            right_analog: (0x7F, 0x7F),
            left_analog: (0x7F, 0x7F),
        }
    }
}

pub struct InputState {
    // Keyboard hook state
    pub key_hook_installed: bool,
    pub key_hook_desired: bool,
    pub hook_claims: HashSet<String>,
    pub key_claims: HashMap<u8, String>,
    pub suppress_shadow: HashMap<u8, bool>,
    pub text_capture_owners: HashMap<String, Instant>,
    pub text_capture: bool,
    pub key_events: Vec<RawKeyEvent>,
    pub key_cursors: HashMap<String, u32>,
    pub unowned_cursor: u32,
    pub event_seq: u32,
    pub dropped_keys: u32,

    // D-pad hook state
    pub dpad_installed: bool,
    pub dpad_desired: bool,
    pub dpad_rotated: bool,
    pub dpad_claims: HashSet<String>,
    pub dpad_requests: HashMap<String, bool>,
    pub dpad_error: Option<String>,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            key_hook_installed: false,
            key_hook_desired: false,
            hook_claims: HashSet::new(),
            key_claims: HashMap::new(),
            suppress_shadow: HashMap::new(),
            text_capture_owners: HashMap::new(),
            text_capture: false,
            key_events: Vec::with_capacity(KEY_RING_SIZE),
            key_cursors: HashMap::new(),
            unowned_cursor: 0,
            event_seq: 0,
            dropped_keys: 0,

            dpad_installed: false,
            dpad_desired: false,
            dpad_rotated: false,
            dpad_claims: HashSet::new(),
            dpad_requests: HashMap::new(),
            dpad_error: None,
        }
    }
}

thread_local! {
    static INPUT_STATE: RefCell<InputState> = RefCell::new(InputState::default());
}

pub fn reset_input_state() {
    INPUT_STATE.with(|s| {
        *s.borrow_mut() = InputState::default();
    });
}

pub fn record_key_event(key: u8, is_down: bool) {
    INPUT_STATE.with(|s| {
        let mut state = s.borrow_mut();
        let seq = state.event_seq;
        state.event_seq = state.event_seq.wrapping_add(1);

        let event = RawKeyEvent { key, is_down, seq };
        if state.key_events.len() >= KEY_RING_SIZE {
            state.key_events.remove(0);
        }
        state.key_events.push(event);
    });
}

#[allow(dead_code)]
pub fn is_dpad_rotated() -> bool {
    INPUT_STATE.with(|s| s.borrow().dpad_rotated)
}

pub fn is_key_claimed(key: u8) -> bool {
    INPUT_STATE.with(|s| s.borrow().key_claims.contains_key(&key))
}

pub fn is_text_capture_active() -> bool {
    INPUT_STATE.with(|s| {
        let mut state = s.borrow_mut();
        let now = Instant::now();
        state
            .text_capture_owners
            .retain(|_, t| now.duration_since(*t).as_secs_f32() <= TEXT_CAPTURE_LAPSE_SECS);
        state.text_capture = !state.text_capture_owners.is_empty();
        state.text_capture
    })
}

pub fn is_key_suppressed(key: u8) -> bool {
    INPUT_STATE.with(|s| {
        let mut state = s.borrow_mut();
        let now = Instant::now();
        state
            .text_capture_owners
            .retain(|_, t| now.duration_since(*t).as_secs_f32() <= TEXT_CAPTURE_LAPSE_SECS);
        state.text_capture = !state.text_capture_owners.is_empty();
        if state.text_capture && key < 0x80 {
            return true;
        }
        state.suppress_shadow.get(&key).copied().unwrap_or(false)
    })
}

pub fn logical_key_to_code(logical_key: &Key) -> Option<u8> {
    match logical_key {
        Key::Named(NamedKey::Enter) => Some(0x0D),
        Key::Named(NamedKey::Backspace) => Some(0x08),
        Key::Named(NamedKey::Tab) => Some(0x09),
        Key::Named(NamedKey::Escape) => Some(0x1B),
        Key::Named(NamedKey::Space) => Some(0x20),
        Key::Character(s) => {
            let c = s.chars().next()?;
            if c.is_ascii() && !c.is_ascii_control() {
                Some(c as u8)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn winit_key_to_code(event: &KeyEvent) -> Option<u8> {
    logical_key_to_code(&event.logical_key)
}

/// Evaluates whether a keyboard event should be captured by the addon runtime.
/// Key suppression only replaces what that key does in plain gameplay (opening
/// the client's Say window), never input to game pop-ups, dialogs, menus, or
/// native windows (`plain_play` is false). Text capture and focused widgets
/// take the keyboard regardless.
pub fn wants_key_capture(
    keyboard: bool,
    code: Option<u8>,
    is_synthetic: bool,
    plain_play: bool,
) -> bool {
    (keyboard || is_text_capture_active() || (plain_play && code.is_some_and(is_key_suppressed)))
        && !is_synthetic
}

fn get_key_name(code: u8) -> String {
    match code {
        0x0D => "Enter".to_string(),
        0x08 => "Backspace".to_string(),
        0x09 => "Tab".to_string(),
        0x1B => "Escape".to_string(),
        0x20 => "Space".to_string(),
        0x21..=0x7E => (code as char).to_string(),
        _ => format!("0x{:02X}", code),
    }
}

pub(crate) fn create_module(
    lua: &mlua::Lua,
    snapshot: &Rc<std::cell::Cell<super::host::Snapshot>>,
) -> mlua::Result<mlua::Table> {
    let t = lua.create_table()?;

    // 1. button_mask
    let mask_tbl = lua.create_table()?;
    mask_tbl.set("square", 0x8000u32)?;
    mask_tbl.set("x", 0x4000u32)?;
    mask_tbl.set("circle", 0x2000u32)?;
    mask_tbl.set("triangle", 0x1000u32)?;
    mask_tbl.set("r1", 0x0800u32)?;
    mask_tbl.set("l1", 0x0400u32)?;
    mask_tbl.set("r2", 0x0200u32)?;
    mask_tbl.set("l2", 0x0100u32)?;
    mask_tbl.set("dpad_left", 0x0080u32)?;
    mask_tbl.set("dpad_down", 0x0040u32)?;
    mask_tbl.set("dpad_right", 0x0020u32)?;
    mask_tbl.set("dpad_up", 0x0010u32)?;
    mask_tbl.set("start", 0x0008u32)?;
    mask_tbl.set("r3", 0x0004u32)?;
    mask_tbl.set("l3", 0x0002u32)?;
    mask_tbl.set("select", 0x0001u32)?;
    t.set("button_mask", mask_tbl)?;

    // 2. Key constants
    let key_tbl = lua.create_table()?;
    key_tbl.set("Enter", 0x0Du32)?;
    key_tbl.set("Backspace", 0x08u32)?;
    key_tbl.set("Tab", 0x09u32)?;
    key_tbl.set("Escape", 0x1Bu32)?;
    key_tbl.set("Space", 0x20u32)?;
    t.set("Key", key_tbl)?;

    // 3. GetKeyName(key_char)
    t.set(
        "GetKeyName",
        lua.create_function(|_, key_char: u32| {
            let code = (key_char & 0xFF) as u8;
            Ok(get_key_name(code))
        })?,
    )?;

    // 4. IsButtonPressed(button_mask)
    let snap_btn = Rc::clone(snapshot);
    t.set(
        "IsButtonPressed",
        lua.create_function(move |_, button_mask: u32| {
            let snap = snap_btn.get();
            let pressed = (snap.input.button_bitfield as u32 & button_mask) == 0;
            Ok(pressed)
        })?,
    )?;

    // 5. GetRawAnalogStickState()
    let snap_raw = Rc::clone(snapshot);
    t.set(
        "GetRawAnalogStickState",
        lua.create_function(move |lua, ()| {
            let snap = snap_raw.get();
            let tbl = lua.create_table()?;
            tbl.set("right_x", snap.input.right_analog.0)?;
            tbl.set("right_y", snap.input.right_analog.1)?;
            tbl.set("left_x", snap.input.left_analog.0)?;
            tbl.set("left_y", snap.input.left_analog.1)?;
            Ok(tbl)
        })?,
    )?;

    // 6. GetNormalizedAnalogStickState()
    let snap_norm = Rc::clone(snapshot);
    t.set(
        "GetNormalizedAnalogStickState",
        lua.create_function(move |lua, ()| {
            let snap = snap_norm.get();
            let normalize = |v: u8| -> f32 {
                let val = v as f32;
                let sub = if v <= 0x7F { 0x7F as f32 } else { 0x80 as f32 };
                (val - sub) / 127.0
            };
            let tbl = lua.create_table()?;
            tbl.set("right_x", normalize(snap.input.right_analog.0))?;
            tbl.set("right_y", normalize(snap.input.right_analog.1))?;
            tbl.set("left_x", normalize(snap.input.left_analog.0))?;
            tbl.set("left_y", normalize(snap.input.left_analog.1))?;
            Ok(tbl)
        })?,
    )?;

    // 7. IsKeyHookInstalled()
    t.set(
        "IsKeyHookInstalled",
        lua.create_function(|_, ()| Ok(INPUT_STATE.with(|s| s.borrow().key_hook_installed)))?,
    )?;

    // 8. InstallKeyHook()
    t.set(
        "InstallKeyHook",
        lua.create_function(|_, ()| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                state.key_hook_installed = true;
                state.key_hook_desired = true;
            });
            Ok((true, mlua::Value::Nil))
        })?,
    )?;

    // 9. UninstallKeyHook()
    t.set(
        "UninstallKeyHook",
        lua.create_function(|_, ()| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                state.key_hook_installed = false;
                state.key_hook_desired = false;
                state.hook_claims.clear();
                state.key_claims.clear();
                state.suppress_shadow.clear();
                state.text_capture_owners.clear();
                state.text_capture = false;
                state.key_cursors.clear();
                state.unowned_cursor = state.event_seq;
            });
            Ok(true)
        })?,
    )?;

    // 10. IsKeyHookSuspended()
    let snap_susp = Rc::clone(snapshot);
    t.set(
        "IsKeyHookSuspended",
        lua.create_function(move |_, ()| {
            let in_game = snap_susp.get().in_game;
            let suspended = INPUT_STATE.with(|s| {
                let state = s.borrow();
                state.key_hook_desired && (!state.key_hook_installed || !in_game)
            });
            Ok(suspended)
        })?,
    )?;

    // 11. AcquireKeyHook(owner)
    t.set(
        "AcquireKeyHook",
        lua.create_function(|_, owner: String| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                state.key_hook_installed = true;
                state.key_hook_desired = true;
                if !state.hook_claims.contains(&owner) {
                    state.hook_claims.insert(owner.clone());
                    let seq = state.event_seq;
                    state.key_cursors.insert(owner, seq);
                }
            });
            Ok((true, mlua::Value::Nil))
        })?,
    )?;

    // 12. ReleaseKeyHook(owner)
    t.set(
        "ReleaseKeyHook",
        lua.create_function(|_, owner: String| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                if !state.hook_claims.remove(&owner) {
                    return;
                }
                // Drop all key claims owned by this owner
                let keys_to_remove: Vec<u8> = state
                    .key_claims
                    .iter()
                    .filter(|(_, o)| **o == owner)
                    .map(|(k, _)| *k)
                    .collect();
                for k in keys_to_remove {
                    state.key_claims.remove(&k);
                    state.suppress_shadow.remove(&k);
                }
                state.text_capture_owners.remove(&owner);
                state.key_cursors.remove(&owner);

                // Update text capture
                let now = Instant::now();
                state
                    .text_capture_owners
                    .retain(|_, t| now.duration_since(*t).as_secs_f32() <= TEXT_CAPTURE_LAPSE_SECS);
                state.text_capture = !state.text_capture_owners.is_empty();

                if state.hook_claims.is_empty() {
                    state.key_hook_installed = false;
                    state.key_hook_desired = false;
                }
            });
            Ok(())
        })?,
    )?;

    // 13. AcquireKey(owner, key_char)
    t.set(
        "AcquireKey",
        lua.create_function(|_, (owner, key_char): (String, u32)| {
            let code = (key_char & 0xFF) as u8;
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                if let Some(curr) = state.key_claims.get(&code) {
                    if *curr == owner {
                        return Ok((true, None, None));
                    } else {
                        let err_msg =
                            format!("the {} key is owned by {}", get_key_name(code), curr);
                        return Ok((false, Some(err_msg), Some(curr.clone())));
                    }
                }

                // Acquire hook claim if not present
                state.key_hook_installed = true;
                state.key_hook_desired = true;
                if !state.hook_claims.contains(&owner) {
                    state.hook_claims.insert(owner.clone());
                    let seq = state.event_seq;
                    state.key_cursors.insert(owner.clone(), seq);
                }
                state.key_claims.insert(code, owner);
                Ok((true, None, None))
            })
        })?,
    )?;

    // 14. ReleaseKey(owner, key_char)
    t.set(
        "ReleaseKey",
        lua.create_function(|_, (owner, key_char): (String, u32)| {
            let code = (key_char & 0xFF) as u8;
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                if state
                    .key_claims
                    .get(&code)
                    .map(|o| o == &owner)
                    .unwrap_or(false)
                {
                    state.key_claims.remove(&code);
                    state.suppress_shadow.remove(&code);
                }
            });
            Ok(())
        })?,
    )?;

    // 15. GetKeyOwner(key_char)
    t.set(
        "GetKeyOwner",
        lua.create_function(|_, key_char: u32| {
            let code = (key_char & 0xFF) as u8;
            let owner = INPUT_STATE.with(|s| s.borrow().key_claims.get(&code).cloned());
            Ok(owner)
        })?,
    )?;

    // 16. GetKeyOwners()
    t.set(
        "GetKeyOwners",
        lua.create_function(|lua, ()| {
            let tbl = lua.create_table()?;
            INPUT_STATE.with(|s| {
                let state = s.borrow();
                for (k, v) in &state.key_claims {
                    tbl.set(*k, v.as_str())?;
                }
                Ok(tbl)
            })
        })?,
    )?;

    // 17. SetTextCaptureActive(owner, active)
    t.set(
        "SetTextCaptureActive",
        lua.create_function(|_, (owner, active): (String, bool)| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                if !state.hook_claims.contains(&owner) {
                    return Ok(false);
                }
                if active {
                    state.text_capture_owners.insert(owner, Instant::now());
                } else {
                    state.text_capture_owners.remove(&owner);
                }
                let now = Instant::now();
                state
                    .text_capture_owners
                    .retain(|_, t| now.duration_since(*t).as_secs_f32() <= TEXT_CAPTURE_LAPSE_SECS);
                state.text_capture = !state.text_capture_owners.is_empty();
                Ok(true)
            })
        })?,
    )?;

    // 18. IsTextCaptureActive()
    t.set(
        "IsTextCaptureActive",
        lua.create_function(|_, ()| Ok(INPUT_STATE.with(|s| s.borrow().text_capture)))?,
    )?;

    // 19. SetKeySuppressed(owner, key_char, suppressed) OR (key_char, suppressed)
    t.set(
        "SetKeySuppressed",
        lua.create_function(|lua, args: mlua::MultiValue| {
            let slice: Vec<mlua::Value> = args.into_iter().collect();
            let (owner, key_char, suppressed): (Option<String>, u32, bool) = match slice.len() {
                2 => {
                    let kc: u32 = lua.unpack(slice[0].clone())?;
                    let sup: bool = lua.unpack(slice[1].clone())?;
                    (None, kc, sup)
                }
                3 => {
                    let o: Option<String> = match &slice[0] {
                        mlua::Value::String(s) => Some(s.to_str()?.to_string()),
                        _ => None,
                    };
                    let kc: u32 = lua.unpack(slice[1].clone())?;
                    let sup: bool = lua.unpack(slice[2].clone())?;
                    (o, kc, sup)
                }
                _ => return Ok(false),
            };

            let code = (key_char & 0xFF) as u8;
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                if !state.key_hook_installed && !state.key_hook_desired {
                    return Ok(false);
                }
                let claim = state.key_claims.get(&code).cloned();
                if claim != owner {
                    return Ok(false);
                }
                if suppressed {
                    state.suppress_shadow.insert(code, true);
                } else {
                    state.suppress_shadow.remove(&code);
                }
                Ok(true)
            })
        })?,
    )?;

    // 20. IsKeySuppressed(key_char)
    t.set(
        "IsKeySuppressed",
        lua.create_function(|_, key_char: u32| {
            let code = (key_char & 0xFF) as u8;
            let sup = INPUT_STATE.with(|s| {
                let state = s.borrow();
                if state.text_capture && code < 0x80 {
                    return true;
                }
                state.suppress_shadow.get(&code).copied().unwrap_or(false)
            });
            Ok(sup)
        })?,
    )?;

    // 21. ClearAllSuppressedKeys(owner)
    t.set(
        "ClearAllSuppressedKeys",
        lua.create_function(|_, owner: Option<String>| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                let keys_to_clear: Vec<u8> = state
                    .suppress_shadow
                    .keys()
                    .copied()
                    .filter(|k| state.key_claims.get(k) == owner.as_ref())
                    .collect();
                for k in keys_to_clear {
                    state.suppress_shadow.remove(&k);
                }
            });
            Ok(())
        })?,
    )?;

    // 22. PollKeyEvents(owner)
    t.set(
        "PollKeyEvents",
        lua.create_function(|lua, owner: Option<String>| {
            // First run Update()
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                let now = Instant::now();
                state
                    .text_capture_owners
                    .retain(|_, t| now.duration_since(*t).as_secs_f32() <= TEXT_CAPTURE_LAPSE_SECS);
                state.text_capture = !state.text_capture_owners.is_empty();
            });

            let events_tbl = lua.create_table()?;
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                if !state.key_hook_installed {
                    return Ok(events_tbl);
                }

                let count = state.event_seq;
                let first = if let Some(ref o) = owner {
                    state.key_cursors.get(o).copied().unwrap_or(count)
                } else {
                    state.unowned_cursor
                };

                if count != first {
                    let window_start = count.saturating_sub(KEY_RING_SIZE as u32);
                    let actual_first = if first < window_start {
                        state.dropped_keys += window_start - first;
                        window_start
                    } else {
                        first
                    };

                    let mut idx = 1;
                    for ev in &state.key_events {
                        if ev.seq >= actual_first && ev.seq < count {
                            let claim = state.key_claims.get(&ev.key);
                            if claim.is_none() || claim == owner.as_ref() {
                                let ev_tbl = lua.create_table()?;
                                ev_tbl.set("key", ev.key)?;
                                ev_tbl.set("is_down", ev.is_down)?;
                                ev_tbl.set("seq", ev.seq)?;
                                events_tbl.set(idx, ev_tbl)?;
                                idx += 1;
                            }
                        }
                    }
                }

                if let Some(o) = owner {
                    state.key_cursors.insert(o, count);
                } else {
                    state.unowned_cursor = count;
                }

                Ok(events_tbl)
            })
        })?,
    )?;

    // 23. GetDroppedKeyCount()
    t.set(
        "GetDroppedKeyCount",
        lua.create_function(|_, ()| Ok(INPUT_STATE.with(|s| s.borrow().dropped_keys)))?,
    )?;

    // 24. IsDpadHookInstalled()
    t.set(
        "IsDpadHookInstalled",
        lua.create_function(|_, ()| Ok(INPUT_STATE.with(|s| s.borrow().dpad_installed)))?,
    )?;

    // 25. InstallDpadHook()
    t.set(
        "InstallDpadHook",
        lua.create_function(|_, ()| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                state.dpad_installed = true;
                state.dpad_desired = true;
                state.dpad_error = None;
            });
            Ok((true, mlua::Value::Nil))
        })?,
    )?;

    // 26. UninstallDpadHook()
    t.set(
        "UninstallDpadHook",
        lua.create_function(|_, ()| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                state.dpad_installed = false;
                state.dpad_desired = false;
                state.dpad_rotated = false;
                state.dpad_claims.clear();
                state.dpad_requests.clear();
            });
            Ok(true)
        })?,
    )?;

    // 27. IsDpadHookSuspended()
    let snap_dpad_susp = Rc::clone(snapshot);
    t.set(
        "IsDpadHookSuspended",
        lua.create_function(move |_, ()| {
            let in_game = snap_dpad_susp.get().in_game;
            let suspended = INPUT_STATE.with(|s| {
                let state = s.borrow();
                state.dpad_desired && (!state.dpad_installed || !in_game)
            });
            Ok(suspended)
        })?,
    )?;

    // 28. GetDpadHookError()
    t.set(
        "GetDpadHookError",
        lua.create_function(|_, ()| Ok(INPUT_STATE.with(|s| s.borrow().dpad_error.clone())))?,
    )?;

    // 29. AcquireDpadRotation(owner)
    t.set(
        "AcquireDpadRotation",
        lua.create_function(|_, owner: String| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                state.dpad_installed = true;
                state.dpad_desired = true;
                state.dpad_claims.insert(owner);
            });
            Ok((true, mlua::Value::Nil))
        })?,
    )?;

    // 30. ReleaseDpadRotation(owner)
    t.set(
        "ReleaseDpadRotation",
        lua.create_function(|_, owner: String| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                if !state.dpad_claims.remove(&owner) {
                    return;
                }
                state.dpad_requests.remove(&owner);
                if state.dpad_claims.is_empty() {
                    state.dpad_installed = false;
                    state.dpad_desired = false;
                    state.dpad_rotated = false;
                } else {
                    state.dpad_rotated = state.dpad_requests.values().any(|&v| v);
                }
            });
            Ok(())
        })?,
    )?;

    // 31. SetDpadRotated(owner, rotated)
    t.set(
        "SetDpadRotated",
        lua.create_function(|_, (owner, rotated): (String, bool)| {
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                if !state.dpad_claims.contains(&owner) {
                    return;
                }
                state.dpad_requests.insert(owner, rotated);
                state.dpad_rotated = state.dpad_requests.values().any(|&v| v);
            });
            Ok(())
        })?,
    )?;

    // 32. IsDpadRotated()
    t.set(
        "IsDpadRotated",
        lua.create_function(|_, ()| Ok(INPUT_STATE.with(|s| s.borrow().dpad_rotated)))?,
    )?;

    // 33. Update()
    let snap_upd = Rc::clone(snapshot);
    t.set(
        "Update",
        lua.create_function(move |_, ()| {
            let in_game = snap_upd.get().in_game;
            INPUT_STATE.with(|s| {
                let mut state = s.borrow_mut();
                // Check text capture timeout
                let now = Instant::now();
                state
                    .text_capture_owners
                    .retain(|_, t| now.duration_since(*t).as_secs_f32() <= TEXT_CAPTURE_LAPSE_SECS);
                state.text_capture = !state.text_capture_owners.is_empty();

                // Check suspension / re-installation
                if !in_game {
                    // Out of game suspends installed hooks
                    if state.key_hook_installed {
                        state.key_hook_installed = false;
                    }
                    if state.dpad_installed {
                        state.dpad_installed = false;
                    }
                } else {
                    // In-game restores desired hooks
                    if state.key_hook_desired {
                        state.key_hook_installed = true;
                    }
                    if state.dpad_desired {
                        state.dpad_installed = true;
                    }
                }

                state.dpad_rotated = state.dpad_requests.values().any(|&v| v);
            });
            Ok(())
        })?,
    )?;

    Ok(t)
}

pub fn install_input_adapter(host: &super::host::Host) -> mlua::Result<()> {
    let input_mod = create_module(&host.lua, &host.snapshot)?;
    host.register_host_module("frontiers_forge.input", mlua::Value::Table(input_mod))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::discovery::DiscoveredAddon;
    use crate::fork_ext::addons::host::Host;
    use mlua::ObjectLike;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn temp_test_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("eqoa_input_test_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_addon(root: &Path) -> DiscoveredAddon {
        let entry = root.join("main.lua");
        fs::write(&entry, "-- test input addon").unwrap();
        DiscoveredAddon {
            id: "test_input".to_string(),
            name: "Test Input".to_string(),
            entry,
            script_root: root.to_path_buf(),
            package_root: root.to_path_buf(),
            module_dirs: vec![root.to_path_buf()],
            resource_dirs: vec![],
            api: Some("eqoa/1".to_string()),
            requested_capabilities: vec![],
            initially_enabled: true,
        }
    }

    fn test_host(name: &str) -> Host {
        let dir = temp_test_dir(name);
        let addon = test_addon(&dir);
        let host = Host::for_addon(&addon).unwrap();
        host.snapshot.set(crate::fork_ext::addons::host::Snapshot {
            in_game: true,
            ..Default::default()
        });
        host
    }

    #[test]
    fn forge_input_constants_and_types() {
        let host = test_host("constants");
        let lua = &host.lua;
        let input: mlua::Table = lua
            .load("return require('frontiers_forge.input')")
            .eval()
            .unwrap();

        // button_mask
        let mask: mlua::Table = input.get("button_mask").unwrap();
        assert_eq!(mask.get::<u32>("square").unwrap(), 0x8000);
        assert_eq!(mask.get::<u32>("x").unwrap(), 0x4000);
        assert_eq!(mask.get::<u32>("dpad_left").unwrap(), 0x0080);
        assert_eq!(mask.get::<u32>("dpad_up").unwrap(), 0x0010);

        // Key
        let key: mlua::Table = input.get("Key").unwrap();
        assert_eq!(key.get::<u32>("Enter").unwrap(), 0x0D);
        assert_eq!(key.get::<u32>("Backspace").unwrap(), 0x08);
        assert_eq!(key.get::<u32>("Tab").unwrap(), 0x09);
        assert_eq!(key.get::<u32>("Escape").unwrap(), 0x1B);
        assert_eq!(key.get::<u32>("Space").unwrap(), 0x20);

        // GetKeyName
        let name_enter: String = input.call_function("GetKeyName", 0x0Du32).unwrap();
        assert_eq!(name_enter, "Enter");
        let name_a: String = input.call_function("GetKeyName", b'A' as u32).unwrap();
        assert_eq!(name_a, "A");
        let name_hex: String = input.call_function("GetKeyName", 0x01u32).unwrap();
        assert_eq!(name_hex, "0x01");
    }

    #[test]
    fn forge_input_dpad_rotation_lifecycle() {
        reset_input_state();
        let host = test_host("dpad_lifecycle");
        let lua = &host.lua;

        let script = r#"
            local Input = require('frontiers_forge.input')
            assert(not Input.IsDpadRotated(), "initially not rotated")
            assert(not Input.IsDpadHookInstalled(), "initially not installed")

            -- Acquire claim
            local ok, err = Input.AcquireDpadRotation("test_mod")
            assert(ok == true and err == nil)
            assert(Input.IsDpadHookInstalled(), "hook installed after acquire")
            assert(not Input.IsDpadRotated(), "still not rotated without request")

            -- Turn rotation on
            Input.SetDpadRotated("test_mod", true)
            assert(Input.IsDpadRotated(), "rotated now")

            -- Multi-owner request
            Input.AcquireDpadRotation("second_mod")
            Input.SetDpadRotated("second_mod", false)
            assert(Input.IsDpadRotated(), "first mod still requesting, should remain rotated")

            -- Release first mod
            Input.ReleaseDpadRotation("test_mod")
            assert(not Input.IsDpadRotated(), "second mod requested false, should now be false")
            assert(Input.IsDpadHookInstalled(), "second mod still claims hook")

            -- Release second mod
            Input.ReleaseDpadRotation("second_mod")
            assert(not Input.IsDpadHookInstalled(), "uninstalled when all owners release")
        "#;
        lua.load(script).exec().unwrap();
    }

    #[test]
    fn forge_input_keyboard_hook_claims_and_suppression() {
        reset_input_state();
        let host = test_host("keyboard_claims");
        let lua = &host.lua;

        let script = r#"
            local Input = require('frontiers_forge.input')
            local enter = Input.Key.Enter

            -- Acquire key
            local ok, err, other = Input.AcquireKey("wereoxxs", enter)
            assert(ok == true and err == nil and other == nil)
            assert(Input.GetKeyOwner(enter) == "wereoxxs")

            -- Conflicting claim from another mod
            local ok2, err2, other2 = Input.AcquireKey("other_mod", enter)
            assert(ok2 == false, "conflicting claim must fail")
            assert(other2 == "wereoxxs", "must identify current owner")
            assert(string.find(err2, "owned by wereoxxs") ~= nil)

            -- Suppress key
            assert(not Input.IsKeySuppressed(enter))
            assert(Input.SetKeySuppressed("wereoxxs", enter, true) == true)
            assert(Input.IsKeySuppressed(enter) == true)

            -- Other mod cannot unsuppress
            assert(Input.SetKeySuppressed("other_mod", enter, false) == false)
            assert(Input.IsKeySuppressed(enter) == true)

            -- Text capture swallows all keys < 0x80
            assert(Input.SetTextCaptureActive("wereoxxs", true) == true)
            assert(Input.IsTextCaptureActive() == true)
            assert(Input.IsKeySuppressed(Input.Key.Space) == true)

            -- Release text capture and key
            Input.SetTextCaptureActive("wereoxxs", false)
            assert(Input.IsTextCaptureActive() == false)
            assert(Input.IsKeySuppressed(Input.Key.Space) == false)

            Input.ReleaseKey("wereoxxs", enter)
            assert(Input.GetKeyOwner(enter) == nil)
            assert(Input.IsKeySuppressed(enter) == false)
        "#;
        lua.load(script).exec().unwrap();
    }

    #[test]
    fn forge_input_poll_key_events_and_delivery() {
        reset_input_state();
        let host = test_host("poll_events");
        let lua = &host.lua;

        let setup_script = r#"
            local Input = require('frontiers_forge.input')
            Input.AcquireKey("chat_mod", Input.Key.Enter)
        "#;
        lua.load(setup_script).exec().unwrap();

        // Push some events from Rust
        record_key_event(0x0D, true); // Enter pressed
        record_key_event(0x0D, false); // Enter released
        record_key_event(0x20, true); // Space pressed (unclaimed)

        let poll_script = r#"
            local Input = require('frontiers_forge.input')
            local events = Input.PollKeyEvents("chat_mod")
            assert(#events == 3, "should receive 2 claimed Enter events + 1 unclaimed Space event")
            assert(events[1].key == Input.Key.Enter and events[1].is_down == true)
            assert(events[2].key == Input.Key.Enter and events[2].is_down == false)
            assert(events[3].key == Input.Key.Space and events[3].is_down == true)

            -- Polling again should yield empty table since cursor advanced
            local events2 = Input.PollKeyEvents("chat_mod")
            assert(#events2 == 0, "no new events since last poll")
        "#;
        lua.load(poll_script).exec().unwrap();
    }

    #[test]
    fn wereoxxs_ui_input_lifecycle_integration() {
        reset_input_state();
        let host = test_host("wereoxxs_ui_integration");
        let lua = &host.lua;

        let script = r#"
            local Input = require('frontiers_forge.input')
            local HOOK_OWNER = "wereoxxs_ui"

            -- 1. PumpEnterKey flow
            local ok, err = Input.AcquireKey(HOOK_OWNER, Input.Key.Enter)
            assert(ok == true and err == nil)
            assert(Input.SetKeySuppressed(HOOK_OWNER, Input.Key.Enter, true) == true)
            assert(Input.SetTextCaptureActive(HOOK_OWNER, true) == true)
            assert(Input.IsTextCaptureActive() == true)

            -- Check queries used in Chat settings UI
            assert(Input.GetKeyOwner(Input.Key.Enter) == HOOK_OWNER)
            assert(Input.IsKeyHookInstalled() == true)
            assert(Input.IsKeyHookSuspended() == false)
            assert(Input.GetDroppedKeyCount() == 0)

            -- Polling
            local evs = Input.PollKeyEvents(HOOK_OWNER)
            assert(type(evs) == "table")

            -- Turn text capture off
            assert(Input.SetTextCaptureActive(HOOK_OWNER, false) == true)
            assert(Input.IsTextCaptureActive() == false)

            -- Release enter
            Input.ReleaseKey(HOOK_OWNER, Input.Key.Enter)
            assert(Input.GetKeyOwner(Input.Key.Enter) == nil)

            -- 2. D-pad rotation flow (for horizontal ability bars)
            local dpad_ok, dpad_err = Input.AcquireDpadRotation(HOOK_OWNER)
            assert(dpad_ok == true and dpad_err == nil)
            Input.Update()
            Input.SetDpadRotated(HOOK_OWNER, true)
            assert(Input.IsDpadRotated() == true)
            assert(Input.IsDpadHookSuspended() == false)
            assert(Input.GetDpadHookError() == nil)

            -- Ability bar disabled: rotate turned off
            Input.SetDpadRotated(HOOK_OWNER, false)
            assert(Input.IsDpadRotated() == false)

            -- Cleanup
            Input.ReleaseDpadRotation(HOOK_OWNER)
            Input.ReleaseKeyHook(HOOK_OWNER)
            assert(Input.IsDpadHookInstalled() == false)
            assert(Input.IsKeyHookInstalled() == false)
        "#;
        lua.load(script).exec().unwrap();
    }

    #[test]
    fn forge_input_suspension_lifecycle_and_restoration() {
        reset_input_state();
        let host = test_host("suspension_lifecycle");
        let lua = &host.lua;

        let setup = r#"
            local Input = require('frontiers_forge.input')
            local ok, err = Input.AcquireKey("test_mod", Input.Key.Enter)
            assert(ok == true and err == nil)
            local dok, derr = Input.AcquireDpadRotation("test_mod")
            assert(dok == true and derr == nil)
            Input.SetDpadRotated("test_mod", true)

            assert(Input.IsKeyHookInstalled() == true)
            assert(Input.IsKeyHookSuspended() == false)
            assert(Input.IsDpadHookInstalled() == true)
            assert(Input.IsDpadHookSuspended() == false)
            assert(Input.IsDpadRotated() == true)
        "#;
        lua.load(setup).exec().unwrap();

        // Transition out of game
        host.snapshot.set(crate::fork_ext::addons::host::Snapshot {
            in_game: false,
            ..Default::default()
        });

        let out_of_game = r#"
            local Input = require('frontiers_forge.input')
            Input.Update()
            assert(Input.IsKeyHookInstalled() == false, "hook uninstalled while out of game")
            assert(Input.IsKeyHookSuspended() == true, "hook suspended while out of game")
            assert(Input.IsDpadHookInstalled() == false, "dpad hook uninstalled while out of game")
            assert(Input.IsDpadHookSuspended() == true, "dpad hook suspended while out of game")
            assert(Input.IsDpadRotated() == true, "dpad desired rotation survives suspension")
        "#;
        lua.load(out_of_game).exec().unwrap();

        // Transition back in game
        host.snapshot.set(crate::fork_ext::addons::host::Snapshot {
            in_game: true,
            ..Default::default()
        });

        let back_in_game = r#"
            local Input = require('frontiers_forge.input')
            Input.Update()
            assert(Input.IsKeyHookInstalled() == true, "hook restored on entering game")
            assert(Input.IsKeyHookSuspended() == false, "hook no longer suspended")
            assert(Input.IsDpadHookInstalled() == true, "dpad hook restored on entering game")
            assert(Input.IsDpadHookSuspended() == false, "dpad hook no longer suspended")
            assert(Input.IsDpadRotated() == true, "dpad remains rotated")

            Input.ReleaseKeyHook("test_mod")
            Input.ReleaseDpadRotation("test_mod")
        "#;
        lua.load(back_in_game).exec().unwrap();
    }
}
