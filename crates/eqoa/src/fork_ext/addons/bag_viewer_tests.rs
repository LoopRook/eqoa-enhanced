// Real Manager + GPU evidence for the bundled eqoa_bag_viewer mod (included in manager tests, round 5 Part B).
// The game state is protocol-shaped: the zone-in dump's pack and bank, and the loot window, the bank's and the
// pack's changes arrive through the client's own opcode and event handlers (`Play::bags_*`, items_tests.rs).
// Set EQOA_R5B_TEST_SOURCE to a copy of the package folder to run that instead of the embedded bytes (negative
// controls), and EQOA_R5B_CAPTURE_DIR to save the pictures.
const BV: &str = "eqoa_bag_viewer";
const BV_SIZE: (u32, u32) = (1024, 768);
const BV_BACKGROUND: [u8; 4] = [26, 38, 26, 255];

/// A flat colour per icon id (the fixture's art cache); none of them is the worn marker's green.
fn bv_icon_color(id: u32) -> [u8; 4] {
    let table = [[200, 80, 80], [70, 120, 220], [220, 190, 60], [170, 90, 200], [230, 130, 60], [200, 200, 210], [160, 120, 80], [210, 110, 160]];
    let c = table.get((id as usize).wrapping_sub(4001)).copied().unwrap_or([120, 130, 170 + (id % 60) as u8]);
    [c[0], c[1], c[2], 255]
}
struct BvIcons;
impl super::super::icons::IconProvider for BvIcons {
    fn decode_icon(&self, id: u32) -> Option<super::super::icons::DecodedIcon> {
        let color = bv_icon_color(id);
        Some(super::super::icons::DecodedIcon { width: 32, height: 32, rgba: color.repeat(32 * 32) })
    }
}

fn bv_world() -> serde_json::Value {
    serde_json::json!({
        "pack": [
            {"name":"Rusty Sword","id":900001,"key":11,"wear":12,"equip":0,"cur_hp":30,"max_hp":40,"value":100,"icon":4001,"stats":[[0,3],[14,-2]]},
            {"name":"Healing Potion","id":900002,"key":12,"qty":5,"max_stack":20,"quick":1,"icon":4002},
            {"name":"Iron Ring","id":900003,"key":13,"wear":8,"class_mask":3,"race_mask":5,"level":20,"rent":0,"icon":4003},
            {"name":"Mage Robe","id":900004,"key":14,"wear":2,"class_mask":32,"icon":4004},
            {"name":"Elf Cloak","id":900005,"key":15,"wear":9,"race_mask":64,"icon":4005},
            {"name":"Hero Helm","id":900006,"key":16,"wear":1,"level":55,"icon":4006},
            {"name":"Rat Pelt","id":900007,"key":17,"qty":3,"max_stack":100,"icon":4007},
            {"name":"Odd Thing","id":900008,"key":18,"wear":0,"icon":4008}
        ],
        "bank": [
            {"name":"Moonstone","id":900101,"key":21,"qty":3,"max_stack":100,"icon":4010},
            {"name":"Spare Dagger","id":900102,"key":22,"wear":12,"icon":4011}
        ],
        "tunar": 1234,
        "bank_tunar": 99000
    })
}
fn bv_corpse() -> serde_json::Value {
    serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":31,"qty":1,"max_stack":100,"icon":4020},
        {"name":"Spider Silk","id":2002,"key":32,"qty":4,"max_stack":100,"icon":4021},
        {"name":"Bone Chip","id":2003,"key":33,"qty":2,"max_stack":100,"icon":4022}
    ])
}

struct Bv {
    gpu: Gpu,
    _root: Scratch,
    _config: Scratch,
    manager: Manager,
    play: crate::play::Play,
    win: (u32, u32),
    target: Option<wgpu::Texture>,
}
impl Drop for Bv {
    fn drop(&mut self) {
        super::super::icons::set_custom_icon_provider(None);
        super::super::hud_layout::set_window_size(0, 0);
    }
}
impl Bv {
    fn new() -> Option<Bv> {
        Bv::sized(BV_SIZE)
    }
    fn sized(win: (u32, u32)) -> Option<Bv> {
        let gpu = match Gpu::new() {
            Some(gpu) => gpu,
            None => {
                eprintln!("SKIP bag viewer: no GPU adapter");
                return None;
            }
        };
        crate::settings::testing::reset(false);
        crate::hudlayout::testing_reset();
        crate::hideui::show();
        super::super::input::reset_input_state();
        crate::fork_ext::bindpage::reset_in_memory();
        // an earlier test's saved key for this mod's row must not make it look bound
        let prefix = format!("mod.{BV}.");
        let saved = crate::settings::read_config("mod-keys.opt").unwrap_or_default();
        let kept: String = String::from_utf8_lossy(&saved)
            .lines()
            .filter(|l| !l.starts_with(&prefix))
            .map(|l| format!("{l}\n"))
            .collect();
        let _ = crate::settings::write_config("mod-keys.opt", kept.as_bytes());
        super::super::hud_layout::set_window_size(win.0, win.1);
        super::super::hud_layout::set_hud_state(true, true, false, 93, 0);
        super::super::icons::set_custom_icon_provider(Some(Box::new(BvIcons)));
        let root = Scratch::new();
        let config = Scratch::new();
        for file in crate::fork_ext::bundled::EMBEDDED_FILES.iter().filter(|f| f.package == BV) {
            let path = root.0.join(file.path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            // a negative control points this at a mutated copy of the package folder
            match std::env::var_os("EQOA_R5B_TEST_SOURCE") {
                Some(dir) => {
                    fs::copy(PathBuf::from(dir).join(file.path.strip_prefix("eqoa_bag_viewer/").unwrap()), &path).unwrap();
                }
                None => fs::write(path, file.data).unwrap(),
            }
        }
        let mut play = crate::play::Play::bundled_ui_fixture();
        play.bags_player(0, 0, 50);
        play.bags_words();
        play.bags_set(&bv_world());
        let manager = gpu.manager(&root.0, Some(config.0.clone()));
        Some(Bv { gpu, _root: root, _config: config, manager, play, win, target: None })
    }
    fn enable(&mut self) {
        self.manager.queue(Command::Enable(BV.into(), true));
        self.frames(3);
        assert!(self.manager.entries().iter().all(|e| e.status == Status::Enabled), "{:?}", self.manager.diagnostics());
        self.clean();
    }
    /// Every bundled-mod test session asserts zero Addon Manager diagnostics.
    fn clean(&self) {
        let d = self.manager.diagnostics();
        assert!(d.is_empty(), "{d:?}");
    }
    fn frame(&mut self) -> wgpu::Texture {
        let mut snapshot = self.play.forge_snapshot(None);
        snapshot.capture_pending = false;
        self.manager.flush_pending_fonts(&mut self.gpu.context);
        {
            let io = self.gpu.context.io_mut();
            io.set_display_size([self.win.0 as f32, self.win.1 as f32]);
            io.set_display_framebuffer_scale([1., 1.]);
            io.set_delta_time(1. / 60.);
        }
        self.gpu.context.frame();
        super::super::native_core::presentation();
        self.manager.frame(snapshot);
        let size = self.win;
        let gpu = &mut self.gpu;
        if self.target.as_ref().is_none_or(|t| (t.width(), t.height()) != size) {
            self.target = Some(gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("bag viewer frame"),
                size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }));
        }
        let target = self.target.as_ref().unwrap();
        let view = target.create_view(&Default::default());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let [r, g, b, _] = BV_BACKGROUND.map(|v| v as f64 / 255.);
        {
            let mut renderer = gpu.renderer.borrow_mut();
            let frame = gpu.context.render(renderer.renderer_consumer().unwrap());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1. }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            renderer.render(frame, &mut pass, FramebufferExtent::new(size.0, size.1)).unwrap();
        }
        gpu.queue.submit([encoder.finish()]);
        self.manager.after_render();
        self.target.as_ref().unwrap().clone()
    }
    fn frames(&mut self, n: usize) -> wgpu::Texture {
        let mut t = self.frame();
        for _ in 1..n {
            t = self.frame();
        }
        t
    }
    fn lua(&self) -> Lua {
        lua(&self.manager, BV)
    }
    fn eval<T: mlua::FromLuaMulti>(&self, expr: &str) -> T {
        self.lua().load(format!("return {expr}")).eval().unwrap()
    }
    fn names(&self) -> Vec<String> {
        self.eval("(function() local t = {} for _, e in ipairs(require('bags').grid.items) do t[#t + 1] = e.item.name end return t end)()")
    }
    fn log(&self) -> Vec<String> {
        self.eval("(function() local t = {} for _, e in ipairs(require('bags').log) do t[#t + 1] = e.text end return t end)()")
    }
    /// Presses a key the way the viewer routes a physical key to the mods, then lets it go.
    fn key(&mut self, key: winit::keyboard::Key) {
        super::super::input::route_mod_key(&key, true, false, false, true, false);
        self.frame();
        super::super::input::route_mod_key(&key, false, false, false, true, false);
        self.frame();
    }
    fn bind_k(&mut self) {
        assert!(crate::fork_ext::bindpage::set_key(BV, "open", "k"));
    }
    fn toggle(&mut self) {
        self.key(winit::keyboard::Key::Character("k".into()));
    }
    fn open(&mut self) {
        self.bind_k();
        self.toggle();
        assert!(self.eval::<bool>("require('bags').open"), "{:?}", self.manager.diagnostics());
        self.frames(2);
    }
    fn is_open(&self) -> bool {
        self.eval("require('bags').open")
    }
    /// The window's rectangle and scale as the mod drew it last: (x, y, w, h, k).
    fn rect(&self) -> (f64, f64, f64, f64, f64) {
        self.eval("(function() local r = require('bags').rect return r.x, r.y, r.w, r.h, r.k end)()")
    }
    /// Window pixel of a point in the mod's frame units (the window's own top-left is 0, 0).
    fn at(&self, x: f64, y: f64) -> (f64, f64) {
        let (rx, ry, _, _, k) = self.rect();
        (rx + x * k, ry + y * k)
    }
    /// Centre of grid slot `n` (1-based) in window pixels (8 columns of 36 with a gap of 2, from (8, 46)).
    fn slot(&self, n: usize) -> (f64, f64) {
        let (col, row) = ((n - 1) % 8, (n - 1) / 8);
        self.at(8. + col as f64 * 38. + 18., 46. + row as f64 * 38. + 18.)
    }
    fn point(&self, p: (f64, f64)) {
        super::super::pointer_api::position([p.0 as f32, p.1 as f32], true, self.win);
    }
    fn hover(&mut self, n: usize) -> Vec<String> {
        self.point(self.slot(n));
        self.frames(2);
        self.eval("(function() local h = require('bags').hover local t = {} if h then for _, l in ipairs(h.lines) do t[#t + 1] = l[1] .. '|' .. l[2] end end return t end)()")
    }
    fn click(&mut self, p: (f64, f64)) {
        use winit::event::{DeviceId, ElementState, MouseButton, WindowEvent};
        self.point(p);
        for state in [ElementState::Pressed, ElementState::Released] {
            let ev = WindowEvent::MouseInput { device_id: DeviceId::dummy(), state, button: MouseButton::Left };
            super::super::pointer_api::observe(&ev, self.win);
            self.frame();
        }
    }
    /// The centre of tab `n` (1-based) in window pixels, from the hit rectangles the mod clicks through.
    fn tab(&self, n: usize) -> (f64, f64) {
        self.eval(&format!("(function() local r = require('bags').tab_hits[{n}] return r.x + r.w / 2, r.y + r.h / 2 end)()"))
    }
}
fn bv_px(pixels: &[u8], width: u32, x: f64, y: f64) -> [u8; 4] {
    let i = ((y as usize) * width as usize + x as usize) * 4;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}
