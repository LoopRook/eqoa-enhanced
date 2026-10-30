//! HUD-number adapter; stock HUD layout remains in the parent.
use super::{HudState, UiRaster};
use render::ui::{Anchor, Vertical};
use vi::ui::{Raster, Vui};

pub(crate) fn health_visible(s: &HudState) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        !s.forge_health_hidden
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = s;
        true
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn published<F: FnOnce(&crate::fork_ext::addons::visibility::PublishedVisibility) -> bool>(f: F) -> bool {
    f(&crate::fork_ext::addons::visibility::published_visibility())
}

#[cfg(target_arch = "wasm32")]
mod visibility_stub {
    pub struct PublishedVisibility {
        pub compass: bool,
        pub group_compass_marks: bool,
        pub target: bool,
        pub group_panel: bool,
        pub pet: bool,
        pub power_bar: bool,
        pub exp_bar: bool,
        pub chat: bool,
        pub ability_bar: bool,
        pub active_effects: bool,
    }
}

#[cfg(target_arch = "wasm32")]
fn published<F: FnOnce(&visibility_stub::PublishedVisibility) -> bool>(_: F) -> bool {
    true
}

pub(crate) fn compass_visible(_s: &HudState) -> bool {
    published(|v| !v.compass)
}

pub(crate) fn group_compass_marks_visible(_s: &HudState) -> bool {
    published(|v| !v.group_compass_marks)
}

pub(crate) fn target_visible(_s: &HudState) -> bool {
    published(|v| !v.target)
}

pub(crate) fn group_visible(_s: &HudState) -> bool {
    published(|v| !v.group_panel)
}

pub(crate) fn pet_visible(_s: &HudState) -> bool {
    published(|v| !v.pet)
}

pub(crate) fn power_bar_visible(_s: &HudState) -> bool {
    published(|v| !v.power_bar)
}

pub(crate) fn exp_bar_visible(_s: &HudState) -> bool {
    published(|v| !v.exp_bar)
}

pub(crate) fn chat_window_visible() -> bool {
    published(|v| !v.chat)
}

pub(crate) fn ability_bar_visible() -> bool {
    published(|v| !v.ability_bar)
}

pub(crate) fn active_effects_visible() -> bool {
    published(|v| !v.active_effects)
}

/// Draws the stock active-effects bar unless an addon has claimed it.
pub(crate) fn draw_active_effects(vui: &Vui, r: &mut dyn Raster, fx: &net::dump::Effects) {
    if active_effects_visible() {
        crate::influence::draw(vui, r, fx);
    }
}

/// Draws the stock chat window unless an addon has claimed it.
pub(crate) fn draw_chat_window(
    wnd: &mut crate::chat::WndChat,
    vui: &Vui,
    r: &mut dyn Raster,
    now: u32,
    dt: f32,
) {
    if chat_window_visible() {
        wnd.draw(vui, r, now, dt);
    }
}

/// Draws the HUD menu carousel, suppressing only the ability/hotbar menus
/// (indices 0, 1, 4 and 5) when an addon has claimed them.
pub(crate) fn draw_hud_menus(
    menus: &mut crate::hudmenu::HudMenus,
    vui: &Vui,
    r: &mut dyn Raster,
    now: i64,
    name: &dyn Fn(u32, i64) -> Vec<u16>,
) {
    if ability_bar_visible() || !menus.selected_is_ability_bar() {
        menus.draw(vui, r, now, name);
    }
}

