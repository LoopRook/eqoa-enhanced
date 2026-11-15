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