/// Whether a pixel within `radius` of (x, y) is within `tol` of `color` in each channel.
fn bv_near(pixels: &[u8], width: u32, p: (f64, f64), radius: i32, color: [u8; 4], tol: i32) -> bool {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let px = bv_px(pixels, width, p.0 + dx as f64, p.1 + dy as f64);
            if (0..3).all(|c| (px[c] as i32 - color[c] as i32).abs() <= tol) {
                return true;
            }
        }
    }
    false
}
/// Whether a pixel within `radius` of (x, y) is red-dominant (the mod's accent, as a thin line comes out blended).
fn bv_red_near(pixels: &[u8], width: u32, p: (f64, f64), radius: i32) -> bool {
    (-radius..=radius).any(|dy| (-radius..=radius).any(|dx| {
        let c = bv_px(pixels, width, p.0 + dx as f64, p.1 + dy as f64);
        c[0] >= 120 && c[1] <= 70 && c[2] <= 70
    }))
}
/// The same for the worn marker's green.
fn bv_green_near(pixels: &[u8], width: u32, p: (f64, f64), radius: i32) -> bool {
    (-radius..=radius).any(|dy| (-radius..=radius).any(|dx| {
        let c = bv_px(pixels, width, p.0 + dx as f64, p.1 + dy as f64);
        c[1] >= 90 && c[2] <= c[1] && c[1] as i32 - c[0] as i32 >= 40
    }))
}
fn bv_png(pixels: &[u8], size: (u32, u32), name: &str) {
    if let Some(dir) = std::env::var_os("EQOA_R5B_CAPTURE_DIR") {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).unwrap();
        let mut enc = png::Encoder::new(fs::File::create(dir.join(name)).unwrap(), size.0, size.1);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(pixels).unwrap();
    }
}

#[test]
fn round5b_off_by_default_loads_clean_has_an_unbound_controls_row_and_its_key_opens_and_closes_it() {
    use winit::keyboard::{Key, NamedKey};
    let Some(mut bv) = Bv::new() else { return };
    // off: the package is there, nothing runs, no Controls row yet
    bv.frames(3);
    let entry = bv.manager.entries().into_iter().find(|e| e.metadata.id == BV).expect("the package is discovered");
    assert_eq!(entry.status, Status::Disabled, "off by default");
    assert!(!entry.metadata.initially_enabled);
    assert_eq!(crate::fork_ext::bindpage::get_key(BV, "open"), None, "no Controls row while it is off");
    bv.enable();
    // the row exists, and it is unbound: the game's keys are not changed
    assert_eq!(crate::fork_ext::bindpage::get_key(BV, "open"), Some(String::new()));
    assert_eq!(bv.eval::<Option<String>>("eqoa.binds.get('open')"), Some(String::new()));
    bv.key(Key::Character("k".into()));
    assert!(!bv.is_open(), "an unbound row opens nothing");
    // bound to K: the key opens and closes
    bv.bind_k();
    bv.toggle();
    assert!(bv.is_open());
    bv.frames(2);
    assert!(bv.eval::<u32>("require('bags').draws") > 0, "an open window draws");
    bv.toggle();
    assert!(!bv.is_open());
    // Escape closes it too
    bv.toggle();
    assert!(bv.is_open());
    bv.key(Key::Named(NamedKey::Escape));
    assert!(!bv.is_open(), "Escape closes the window");
    bv.clean();
    // the control: another key opens nothing
    bv.key(Key::Character("j".into()));
    assert!(!bv.is_open());
    bv.manager.queue(Command::Enable(BV.into(), false));
    bv.frames(2);
    assert_eq!(crate::fork_ext::bindpage::dispatch_keypress("k"), None, "a disabled mod's key fires nothing");
    let _ = crate::fork_ext::bindpage::set_key(BV, "open", "");
}

#[test]
fn round5b_the_grid_shows_the_pack_as_the_dump_gave_it_and_follows_its_changes() {
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    bv.open();
    assert_eq!(bv.names(), ["Rusty Sword", "Healing Potion", "Iron Ring", "Mage Robe", "Elf Cloak", "Hero Helm", "Rat Pelt", "Odd Thing"]);
    // 40 slots: five rows of eight; the numbers under them are the dump's
    assert_eq!(bv.eval::<(u32, u32, Option<u32>, i64)>("(function() local g = require('bags').grid return g.rows, g.used, g.capacity, g.money end)()"), (5, 8, Some(40), 1234));
    // right icons and stack counts, and the worn marker, from the item records
    let rows: Vec<String> = bv.eval(
        "(function() local t = {} for _, e in ipairs(require('bags').grid.items) do t[#t + 1] = e.item.icon .. ':' .. e.item.quantity .. ':' .. tostring(e.item.equipped) .. ':' .. tostring(e.texture ~= nil) end return t end)()",
    );
    assert_eq!(rows[0], "4001:1:true:true", "the sword is worn and has its art");
    assert_eq!(rows[1], "4002:5:false:true", "five potions");
    assert_eq!(rows[6], "4007:3:false:true");
    assert_eq!(rows.iter().filter(|r| r.contains(":true:")).count(), 1, "one worn item");
    let builds = bv.eval::<u32>("require('bags').builds");
    // a quiet frame rebuilds nothing
    bv.frames(5);
    assert_eq!(bv.eval::<u32>("require('bags').builds"), builds);
    // an item taken (0x003C for the potions) and an item added (0x003B), each seen after its event
    bv.play.bags_took(1, 5);
    bv.frames(2);
    assert!(!bv.names().contains(&"Healing Potion".to_string()), "the taken stack is gone");
    assert_eq!(bv.eval::<u32>("require('bags').grid.used"), 7);
    bv.play.bags_added(&serde_json::json!({"name":"Silk Scrap","id":900009,"key":19,"qty":2,"max_stack":50,"icon":4009}));
    bv.frames(2);
    assert_eq!(bv.names().last().map(String::as_str), Some("Silk Scrap"));
    assert_eq!(bv.eval::<u32>("require('bags').grid.used"), 8);
    // tunar (0x0052)
    bv.play.bags_tunar(2000);
    bv.frames(2);
    assert_eq!(bv.eval::<i64>("require('bags').grid.money"), 2000);
    assert_eq!(bv.eval::<u32>("require('bags').builds"), builds + 3, "one read per change event");
    bv.clean();
}

#[test]
fn round5b_equipment_follows_the_games_own_tab_and_the_bank_reads_away_from_a_banker() {
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    bv.open();
    // the tab strip: the game's four item-page tabs between All and the bank
    assert_eq!(bv.eval::<Vec<String>>("(function() local t = {} for _, tab in ipairs(require('bags').TABS) do t[#t + 1] = tab.label end return t end)()"), ["All", "Usable", "Equipment", "Charged", "Misc", "Bank"]);
    let tab = bv.tab(3);
    bv.click(tab);
    bv.frames(2);
    // what the game's own item page lists under Equipment, computed from the same pack by its predicates
    let native = bv.play.bags_native_tab("equipment");
    assert_eq!(native.len(), 6, "the fixture has six wearable stacks, one with wear slot 0");
    assert_eq!(bv.names(), native);
    assert!(bv.names().contains(&"Odd Thing".to_string()), "wear slot 0 is wearable to the game");
    assert_eq!(bv.eval::<u32>("require('bags').grid.used"), 8, "used/capacity still counts the whole pack");
    // the bank: its items and its tunar, with no banker near (the bank window is closed)
    assert!(!bv.eval::<bool>("eqoa.bank.get().open"));
    let tab = bv.tab(6);
    bv.click(tab);
    bv.frames(2);
    assert_eq!(bv.names(), ["Moonstone", "Spare Dagger"]);
    assert_eq!(bv.eval::<(i64, u32, bool)>("(function() local g = require('bags').grid return g.money, g.used, g.bank end)()"), (99000, 2, true));
    // back to All
    let tab = bv.tab(1);
    bv.click(tab);
    bv.frames(2);
    assert_eq!(bv.names().len(), 8);
    bv.clean();
}