#[derive(Debug)]
struct Readout {
    text: String,
    center: [i32; 2],
    scale: f32,
    color: [f32; 4],
    vertical: Vertical,
}
fn readouts(s: &HudState, need: u32, flags: [bool; 3]) -> Vec<Readout> {
    let [target_hp_percent, xp_details, hud_numbers] = flags;
    let mut out = Vec::with_capacity(4);
    if hud_numbers {
        if health_visible(s) {
            let pct = ((s.self_hp as f32 / 255.0) * 100.0).round() as i32;
            out.push(Readout {
                text: format!("{pct}%"),
                center: [99, 35],
                scale: 0.6,
                color: [1.0; 4],
                vertical: Vertical::Top,
            });
        }
        if s.max_power > 0 {
            out.push(Readout {
                text: format!("{}/{}", s.power, s.max_power),
                center: [99, 45],
                scale: 0.6,
                color: [0.4, 0.7, 1.0, 1.0],
                vertical: Vertical::Top,
            });
        }
    }
    if xp_details && need > 0 {
        let pct = (s.exp as f32 / need as f32 * 100.0).clamp(0.0, 100.0);
        out.push(Readout {
            text: format!("{}/{} ({:.1}%)", s.exp, need, pct),
            center: [99, 55],
            scale: 0.6,
            color: [1.0, 0.85, 0.35, 1.0],
            vertical: Vertical::Top,
        });
    }
    if target_hp_percent && s.target_name.is_some() {
        let pct = ((s.target_hp as f32 / 255.0) * 100.0).round() as i32;
        let color = if pct > 50 {
            [0.4, 1.0, 0.4, 1.0]
        } else if pct > 20 {
            [1.0, 0.9, 0.3, 1.0]
        } else {
            [1.0, 0.3, 0.3, 1.0]
        };
        out.push(Readout {
            text: format!("{pct}%"),
            center: [super::TARGET_AT[0] + 40, super::TARGET_AT[1] + 49],
            scale: 0.65,
            color,
            vertical: Vertical::Bottom,
        });
    }
    out
}

