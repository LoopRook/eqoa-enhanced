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
        pub main_exp_bar: bool,
        pub secondary_exp_bar: bool,
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

pub(crate) fn main_exp_bar_visible(_s: &HudState) -> bool {
    published(|v| !v.main_exp_bar)
}

pub(crate) fn secondary_exp_bar_visible(_s: &HudState) -> bool {
    published(|v| !v.secondary_exp_bar)
}

pub(crate) fn exp_bar_visible(_s: &HudState) -> bool {
    published(|v| !v.main_exp_bar && !v.secondary_exp_bar)
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

/// A raster that throws away all draw commands, used to keep WndChat's
/// per-frame state (text.age, scroll repeat, open/close animation) advancing
/// while the stock chat window is hidden by an addon claim.
struct NullRaster;
impl Raster for NullRaster {
    fn blit(&mut self, _id: u32, _flags: u32, _at: [i32; 2], _a: [i32; 2], _b: [i32; 2], _mode: i32, _alpha: f32) {}
    fn stretch_blit(&mut self, _id: u32, _flags: u32, _at: [i32; 2], _size: [i32; 2], _a: [i32; 2], _b: [i32; 2], _mode: i32, _alpha: f32) {}
    fn fill_fan(&mut self, _color: [f32; 4], _corners: [[i32; 2]; 4]) {}
    fn blit_full(&mut self, _id: u32, _flags: u32, _at: [i32; 2], _mode: i32, _alpha: f32) {}
    fn draw_string(&mut self, _at: [i32; 2], _color: [f32; 4], _s: &[u16]) {}
    fn draw_centered_string(&mut self, _at: [i32; 2], _color: [f32; 4], _s: &[u16], _mode: u32) {}
    fn string_width(&self, _s: &[u16]) -> i32 { 0 }
    fn point_size(&self) -> i32 { 12 }
    fn viewport(&self) -> [i32; 4] { [0, 0, 639, 447] }
    fn set_viewport(&mut self, _v: [i32; 4]) {}
}

/// Draws the stock chat window unless an addon has claimed it. The window's
/// state still advances every frame so that scroll repeats, open/close
/// animation and text ageing stay in sync with the server/chat stream.
pub(crate) fn draw_chat_window(
    wnd: &mut crate::chat::WndChat,
    vui: &Vui,
    r: &mut dyn Raster,
    now: u32,
    dt: f32,
) {
    if chat_window_visible() {
        wnd.draw(vui, r, now, dt);
    } else {
        let mut null = NullRaster;
        wnd.draw(vui, &mut null, now, dt);
    }
}

/// Whether the currently selected HUD menu is visible (only ability/hotbar
/// menus 0, 1, 4 and 5 are suppressed when an addon claims ability bars).
pub(crate) fn hud_menu_visible(menus: &crate::hudmenu::HudMenus) -> bool {
    let k = menus.sel.clamp(0, crate::hudmenu::MENUS as i32 - 1) as usize;
    let is_ability_bar = k != 2 && k != 3;
    ability_bar_visible() || !is_ability_bar
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
    if hud_menu_visible(menus) {
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
        if power_bar_visible(s) && s.max_power > 0 {
            out.push(Readout {
                text: format!("{}/{}", s.power, s.max_power),
                center: [99, 45],
                scale: 0.6,
                color: [0.4, 0.7, 1.0, 1.0],
                vertical: Vertical::Top,
            });
        }
    }
    if xp_details && need > 0 && main_exp_bar_visible(s) {
        let pct = (s.exp as f32 / need as f32 * 100.0).clamp(0.0, 100.0);
        out.push(Readout {
            text: format!("{}/{} ({:.1}%)", s.exp, need, pct),
            center: [99, 55],
            scale: 0.6,
            color: [1.0, 0.85, 0.35, 1.0],
            vertical: Vertical::Top,
        });
    }
    if target_hp_percent && s.target_name.is_some() && target_visible(s) {
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
    use super::super::{Hud, HudState, MemberState, PatchMember, PetState};
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

    /// A layer on the machine's GPU (any backend) with device and queue so
    /// tests can upload placeholder surfaces; None without an adapter.
    fn layer_with_device() -> Option<(render::ui::Ui, render::wgpu::Device, render::wgpu::Queue)> {
        let instance = render::wgpu::Instance::new(render::wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&render::wgpu::RequestAdapterOptions::default())).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).ok()?;
        let ui = render::ui::Ui::new(&device, render::TARGET_FORMAT, render::DEPTH_FORMAT);
        Some((ui, device, queue))
    }

    fn test_assets(ui: &mut render::ui::Ui, device: &render::wgpu::Device, queue: &render::wgpu::Queue) -> crate::ui::UiAssets {
        let mut assets = crate::ui::UiAssets::default();
        let tex = ui.add_texture(device, queue, 1, 1, &[255; 4]);
        // Empty atlas rows resolve to "(null)" through Vui::entry_id.
        assets.surfaces.insert(vi::ui::icon_id("(null)"), tex);
        assets.surfaces.insert(vi::ui::icon_id(""), tex);
        assets.surfaces.insert(vi::ui::icon_id("sheet1"), tex);
        assets.surfaces.insert(vi::ui::icon_id("Patch_Art"), tex);
        assets
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
        for i in 0..6 {
            wnd.text.add_text(&crate::ui::wide(&format!("hello {i}")), [1.0; 4], 0, |_| 0);
        }

        clear_claims();
        let mut r = BlitRecorder::default();
        draw_chat_window(&mut wnd, &vui, &mut r, 0, 0.0);
        let shown = r.blits.len() + r.centered.len();
        assert!(shown > 0, "chat window draws when unclaimed");

        set_claims(&["chat_window"]);
        let mut hidden_wnd = crate::chat::WndChat::new();
        for i in 0..6 {
            hidden_wnd.text.add_text(&crate::ui::wide(&format!("line {i}")), [1.0; 4], 0, |_| 0);
        }
        assert_eq!(hidden_wnd.text.line(0).unwrap().kind, 1);
        let mut r = BlitRecorder::default();
        // Step at t=1000ms with dt=0.5s: window opens and animates height while hidden
        draw_chat_window(&mut hidden_wnd, &vui, &mut r, 1000, 0.5);
        assert_eq!(r.blits.len() + r.centered.len(), 0, "chat window does not draw when claimed");
        assert!(hidden_wnd.contains([422.0, 440.0]), "open/close state still advances while hidden");
        // Hold R2 scroll-up key (19) and step 0.5s while hidden: repeat scroll fires
        hidden_wnd.set_scroll(1);
        hidden_wnd.key_down(19);
        hidden_wnd.cues.clear();
        draw_chat_window(&mut hidden_wnd, &vui, &mut r, 25_000, 0.5);
        assert_eq!(r.blits.len() + r.centered.len(), 0, "still no draw calls while hidden");
        assert_eq!(hidden_wnd.text.line(0).unwrap().kind, 0, "text.age still advances while hidden");
        assert!(!hidden_wnd.cues.is_empty(), "R2 scroll repeat still advances while hidden");

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
        clear_claims();
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

        set_claims(&["power_bar"]);
        let no_power = readouts(&state, 100, [true, true, true]);
        assert_eq!(
            no_power.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["50%", "20/100 (20.0%)", "50%"]
        );

        set_claims(&["secondary_exp_bar"]);
        assert_eq!(
            readouts(&state, 100, [true, true, true]).len(),
            4,
            "secondary exp claim leaves main XP readout visible"
        );

        set_claims(&["main_exp_bar"]);
        let no_xp = readouts(&state, 100, [true, true, true]);
        assert_eq!(
            no_xp.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["50%", "4/10", "50%"]
        );

        set_claims(&["target_nameplate"]);
        let no_target = readouts(&state, 100, [true, true, true]);
        assert_eq!(
            no_target.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["50%", "4/10", "20/100 (20.0%)"]
        );

        clear_claims();
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

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_hud_compass_hides_at_draw_site_on_widescreen() {
        let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut ui, device, queue)) = layer_with_device() else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        let assets = test_assets(&mut ui, &device, &queue);
        let vui = test_vui();
        let size = (1280, 720);

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState { has_self: false, ..Default::default() });
        let baseline = ui.quads.len();

        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState { has_self: true, heading: 0.0, ..Default::default() });
        let unclaimed = ui.quads.len();
        assert!(unclaimed > baseline, "compass draws when unclaimed");

        set_claims(&["compass_face", "compass_back"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState { has_self: true, heading: 0.0, ..Default::default() });
        let claimed = ui.quads.len();
        assert_eq!(claimed, baseline, "compass does not draw when claimed");

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState { has_self: true, heading: 0.0, ..Default::default() });
        assert_eq!(ui.quads.len(), unclaimed, "releasing the claim restores drawing");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_hud_power_bar_hides_at_draw_site() {
        let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut ui, device, queue)) = layer_with_device() else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        let assets = test_assets(&mut ui, &device, &queue);
        let vui = test_vui();
        let size = (1280, 720);
        let with_power = HudState { power: 5, max_power: 10, ..Default::default() };

        set_claims(&["power_bar"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_power);
        let baseline = ui.quads.len();

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_power);
        let unclaimed = ui.quads.len();
        assert!(unclaimed > baseline, "power bar draws when unclaimed");

        set_claims(&["power_bar"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_power);
        assert_eq!(ui.quads.len(), baseline, "power bar does not draw when claimed");

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_power);
        assert_eq!(ui.quads.len(), unclaimed, "releasing the claim restores drawing");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_hud_exp_bars_hides_at_draw_site() {
        let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut ui, device, queue)) = layer_with_device() else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        let assets = test_assets(&mut ui, &device, &queue);
        let vui = test_vui();
        let size = (1280, 720);
        let with_exp = HudState { level: 1, exp: 50, max_power: 0, ..Default::default() };

        set_claims(&["main_exp_bar", "secondary_exp_bar"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_exp);
        let baseline = ui.quads.len();

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_exp);
        let unclaimed = ui.quads.len();
        assert!(unclaimed > baseline, "exp bars draw when unclaimed");

        set_claims(&["main_exp_bar"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_exp);
        let main_hidden = ui.quads.len();
        assert!(
            main_hidden > baseline && main_hidden < unclaimed,
            "claiming main_exp_bar hides only the main exp bar"
        );

        set_claims(&["secondary_exp_bar"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_exp);
        let secondary_hidden = ui.quads.len();
        assert!(
            secondary_hidden > baseline && secondary_hidden < unclaimed,
            "claiming secondary_exp_bar hides only the secondary exp bar"
        );

        set_claims(&["main_exp_bar", "secondary_exp_bar"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_exp);
        assert_eq!(ui.quads.len(), baseline, "exp bars do not draw when claimed");

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_exp);
        assert_eq!(ui.quads.len(), unclaimed, "releasing the claim restores drawing");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_hud_group_panel_hides_at_draw_site() {
        let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut ui, device, queue)) = layer_with_device() else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        let assets = test_assets(&mut ui, &device, &queue);
        let vui = test_vui();
        let size = (1280, 720);

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState { group_count: 1, ..Default::default() });
        let baseline = ui.quads.len();

        let with_group = HudState {
            group_count: 2,
            group: vec![MemberState { id: 2, name: "Ally".into(), is_target: false, hp: Some((128, false, false)), power: Some(128) }],
            ..Default::default()
        };
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_group);
        let unclaimed = ui.quads.len();
        assert!(unclaimed > baseline, "group panel draws when unclaimed");

        set_claims(&["group_member_panel"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_group);
        assert_eq!(ui.quads.len(), baseline, "group panel and member power gauge do not draw when claimed");

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_group);
        assert_eq!(ui.quads.len(), unclaimed, "releasing the claim restores drawing");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_hud_pet_panel_hides_at_draw_site() {
        let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut ui, device, queue)) = layer_with_device() else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        let assets = test_assets(&mut ui, &device, &queue);
        let vui = test_vui();
        let size = (1280, 720);

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState { group_count: 1, ..Default::default() });
        let baseline = ui.quads.len();

        let with_pet = HudState {
            group_count: 1,
            pet: Some(PetState { name: "Bub".into(), is_target: false, creature: None }),
            ..Default::default()
        };
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_pet);
        let unclaimed = ui.quads.len();
        assert!(unclaimed > baseline, "pet panel draws when unclaimed");

        set_claims(&["pet_panel"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_pet);
        assert_eq!(ui.quads.len(), baseline, "pet panel does not draw when claimed");

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_pet);
        assert_eq!(ui.quads.len(), unclaimed, "releasing the claim restores drawing");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_hud_target_nameplate_hides_at_draw_site() {
        let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut ui, device, queue)) = layer_with_device() else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        let assets = test_assets(&mut ui, &device, &queue);
        let vui = test_vui();
        let size = (1280, 720);
        let without_target = HudState::default();
        let with_target = HudState {
            target_name: Some("Casil".into()),
            target_hp: 128,
            target_power: Some(128),
            target_info: Some(0),
            target_of_target: Some("Luck".into()),
            tot_hp: Some(200),
            tot_power: Some(100),
            ..Default::default()
        };

        set_claims(&["target_nameplate"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &without_target);
        let baseline = ui.quads.len();

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_target);
        let unclaimed = ui.quads.len();
        assert!(unclaimed > baseline, "target nameplate draws when unclaimed");

        set_claims(&["target_nameplate"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_target);
        let claimed = ui.quads.len();
        assert_eq!(claimed, baseline, "target nameplate, target of target, and target gauges do not draw when claimed");

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_target);
        assert_eq!(ui.quads.len(), unclaimed, "releasing the claim restores drawing");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_hud_group_compass_marks_hides_at_draw_site() {
        let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut ui, device, queue)) = layer_with_device() else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        let assets = test_assets(&mut ui, &device, &queue);
        let vui = test_vui();
        let size = (1280, 720);

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState { has_self: true, heading: 0.0, ..Default::default() });
        let baseline = ui.quads.len();

        let with_marks = HudState {
            has_self: true,
            heading: 0.0,
            camera: [0.0, 0.0, 0.0],
            patches: vec![PatchMember { index: 1, id: 2, pos: Some([0.0, 0.0, 100.0]) }],
            ..Default::default()
        };
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_marks);
        let unclaimed = ui.quads.len();
        assert!(unclaimed > baseline, "group compass marks draw when unclaimed");

        set_claims(&["group_compass_marks"]);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_marks);
        assert_eq!(ui.quads.len(), baseline, "group compass marks do not draw when claimed");

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &with_marks);
        assert_eq!(ui.quads.len(), unclaimed, "releasing the claim restores drawing");
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_hud_default_no_addon_quad_count_1280x720() {
        let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut ui, device, queue)) = layer_with_device() else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        let assets = test_assets(&mut ui, &device, &queue);
        let vui = test_vui();
        let size = (1280, 720);

        crate::fork_ext::addons::visibility::publish(false);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState::default());
        let default_quads = ui.quads.len();
        assert_eq!(default_quads, 14, "stock default HUD quad count at 1280x720 with no addon");

        let full_state = HudState {
            has_self: true,
            self_hp: 200,
            power: 80,
            max_power: 100,
            level: 10,
            exp: 500,
            heading: 0.5,
            chat_open: true,
            group_count: 2,
            group: vec![MemberState { id: 2, name: "Ally".into(), is_target: false, hp: Some((200, false, false)), power: Some(150) }],
            pet: Some(PetState { name: "Bub".into(), is_target: false, creature: Some((200, false, false)) }),
            target_name: Some("Casil".into()),
            target_hp: 128,
            target_power: Some(128),
            target_info: Some(0),
            target_of_target: Some("Luck".into()),
            tot_hp: Some(200),
            tot_power: Some(100),
            patches: vec![PatchMember { index: 1, id: 2, pos: Some([0.0, 0.0, 100.0]) }],
            ..Default::default()
        };
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &full_state);
        let full_quads = ui.quads.len();
        assert_eq!(full_quads, 38, "stock populated HUD quad count at 1280x720 with no addon");

        set_claims(crate::fork_ext::addons::visibility::ALL_UI_ELEMENTS);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState { forge_health_hidden: true, ..full_state.clone() });
        assert!(ui.quads.len() < default_quads, "claiming all elements suppresses stock HUD quads");

        crate::fork_ext::addons::visibility::publish(false);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState::default());
        assert_eq!(ui.quads.len(), default_quads);
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &full_state);
        assert_eq!(ui.quads.len(), full_quads);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn forge_hud_claims_draw_again_after_pause_reload_and_shutdown() {
        use crate::fork_ext::addons::{
            health::HealthSnapshot,
            host::Snapshot,
            manager::{Command, Manager},
        };
        use dear_imgui_rs::{Context, sys};
        use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
        use std::{
            cell::RefCell,
            fs,
            rc::Rc,
            sync::atomic::{AtomicU64, Ordering},
        };

        let _gpu = crate::GPU_TEST.lock().unwrap_or_else(|e| e.into_inner());
        let Some((mut ui, device, queue)) = layer_with_device() else {
            eprintln!("SKIP: no graphics adapter");
            return;
        };
        let assets = test_assets(&mut ui, &device, &queue);
        let vui = test_vui();
        let size = (1280, 720);
        let compass_state = HudState { has_self: true, heading: 0.0, ..Default::default() };

        clear_claims();
        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &HudState { has_self: false, ..Default::default() });
        let baseline_compass = ui.quads.len();

        ui.begin(size);
        let mut hud = Hud::new();
        hud.draw(&vui, &mut assets.raster(&mut ui), &compass_state);
        let unclaimed_compass = ui.quads.len();
        assert!(unclaimed_compass > baseline_compass);

        let mut wnd = crate::chat::WndChat::new();
        wnd.mode = 1;
        wnd.text.add_text(&crate::ui::wide("hello"), [1.0; 4], 0, |_| 0);
        let mut r = BlitRecorder::default();
        draw_chat_window(&mut wnd, &vui, &mut r, 0, 0.0);
        let unclaimed_chat = r.blits.len() + r.centered.len();
        assert!(unclaimed_chat > 0);

        let mut menus = crate::hudmenu::HudMenus::default();
        menus.build(&Default::default(), &Default::default(), &Default::default(), Default::default());
        menus.sel = 0;
        let mut r = BlitRecorder::default();
        draw_hud_menus(&mut menus, &vui, &mut r, 0, &|_, _| Vec::new());
        let unclaimed_menus = r.blits.len();
        assert!(unclaimed_menus > 0);

        static NEXT: AtomicU64 = AtomicU64::new(0);
        let scratch = std::env::temp_dir().join(format!(
            "eqoa-forge-hud-release-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let addon_dir = scratch.join("mods/a");
        let config_dir = scratch.join("config");
        fs::create_dir_all(&addon_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            addon_dir.join("main.lua"),
            r#"
                local UI = require("frontiers_forge.ui")
                UI.DisableCompass("test")
                UI.DisableChatWindow("test")
                UI.DisableAbilityBar("test")
            "#,
        )
        .unwrap();
        fs::write(
            addon_dir.join("mod.json"),
            r#"{"schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":"a","name":"a","entry":"main.lua","enabled":true}"#,
        )
        .unwrap();

        let original_context = unsafe { sys::igGetCurrentContext() };
        let mut context = Context::create();
        context.set_ini_filename::<std::path::PathBuf>(None).unwrap();
        let renderer = Rc::new(RefCell::new(
            WgpuRenderer::new(
                WgpuInitInfo::new(
                    device.clone(),
                    queue.clone(),
                    render::wgpu::TextureFormat::Rgba8Unorm,
                ),
                &mut context,
            )
            .unwrap(),
        ));
        let mut manager = Manager::new(
            scratch.join("mods"),
            Some(config_dir),
            &device,
            &queue,
            renderer,
        );
        let ready = Snapshot {
            in_game: true,
            health: HealthSnapshot::new(true, 100, 100),
            ..Default::default()
        };

        let mut step_manager = |mgr: &mut Manager| {
            context.io_mut().set_display_size([256.0, 192.0]);
            context.io_mut().set_display_framebuffer_scale([1.0, 1.0]);
            context.io_mut().set_delta_time(1.0 / 60.0);
            context.frame();
            mgr.frame(ready);
            let _ = context.end_frame();
            mgr.after_render();
        };

        let assert_all_hidden = |ui: &mut render::ui::Ui, wnd: &mut crate::chat::WndChat, menus: &mut crate::hudmenu::HudMenus, phase: &str| {
            ui.begin(size);
            let mut hud = Hud::new();
            hud.draw(&vui, &mut assets.raster(ui), &compass_state);
            assert_eq!(ui.quads.len(), baseline_compass, "{phase}: compass must be hidden");
            let mut r = BlitRecorder::default();
            draw_chat_window(wnd, &vui, &mut r, 0, 0.0);
            assert_eq!(r.blits.len() + r.centered.len(), 0, "{phase}: chat must be hidden");
            let mut r = BlitRecorder::default();
            draw_hud_menus(menus, &vui, &mut r, 0, &|_, _| Vec::new());
            assert_eq!(r.blits.len(), 0, "{phase}: ability bar must be hidden");
        };

        let assert_all_drawn = |ui: &mut render::ui::Ui, wnd: &mut crate::chat::WndChat, menus: &mut crate::hudmenu::HudMenus, phase: &str| {
            ui.begin(size);
            let mut hud = Hud::new();
            hud.draw(&vui, &mut assets.raster(ui), &compass_state);
            assert_eq!(ui.quads.len(), unclaimed_compass, "{phase}: compass must draw again");
            let mut r = BlitRecorder::default();
            draw_chat_window(wnd, &vui, &mut r, 0, 0.0);
            assert_eq!(r.blits.len() + r.centered.len(), unclaimed_chat, "{phase}: chat must draw again");
            let mut r = BlitRecorder::default();
            draw_hud_menus(menus, &vui, &mut r, 0, &|_, _| Vec::new());
            assert_eq!(r.blits.len(), unclaimed_menus, "{phase}: ability bar must draw again");
        };

        // 1. Active frame claims compass + chat + ability bars
        step_manager(&mut manager);
        assert_all_hidden(&mut ui, &mut wnd, &mut menus, "claimed");

        // 2. Pause (suspend_health + publish(false)) -> all draw again
        manager.suspend_health();
        crate::fork_ext::addons::visibility::publish(false);
        assert_all_drawn(&mut ui, &mut wnd, &mut menus, "after pause");

        // 3. Re-step -> claims active again, then Reload -> all draw again on reload boundary
        step_manager(&mut manager);
        assert_all_hidden(&mut ui, &mut wnd, &mut menus, "re-claimed before reload");
        manager.queue(Command::Reload("a".into()));
        step_manager(&mut manager);
        assert_all_drawn(&mut ui, &mut wnd, &mut menus, "after reload");

        // 4. Re-step -> reloaded VM claims again, then Shutdown -> all draw again
        step_manager(&mut manager);
        assert_all_hidden(&mut ui, &mut wnd, &mut menus, "re-claimed after reload");
        manager.shutdown();
        assert_all_drawn(&mut ui, &mut wnd, &mut menus, "after shutdown");

        drop(manager);
        drop(context);
        unsafe { sys::igSetCurrentContext(original_context) };
        let _ = fs::remove_dir_all(&scratch);
    }
}