#[test]
fn round5b_the_card_has_the_items_fields_and_the_red_line_only_when_the_character_cannot_use_it() {
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    bv.open();
    assert_eq!(bv.eval::<String>("eqoa.items.slot_info(12).name"), "Primary");
    let sword = bv.hover(1);
    assert_eq!(sword[0], "Rusty Sword|title");
    assert!(sword.contains(&"Level 1|text".to_string()), "{sword:?}");
    assert!(sword.contains(&"Slot: Primary|text".to_string()), "{sword:?}");
    assert!(sword.contains(&"Strength +3|text".to_string()) && sword.contains(&"Armor -2|text".to_string()), "stats keep their sign: {sword:?}");
    assert!(sword.contains(&"HP 30/40|text".to_string()), "{sword:?}");
    assert!(sword.contains(&"Value 100 tunar|muted".to_string()), "{sword:?}");
    assert!(!sword.iter().any(|l| l.contains("can't use")), "the sword is usable: {sword:?}");
    // the ring names classes 0 and 1, races 0 and 2, level 20: this level-50 class-0 race-0 character can use it
    let ring = bv.hover(3);
    assert_eq!(ring[0], "Iron Ring|title");
    assert!(ring.contains(&"Level 20|text".to_string()));
    assert!(!ring.iter().any(|l| l.contains("can't use")), "{ring:?}");
    // class, race and level each rule an item out (checked against the API's own reasons)
    for (slot, name, reason) in [(4, "Mage Robe", "class"), (5, "Elf Cloak", "race"), (6, "Hero Helm", "level")] {
        let card = bv.hover(slot);
        assert_eq!(card[0], format!("{name}|title"));
        assert_eq!(card.last().map(String::as_str), Some("You can't use this|red"), "{name}: {card:?}");
        let reasons: Vec<String> = bv.eval(&format!(
            "(function() local r = eqoa.items.can_use(require('bags').grid.items[{slot}].item.ref).reasons local t = {{}} for _, v in ipairs(r) do t[#t + 1] = v end return t end)()"
        ));
        assert_eq!(reasons, [reason], "{name} is ruled out by its {reason} alone");
    }
    bv.clean();
}

#[test]
fn round5b_the_loot_log_is_fed_by_pickups_newest_first_and_keeps_fifty() {
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    // the window stays closed: the log is the one thing that listens
    bv.play.bags_target_corpse(5001, "a decaying skeleton");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&bv_corpse());
    bv.frames(2);
    // the loot window knows which corpse it belongs to, and keeps it when the target moves on
    assert_eq!(bv.eval::<(u32, String)>("(function() local s = eqoa.loot.get().source return s.entity_id, s.name end)()"), (5001, "a decaying skeleton".to_string()));
    bv.play.bags_target(2002);
    bv.frames(2);
    assert_eq!(bv.eval::<String>("eqoa.loot.get().source.name"), "a decaying skeleton");
    bv.play.bags_loot_one(0);
    bv.frames(2);
    bv.play.bags_loot_one(0);
    bv.frames(2);
    bv.play.bags_loot_all();
    bv.frames(2);
    let log = bv.log();
    assert_eq!(log.len(), 3, "{log:?}");
    let shaped = |l: &str, name: &str| {
        let (stamp, rest) = l.split_at(5);
        stamp.chars().enumerate().all(|(i, c)| if i == 2 { c == ':' } else { c.is_ascii_digit() }) && rest == format!(" {name}")
    };
    let from = ", from a decaying skeleton";
    assert!(
        shaped(&log[0], &format!("Bone Chip x2{from}")) && shaped(&log[1], &format!("Spider Silk x4{from}")) && shaped(&log[2], &format!("Rat Pelt x1{from}")),
        "newest first, from the corpse that was targeted: {log:?}"
    );
    assert_eq!(bv.eval::<u32>("require('bags').builds"), 0, "a closed window reads nothing");
    // a corpse no one targeted (the target is a living snake): the line has no "from"
    bv.play.bags_target(2002);
    bv.play.bags_corpse(&serde_json::json!([{"name":"Moss Cap","id":2010,"key":41,"qty":1,"max_stack":10},{"name":"Torn Rag","id":2011,"key":42,"qty":1,"max_stack":10}]));
    bv.frames(2);
    bv.play.bags_loot_one(0);
    bv.frames(2);
    assert_eq!(bv.log().len(), 4);
    assert!(bv.log()[0].ends_with(" Moss Cap x1"), "no corpse known, no from: {:?}", bv.log()[0]);
    assert_eq!(bv.eval::<bool>("eqoa.loot.get().source == nil"), true);
    // "Leave All": the window closes with items still on the corpse, and nothing is logged
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(bv.log().len(), 4, "Leave All adds nothing");
    // zoning keeps the session's log
    let before = bv.log();
    bv.play.bags_zone();
    bv.frames(2);
    bv.play.bags_set(&bv_world());
    bv.frames(2);
    assert_eq!(bv.log(), before, "the log survives zoning");
    // 51 pickups keep the newest 50 (the pack takes 200 stacks in this check)
    let trophies: Vec<serde_json::Value> = (1..=51).map(|n| serde_json::json!({"name": format!("Trophy {n}"), "id": 3000 + n, "key": 100 + n, "qty": 1})).collect();
    crate::inventory::with_caps([200, 40], || {
        bv.play.bags_corpse(&serde_json::Value::Array(trophies));
        bv.frames(2);
        for _ in 0..51 {
            bv.play.bags_loot_one(0);
            bv.frames(1);
        }
    });
    let log = bv.log();
    assert_eq!(log.len(), 50, "the newest 50");
    assert!(log[0].ends_with("Trophy 51 x1") && log[49].ends_with("Trophy 2 x1"), "{} .. {}", log[0], log[49]);
    bv.clean();
}

#[test]
fn round5b_a_closed_window_does_no_work_and_an_open_one_rebuilds_only_on_change() {
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    bv.bind_k();
    // closed: changes and frames cost nothing in the window
    bv.play.bags_tunar(5);
    bv.frames(30);
    assert_eq!(bv.eval::<(u32, u32)>("(function() local b = require('bags') return b.builds, b.draws end)()"), (0, 0), "closed: no grid read, no draw");
    // open: one read, then a draw per frame and no more reads
    bv.toggle();
    bv.frames(2);
    let (builds, draws) = bv.eval::<(u32, u32)>("(function() local b = require('bags') return b.builds, b.draws end)()");
    assert_eq!(builds, 1);
    bv.frames(30);
    let (builds2, draws2) = bv.eval::<(u32, u32)>("(function() local b = require('bags') return b.builds, b.draws end)()");
    assert_eq!((builds2, draws2 - draws), (1, 30), "an open window draws each frame and reads the bag once");
    // closed again: it stops drawing, and a change no longer reads anything
    bv.toggle();
    let after = bv.eval::<u32>("require('bags').draws");
    bv.play.bags_tunar(6);
    bv.frames(10);
    assert_eq!(bv.eval::<(u32, u32)>("(function() local b = require('bags') return b.builds, b.draws end)()"), (builds2, after));
    bv.clean();
}

#[test]
fn round5b_the_window_follows_the_ui_scale_and_stays_on_screen() {
    let mut sizes = Vec::new();
    for win in [(640, 480), (1024, 768), (1600, 900), (320, 240)] {
        let Some(mut bv) = Bv::sized(win) else { return };
        bv.enable();
        bv.open();
        let (x, y, w, h, k) = bv.rect();
        assert!(x >= 0. && y >= 0. && x + w <= win.0 as f64 + 0.5 && y + h <= win.1 as f64 + 0.5, "{win:?}: ({x}, {y}, {w}, {h}) is on screen");
        sizes.push((win, w, k));
        bv.clean();
    }
    // the window is as wide as the HUD frame makes it: 318 frame pixels at the frame's scale
    for (win, w, k) in &sizes {
        let (_, _, sx, _, _, _) = super::super::hud_layout::frame_metrics(*win);
        assert!((k - sx as f64).abs() < 1e-3 && (w - 318. * sx as f64).abs() < 1e-2, "{win:?}");
    }
    assert!(sizes[1].1 > sizes[0].1 * 1.5, "a larger window gives a larger Bags window");
}

