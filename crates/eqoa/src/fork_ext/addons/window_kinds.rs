// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The game's window kinds: the `Window.kind` names `eqoa.windows.get()` reports (spec §7, "Camera and
//! windows"). This is the one list that `eqoa.windows.takeover`, `mod.json`'s `replaces` and the Mods &
//! Addons disclosure lines check names against; the window capture names its windows from it too.

/// Native mode ID, its `eqoa` mode name and the title of the window the game shows in that mode
/// (none: the mode shows no window).
const MODES: &[(i32, &str, Option<&str>)] = &[
    (0, "normal", None),
    (1, "loot", Some("Loot")),
    (2, "input", Some("Input")),
    (4, "gm_goto", Some("Go To")),
    (5, "spellbook", Some("Abilities")),
    (6, "recipe_book", Some("Recipe Book")),
    (7, "response_dialog", Some("Dialog")),
    (9, "text_dialog", Some("Dialog")),
    (10, "merchant", Some("Merchant")),
    (11, "bank", Some("Bank")),
    (12, "trainer", Some("Character Summary")),
    (13, "start_menu", Some("Start Menu")),
    (14, "confirm", Some("Confirm")),
    (15, "trade", Some("Trade")),
    (16, "trade_ask", Some("Trade")),
    (17, "buddy_request", Some("Buddy Request")),
    (18, "buddy_list", Some("Buddy List")),
    (19, "blacksmith", Some("Blacksmith")),
    (20, "guild_invite", Some("Guild Invite")),
    (21, "guild", Some("Guild")),
    (22, "quest_log", Some("Quest Journal")),
    (23, "mail_compose", Some("Mail")),
    (24, "mail_inbox", Some("Mail")),
    (26, "ignore_list", Some("Ignore List")),
    (27, "gm_buff_pc", Some("Buff PC")),
    (28, "mail_tell", Some("Mail")),
    (29, "mail_reply", Some("Mail")),
    (30, "quick_chat", Some("Quick Chat")),
    (31, "support_request", Some("Support")),
    (32, "support_inbox", Some("Support")),
    (33, "chat_history", Some("Chat History")),
    (34, "options", Some("Options")),
    (35, "who", Some("Who")),
    (36, "fishing", Some("Fishing")),
    (37, "experiment", Some("Experiment")),
    (38, "manual_camera", None),
    (39, "auction_search", Some("Auction Search")),
    (40, "mastery", Some("Class Mastery")),
    (41, "factions", Some("Factions")),
    (42, "group_search", Some("Group Search")),
    (43, "duel_invite", Some("Duel")),
    (44, "duel", Some("Duel")),
    (60, "combat_record", Some("Combat Record")),
    (61, "mercenary_items", Some("Mercenary Items")),
    (62, "mercenary_spells", Some("Mercenary Spells")),
    (63, "dye_picker", Some("Dye Picker")),
];

/// Windows the game shows beside or over a mode (kind, title).
pub(crate) const INVENTORY: (&str, &str) = ("inventory", "Inventory");
pub(crate) const MAP: (&str, &str) = ("map", "Map");
pub(crate) const SEARCH: (&str, &str) = ("search", "Player Search");
pub(crate) const TYPING: (&str, &str) = ("typing", "Chat");
pub(crate) const RAID_READY: (&str, &str) = ("raid_ready", "Ready Check");
pub(crate) const RAID_ROLL: (&str, &str) = ("raid_roll", "Loot Roll");
pub(crate) const FRIEND_REQUEST: (&str, &str) = ("friend_request", "Friend Request");
const OVERLAYS: [(&str, &str); 7] = [INVENTORY, MAP, SEARCH, TYPING, RAID_READY, RAID_ROLL, FRIEND_REQUEST];

/// The `eqoa` mode name of a native mode ID; `"other"` for an ID no window owns.
pub(crate) fn mode_name(mode: i32) -> &'static str {
    MODES
        .iter()
        .find(|(id, _, _)| *id == mode)
        .map_or("other", |(_, name, _)| name)
}

/// The title of the window the game shows in native mode `mode`.
pub(crate) fn mode_title(mode: i32) -> Option<&'static str> {
    MODES
        .iter()
        .find(|(id, _, _)| *id == mode)
        .and_then(|(_, _, title)| *title)
}

/// The title of the game window `kind` names; none when `kind` is no game window.
pub(crate) fn title(kind: &str) -> Option<&'static str> {
    MODES
        .iter()
        .find(|(_, name, _)| *name == kind)
        .and_then(|(_, _, title)| *title)
        .or_else(|| OVERLAYS.iter().find(|(name, _)| *name == kind).map(|(_, t)| *t))
}

/// `kind` names a window the game shows.
pub(crate) fn is_window(kind: &str) -> bool {
    title(kind).is_some()
}

/// Local views in the window replacement design have no helper session to claim.
pub(super) fn is_view(kind: &str) -> bool {
    matches!(
        kind,
        "inventory" | "spellbook" | "quest_log" | "trainer" | "mastery"
            | "recipe_book" | "experiment" | "start_menu" | "map"
    )
}

/// Server-pushed raid strips have no helper (`own`) session; a mod replaces them with scope `"all"`.
pub(super) fn is_strip(kind: &str) -> bool {
    matches!(kind, "raid_ready" | "raid_roll")
}

/// Server-pushed social prompts (friend request, guild invitation) require scope `"all"`.
pub(super) fn is_request(kind: &str) -> bool {
    matches!(kind, "friend_request" | "guild_invite")
}

#[cfg(test)]
mod tests {
    #[test]
    fn round7c_window_kinds_name_only_real_windows() {
        for kind in ["loot", "merchant", "bank", "inventory", "spellbook", "quest_log", "map"] {
            assert!(super::is_window(kind), "{kind}");
        }
        for kind in ["normal", "manual_camera", "other", "hud.target", "journal", "Loot", ""] {
            assert!(!super::is_window(kind), "{kind}");
        }
        assert_eq!(super::mode_name(7), "response_dialog");
        assert_eq!(super::mode_name(3), "other");
        assert_eq!(super::title("spellbook"), Some("Abilities"));
    }
}