pub(super) fn draw(ui: &mut UiRaster<'_>, s: &HudState) {
    let Some(font) = ui.assets.font else { return };
    let c = crate::settings::client();
    let labels = readouts(
        s,
        super::exp_for_level(s.level + 1),
        [c.target_hp_percent, c.xp_details, c.hud_numbers],
    );
    let (anchor, vertical) = (ui.ui.anchor, ui.ui.vertical);
    ui.ui.anchor = Anchor::Left;
    for r in labels {
        ui.ui.vertical = r.vertical;
        let text = crate::ui::wide(&r.text);
        let width = ui.ui.fonts[font].string_width(&text) as f32 * r.scale;
        let x = (r.center[0] as f32 - width / 2.0).round();
        let y = r.center[1] as f32;
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            ui.ui
                .text_scaled(font, x + dx, y + dy, r.scale, [0.0, 0.0, 0.0, 0.95], &text);
        }
        ui.ui.text_scaled(font, x, y, r.scale, r.color, &text);
    }
    (ui.ui.anchor, ui.ui.vertical) = (anchor, vertical);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork_ext::addons::health::InstanceToken;
    use crate::fork_ext::addons::visibility::HudClaimsRegistry;

    fn set_claims(elements: &[&str]) {
        let mut reg = HudClaimsRegistry::new();
        let token = InstanceToken::next().unwrap();
        for el in elements {
            reg.disable(el, token, "test");
        }
        crate::fork_ext::addons::visibility::publish_claims(&reg);
    }

    fn clear_claims() {
        crate::fork_ext::addons::visibility::publish_claims(&HudClaimsRegistry::new());
    }

    #[derive(Default)]
    struct BlitRecorder {
        blits: Vec<(u32, [i32; 2])>,
        centered: Vec<(String, [i32; 2])>,
    }

    impl Raster for BlitRecorder {
        fn blit(&mut self, id: u32, _flags: u32, at: [i32; 2], _a: [i32; 2], _b: [i32; 2], _mode: i32, _alpha: f32) {
            self.blits.push((id, at));
        }
        fn stretch_blit(&mut self, id: u32, _flags: u32, at: [i32; 2], _size: [i32; 2], _a: [i32; 2], _b: [i32; 2], _mode: i32, _alpha: f32) {
            self.blits.push((id, at));
        }
        fn fill_fan(&mut self, _color: [f32; 4], _corners: [[i32; 2]; 4]) {}
        fn blit_full(&mut self, id: u32, _flags: u32, at: [i32; 2], _mode: i32, _alpha: f32) {
            self.blits.push((id, at));
        }
        fn draw_string(&mut self, at: [i32; 2], _color: [f32; 4], s: &[u16]) {
            self.centered.push((String::from_utf16_lossy(s), at));
        }
        fn draw_centered_string(&mut self, at: [i32; 2], _color: [f32; 4], s: &[u16], _mode: u32) {
            self.centered.push((String::from_utf16_lossy(s), at));
        }
        fn string_width(&self, _s: &[u16]) -> i32 { 0 }
        fn point_size(&self) -> i32 { 12 }
        fn viewport(&self) -> [i32; 4] { [0, 0, 639, 447] }
        fn set_viewport(&mut self, _v: [i32; 4]) {}
    }

    fn test_vui() -> Vui {
        let atlas = (0..252)
            .map(|_| vi::ui::UiTexEntry {
                uv1: [16, 16],
                ..Default::default()
            })
            .collect();
        Vui::new(Default::default(), Default::default(), atlas)
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_visibility_queries_mirror_claims() {
        clear_claims();
        assert!(compass_visible(&HudState::default()));
        assert!(group_compass_marks_visible(&HudState::default()));
        assert!(target_visible(&HudState::default()));
        assert!(group_visible(&HudState::default()));
        assert!(pet_visible(&HudState::default()));
        assert!(power_bar_visible(&HudState::default()));
        assert!(exp_bar_visible(&HudState::default()));
        assert!(chat_window_visible());
        assert!(ability_bar_visible());
        assert!(active_effects_visible());

        set_claims(&[
            "compass_face",
            "compass_back",
            "group_compass_marks",
            "target_nameplate",
            "group_member_panel",
            "pet_panel",
            "power_bar",
            "main_exp_bar",
            "secondary_exp_bar",
            "chat_window",
            "hotbar_draw_compact",
            "hotbar_draw_expanded",
            "active_effects",
        ]);
        assert!(!compass_visible(&HudState::default()));
        assert!(!group_compass_marks_visible(&HudState::default()));
        assert!(!target_visible(&HudState::default()));
        assert!(!group_visible(&HudState::default()));
        assert!(!pet_visible(&HudState::default()));
        assert!(!power_bar_visible(&HudState::default()));
        assert!(!exp_bar_visible(&HudState::default()));
        assert!(!chat_window_visible());
        assert!(!ability_bar_visible());
        assert!(!active_effects_visible());

        clear_claims();
        assert!(compass_visible(&HudState::default()));
        assert!(chat_window_visible());
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_active_effects_wrapper_draws_only_while_unclaimed() {
        let vui = test_vui();
        let mut fx = net::dump::Effects::default();
        fx.count = 1;
        fx.icon[0] = 123;

        clear_claims();
        let mut r = BlitRecorder::default();
        draw_active_effects(&vui, &mut r, &fx);
        let shown = r.blits.len();
        assert!(shown > 0, "active effects bar draws when unclaimed");

        set_claims(&["active_effects"]);
        let mut r = BlitRecorder::default();
        draw_active_effects(&vui, &mut r, &fx);
        assert_eq!(r.blits.len(), 0, "active effects bar does not draw when claimed");

        clear_claims();
        let mut r = BlitRecorder::default();
        draw_active_effects(&vui, &mut r, &fx);
        assert_eq!(r.blits.len(), shown, "releasing the claim restores drawing");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_chat_window_wrapper_draws_only_while_unclaimed() {
        let vui = test_vui();
        let mut wnd = crate::chat::WndChat::new();
        wnd.mode = 1;
        wnd.text.add_text(&crate::ui::wide("hello"), [1.0; 4], 0, |_| 0);

        clear_claims();
        let mut r = BlitRecorder::default();
        draw_chat_window(&mut wnd, &vui, &mut r, 0, 0.0);
        let shown = r.blits.len() + r.centered.len();
        assert!(shown > 0, "chat window draws when unclaimed");

        set_claims(&["chat_window"]);
        let mut r = BlitRecorder::default();
        draw_chat_window(&mut wnd, &vui, &mut r, 0, 0.0);
        assert_eq!(r.blits.len() + r.centered.len(), 0, "chat window does not draw when claimed");

        clear_claims();
        let mut r = BlitRecorder::default();
        draw_chat_window(&mut wnd, &vui, &mut r, 0, 0.0);
        assert_eq!(r.blits.len() + r.centered.len(), shown, "releasing the claim restores drawing");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_hud_menus_wrapper_draws_only_ability_bars_while_unclaimed() {
        use crate::hudmenu::HudMenus;
        let vui = test_vui();
        let mut menus = HudMenus::default();
        menus.build(&Default::default(), &Default::default(), &Default::default(), Default::default());
        // Menu 0 is the first ability bar.
        menus.sel = 0;

        clear_claims();
        let mut r = BlitRecorder::default();
        draw_hud_menus(&mut menus, &vui, &mut r, 0, &|_, _| Vec::new());
        let shown = r.blits.len();
        assert!(shown > 0, "ability bar menu draws when unclaimed");

        set_claims(&["hotbar_draw_compact", "hotbar_draw_expanded"]);
        let mut r = BlitRecorder::default();
        draw_hud_menus(&mut menus, &vui, &mut r, 0, &|_, _| Vec::new());
        assert_eq!(r.blits.len(), 0, "ability bar menu does not draw when claimed");

        // Items menu (2) must still draw while ability bars are claimed.
        menus.sel = 2;
        let mut r = BlitRecorder::default();
        draw_hud_menus(&mut menus, &vui, &mut r, 0, &|_, _| Vec::new());
        assert!(r.blits.len() > 0, "items menu still draws while ability bars are claimed");

        clear_claims();
        menus.sel = 0;
        let mut r = BlitRecorder::default();
        draw_hud_menus(&mut menus, &vui, &mut r, 0, &|_, _| Vec::new());
        assert_eq!(r.blits.len(), shown, "releasing the claim restores drawing");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_health_visibility_suppresses_only_health_gauge_and_percent_label() {
        let mut state = HudState {
            self_hp: 128,
            power: 4,
            max_power: 10,
            exp: 20,
            target_name: Some("Target".into()),
            target_hp: 128,
            ..Default::default()
        };
        let shown = readouts(&state, 100, [true, true, true]);
        assert!(health_visible(&state));
        assert_eq!(shown.len(), 4);
        state.forge_health_hidden = true;
        assert!(
            !health_visible(&state),
            "native health gauge uses this same predicate"
        );
        let hidden = readouts(&state, 100, [true, true, true]);
        assert_eq!(hidden.len(), 3);
        assert_eq!(
            hidden
                .iter()
                .map(|r| (&r.text, r.center))
                .collect::<Vec<_>>(),
            shown[1..]
                .iter()
                .map(|r| (&r.text, r.center))
                .collect::<Vec<_>>(),
            "power, XP and target labels unchanged"
        );
        state.forge_health_hidden = false;
        assert_eq!(readouts(&state, 100, [true, true, true]).len(), 4);
    }
    #[test]
    fn number_readouts_keep_text_positions_and_independent_switches() {
        let s = HudState {
            self_hp: 128,
            power: 123,
            max_power: 456,
            exp: 50,
            target_name: Some("Rat".into()),
            target_hp: 128,
            ..Default::default()
        };
        let all = readouts(&s, 100, [true; 3]);
        assert_eq!(
            all.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["50%", "123/456", "50/100 (50.0%)", "50%"]
        );
        assert_eq!(all[0].center, [99, 35]);
        assert_eq!(
            all[3].center,
            [
                super::super::TARGET_AT[0] + 40,
                super::super::TARGET_AT[1] + 49
            ]
        );
        assert_eq!(all[3].color, [1.0, 0.9, 0.3, 1.0]);
        assert_eq!(all[3].scale, 0.65);
        assert!(
            readouts(&s, 100, [false; 3]).is_empty(),
            "disabled preferences draw no labels"
        );
        let no_values = HudState {
            max_power: 0,
            target_name: None,
            ..s.clone()
        };
        assert_eq!(
            readouts(&no_values, 0, [true; 3]).len(),
            1,
            "unknown power/xp/target has no invented value"
        );
        let target_only = readouts(&s, 100, [true, false, false]);
        assert_eq!(target_only.len(), 1);
        assert_eq!(target_only[0].text, "50%");
    }
}