#[test]
fn round5b_picture_the_window_is_drawn_where_it_says_and_closed_it_is_not() {
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    let closed = bv.frames(3);
    let closed_px = rm_pixels(&bv.gpu, &closed);
    bv.open();
    // a few pickups for the log
    bv.play.bags_corpse(&bv_corpse());
    bv.play.bags_loot_one(0);
    bv.play.bags_loot_one(0);
    bv.frames(3);
    // hover the sword: its card
    bv.point(bv.slot(1));
    let open = bv.frames(3);
    let px = rm_pixels(&bv.gpu, &open);
    let w = bv.win.0;
    let (rx, ry, rw, rh, k) = bv.rect();
    // inside the panel (away from slots and text) the surface is dark and not the background; closed it is background
    let probe = bv.at(282., 120.);
    assert_ne!(bv_px(&px, w, probe.0, probe.1), BV_BACKGROUND, "the panel covers its area");
    assert_eq!(bv_px(&closed_px, w, probe.0, probe.1), BV_BACKGROUND, "closed: the scene shows through");
    // outside the panel nothing changes
    for p in [(rx - 20., ry + rh / 2.), (rx + rw + 20., ry + rh / 2.), (rx + rw / 2., ry - 8.)] {
        assert_eq!(bv_px(&px, w, p.0, p.1), bv_px(&closed_px, w, p.0, p.1), "outside the panel at {p:?}");
    }
    // each slot's art is its icon's colour at the slot's centre: the potion's, the ring's and the pelt's
    for (n, icon) in [(2usize, 4002u32), (3, 4003), (7, 4007)] {
        let c = bv.slot(n);
        let pix = bv_px(&px, w, c.0, c.1);
        let want = bv_icon_color(icon);
        assert!((0..3).all(|i| (pix[i] as i32 - want[i] as i32).abs() <= 3), "slot {n} shows icon {icon}: {pix:?} vs {want:?}");
    }
    // the card: a red frame to the lower right of the pointer
    let ptr = bv.slot(1);
    assert!(bv_red_near(&px, w, (ptr.0 + 14. * k + 40., ptr.1 + 14. * k + 4.), 30), "the hover card's red frame");
    // the pointer away: the worn sword has its green border, the active tab its red underline, the log its lines
    bv.point((1., 1.));
    let calm = bv.frames(3);
    let calm_px = rm_pixels(&bv.gpu, &calm);
    let left = bv.at(8., 46. + 18.);
    assert!(bv_green_near(&calm_px, w, left, 3), "the worn marker's green at the sword's edge");
    let ring = bv.at(8. + 76., 46. + 18.);
    assert!(!bv_green_near(&calm_px, w, ring, 1), "an unworn ring's edge is not green");
    let under = bv.at(8. + 14., 40.);
    assert!(bv_red_near(&calm_px, w, under, 2), "the All tab's red underline");
    let log_row = bv.at(30., 46. + 188. + 5. + 38. + 6.);
    assert!(bv_near(&calm_px, w, log_row, 40, [232, 232, 232, 255], 30), "a log line's text");
    // the Bank tab, nothing hovered
    let tab = bv.tab(6);
    bv.click(tab);
    bv.point((1., 1.));
    bv.frames(3);
    // the log with entries, nothing hovered
    let all = bv.tab(1);
    bv.click(all);
    bv.point((1., 1.));
    bv.frames(3);
    bv.clean();
}

#[test]
fn round5b_every_stack_has_the_tabs_the_games_item_page_gives_it_and_each_tab_lists_exactly_those() {
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    bv.open();
    bv.play.bags_set(&serde_json::json!({
        "pack": [
            {"name":"Rusty Sword","id":900001,"key":11,"wear":12,"equip":0,"icon":4001},
            {"name":"Healing Potion","id":900002,"key":12,"qty":5,"max_stack":20,"p4c":2,"icon":4002},
            {"name":"Mana Potion","id":900003,"key":13,"qty":2,"max_stack":20,"p4c":7,"icon":4003},
            {"name":"Fire Wand","id":900004,"key":14,"p4c":3,"p54":5,"icon":4004},
            {"name":"Charged Ring","id":900005,"key":15,"wear":8,"p54":3,"icon":4005},
            {"name":"Tool Kit","id":900006,"key":16,"p4c":8,"p54":2,"icon":4006},
            {"name":"Rat Pelt","id":900007,"key":17,"qty":3,"max_stack":100,"icon":4007},
            {"name":"Odd Thing","id":900008,"key":18,"wear":0,"icon":4008},
            {"name":"Plain Bead","id":900009,"key":19,"icon":4009}
        ],
        "tunar": 10
    }));
    bv.frames(3);
    // every stack's `tabs` is the native predicates' answer for it, in the page's order
    let all: Vec<String> = bv.names();
    assert_eq!(all.len(), 9);
    for name in &all {
        let want: Vec<&str> = ["usable", "equipment", "charged", "misc"].into_iter().filter(|k| bv.play.bags_native_tab(k).contains(name)).collect();
        let got: String = bv.eval(&format!(
            "(function() for _, e in ipairs(require('bags').grid.items) do if e.item.name == '{name}' then return table.concat(e.item.tabs, ',') end end end)()"
        ));
        assert_eq!(got, want.join(","), "{name}");
    }
    // the fixture has every kind: a worn charged-looking ring is not Charged, a wear-slot-0 item is Equipment
    assert_eq!(bv.play.bags_native_tab("usable"), ["Healing Potion", "Mana Potion", "Tool Kit"]);
    assert_eq!(bv.play.bags_native_tab("charged"), ["Fire Wand"]);
    assert!(bv.play.bags_native_tab("equipment").contains(&"Charged Ring".to_string()) && bv.play.bags_native_tab("equipment").contains(&"Odd Thing".to_string()));
    assert_eq!(bv.play.bags_native_tab("misc"), ["Rat Pelt", "Plain Bead"]);
    // each of the mod's tabs lists exactly what the game's page lists under it
    for (n, key) in [(2, "usable"), (3, "equipment"), (4, "charged"), (5, "misc")] {
        let tab = bv.tab(n);
        bv.click(tab);
        bv.frames(2);
        assert_eq!(bv.names(), bv.play.bags_native_tab(key), "{key}");
    }
    let tab = bv.tab(1);
    bv.click(tab);
    bv.frames(2);
    assert_eq!(bv.names(), all);
    bv.clean();
}

/// The disc's own item art: the first decodable icons of ITEMICON.ESF, given to the live art cache the way the
/// viewer gives it (`update_live_data`). None without `$EQOA_ISO`.
fn bv_disc_icons(count: usize) -> Option<Vec<u32>> {
    let path = std::env::var("EQOA_ISO").ok().filter(|p| !p.is_empty())?;
    let mut disc = iso::Iso::open(&path).ok()?;
    let container = esf::Container::open(disc.read("ITEMICON.ESF").ok()?).ok()?;
    // icons with some picture in them, not a blank surface
    let ids: Vec<u32> = container
        .surfaces
        .iter()
        .filter_map(|s| {
            let d = s.decode(&container.data)?;
            let colours: std::collections::HashSet<[u8; 4]> = d.rgba.chunks(4).map(|p| [p[0], p[1], p[2], p[3]]).collect();
            (colours.len() > 24).then_some(s.dict_id as u32)
        })
        .take(count)
        .collect();
    let mut lib = esf::Library::new();
    lib.add("ITEMICON.ESF", container);
    super::super::icons::update_live_data(&lib, None, &ids);
    (ids.len() == count).then_some(ids)
}

#[test]
fn round5b_captures_for_the_owner_draw_the_discs_own_item_art_and_the_corpse_in_the_log() {
    let Some(mut bv) = Bv::new() else { return };
    // the real art: no stand-in provider, the live cache the disc fills
    super::super::icons::set_custom_icon_provider(None);
    let Some(ids) = bv_disc_icons(12) else {
        eprintln!("SKIP bag viewer captures: no $EQOA_ISO or no item art on it");
        return;
    };
    let mut world = bv_world();
    for (list, offset) in [("pack", 0), ("bank", 8)] {
        for (i, item) in world[list].as_array_mut().unwrap().iter_mut().enumerate() {
            item["icon"] = serde_json::json!(ids[offset + i]);
        }
    }
    bv.play.bags_set(&world);
    bv.enable();
    bv.open();
    let mut corpse = bv_corpse();
    for (i, item) in corpse.as_array_mut().unwrap().iter_mut().enumerate() {
        item["icon"] = serde_json::json!(ids[8 + i]);
    }
    bv.play.bags_target_corpse(5001, "a decaying skeleton");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&corpse);
    bv.frames(2);
    bv.play.bags_loot_one(0);
    bv.frames(2);
    bv.play.bags_loot_one(0);
    bv.frames(2);
    let (w, size) = (bv.win.0, bv.win);
    // every slot of the pack has real art: a picture's many colours, not a flat square
    for n in 1..=8 {
        let c = bv.slot(n);
        bv.point(c);
        let t = bv.frames(2);
        let px = rm_pixels(&bv.gpu, &t);
        let mut colours = std::collections::HashSet::new();
        for dy in -10..=10 {
            for dx in -10..=10 {
                colours.insert(bv_px(&px, w, c.0 + dx as f64, c.1 + dy as f64));
            }
        }
        assert!(colours.len() > 6, "slot {n} shows the disc's art: {} colours", colours.len());
        // the art fills the slot: no black strip of the surface's padding under it (a point near the slot's
        // bottom-left, clear of the count)
        let low = bv.at(8. + ((n - 1) % 8) as f64 * 38. + 6., 46. + ((n - 1) / 8) as f64 * 38. + 33.);
        let under = bv_px(&px, w, low.0, low.1);
        assert!(under[0] as u32 + under[1] as u32 + under[2] as u32 > 120, "slot {n}: black strip under the icon: {under:?}");
        if n == 1 {
            bv_png(&px, size, "r5-bags-all.png");
        }
    }
    let log = bv.log();
    assert!(log[0].ends_with("Spider Silk x4, from a decaying skeleton"), "{log:?}");
    bv.point((1., 1.));
    let tab = bv.tab(6);
    bv.click(tab);
    bv.point((1., 1.));
    let t = bv.frames(3);
    bv_png(&rm_pixels(&bv.gpu, &t), size, "r5-bags-bank.png");
    let all = bv.tab(1);
    bv.click(all);
    bv.point((1., 1.));
    let t = bv.frames(3);
    bv_png(&rm_pixels(&bv.gpu, &t), size, "r5-bags-loot.png");
    bv.clean();
}

