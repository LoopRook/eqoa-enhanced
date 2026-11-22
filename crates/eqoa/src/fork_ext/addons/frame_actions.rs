// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A3: the small actions a unit-frame mod needs (spec section 8): invite the target to a group, send the
//! pet in or call it off, an emote, a jump, and taking a removable buff off. Each is one typed action through the
//! broker (`ActionKind::Frame`), judged by its own capability (`eqoa.actions.<name>`), and runs through the game's
//! own function for it at the next game step (`a3_capture::dispatch`).
//!
//! What a result means: `completed` says the client sent the request; the game gives these no reply the client can
//! tie to them (the other player's answer to an invitation, the server's resent buff list, the walk's take-off), so
//! `detail.confirmed` is false and the mod watches the matching event (`group.changed`, `buffs.changed`,
//! `Player.velocity`).
use super::{
    action_broker::{ActionKind, enqueue_lua, enqueue_lua_with},
    host::Host,
    native::bind,
    native_a3,
};
use mlua::{Lua, Result, Value};

/// The four emotes the game's start menu offers, in its order (its entries 0x0601 to 0x0604: Wave, Bow, Point, Cheer).
pub(crate) const EMOTES: [&str; 4] = ["wave", "bow", "point", "cheer"];

/// What the broker carries for the actions of round A3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FrameAction {
    GroupInvite,
    PetAttack,
    PetBackOff,
    Emote { index: usize },
    Jump,
    /// a line of the buff window, named by its whose, spell id and icon (what the server's removal names)
    RemoveEffect { whose: u8, id: u32, icon: u32, name: String },
}

impl FrameAction {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::GroupInvite => "group_invite",
            Self::PetAttack => "pet_attack",
            Self::PetBackOff => "pet_back_off",
            Self::Emote { .. } => "emote",
            Self::Jump => "jump",
            Self::RemoveEffect { .. } => "remove_effect",
        }
    }
}

/// The names of the new actions, as capabilities (`eqoa.actions.<name>`).
pub(crate) const NAMES: [&str; 6] = ["group_invite", "pet_attack", "pet_back_off", "emote", "jump", "remove_effect"];

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    for (name, action) in [
        ("group_invite", FrameAction::GroupInvite),
        ("pet_attack", FrameAction::PetAttack),
        ("pet_back_off", FrameAction::PetBackOff),
        ("jump", FrameAction::Jump),
    ] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, ()| {
                enqueue_lua(lua, &format!("eqoa.actions.{name}"), ActionKind::Frame(action.clone()))
            })?,
        )?;
    }
    bind(
        lua,
        "actions",
        "emote",
        lua.create_function(|lua, which: Value| {
            enqueue_lua_with(lua, "eqoa.actions.emote", || {
                let name = match &which {
                    Value::String(s) => s.to_str().map(|s| s.to_ascii_lowercase()).unwrap_or_default(),
                    _ => return Err(("invalid_argument", format!("emote must be one of: {}", EMOTES.join(", ")))),
                };
                match EMOTES.iter().position(|e| *e == name) {
                    Some(index) => Ok(ActionKind::Frame(FrameAction::Emote { index })),
                    None => Err(("invalid_argument", format!("emote must be one of: {}", EMOTES.join(", ")))),
                }
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "remove_effect",
        lua.create_function(|lua, buff: Value| {
            enqueue_lua_with(lua, "eqoa.actions.remove_effect", || {
                let (_, line) = native_a3::resolve_buff(&buff)?;
                if !line.removable {
                    return Err(("not_ready", "the server does not let this effect be taken off".into()));
                }
                Ok(ActionKind::Frame(FrameAction::RemoveEffect {
                    whose: line.whose,
                    id: line.id,
                    icon: line.icon,
                    name: line.name,
                }))
            })
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a3_the_label_names_each_new_action_the_mod_declares_and_none_it_does_not() {
        let label = super::super::runtime::autonomous_action_label(&[
            "eqoa.actions.group_invite".to_string(),
            "eqoa.actions.pet_attack".to_string(),
            "eqoa.actions.jump".to_string(),
        ]);
        assert_eq!(label.as_deref(), Some("Actions: can act on its own (group_invite, pet_attack, jump)"));
        // control: a mod that declares only reading shows no such line
        assert_eq!(super::super::runtime::autonomous_action_label(&[]), None);
        for name in super::NAMES {
            let one = super::super::runtime::autonomous_action_label(&[format!("eqoa.actions.{name}")]);
            assert_eq!(one, Some(format!("Actions: can act on its own ({name})")), "{name}");
        }
    }
}