fn bv_r6_png(pixels: &[u8], size: (u32, u32), name: &str) {
    if let Some(dir) = std::env::var_os("EQOA_R6_CAPTURE_DIR").or_else(|| std::env::var_os("EQOA_R5B_CAPTURE_DIR")) {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).unwrap();
        let mut enc = png::Encoder::new(fs::File::create(dir.join(name)).unwrap(), size.0, size.1);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(pixels).unwrap();
    }
}

impl Bv {
    fn menu_row(&self, needle: &str) -> Option<(f64, f64, bool, Option<String>, String)> {
        let (x, y, en, reason, label): (Option<f64>, Option<f64>, Option<bool>, Option<String>, Option<String>) = self.eval(&format!(
            "(function()
                for _, h in ipairs(require('bags').menu_hits or {{}}) do
                    if string.find(h.row.label, {needle:?}, 1, true) then
                        return h.x + h.w / 2, h.y + h.h / 2, h.row.enabled, h.row.reason, h.row.label
                    end
                end
                return nil, nil, nil, nil, nil
            end)()"
        ));
        Some((x?, y?, en?, reason, label?))
    }
    fn menu_labels(&self) -> Vec<String> {
        self.eval(
            "(function() local t = {} local m = require('bags').menu if m then for _, r in ipairs(m.rows) do t[#t + 1] = r.label end end return t end)()",
        )
    }
    fn result_text(&self) -> Option<(String, Option<bool>)> {
        let (text, ok): (Option<String>, Option<bool>) = self.eval(
            "(function() local r = require('bags').result if r then return r.text, r.ok end return nil, nil end)()",
        );
        text.map(|t| (t, ok))
    }
    fn action_hit(&self, needle: &str) -> Option<(f64, f64, String)> {
        let (x, y, label): (Option<f64>, Option<f64>, Option<String>) = self.eval(&format!(
            "(function()
                for _, h in ipairs(require('bags').action_hits or {{}}) do
                    if string.find(h.label, {needle:?}, 1, true) then
                        return h.x + h.w / 2, h.y + h.h / 2, h.label
                    end
                end
                return nil, nil, nil
            end)()"
        ));
        Some((x?, y?, label?))
    }
}

#[test]
fn round6b_bag_viewer_bags_menu_actions_greyed_reasons_no_destroy_and_no_unclicked_actions() {
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    // Give Healing Potion p4c=2 so quick slot 1 fits while quick slot 3 refuses cleanly.
    let mut world = bv_world();
    world["pack"][1]["p4c"] = serde_json::json!(2);
    bv.play.bags_set(&world);
    bv.enable();
    bv.open();

    // Nothing happens without clicking: idle frames send no packets and open no menu.
    let sends_before = bv.play.bags_sends_lost();
    bv.frames(5);
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_sends_lost(), sends_before, "idle frames perform zero actions");
    assert!(!bv.eval::<bool>("require('bags').menu ~= nil"));

    // Click slot 2 (Healing Potion): menu offers Use + Quick slot 1..4 and NEVER Destroy.
    let s2 = bv.slot(2);
    bv.click(s2);
    bv.frames(2);
    let labels = bv.menu_labels();
    assert!(labels.iter().any(|l| l == "Use"), "{labels:?}");
    assert!(labels.iter().any(|l| l == "Quick slot 1"), "{labels:?}");
    assert!(!labels.iter().any(|l| l.to_lowercase().contains("destroy")), "must have no Destroy entry: {labels:?}");

    // Click Use -> 1 action dispatched, result line shows ": ok".
    let (mx, my, enabled, _, _) = bv.menu_row("Use").unwrap();
    assert!(enabled);
    let sends_before = bv.play.bags_sends_lost();
    bv.click((mx, my));
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert!(bv.play.bags_sends_lost() > sends_before, "clicking Use sends one use_item action");
    let (res, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(true), "{res}");
    assert!(res.starts_with("Use:"), "{res}");

    // Click Healing Potion -> Quick slot 3 -> game refuses with exact reason on result line.
    bv.click(s2);
    bv.frames(2);
    let (qx, qy, qen, _, _) = bv.menu_row("Quick slot 3").unwrap();
    assert!(qen);
    bv.click((qx, qy));
    bv.play.bags_broker_tick();
    bv.frames(2);
    let (res, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false), "{res}");
    assert!(res.contains("quick slot 3"), "expected quick slot refusal reason, got {res}");

    // Click Mage Robe (slot 4, wrong class): Equip entry is greyed with game's reason; capture menu PNG.
    let s4 = bv.slot(4);
    bv.click(s4);
    let menu_tex = bv.frames(2);
    bv_r6_png(&rm_pixels(&bv.gpu, &menu_tex), bv.win, "r6-bags-menu.png");
    let (ex, ey, een, ereason, _) = bv.menu_row("Equip").unwrap();
    assert!(!een, "Mage Robe Equip must be greyed");
    assert_eq!(ereason.as_deref(), Some("wrong class"));
    let sends_before = bv.play.bags_sends_lost();
    bv.click((ex, ey));
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_sends_lost(), sends_before, "clicking disabled row queues nothing");
    let (res, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false));
    assert!(res.contains("wrong class"), "{res}");

    // Hero Helm (slot 6) requires level 55 (player is 50) -> greyed with requires level 55.
    let s6 = bv.slot(6);
    bv.click(s6);
    bv.frames(2);
    let (_, _, hen, hreason, _) = bv.menu_row("Equip").unwrap();
    assert!(!hen);
    assert_eq!(hreason.as_deref(), Some("requires level 55"));

    // Rusty Sword (slot 1, worn) -> Unequip is allowed & succeeds.
    let s1 = bv.slot(1);
    bv.click(s1);
    bv.frames(2);
    let (ux, uy, uen, _, ulabel) = bv.menu_row("Unequip").unwrap();
    assert!(uen && ulabel == "Unequip");
    let sends_before = bv.play.bags_sends_lost();
    bv.click((ux, uy));
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert!(bv.play.bags_sends_lost() > sends_before);
    assert_eq!(bv.result_text().unwrap().1, Some(true));

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6b_bag_viewer_merchant_bank_and_corpse_menus_and_captures() {
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.enable();
    bv.open();

    // Open Merchant (mode 10)
    bv.play.bags_open_merchant(888);
    bv.frames(2);

    // Equipped sword has Sell 1 greyed; Healing Potion has Sell 1 & Sell stack.
    let s1 = bv.slot(1);
    bv.click(s1);
    bv.frames(2);
    let (_, _, sw_en, sw_reason, _) = bv.menu_row("Sell 1").unwrap();
    assert!(!sw_en && sw_reason.as_deref() == Some("cannot sell an equipped item"));
    bv.key(winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape));
    bv.frames(1);

    let s2 = bv.slot(2);
    bv.click(s2);
    let merch_tex = bv.frames(2);
    bv_r6_png(&rm_pixels(&bv.gpu, &merch_tex), bv.win, "r6-bags-merchant.png");
    let labels = bv.menu_labels();
    assert!(labels.iter().any(|l| l.starts_with("Sell 1")), "{labels:?}");
    assert!(labels.iter().any(|l| l.starts_with("Sell stack (5")), "{labels:?}");
    assert!(!labels.iter().any(|l| l.to_lowercase().contains("destroy")), "{labels:?}");

    let (sx, sy, sen, _, _) = bv.menu_row("Sell stack").unwrap();
    assert!(sen);
    bv.click((sx, sy));
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert!(bv.play.bags_shop_pending(), "Sell stack dispatches merchant 0x004B");
    bv.play.bags_sell_confirm(1, 5);
    bv.frames(2);
    let (res, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(true), "{res}");
    assert!(res.starts_with("Sell stack (5"), "{res}");

    // Open Bank (mode 11) -> Deposit from bags & Withdraw on Bank tab
    bv.play.bags_close_merchant();
    bv.play.bags_open_bank();
    bv.frames(2);

    let pelt_idx: usize = bv.eval("(function() for i, e in ipairs(require('bags').grid.items) do if e.item.name == 'Rat Pelt' then return i end end end)()");
    let sp = bv.slot(pelt_idx);
    bv.click(sp);
    bv.frames(2);
    let (dx, dy, den, _, _) = bv.menu_row("Deposit stack (3)").unwrap();
    assert!(den);
    bv.click((dx, dy));
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_bank_state(), 2);
    bv.play.bags_bank_deposit_confirm(17, 900007, "Rat Pelt", 3);
    bv.frames(2);
    assert_eq!(bv.result_text().unwrap().1, Some(true));

    // Switch to Bank tab -> click Moonstone -> Withdraw
    let bank_tab = bv.tab(6);
    bv.click(bank_tab);
    bv.frames(2);
    let b1 = bv.slot(1);
    bv.click(b1);
    let bank_tex = bv.frames(2);
    bv_r6_png(&rm_pixels(&bv.gpu, &bank_tex), bv.win, "r6-bags-bank-menu.png");
    let (wx, wy, wen, _, _) = bv.menu_row("Withdraw 1").unwrap();
    assert!(wen);
    bv.click((wx, wy));
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_bank_state(), 2);
    bv.play.bags_bank_withdraw_confirm(21, 1);
    bv.frames(2);
    assert_eq!(bv.result_text().unwrap().1, Some(true));

    // Open Corpse (mode 1) -> Corpse tab appears -> Take / Take all
    bv.play.bags_target_corpse(5001, "a decaying skeleton");
    bv.play.bags_corpse(&bv_corpse());
    bv.play.bags_open_corpse_mode();
    bv.frames(2);
    let corpse_tab = bv.tab(7);
    bv.click(corpse_tab);
    bv.frames(2);
    let c2 = bv.slot(2);
    bv.click(c2);
    let corpse_tex = bv.frames(2);
    bv_r6_png(&rm_pixels(&bv.gpu, &corpse_tex), bv.win, "r6-bags-corpse.png");
    let labels = bv.menu_labels();
    assert!(labels.iter().any(|l| l.starts_with("Take")), "{labels:?}");
    assert!(labels.iter().any(|l| l == "Take all"), "{labels:?}");
    let (tx, ty, ten, _, _) = bv.menu_row("Take all").unwrap();
    assert!(ten);
    bv.click((tx, ty));
    bv.play.bags_broker_tick();
    bv.frames(2);
    bv.play.bags_loot_confirm_all();
    bv.frames(2);
    let (res, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(true), "{res}");
    assert!(res.starts_with("Take all:"), "{res}");

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6c_bag_viewer_junk_marks_persist_across_reload_grey_off_by_default_and_no_unclicked_actions() {
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    let mut world = bv_world();
    world["pack"][6]["value"] = serde_json::json!(15); // Rat Pelt (misc, grey item)
    world["pack"][7]["value"] = serde_json::json!(25); // Odd Thing
    bv.play.bags_set(&world);
    bv.enable();
    bv.open();

    // Grey-items rule must be OFF by default and zero actions run without click.
    assert!(!bv.eval::<bool>("require('bags').sell_grey"));
    let sends_before = bv.play.bags_sends_lost();
    bv.frames(4);
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_sends_lost(), sends_before);

    // Equipped item refuses Mark as junk.
    let s1 = bv.slot(1);
    bv.click(s1);
    bv.frames(2);
    let (_, _, en_worn, reason_worn, _) = bv.menu_row("Mark as junk").unwrap();
    assert!(!en_worn);
    assert!(reason_worn.as_deref().unwrap_or("").contains("equipped"), "{reason_worn:?}");
    bv.key(winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape));
    bv.frames(1);

    // Click slot 8 (Odd Thing) -> Mark as junk -> shows "Marked ..." and menu flips to Unmark.
    let s8 = bv.slot(8);
    bv.click(s8);
    bv.frames(2);
    let (mx, my, men, _, _) = bv.menu_row("Mark as junk").unwrap();
    assert!(men);
    bv.click((mx, my));
    bv.frames(2);
    assert!(bv.eval::<bool>("require('bags').junk_ids['900008'] == true"));
    assert!(!bv.eval::<bool>("require('bags').is_junk(require('bags').grid.items[7].item)"), "grey item stays non-junk while grey rule off");
    bv.click(s8);
    bv.frames(2);
    assert!(bv.menu_row("Unmark").is_some(), "menu switches to Unmark once marked");
    bv.key(winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape));
    bv.frames(1);

    // Toggle "Also sell grey items" ON -> Rat Pelt (grey misc) gains junk badge too.
    let (gx, gy, glbl) = bv.action_hit("Also sell grey items").unwrap();
    assert_eq!(glbl, "Also sell grey items");
    bv.click((gx, gy));
    let badge_tex = bv.frames(2);
    bv_r6_png(&rm_pixels(&bv.gpu, &badge_tex), bv.win, "r6-junk-badges.png");
    assert!(bv.eval::<bool>("require('bags').sell_grey == true"));
    assert!(bv.eval::<bool>("require('bags').is_junk(require('bags').grid.items[7].item)"), "Rat Pelt matches grey rule when ON");

    // Toggle back OFF before reload test -> disables & re-enables addon -> marks survive reload!
    let (gx, gy, _) = bv.action_hit("Also sell grey items").unwrap();
    bv.click((gx, gy));
    bv.frames(2);
    assert!(!bv.eval::<bool>("require('bags').sell_grey"));

    bv.manager.queue(Command::Enable(BV.into(), false));
    bv.frames(2);
    bv.manager.queue(Command::Enable(BV.into(), true));
    bv.frames(3);
    // Give the player a new stack with the same item id 900008 -> mark applies by item type.
    bv.play.bags_added(&serde_json::json!({"name":"Odd Thing Copy","id":900008,"key":99,"wear":0,"icon":4008}));
    bv.open();
    assert!(!bv.eval::<bool>("require('bags').sell_grey"), "grey=false survived reload");
    assert!(bv.eval::<bool>("require('bags').junk_ids['900008'] == true"), "junk id 900008 survived reload");
    assert!(bv.eval::<bool>("(function() local ok=true for _,e in ipairs(require('bags').grid.items) do if e.item.id==900008 and not require('bags').is_junk(e.item) then ok=false end end return ok end)()"));

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6c_bag_viewer_merchant_sell_junk_batch_stop_close_zone_and_corpse_loot() {
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    let world = serde_json::json!({
        "pack": [
            {"name":"Rusty Sword","id":900001,"key":11,"wear":12,"equip":0,"value":100,"icon":4001,"stats":[[0,3]]},
            {"name":"Rat Pelt","id":900007,"key":17,"qty":2,"max_stack":100,"value":10,"icon":4007},
            {"name":"Chipped Tooth","id":900010,"key":20,"qty":1,"max_stack":20,"value":20,"icon":4008},
            {"name":"Torn Cloth","id":900011,"key":21,"qty":1,"max_stack":20,"value":40,"icon":4003},
            {"name":"Cracked Fang","id":900012,"key":22,"qty":1,"max_stack":20,"value":30,"icon":4004}
        ],
        "bank": [],
        "tunar": 100,
        "bank_tunar": 0
    });
    bv.play.bags_set(&world);
    bv.enable();
    bv.open();

    // Mark Chipped Tooth, Torn Cloth, and Cracked Fang as junk (leave Rat Pelt unmarked grey).
    for slot in [3usize, 4, 5] {
        let sp = bv.slot(slot);
        bv.click(sp);
        bv.frames(2);
        let (mx, my, en, _, _) = bv.menu_row("Mark as junk").unwrap();
        assert!(en);
        bv.click((mx, my));
        bv.frames(1);
    }

    // Open merchant: grey OFF -> shows 3 items; turn grey ON -> shows 4 items with native quote.
    bv.play.bags_open_merchant(44);
    bv.frames(2);
    let (_, _, lbl_off) = bv.action_hit("Sell junk (").unwrap();
    let quote_3: u64 = bv.eval("select(3, require('bags').junk_sell_list())");
    assert!(lbl_off.starts_with("Sell junk (3 items, "), "{lbl_off}");
    assert!(quote_3 > 0, "native quote must be positive");

    // Idle frames perform zero sells even with merchant open.
    let sends_idle = bv.play.bags_sends_lost();
    bv.frames(4);
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_sends_lost(), sends_idle);

    // Enable grey items -> count includes Rat Pelt (4 items) -> capture merchant PNG.
    let (gx, gy, _) = bv.action_hit("Also sell grey items").unwrap();
    bv.click((gx, gy));
    let merch_tex = bv.frames(2);
    bv_r6_png(&rm_pixels(&bv.gpu, &merch_tex), bv.win, "r6-junk-merchant.png");
    let (bx, by, lbl_on) = bv.action_hit("Sell junk (").unwrap();
    assert!(lbl_on.starts_with("Sell junk (4 items, "), "{lbl_on}");

    // One click starts ONE batch; only step 1 dispatches while waiting for merchant reply.
    bv.click((bx, by));
    bv.frames(1);
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert!(bv.play.bags_shop_pending(), "step 1 dispatched");
    let sends_after_s1 = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    bv.frames(1);
    assert_eq!(bv.play.bags_sends_lost(), sends_after_s1, "step 2 waits for step 1 server reply");

    // Server confirms step 1 (Rat Pelt at inv slot 1) -> progress shows 1 of 4 -> click Stop!
    bv.play.bags_sell_confirm(1, 2);
    bv.frames(2);
    let (sx, sy, slbl) = bv.action_hit("Stop").unwrap();
    assert_eq!(slbl, "Stop");
    bv.click((sx, sy));
    bv.play.bags_broker_tick();
    let stopped_tex = bv.frames(2);
    bv_r6_png(&rm_pixels(&bv.gpu, &stopped_tex), bv.win, "r6-junk-stopped.png");
    let (res, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false));
    assert_eq!(res, "Sold 1 of 4: stopped");

    // Start batch for remaining 3 junk items, confirm step 1, then close merchant -> stops cleanly!
    let (bx, by, lbl2) = bv.action_hit("Sell junk (").unwrap();
    assert!(lbl2.starts_with("Sell junk (3 items, "), "{lbl2}");
    bv.click((bx, by));
    bv.play.bags_broker_tick();
    bv.frames(2);
    bv.play.bags_sell_confirm(1, 1);
    bv.frames(2);
    bv.play.bags_close_merchant();
    bv.play.bags_broker_tick();
    bv.frames(2);
    let (res, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false));
    assert_eq!(res, "Sold 1 of 3: merchant closed");

    // Remaining 2 junk items: start batch, confirm step 1, then zone -> stops cleanly without replay!
    bv.play.bags_open_merchant(44);
    bv.frames(2);
    let (bx, by, lbl3) = bv.action_hit("Sell junk (").unwrap();
    assert!(lbl3.starts_with("Sell junk (2 items, "), "{lbl3}");
    bv.click((bx, by));
    bv.play.bags_broker_tick();
    bv.frames(2);
    bv.play.bags_sell_confirm(1, 1);
    bv.frames(2);
    bv.play.bags_zone();
    bv.play.bags_broker_tick();
    bv.frames(2);
    let (res, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false));
    assert!(res.starts_with("Sold 1 of 2: zone"), "{res}");

    // Turn grey back OFF for corpse test: Rat Pelt id=2001 will be marked explicitly as junk while Bone Chip & Spider Silk stay non-junk.
    let (gx, gy, _) = bv.action_hit("Also sell grey items").unwrap();
    bv.click((gx, gy));
    bv.frames(1);

    // Corpse open: mark Rat Pelt (id 2001) as junk, switch to Corpse tab, capture r6-junk-corpse.png.
    bv.play.bags_target_corpse(5001, "a decaying skeleton");
    bv.play.bags_corpse(&bv_corpse());
    bv.play.bags_open_corpse_mode();
    bv.frames(2);
    let corpse_tab = bv.tab(7);
    bv.click(corpse_tab);
    bv.frames(2);
    let c1 = bv.slot(1); // Rat Pelt id 2001 on corpse
    bv.click(c1);
    bv.frames(2);
    let (mx, my, en, _, _) = bv.menu_row("Mark as junk").unwrap();
    assert!(en);
    bv.click((mx, my));
    let corpse_tex = bv.frames(2);
    bv_r6_png(&rm_pixels(&bv.gpu, &corpse_tex), bv.win, "r6-junk-corpse.png");

    // Fill inventory to 39/40 before "Take all but junk" -> loots Spider Silk, stops on Bone Chip with "inventory is full"!
    bv.play.bags_fill_inventory_to(39);
    bv.frames(2);
    let (lx, ly, llbl) = bv.action_hit("Take all but junk").unwrap();
    assert_eq!(llbl, "Take all but junk");
    bv.click((lx, ly));
    bv.play.bags_broker_tick();
    bv.frames(2);
    bv.play.bags_loot_confirm_one(1, Some(&serde_json::json!({"name":"Spider Silk","id":2002,"key":32,"qty":4,"max_stack":100})));
    bv.frames(2);
    bv.play.bags_broker_tick();
    bv.frames(2);
    let (res, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false));
    assert_eq!(res, "Looted 1 of 2: inventory is full");
    assert!(bv.eval::<bool>("(function() local l=eqoa.loot.get() return l and l.items[1].name=='Rat Pelt' end)()"), "junk Rat Pelt stayed on corpse");

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

fn bv_test_surface_container(surfaces: &[(i32, [u8; 4])]) -> esf::Container {
    let mut rec = Vec::new();
    for &(dict_id, rgba) in surfaces {
        let (w, h, pal_entries) = (16i32, 16i32, 256);
        let mut body = Vec::new();
        body.extend_from_slice(&dict_id.to_le_bytes());
        body.extend_from_slice(&w.to_le_bytes());
        body.extend_from_slice(&h.to_le_bytes());
        body.extend_from_slice(&1i32.to_le_bytes());
        body.extend_from_slice(&1i32.to_le_bytes());
        body.extend_from_slice(&(pal_entries as i32).to_le_bytes());
        let mut pal = vec![0u8; pal_entries * 4];
        pal[4] = rgba[0];
        pal[5] = rgba[1];
        pal[6] = rgba[2];
        pal[7] = rgba[3];
        body.extend_from_slice(&pal);
        body.extend_from_slice(&w.to_le_bytes());
        let pix = vec![1u8; (w * h) as usize];
        body.extend_from_slice(&pix);

        rec.extend_from_slice(&0x1000u16.to_le_bytes());
        rec.extend_from_slice(&0i16.to_le_bytes());
        rec.extend_from_slice(&(body.len() as i32).to_le_bytes());
        rec.extend_from_slice(&0u32.to_le_bytes());
        rec.extend_from_slice(&body);
    }
    esf::Container::parse_records(rec)
}

#[test]
fn round6_item4_lazy_icons_load_without_native_windows_and_zero_per_frame_requests() {
    let Some(mut bv) = Bv::new() else { return };
    super::super::icons::set_custom_icon_provider(None);
    super::super::icons::reset_test_counters();

    let pos_id: i32 = 40011;
    let neg_id: i32 = -40012;
    let bank_id: i32 = 40013;
    let unresolvable_id: u32 = 999_999;
    let mut lib = esf::Library::new();
    lib.add(
        "ITEMICON.ESF",
        bv_test_surface_container(&[
            (pos_id, [220, 40, 40, 255]),
            (neg_id, [40, 180, 220, 255]),
            (bank_id, [90, 220, 90, 255]),
        ]),
    );
    // Prime live library WITHOUT prefetching pos_id/neg_id/bank_id (mirrors bank, corpse, or unopened UI).
    super::super::icons::update_live_data(&lib, None, &[]);

    let world = serde_json::json!({
        "pack": [
            {"name":"Red Sword","id":900001,"key":11,"wear":12,"icon":pos_id},
            {"name":"Blue Orb","id":900002,"key":12,"qty":2,"max_stack":20,"icon":neg_id},
            {"name":"Ghost Charm","id":900003,"key":13,"qty":1,"icon":unresolvable_id}
        ],
        "bank": [
            {"name":"Bank Gem","id":900101,"key":21,"qty":1,"icon":bank_id}
        ],
        "tunar": 100,
        "bank_tunar": 0
    });
    bv.play.bags_set(&world);
    bv.enable();
    bv.open();

    // Signed negative 32-bit dict_id must not be dropped to nil by items_capture.
    let neg_icon: Option<u32> = bv.eval("require('bags').grid.items[2].item.icon");
    assert_eq!(neg_icon, Some(neg_id as u32), "negative signed dict_id must reach Lua as u32");

    // Next Play::frame runs frame_library -> update_live_data, decoding queued lookups into SAFE_LIBRARY.
    super::super::icons::update_live_data(&lib, None, &[]);
    bv.frames(1);

    let (t1, t2, t3, r1, r2, pend): (bool, bool, bool, String, String, bool) = bv.eval(
        "require('bags').grid.items[1].texture ~= nil, require('bags').grid.items[2].texture ~= nil, require('bags').grid.items[3].texture ~= nil, tostring(require('bags').icon_req[40011]), tostring(require('bags').icon_req[4294927284]), require('bags').has_pending_icons",
    );
    assert!(
        t1 && t2,
        "lazy icons must resolve once SAFE_LIBRARY loads without opening native windows (t1={t1}, t2={t2}, r1={r1}, r2={r2}, pend={pend}, snap1={}, snap2={}, res_cnt={}, lkp_cnt={})",
        super::super::icons::snapshot_has_icon(pos_id as u32),
        super::super::icons::snapshot_has_icon(neg_id as u32),
        super::super::icons::resource_call_count(),
        super::super::icons::lookup_attempt_count(),
    );
    assert!(!t3, "negative control: unresolvable icon stays nil and uses fallback letter");

    // Steady-state frames must issue ZERO resource requests, ZERO ESF lookups, and avoid per-frame allocations.
    let res_before = super::super::icons::resource_call_count();
    let lkp_before = super::super::icons::lookup_attempt_count();
    bv.lua().gc_collect().unwrap();
    let bytes_before = bv.lua().used_memory();
    bv.frames(10);
    let res_after = super::super::icons::resource_call_count();
    let lkp_after = super::super::icons::lookup_attempt_count();
    bv.lua().gc_collect().unwrap();
    let bytes_after = bv.lua().used_memory();
    assert_eq!(res_after, res_before, "steady frames must not re-request eqoa.icons.resource");
    assert_eq!(lkp_after, lkp_before, "steady frames must not trigger ESF surface lookups");
    assert!(bytes_after.saturating_sub(bytes_before) <= 1024, "steady frames must not leak Lua tables (delta={} bytes)", bytes_after.saturating_sub(bytes_before));

    // Bank tab loads its own non-prefetched icon without ever opening native bank UI.
    bv.click(bv.tab(6));
    bv.frames(1);
    super::super::icons::update_live_data(&lib, None, &[]);
    bv.frames(1);
    let bank_loaded: bool = bv.eval("require('bags').grid.items[1].texture ~= nil");
    assert!(bank_loaded, "bank icon must load lazily without opening native bank window");
    let bank_res_before = super::super::icons::resource_call_count();
    bv.frames(5);
    assert_eq!(super::super::icons::resource_call_count(), bank_res_before, "bank tab steady frames must not re-request icons");

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6_item1_stale_corpse_grid_and_loot_batch_and_log() {
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_target_corpse(5001, "a decaying skeleton");
    bv.play.bags_corpse(&bv_corpse());
    bv.enable();
    bv.open();

    // Select Corpse tab (3 items: Rat Pelt, Spider Silk, Bone Chip).
    bv.click(bv.tab(7));
    bv.frames(1);
    assert_eq!(bv.names(), vec!["Rat Pelt", "Spider Silk", "Bone Chip"]);

    // Item 1 check: M.grid tracks container revision & draw() invalidates stale grids even if B.dirty was bypassed.
    let stale_rev_before: Option<u64> = bv.eval("require('bags').grid.rev");
    assert!(stale_rev_before.is_some(), "M.grid must track container revision to detect staleness");

    bv.play.bags_corpse(&serde_json::json!([]));
    bv.frames(1);
    assert_eq!(bv.names(), Vec::<String>::new(), "empty corpse must show 0 items, never stale slots");
    let (_, _, take_lbl) = bv.action_hit("Take all but junk").unwrap();
    bv.click(bv.action_hit(&take_lbl).map(|(x, y, _)| (x, y)).unwrap());
    bv.frames(1);
    assert_eq!(
        bv.eval::<(String, Option<bool>)>("require('bags').result.text, require('bags').result.ok"),
        ("Corpse has no items".into(), Some(false)),
        "negative control: genuine 0/40 corpse says Corpse has no items"
    );

    // Re-open 1-item corpse; mark Rat Pelt as junk -> negative control on junk-only, then multi-item batch + log.
    bv.play.bags_target_corpse(5001, "a decaying skeleton");
    bv.play.bags_corpse(&serde_json::json!([bv_corpse()[0].clone()]));
    bv.frames(1);
    bv.click(bv.tab(7));
    bv.frames(1);
    bv.click(bv.slot(1));
    bv.frames(1);
    let (jx, jy, _, _, _) = bv.menu_row("Mark as junk").unwrap();
    bv.click((jx, jy));
    bv.frames(1);
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.frames(1);
    assert_eq!(
        bv.eval::<(String, Option<bool>)>("require('bags').result.text, require('bags').result.ok"),
        ("Corpse has only junk items".into(), Some(false)),
        "negative control: junk-only corpse stops before dispatching batch"
    );

    // Full 3-item corpse: Rat Pelt (junk), Spider Silk, Bone Chip -> ONE click must take BOTH non-junk items and log both.
    bv.play.bags_target_corpse(5001, "a decaying skeleton");
    bv.play.bags_corpse(&bv_corpse());
    bv.frames(1);
    bv.click(bv.tab(7));
    bv.frames(1);
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick();
    bv.frames(2);
    bv.play.bags_loot_confirm_one(
        1,
        Some(&serde_json::json!({"name":"Spider Silk","id":2002,"key":32,"qty":4,"max_stack":100})),
    );
    bv.play.bags_broker_tick();
    bv.frames(2);
    bv.play.bags_loot_confirm_one(
        1,
        Some(&serde_json::json!({"name":"Bone Chip","id":2003,"key":33,"qty":2,"max_stack":100})),
    );
    bv.play.bags_broker_tick();
    bv.frames(2);

    assert_eq!(
        bv.eval::<(String, Option<bool>)>("require('bags').result.text, require('bags').result.ok"),
        ("Looted 2 of 2".into(), Some(true)),
        "one click of Take all but junk must finish both non-junk steps sequentially"
    );
    assert_eq!(bv.names(), vec!["Rat Pelt"], "junk item must remain on corpse");
    let log = bv.log();
    assert_eq!(log.len(), 2, "loot log must record both looted items: {log:?}");
    assert!(log[0].contains("Bone Chip x2, from a decaying skeleton"), "{log:?}");
    assert!(log[1].contains("Spider Silk x4, from a decaying skeleton"), "{log:?}");

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6_item5_window_fits_hud_and_scrolls_100_slot_pack() {
    use winit::event::{DeviceId, MouseScrollDelta, TouchPhase, WindowEvent};
    let Some(mut bv) = Bv::new() else { return };
    crate::inventory::with_caps([100, 40], || {
        let pack100: Vec<serde_json::Value> = (1..=100)
            .map(|n| {
                serde_json::json!({
                    "name": format!("Pack Item {n}"),
                    "id": 920000 + n,
                    "key": 200 + n,
                    "qty": 1,
                    "max_stack": 1,
                    "wear": 1,
                    "icon": 4001 + (n % 8)
                })
            })
            .collect();
        let world = serde_json::json!({
            "pack": pack100,
            "bank": [],
            "tunar": 500,
            "bank_tunar": 0
        });
        bv.play.bags_set(&world);
        bv.play.bags_open_merchant(44);
        bv.enable();
        bv.open();

        // 1. Window + footer must fit entirely within HUD frame (<= 448) and display height.
        let (rows, total_rows, cap): (u32, u32, Option<u32>) =
            bv.eval("require('bags').grid.rows, require('bags').grid.total_rows, require('bags').grid.capacity");
        assert_eq!((rows, total_rows, cap), (6, 13, Some(100)));

        let (_x, y, _w, h, k) = bv.rect();
        assert!(y >= 0.0 && y + h <= bv.win.1 as f64 + 0.5, "window must stay inside display: y={y}, h={h}, dh={}", bv.win.1);
        assert!(h / k <= 448.0, "window frame height {} exceeds HUD frame 448", h / k);

        let all_btns_visible: bool = bv.eval(&format!(
            "(function() local h = require('bags').action_hits if #h == 0 then return false end for _, r in ipairs(h) do if r.y < 0 or r.y + r.h > {} + 0.5 then return false end end return true end)()",
            bv.win.1
        ));
        assert!(all_btns_visible, "all footer buttons must remain inside display bounds");

        let tex_count: usize = bv.eval(
            "(function() local n = 0 for _, e in ipairs(require('bags').grid.items) do if e.texture then n = n + 1 end end return n end)()",
        );
        assert!(tex_count <= 48, "only visible grid slots should hold live textures ({tex_count} <= 48)");

        // 2. Scroll grid to bottom with mouse wheel over grid slots -> reaches Pack Item 100 (row 13, col 4 -> viewport slot 44).
        bv.point(bv.slot(1));
        for _ in 0..8 {
            super::super::pointer_api::observe(
                &WindowEvent::MouseWheel {
                    device_id: DeviceId::dummy(),
                    delta: MouseScrollDelta::LineDelta(0.0, -1.0),
                    phase: TouchPhase::Moved,
                },
                bv.win,
            );
            bv.frame();
        }
        assert_eq!(bv.eval::<u32>("require('bags').grid_scroll"), 7, "should clamp scroll at bottom row");
        bv.click(bv.slot(44));
        bv.frames(1);
        assert_eq!(
            bv.eval::<String>("require('bags').menu and require('bags').menu.title or ''"),
            "Pack Item 100",
            "clicking scrolled bottom slot must target Pack Item 100"
        );
        let tex_count_scrolled: usize = bv.eval(
            "(function() local n = 0 for _, e in ipairs(require('bags').grid.items) do if e.texture then n = n + 1 end end return n end)()",
        );
        assert!(tex_count_scrolled <= 48, "scrolled grid must still keep live textures <= 48 ({tex_count_scrolled})");
    });

    // Negative control: 40-slot pack keeps 5 rows and wheel over grid does not scroll.
    crate::inventory::with_caps([40, 40], || {
        bv.play.bags_set(&bv_world());
        bv.frames(2);
        let (rows40, total40): (u32, u32) = bv.eval("require('bags').grid.rows, require('bags').grid.total_rows");
        assert_eq!((rows40, total40), (5, 5));
        bv.point(bv.slot(1));
        super::super::pointer_api::observe(
            &WindowEvent::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: MouseScrollDelta::LineDelta(0.0, -1.0),
                phase: TouchPhase::Moved,
            },
            bv.win,
        );
        bv.frame();
        assert_eq!(bv.eval::<u32>("require('bags').grid_scroll"), 0);
    });

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6_item6_take_all_button_on_corpse_tab() {
    let Some(mut bv) = Bv::new() else { return };
    bv.play.session.conn = Some(usize::MAX);
    bv.play.bags_target_corpse(5001, "a decaying skeleton");
    bv.play.bags_corpse(&bv_corpse());
    bv.enable();
    bv.open();

    // Mark Rat Pelt as junk so both Take all and Take all but junk are active side by side.
    bv.click(bv.tab(7));
    bv.frames(1);
    bv.click(bv.slot(1));
    bv.frames(1);
    let (jx, jy, _, _, _) = bv.menu_row("Mark as junk").unwrap();
    bv.click((jx, jy));
    bv.frames(1);

    let (tax, tay, talbl) = bv.action_hit("Take all").unwrap();
    assert_eq!(talbl, "Take all");
    let (_, _, tnjlbl) = bv.action_hit("Take all but junk").unwrap();
    assert_eq!(tnjlbl, "Take all but junk");

    // Clicking "Take all" dispatches one LootAll (0x001A) and takes everything including junk.
    let sends_before = bv.play.session.sends_lost;
    bv.click((tax, tay));
    bv.play.bags_broker_tick();
    bv.frames(1);
    assert!(
        bv.play.session.sends_lost > sends_before,
        "Take all button must send 0x001A LootAll"
    );
    bv.play.bags_loot_confirm_all();
    bv.play.bags_broker_tick();
    bv.frames(2);

    assert_eq!(
        bv.eval::<(String, Option<bool>)>("require('bags').result.text, require('bags').result.ok"),
        ("Take all: ok".into(), Some(true))
    );
    assert_eq!(bv.names(), Vec::<String>::new(), "Take all must take junk items too");
    assert_eq!(bv.log().len(), 3, "Take all logs all 3 corpse items");

    // Negative control: clicking Take all on empty 0/40 corpse shows refusal and sends nothing.
    let sends_empty = bv.play.session.sends_lost;
    let (tax2, tay2, _) = bv.action_hit("Take all").unwrap();
    bv.click((tax2, tay2));
    bv.play.bags_broker_tick();
    bv.frames(1);
    assert_eq!(
        bv.eval::<(String, Option<bool>)>("require('bags').result.text, require('bags').result.ok"),
        ("Corpse has no items".into(), Some(false))
    );
    assert_eq!(bv.play.session.sends_lost, sends_empty, "empty corpse Take all must send no packet");

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}
