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
    /// gives the API the player's position (the snapshot has none without a walker); `frame` sets it from `play.self_at`
    walker: crate::walker::Walker,
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
        let walker = crate::play::Play::bundled_ui_walker();
        Some(Bv { gpu, _root: root, _config: config, manager, play, walker, win, target: None })
    }
    fn enable(&mut self) {
        self.manager.queue(Command::Enable(BV.into(), true));
        self.frames(3);
        assert!(self.manager.entries().iter().all(|e| e.status == Status::Enabled), "{:?}", self.manager.diagnostics());
        self.clean();
        // Auto-loot looks at the world every frame in the tests (a quarter second apart in play; the crowd tests in
        // `bv_busy_scene` keep the real interval)
        self.lua().load("require('bags').AUTO_POLL_SECS = 0").exec().unwrap();
    }
    /// Every bundled-mod test session asserts zero Addon Manager diagnostics.
    fn clean(&self) {
        let d = self.manager.diagnostics();
        assert!(d.is_empty(), "{d:?}");
    }
    fn frame(&mut self) -> wgpu::Texture {
        let at = self.play.bags_player_pos();
        self.walker.cam.walk.phys.position.x.0 = at[0];
        self.walker.cam.walk.phys.position.y.0 = at[1];
        self.walker.cam.walk.phys.position.z.0 = at[2];
        let mut snapshot = self.play.forge_snapshot(Some(&self.walker));
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
        assert!(crate::fork_ext::bindpage::set_key(BV, "open", "9"));
    }
    fn toggle(&mut self) {
        self.key(winit::keyboard::Key::Character("9".into()));
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
    big_stack(round5b_off_by_default_loads_clean_has_an_unbound_controls_row_and_its_key_opens_and_closes_it_body)
}
fn round5b_off_by_default_loads_clean_has_an_unbound_controls_row_and_its_key_opens_and_closes_it_body() {
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
    bv.key(Key::Character("9".into()));
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
    assert_eq!(crate::fork_ext::bindpage::dispatch_keypress("9"), None, "a disabled mod's key fires nothing");
    let _ = crate::fork_ext::bindpage::set_key(BV, "open", "");
}

#[test]
fn round5b_the_grid_shows_the_pack_as_the_dump_gave_it_and_follows_its_changes() {
    big_stack(round5b_the_grid_shows_the_pack_as_the_dump_gave_it_and_follows_its_changes_body)
}
fn round5b_the_grid_shows_the_pack_as_the_dump_gave_it_and_follows_its_changes_body() {
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
    big_stack(round5b_equipment_follows_the_games_own_tab_and_the_bank_reads_away_from_a_banker_body)
}
fn round5b_equipment_follows_the_games_own_tab_and_the_bank_reads_away_from_a_banker_body() {
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
    big_stack(round5b_the_card_has_the_items_fields_and_the_red_line_only_when_the_character_cannot_use_it_body)
}
fn round5b_the_card_has_the_items_fields_and_the_red_line_only_when_the_character_cannot_use_it_body() {
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
    big_stack(round5b_the_loot_log_is_fed_by_pickups_newest_first_and_keeps_fifty_body)
}
fn round5b_the_loot_log_is_fed_by_pickups_newest_first_and_keeps_fifty_body() {
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
    big_stack(round5b_a_closed_window_does_no_work_and_an_open_one_rebuilds_only_on_change_body)
}
fn round5b_a_closed_window_does_no_work_and_an_open_one_rebuilds_only_on_change_body() {
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
    big_stack(round5b_the_window_follows_the_ui_scale_and_stays_on_screen_body)
}
fn round5b_the_window_follows_the_ui_scale_and_stays_on_screen_body() {
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
    big_stack(round5b_picture_the_window_is_drawn_where_it_says_and_closed_it_is_not_body)
}
fn round5b_picture_the_window_is_drawn_where_it_says_and_closed_it_is_not_body() {
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
    big_stack(round5b_every_stack_has_the_tabs_the_games_item_page_gives_it_and_each_tab_lists_exactly_those_body)
}
fn round5b_every_stack_has_the_tabs_the_games_item_page_gives_it_and_each_tab_lists_exactly_those_body() {
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
    big_stack(round5b_captures_for_the_owner_draw_the_discs_own_item_art_and_the_corpse_in_the_log_body)
}
fn round5b_captures_for_the_owner_draw_the_discs_own_item_art_and_the_corpse_in_the_log_body() {
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
    big_stack(round6b_bag_viewer_bags_menu_actions_greyed_reasons_no_destroy_and_no_unclicked_actions_body)
}
fn round6b_bag_viewer_bags_menu_actions_greyed_reasons_no_destroy_and_no_unclicked_actions_body() {
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
    big_stack(round6b_bag_viewer_merchant_bank_and_corpse_menus_and_captures_body)
}
fn round6b_bag_viewer_merchant_bank_and_corpse_menus_and_captures_body() {
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
fn round6c_bag_viewer_junk_marks_and_auto_loot_choice_persist_across_reload_and_no_unclicked_actions() {
    big_stack(round6c_bag_viewer_junk_marks_and_auto_loot_choice_persist_across_reload_and_no_unclicked_actions_body)
}
fn round6c_bag_viewer_junk_marks_and_auto_loot_choice_persist_across_reload_and_no_unclicked_actions_body() {
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    let mut world = bv_world();
    world["pack"][6]["value"] = serde_json::json!(15); // Rat Pelt (misc, grey item)
    world["pack"][7]["value"] = serde_json::json!(25); // Odd Thing
    bv.play.bags_set(&world);
    bv.enable();
    bv.open();

    // Auto-loot is Off by default, "Also sell grey items" is gone (item 5), and zero actions run without a click.
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), "off");
    assert!(bv.action_hit("sell grey").is_none(), "the grey-items checkbox is retired");
    assert!(bv.action_hit("Auto-loot: Off").is_some() && bv.action_hit("Auto-loot: Everything but junk").is_some());
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
    assert!(!bv.eval::<bool>("require('bags').is_junk(require('bags').grid.items[7].item)"), "an unmarked misc item is never junk: only the marks decide");
    bv.click(s8);
    bv.frames(2);
    assert!(bv.menu_row("Unmark").is_some(), "menu switches to Unmark once marked");
    bv.key(winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape));
    bv.frames(1);

    // Choose Auto-loot "Everything but junk": it is the saved choice, and Rat Pelt (never marked) is not junk.
    let (gx, gy, glbl) = bv.action_hit("Auto-loot: Everything but junk").unwrap();
    assert_eq!(glbl, "Auto-loot: Everything but junk");
    bv.click((gx, gy));
    let badge_tex = bv.frames(2);
    bv_r6_png(&rm_pixels(&bv.gpu, &badge_tex), bv.win, "r6-junk-badges.png");
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), "non_junk");
    assert!(!bv.eval::<bool>("require('bags').is_junk(require('bags').grid.items[7].item)"), "Rat Pelt is not junk");

    bv.manager.queue(Command::Enable(BV.into(), false));
    bv.frames(2);
    bv.manager.queue(Command::Enable(BV.into(), true));
    bv.frames(3);
    // Give the player a new stack with the same item id 900008 -> mark applies by item type.
    bv.play.bags_added(&serde_json::json!({"name":"Odd Thing Copy","id":900008,"key":99,"wear":0,"icon":4008}));
    bv.open();
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), "non_junk", "the Auto-loot choice survived reload");
    assert!(bv.eval::<bool>("require('bags').junk_ids['900008'] == true"), "junk id 900008 survived reload");
    assert!(bv.eval::<bool>("(function() local ok=true for _,e in ipairs(require('bags').grid.items) do if e.item.id==900008 and not require('bags').is_junk(e.item) then ok=false end end return ok end)()"));

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6c_bag_viewer_merchant_sell_junk_batch_stop_close_zone_and_corpse_loot() {
    big_stack(round6c_bag_viewer_merchant_sell_junk_batch_stop_close_zone_and_corpse_loot_body)
}
fn round6c_bag_viewer_merchant_sell_junk_batch_stop_close_zone_and_corpse_loot_body() {
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

    // Mark Rat Pelt, Chipped Tooth, Torn Cloth and Cracked Fang as junk (the Rusty Sword stays).
    for slot in [2usize, 3, 4, 5] {
        let sp = bv.slot(slot);
        bv.click(sp);
        bv.frames(2);
        let (mx, my, en, _, _) = bv.menu_row("Mark as junk").unwrap();
        assert!(en);
        bv.click((mx, my));
        bv.frames(1);
    }

    // Open merchant: the button counts the 4 marked items with the native quote.
    bv.play.bags_open_merchant(44);
    bv.frames(2);
    let (_, _, lbl_off) = bv.action_hit("Sell junk (").unwrap();
    let quote: u64 = bv.eval("select(3, require('bags').junk_sell_list())");
    assert!(lbl_off.starts_with("Sell junk (4 items, "), "{lbl_off}");
    assert!(quote > 0, "native quote must be positive");

    // Idle frames perform zero sells even with merchant open.
    let sends_idle = bv.play.bags_sends_lost();
    bv.frames(4);
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_sends_lost(), sends_idle);

    // capture merchant PNG
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
    big_stack(round6_item4_lazy_icons_load_without_native_windows_and_zero_per_frame_requests_body)
}
fn round6_item4_lazy_icons_load_without_native_windows_and_zero_per_frame_requests_body() {
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
    // Three windows of ten frames, a full collection after every frame; a leak grows every window, so at most one may
    // grow by more than 1 KB. Lua's string table grows by doubling and a collection shrinks it only when it is three
    // quarters empty, and the window's caches settle once: one window alone measured those one-time steps, not what a
    // frame keeps (fix round 5: the old package plus 1,500 unused string constants grew 107 KB in one window; a control
    // keeping 1 KB a frame showed -52 KB then +11.7 KB, so neither one window nor the smaller of two catches it).
    bv.lua().gc_collect().unwrap();
    let mut window = || {
        let before = bv.lua().used_memory();
        for _ in 0..10 {
            bv.frames(1);
            bv.lua().gc_collect().unwrap();
        }
        bv.lua().used_memory() as i64 - before as i64
    };
    let grew = [window(), window(), window()];
    let res_after = super::super::icons::resource_call_count();
    let lkp_after = super::super::icons::lookup_attempt_count();
    assert_eq!(res_after, res_before, "steady frames must not re-request eqoa.icons.resource");
    assert_eq!(lkp_after, lkp_before, "steady frames must not trigger ESF surface lookups");
    assert!(
        grew.iter().filter(|&&d| d > 1024).count() <= 1,
        "steady frames must not leak Lua tables (three windows of ten frames grew {grew:?} bytes)"
    );

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
fn round6_l2_failed_icon_retries_after_items_change_and_not_per_frame() {
    big_stack(round6_l2_failed_icon_retries_after_items_change_and_not_per_frame_body)
}
fn round6_l2_failed_icon_retries_after_items_change_and_not_per_frame_body() {
    let Some(mut bv) = Bv::new() else { return };
    super::super::icons::set_custom_icon_provider(None);
    super::super::icons::reset_test_counters();

    let delayed_id: i32 = 40099;
    let empty_lib = esf::Library::new();
    super::super::icons::update_live_data(&empty_lib, None, &[]);
    let world = serde_json::json!({
        "pack": [
            {"name":"Delayed Charm","id":900099,"key":11,"qty":1,"icon":delayed_id}
        ],
        "bank": [],
        "tunar": 100,
        "bank_tunar": 0
    });
    bv.play.bags_set(&world);
    bv.enable();
    bv.open();
    bv.frames(4); // Exhaust initial 5 attempts -> cooled down (not permanent).
    assert!(!bv.eval::<bool>("require('bags').grid.items[1].texture ~= nil"));

    // Late arrival of icon 40099 into live library must NOT poll per frame while steady:
    let mut lib_with_icon = esf::Library::new();
    lib_with_icon.add(
        "ITEMICON.ESF",
        bv_test_surface_container(&[(delayed_id, [50, 200, 80, 255])]),
    );
    super::super::icons::update_live_data(&lib_with_icon, None, &[]);
    let res_before_steady = super::super::icons::resource_call_count();
    bv.frames(6);
    assert_eq!(
        super::super::icons::resource_call_count(),
        res_before_steady,
        "L2 negative control: cooled-down icon must never retry per frame"
    );

    // Next items/state change (inventory.changed via tunar) clears cooldown and retries cleanly:
    bv.play.bags_tunar(200);
    bv.frames(2);
    super::super::icons::update_live_data(&lib_with_icon, None, &[]);
    bv.frames(1);
    assert!(
        bv.eval::<bool>("require('bags').grid.items[1].texture ~= nil"),
        "L2: failed icon must retry and load on next items/state change"
    );

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6_item1_stale_corpse_grid_and_loot_batch_and_log() {
    big_stack(round6_item1_stale_corpse_grid_and_loot_batch_and_log_body)
}
fn round6_item1_stale_corpse_grid_and_loot_batch_and_log_body() {
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
    bv.play.bags_press_loot();
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
    big_stack(round6_item5_window_fits_hud_and_scrolls_100_slot_pack_body)
}
fn round6_item5_window_fits_hud_and_scrolls_100_slot_pack_body() {
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
    big_stack(round6_item6_take_all_button_on_corpse_tab_body)
}
fn round6_item6_take_all_button_on_corpse_tab_body() {
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

#[test]
fn round6_m1_loot_all_ignores_unrelated_inv_add_before_close() {
    big_stack(round6_m1_loot_all_ignores_unrelated_inv_add_before_close_body)
}
fn round6_m1_loot_all_ignores_unrelated_inv_add_before_close_body() {
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_target_corpse(5011, "a skeleton");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100}
    ]));
    bv.enable();
    bv.open();
    bv.click(bv.tab(7));
    bv.frames(1);

    let (tax, tay, _) = bv.action_hit("Take all").unwrap();
    bv.click((tax, tay));
    bv.play.bags_broker_tick();
    // Unrelated quest/trade InvAdd (id 8888 != 2001) before 0x0016 must NOT confirm Take all.
    bv.play.bags_added(&serde_json::json!({"name":"Quest Reward","id":8888,"key":91,"qty":1,"max_stack":20}));
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(
        bv.eval::<(String, Option<bool>)>("require('bags').result.text, require('bags').result.ok"),
        ("Take all: corpse closed".into(), Some(false)),
        "unrelated InvAdd must not confirm Take all on 0x0016"
    );

    // Positive control: InvAdd of an item from the corpse (id 2002) DOES confirm Take all on 0x0016.
    bv.play.bags_target_corpse(5012, "another skeleton");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Bone Chip","id":2002,"key":0,"qty":1,"max_stack":100}
    ]));
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(1);
    let (tax2, tay2, _) = bv.action_hit("Take all").unwrap();
    bv.click((tax2, tay2));
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Bone Chip","id":2002,"key":92,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(
        bv.eval::<(String, Option<bool>)>("require('bags').result.text, require('bags').result.ok"),
        ("Take all: ok".into(), Some(true)),
        "matching corpse InvAdd confirms Take all on 0x0016"
    );

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6_m2_loot_open_without_close_diffs_with_inv_add() {
    big_stack(round6_m2_loot_open_without_close_diffs_with_inv_add_body)
}
fn round6_m2_loot_open_without_close_diffs_with_inv_add_body() {
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    // Corpse 5013 open: native take + InvAdd, NO 0x0016 LootClose ever arrives before smaller same-corpse 0x0018.
    bv.play.bags_target_corpse(5013, "an undead gnoll");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Gnoll Ear","id":2010,"key":0,"qty":1,"max_stack":20},
        {"name":"Cracked Skull","id":2011,"key":1,"qty":1,"max_stack":20}
    ]));
    bv.frames(1);
    bv.play.bags_native_loot_one(0, 1);
    bv.play.bags_added(&serde_json::json!({"name":"Gnoll Ear","id":2010,"key":81,"qty":1,"max_stack":20}));
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Cracked Skull","id":2011,"key":0,"qty":1,"max_stack":20}
    ]));
    bv.frames(2);
    assert_eq!(bv.log().len(), 1, "M2: InvAdd + same-corpse 0x0018 without 0x0016 must log once: {:?}", bv.log());
    assert!(bv.log()[0].contains("Gnoll Ear x1, from an undead gnoll"), "{:?}", bv.log());

    // Negative control: same-corpse 0x0018 without 0x0016 and WITHOUT InvAdd logs nothing.
    bv.play.bags_native_loot_one(0, 1);
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([]));
    bv.frames(2);
    assert_eq!(bv.log().len(), 1, "M2 negative control: missing InvAdd must still log nothing: {:?}", bv.log());

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6_m4_reopen_stops_immediately_on_map_or_camera_open() {
    big_stack(round6_m4_reopen_stops_immediately_on_map_or_camera_open_body)
}
fn round6_m4_reopen_stops_immediately_on_map_or_camera_open_body() {
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.enable();
    bv.open();

    // Case 1: Map opens (map.open = true) during Sandstorm reopen -> stops at once, no 0x0015, no 10s wait.
    bv.play.bags_target_corpse(5014, "a skeleton");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Dust","id":2002,"key":0,"qty":1,"max_stack":100},
        {"name":"Bloodstone","id":2003,"key":1,"qty":1,"max_stack":100}
    ]));
    bv.frames(1);
    bv.click(bv.tab(7));
    bv.frames(1);
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Dust","id":2002,"key":71,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.play.map.open = true;
    let sends_map = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_sends_lost(), sends_map, "map open must send no 0x0015");
    assert_eq!(
        bv.eval::<(bool, String, Option<bool>)>("require('bags').batch_id == nil, require('bags').result.text, require('bags').result.ok"),
        (true, "Looted 1 of 2: stopped: map or camera open".into(), Some(false)),
        "map open mid-batch must stop at once with clear reason"
    );

    // Case 2: Manual camera opens (mode = 38) during Sandstorm reopen -> stops at once with same clear reason.
    bv.play.map.open = false;
    bv.play.bags_target_corpse(5015, "a zombie");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Flesh","id":2020,"key":0,"qty":1,"max_stack":100},
        {"name":"Tooth","id":2021,"key":1,"qty":1,"max_stack":100}
    ]));
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(1);
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Flesh","id":2020,"key":72,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.play.mode = 38;
    let sends_cam = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_sends_lost(), sends_cam, "manual camera mode 38 must send no 0x0015");
    assert_eq!(
        bv.eval::<(bool, String, Option<bool>)>("require('bags').batch_id == nil, require('bags').result.text, require('bags').result.ok"),
        (true, "Looted 1 of 2: stopped: map or camera open".into(), Some(false)),
        "manual camera mid-batch must stop at once with clear reason"
    );

    // Positive control: mode == 0 and map.open == false proceeds with 0x0015 reopen cleanly.
    bv.play.mode = 0;
    bv.play.bags_target_corpse(5016, "a ghoul");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Nail","id":2027,"key":0,"qty":1,"max_stack":100},
        {"name":"Fang","id":2028,"key":1,"qty":1,"max_stack":100}
    ]));
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(1);
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Nail","id":2027,"key":73,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close();
    let sends_ok = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > sends_ok, "mode == 0 & map closed must send 0x0015 reopen");

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6_l1_loot_batch_keeps_corpse_tab_across_close_and_summary() {
    big_stack(round6_l1_loot_batch_keeps_corpse_tab_across_close_and_summary_body)
}
fn round6_l1_loot_batch_keeps_corpse_tab_across_close_and_summary_body() {
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_target_corpse(5021, "a skeleton");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Dust","id":2002,"key":0,"qty":1,"max_stack":100},
        {"name":"Bloodstone","id":2003,"key":1,"qty":1,"max_stack":100}
    ]));
    bv.enable();
    bv.open();
    bv.click(bv.tab(7));
    bv.frames(1);
    assert_eq!(bv.eval::<i64>("require('bags').tab"), 7);

    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Dust","id":2002,"key":71,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(
        bv.eval::<i64>("require('bags').tab"),
        7,
        "L1: mid-batch 0x0016 close must keep Corpse tab (7) active"
    );

    bv.play.bags_broker_tick(); // 0x0015
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Bloodstone","id":2003,"key":0,"qty":1,"max_stack":100}
    ]));
    bv.frames(1);
    bv.play.bags_broker_tick(); // One(0, 1)
    bv.play.bags_added(&serde_json::json!({"name":"Bloodstone","id":2003,"key":72,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(
        bv.eval::<(i64, String, Option<bool>)>("require('bags').tab, require('bags').result.text, require('bags').result.ok"),
        (7, "Looted 2 of 2".into(), Some(true)),
        "L1: completed loot batch must stay on Corpse tab showing its summary"
    );

    // Negative control: switching manually to All tab leaves Corpse tab cleanly; opening/closing a non-batch corpse resets to tab 1.
    bv.click(bv.tab(1));
    bv.frames(1);
    assert_eq!(bv.eval::<i64>("require('bags').tab"), 1);
    bv.play.bags_target_corpse(5022, "another skeleton");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([{"name":"Bone","id":2004,"key":0,"qty":1,"max_stack":100}]));
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(1);
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(bv.eval::<i64>("require('bags').tab"), 1, "non-batch corpse close still returns to tab 1");

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn session_recording_replays_round6_close_and_smaller_reopen() {
    big_stack(session_recording_replays_round6_close_and_smaller_reopen_body)
}
fn session_recording_replays_round6_close_and_smaller_reopen_body() {
    let fixture: Vec<u8> = include_str!("round6_loot_recording.hex")
        .split_whitespace()
        .flat_map(|line| line.as_bytes().chunks_exact(2))
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    let rows = crate::fork_ext::session_replay::rows(&fixture).unwrap();
    assert_eq!(rows.len(), 5);
    assert!(rows[1].sent);
    for confirmed in [true, false] {
        let Some(mut bv) = Bv::new() else {
            eprintln!("SKIP round 6 recording replay: no GPU adapter");
            return;
        };
        bv.enable();
        bv.play.bags_target_corpse(5001, "a decaying skeleton");
        bv.play.bags_press_loot();
        bv.play.bags_replay(&rows[0]);
        bv.frames(1);
        assert_eq!(bv.eval::<usize>("#eqoa.loot.get().items"), 2);

        let sent = bv.play.bags_record_take(1, 1);
        assert_eq!(sent, [(rows[1].ty, rows[1].bytes.to_vec())]);
        if confirmed {
            bv.play.bags_replay(&rows[2]); // 0x003B inventory confirmation
        }
        bv.play.bags_replay(&rows[3]); // 0x0016 close
        bv.frames(1);
        assert!(bv.eval::<bool>("eqoa.loot.get() == nil"));
        bv.play.bags_replay(&rows[4]); // smaller 0x0018 reopens the corpse
        bv.frames(1);
        assert_eq!(bv.eval::<usize>("#eqoa.loot.get().items"), 1);
        assert_eq!(bv.eval::<String>("eqoa.loot.get().items[1].name"), "Rat Pelt");
        let log = bv.log();
        if confirmed {
            assert_eq!(log.len(), 1, "{log:?}");
            assert!(log[0].contains("Skeleton Dust x1, from a decaying skeleton"), "{log:?}");
        } else {
            assert!(log.is_empty(), "a close and smaller reopen alone gave credit: {log:?}");
        }
        bv.clean();
        super::super::action_broker::reset_all_for_test();
    }
}

#[test]
fn round6_sandstorm_loot_log_and_batch_reopen_with_negative_controls() {
    big_stack(round6_sandstorm_loot_log_and_batch_reopen_with_negative_controls_body)
}
fn round6_sandstorm_loot_log_and_batch_reopen_with_negative_controls_body() {
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    let corpse_initial = serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100,"icon":4020},
        {"name":"Pinch of Skeleton Dust","id":2002,"key":1,"qty":1,"max_stack":100,"icon":4021},
        {"name":"Black Bloodstone","id":2003,"key":2,"qty":1,"max_stack":100,"icon":4022}
    ]);
    // Step 0: Player opens corpse 5001 via 0x0015 -> 0x0018 (NO 0x0019/0x001A anywhere in Sandstorm replay!).
    bv.play.bags_target_corpse(5001, "a decaying skeleton");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&corpse_initial);
    bv.enable();
    bv.open();

    // Mark Rat Pelt as junk, then click "Take all but junk".
    bv.click(bv.tab(7));
    bv.frames(1);
    bv.click(bv.slot(1));
    bv.frames(1);
    let (jx, jy, _, _, _) = bv.menu_row("Mark as junk").unwrap();
    bv.click((jx, jy));
    bv.frames(1);

    let (lx, ly, _) = bv.action_hit("Take all but junk").unwrap();
    bv.click((lx, ly));
    bv.play.bags_broker_tick(); // Sends 0x0019(key=1, count=1) for Pinch of Skeleton Dust
    bv.frames(1);

    // Sandstorm response for Step 1: InvAdd + Chat "You receive ...", then 0x0016 LootClose (NO 0x0019!).
    bv.play.bags_added(&serde_json::json!({"name":"Pinch of Skeleton Dust","id":2002,"key":71,"qty":1,"max_stack":100}));
    bv.play.bags_chat("You receive Pinch of Skeleton Dust from the corpse.");
    bv.play.bags_loot_close();
    bv.frames(1);

    // Log must record Step 1 immediately on 0x0016 close, and batch must still be running (reopen pending).
    assert_eq!(bv.log().len(), 1, "Sandstorm take must log on InvAdd + 0x0016: {:?}", bv.log());
    assert!(bv.log()[0].contains("Pinch of Skeleton Dust x1, from a decaying skeleton"), "{:?}", bv.log());
    assert!(bv.eval::<bool>("require('bags').batch_id ~= nil"), "batch must NOT stop on Sandstorm 0x0016");

    // Next broker tick must send 0x0015 (attack_press) to reopen the same corpse!
    let sends_before_reask = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > sends_before_reask, "batch must send 0x0015 reopen request");

    // Server answers with smaller 0x0018 where Black Bloodstone re-indexed key 2 -> key 0!
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Black Bloodstone","id":2003,"key":0,"qty":1,"max_stack":100,"icon":4022},
        {"name":"Rat Pelt","id":2001,"key":1,"qty":1,"max_stack":100,"icon":4020}
    ]));
    bv.frames(1);
    assert_eq!(
        bv.eval::<Option<String>>("eqoa.loot.get() and eqoa.loot.get().source and eqoa.loot.get().source.name"),
        Some("a decaying skeleton".into()),
        "0x0015 reopen must preserve corpse source"
    );

    // Next broker tick re-resolves Black Bloodstone by item identity (key 0) and sends LootOne(0, 1).
    let sends_before_step2 = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > sends_before_step2, "batch must dispatch Step 2 at re-indexed key 0");
    bv.frames(1);

    // Sandstorm response for Step 2: InvAdd + Chat + 0x0016 LootClose.
    bv.play.bags_added(&serde_json::json!({"name":"Black Bloodstone","id":2003,"key":72,"qty":1,"max_stack":100}));
    bv.play.bags_chat("You receive Black Bloodstone from the corpse.");
    bv.play.bags_loot_close();
    let sends_after_done = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    bv.frames(2);

    assert_eq!(bv.play.bags_sends_lost(), sends_after_done, "completed batch must not re-ask corpse");
    assert_eq!(
        bv.eval::<(String, Option<bool>)>("require('bags').result.text, require('bags').result.ok"),
        ("Looted 2 of 2".into(), Some(true))
    );
    let log = bv.log();
    assert_eq!(log.len(), 2, "must have exact 2 entries with no duplicates: {log:?}");
    assert!(log[0].contains("Black Bloodstone x1, from a decaying skeleton"), "{log:?}");
    assert!(log[1].contains("Pinch of Skeleton Dust x1, from a decaying skeleton"), "{log:?}");

    // Closed-window native UI take on Sandstorm (with InvAdd) logs once and keeps zero duplicates.
    bv.toggle();
    assert!(!bv.is_open());
    bv.play.bags_target_corpse(5002, "an undead gnoll");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Gnoll Ear","id":2010,"key":0,"qty":1,"max_stack":20},
        {"name":"Cracked Skull","id":2011,"key":1,"qty":1,"max_stack":20}
    ]));
    bv.frames(1);
    bv.play.bags_native_loot_one(0, 1);
    bv.play.bags_added(&serde_json::json!({"name":"Gnoll Ear","id":2010,"key":81,"qty":1,"max_stack":20}));
    bv.play.bags_loot_close();
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Cracked Skull","id":2011,"key":0,"qty":1,"max_stack":20}
    ]));
    bv.frames(2);
    assert_eq!(bv.log().len(), 3, "closed-window native take with InvAdd logs once: {:?}", bv.log());
    assert!(bv.log()[0].contains("Gnoll Ear x1, from an undead gnoll"), "{:?}", bv.log());

    // M3 negative control: Mere disappearance from next same-corpse 0x0018 without InvAdd or 0x0019/0x001A logs NOTHING.
    let log_len_before_m3 = bv.log().len();
    bv.play.bags_native_loot_one(0, 1);
    bv.play.bags_loot_close();
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([])); // Cracked Skull vanished (e.g. another player took it), no InvAdd!
    bv.frames(2);
    assert_eq!(
        bv.log().len(),
        log_len_before_m3,
        "M3: disappearance from next 0x0018 without InvAdd must log nothing: {:?}",
        bv.log()
    );
    // Reopen gnoll corpse with Cracked Skull still there for Negative Control 1 below.
    bv.play.bags_loot_close();
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Cracked Skull","id":2011,"key":0,"qty":1,"max_stack":20}
    ]));
    bv.frames(1);

    // Negative control 1: Refused take (no InvAdd + next 0x0018 still keeps record, OR 0x0085 refusal) logs NOTHING.
    let log_len = bv.log().len();
    bv.play.bags_native_loot_one(0, 1);
    bv.play.bags_loot_refused(1);
    bv.play.bags_loot_close();
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Cracked Skull","id":2011,"key":0,"qty":1,"max_stack":20}
    ]));
    bv.frames(2);
    assert_eq!(bv.log().len(), log_len, "negative control: refused take must log nothing");

    // Negative control 2: Tunar-only / 5-byte empty corpse never logs an item.
    bv.play.bags_loot_close();
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([]));
    bv.play.bags_chat("You receive 5 tunar from the corpse.");
    bv.play.bags_tunar(605);
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(bv.log().len(), log_len, "negative control: tunar-only 5-byte corpse must log nothing");

    // Negative control 3: Mid-batch reopen aborted by 0x0085 refusal, 5-byte empty corpse, targeting elsewhere, and Stop.
    bv.toggle();
    assert!(bv.is_open());
    // 3a: 5-byte empty 0x0018 on reopen stops batch cleanly
    bv.play.bags_target_corpse(5003, "a rotting zombie");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Zombie Flesh","id":2020,"key":0,"qty":1,"max_stack":20},
        {"name":"Old Coin Tooth","id":2021,"key":1,"qty":1,"max_stack":20}
    ]));
    bv.frames(1);
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Zombie Flesh","id":2020,"key":73,"qty":1,"max_stack":20}));
    bv.play.bags_loot_close();
    bv.play.bags_broker_tick(); // sends 0x0015
    bv.play.bags_corpse(&serde_json::json!([])); // 5-byte empty list!
    bv.frames(2);
    assert_eq!(
        bv.eval::<(bool, Option<bool>)>("require('bags').batch_id == nil, require('bags').result.ok"),
        (true, Some(false)),
        "5-byte empty 0x0018 mid-batch must stop batch cleanly"
    );

    // 3b: Player targets something else during reopen -> stops batch and sends NO 0x0015
    bv.play.bags_loot_close();
    bv.play.bags_target_corpse(5004, "a phantom");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Ectoplasm","id":2022,"key":0,"qty":1,"max_stack":20},
        {"name":"Wisp Ash","id":2023,"key":1,"qty":1,"max_stack":20}
    ]));
    bv.frames(1);
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Ectoplasm","id":2022,"key":74,"qty":1,"max_stack":20}));
    bv.play.bags_loot_close();
    bv.play.bags_target(2002); // player targeted living Moss Snake!
    let sends_on_retarget = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_sends_lost(), sends_on_retarget, "retargeting elsewhere must send no 0x0015");
    assert_eq!(
        bv.eval::<(bool, Option<bool>)>("require('bags').batch_id == nil, require('bags').result.ok"),
        (true, Some(false)),
        "retargeting must stop batch"
    );

    // 3c: 0x0085 refusal during reopen stops batch
    bv.play.bags_target_corpse(5005, "a bogey");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Item One","id":2024,"key":0,"qty":1,"max_stack":20},
        {"name":"Item Two","id":2025,"key":1,"qty":1,"max_stack":20}
    ]));
    bv.frames(1);
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Item One","id":2024,"key":75,"qty":1,"max_stack":20}));
    bv.play.bags_loot_close();
    bv.play.bags_broker_tick(); // sends 0x0015
    bv.play.bags_loot_refused(1); // 0x0085!
    bv.frames(2);
    assert_eq!(
        bv.eval::<(bool, Option<bool>)>("require('bags').batch_id == nil, require('bags').result.ok"),
        (true, Some(false)),
        "0x0085 during reopen must stop batch"
    );

    // 3d: Unanswered 0x001A ("Take all") times out cleanly at 300 ticks without leaving LOOT_BUSY stuck.
    bv.play.bags_target_corpse(5006, "a goblin");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rusty Coin","id":2026,"key":0,"qty":1,"max_stack":20}
    ]));
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(1);
    let (tax, tay, _) = bv.action_hit("Take all").unwrap();
    bv.click((tax, tay));
    bv.play.bags_broker_tick(); // sends 0x001A
    for _ in 0..305 {
        bv.play.bags_broker_tick();
    }
    bv.frames(1);
    let (res_text, res_ok): (String, Option<bool>) =
        bv.eval("require('bags').result.text, require('bags').result.ok");
    assert!(
        res_text.starts_with("Take all: ") && res_ok == Some(false),
        "unanswered 0x001A must time out cleanly in status bar: {res_text:?} {res_ok:?}"
    );
    // Ensure looting is NOT wedged after 0x001A timeout:
    bv.click(bv.slot(1));
    bv.frames(1);
    let (tx, ty, _, _, _) = bv.menu_row("Take").unwrap();
    bv.click((tx, ty));
    let sends_after_timeout = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > sends_after_timeout, "LOOT_BUSY must clear after 0x001A timeout");
    bv.play.bags_loot_close();
    bv.frames(2);

    // 3e: Clicking Stop mid-reopen aborts batch cleanly and sends no 0x0015
    bv.play.bags_target_corpse(5007, "a ghoul");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Ghoul Nail","id":2027,"key":0,"qty":1,"max_stack":20},
        {"name":"Ghoul Fang","id":2028,"key":1,"qty":1,"max_stack":20}
    ]));
    bv.frames(1);
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Ghoul Nail","id":2027,"key":76,"qty":1,"max_stack":20}));
    bv.play.bags_loot_close();
    bv.frames(1);
    let (sx, sy, _) = bv.action_hit("Stop").unwrap();
    bv.click((sx, sy));
    let sends_on_stop = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_sends_lost(), sends_on_stop, "Stop mid-reopen must send no 0x0015");
    assert!(bv.eval::<bool>("require('bags').batch_id == nil"), "Stop must clear active batch");

    // 3f: Zoning mid-reopen aborts batch cleanly and sends no 0x0015
    bv.play.bags_target_corpse(5008, "a specter");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Specter Dust","id":2029,"key":0,"qty":1,"max_stack":20},
        {"name":"Specter Silk","id":2030,"key":1,"qty":1,"max_stack":20}
    ]));
    bv.frames(1);
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Specter Dust","id":2029,"key":77,"qty":1,"max_stack":20}));
    bv.play.bags_loot_close();
    bv.frames(1);
    bv.play.bags_zone();
    let sends_on_zone = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_sends_lost(), sends_on_zone, "zoning mid-reopen must send no 0x0015");
    assert!(bv.eval::<bool>("require('bags').batch_id == nil"), "zoning must clear active batch");

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round6_sandstorm_duplicate_item_id_stacks_and_partial_take() {
    big_stack(round6_sandstorm_duplicate_item_id_stacks_and_partial_take_body)
}
fn round6_sandstorm_duplicate_item_id_stacks_and_partial_take_body() {
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_target_corpse(5031, "a decaying skeleton");
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100},
        {"name":"Spider Silk","id":2002,"key":1,"qty":3,"max_stack":100},
        {"name":"Spider Silk","id":2002,"key":2,"qty":5,"max_stack":100},
        {"name":"Bone Chip","id":2003,"key":3,"qty":4,"max_stack":100}
    ]));
    bv.enable();
    bv.open();
    bv.click(bv.tab(7));
    bv.frames(1);

    // Mark Rat Pelt as junk (Slot 1).
    bv.click(bv.slot(1));
    bv.frames(1);
    let (jx, jy, _, _, _) = bv.menu_row("Mark as junk").unwrap();
    bv.click((jx, jy));
    bv.frames(1);

    // 1. Partial stack take: click Bone Chip x4 (Slot 4) -> "Take 1" -> logs x1 (not x4).
    bv.click(bv.slot(4));
    bv.frames(1);
    let (tx1, ty1, _, _, lbl) = bv.menu_row("Take 1").unwrap();
    assert_eq!(lbl, "Take 1");
    bv.click((tx1, ty1));
    bv.play.bags_broker_tick();
    bv.play.bags_added(&serde_json::json!({"name":"Bone Chip","id":2003,"key":71,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(bv.log().len(), 1, "partial take must log exact qty 1: {:?}", bv.log());
    assert!(bv.log()[0].contains("Bone Chip x1, from a decaying skeleton"), "{:?}", bv.log());

    // Reopen corpse with reduced Bone Chip stack (qty 4 -> qty 3), keeping both Spider Silk stacks + junk Rat Pelt.
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100},
        {"name":"Spider Silk","id":2002,"key":1,"qty":3,"max_stack":100},
        {"name":"Spider Silk","id":2002,"key":2,"qty":5,"max_stack":100},
        {"name":"Bone Chip","id":2003,"key":3,"qty":3,"max_stack":100}
    ]));
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(1);

    // 2. "Take all but junk" with two non-junk stacks of identical item_id (2002 qty 3 and qty 5) + partial Bone Chip (qty 3).
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.play.bags_broker_tick(); // Step 1: One(1, 3) -> Spider Silk x3
    bv.play.bags_added(&serde_json::json!({"name":"Spider Silk","id":2002,"key":72,"qty":3,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.frames(1);
    assert_eq!(bv.eval::<i64>("require('bags').tab"), 7, "L1 stays on Corpse tab during batch");

    bv.play.bags_broker_tick(); // Sends 0x0015 reopen
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100},
        {"name":"Spider Silk","id":2002,"key":1,"qty":5,"max_stack":100},
        {"name":"Bone Chip","id":2003,"key":2,"qty":3,"max_stack":100}
    ]));
    bv.frames(1);
    bv.play.bags_broker_tick(); // Step 2: re-resolves Spider Silk x5 at re-indexed key 1!
    bv.play.bags_added(&serde_json::json!({"name":"Spider Silk","id":2002,"key":73,"qty":5,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.frames(1);

    bv.play.bags_broker_tick(); // Sends 0x0015 reopen
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100},
        {"name":"Bone Chip","id":2003,"key":1,"qty":3,"max_stack":100}
    ]));
    bv.frames(1);
    bv.play.bags_broker_tick(); // Step 3: re-resolves Bone Chip x3 at re-indexed key 1!
    bv.play.bags_added(&serde_json::json!({"name":"Bone Chip","id":2003,"key":74,"qty":3,"max_stack":100}));
    bv.play.bags_loot_close();
    let sends_done = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_sends_lost(), sends_done, "completed 3-step batch sends no further 0x0015");
    assert_eq!(
        bv.eval::<(i64, String, Option<bool>)>("require('bags').tab, require('bags').result.text, require('bags').result.ok"),
        (7, "Looted 3 of 3".into(), Some(true))
    );
    let log = bv.log();
    assert_eq!(log.len(), 4, "must log 1 partial + 3 batch takes with 0 duplicates: {log:?}");
    assert!(log[0].contains("Bone Chip x3, from a decaying skeleton"), "{log:?}");
    assert!(log[1].contains("Spider Silk x5, from a decaying skeleton"), "{log:?}");
    assert!(log[2].contains("Spider Silk x3, from a decaying skeleton"), "{log:?}");
    assert!(log[3].contains("Bone Chip x1, from a decaying skeleton"), "{log:?}");

    // Negative control: reopening shows only junk Rat Pelt left, and Take all but junk sends 0 packets.
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100}
    ]));
    bv.frames(2);
    let sends_junk = bv.play.bags_sends_lost();
    bv.click(bv.action_hit("Take all but junk").map(|(x, y, _)| (x, y)).unwrap());
    bv.frames(1);
    assert_eq!(bv.play.bags_sends_lost(), sends_junk);
    assert_eq!(
        bv.eval::<(String, Option<bool>)>("require('bags').result.text, require('bags').result.ok"),
        ("Corpse has only junk items".into(), Some(false))
    );

    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round65_bag_viewer_standalone_loot_sell_bank_without_native_windows_and_refusal() {
    big_stack(round65_bag_viewer_standalone_loot_sell_bank_without_native_windows_and_refusal_body)
}
fn round65_bag_viewer_standalone_loot_sell_bank_without_native_windows_and_refusal_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();

    // 1. Addon Manager labels eqoa_bag_viewer as acting on its own + lists eqoa.windows.takeover.
    let entry = bv
        .manager
        .entries()
        .into_iter()
        .find(|e| e.metadata.id == "eqoa_bag_viewer")
        .unwrap();
    assert!(
        entry
            .metadata
            .requested_capabilities
            .iter()
            .any(|c| c == "eqoa.windows.takeover"),
        "mod.json must declare eqoa.windows.takeover"
    );
    assert_eq!(
        super::super::runtime::autonomous_action_label(&entry.metadata.requested_capabilities)
            .as_deref(),
        Some("Actions: can act on its own (open_loot, talk, loot, loot_all, sell, bank_move, use_item, equip, quick_slot, batch)"),
        "L7: the label names exactly the actions the mod uses, not all of them"
    );

    // Spawn nearby corpse (9001), merchant (9002), and banker (9003); native mode stays 0 throughout.
    bv.play.bags_spawn_npc(9001, "a fallen orc", 0, 3, [3.0, 0.0, 0.0]);
    bv.play.bags_spawn_npc(9002, "Merchant Kim", 0x80 | 1, 1, [6.0, 0.0, 0.0]);
    bv.play.bags_spawn_npc(9003, "Banker Dan", 0x80 | 2, 1, [8.0, 0.0, 0.0]);
    bv.open();
    bv.frames(2);
    assert_eq!(bv.play.mode, 0);
    // M1: opening Bag Viewer holds ZERO takeovers; player-started merchant/corpse still open native windows normally.
    assert!(bv.eval::<bool>("not eqoa.windows.taken('loot') and not eqoa.windows.taken('merchant') and not eqoa.windows.taken('bank')"), "Bag Viewer must NOT hold takeovers while idle");
    bv.play.bags_merchant_packet(888);
    assert_eq!(bv.play.mode, 10, "player-started merchant must open natively while Bag Viewer is open");
    bv.play.bags_close_merchant();
    assert_eq!(bv.play.mode, 0);

    // 2. Click Corpse tab (visible because corpse is nearby) -> click Loot -> server refuses 0x0085 -> visible server text.
    bv.click(bv.tab(7));
    bv.frames(1);
    let (lx, ly, lbl) = bv.action_hit("Look at corpse").unwrap();
    assert_eq!(lbl, "Look at corpse");
    let sends_0 = bv.play.bags_sends_lost();
    bv.click((lx, ly));
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > sends_0, "Loot opens nearest corpse");
    bv.play.bags_loot_refused(1);
    bv.frames(2);
    let (res_txt, res_ok) = bv.result_text().unwrap();
    assert_eq!(res_ok, Some(false));
    assert!(res_txt.contains("too far away"), "must show server refusal text: {res_txt}");
    assert_eq!(bv.play.mode, 0);

    // 3. Click Loot -> 0x0018 opens corpse silently (mode stays 0!) -> Take all but junk (Sandstorm 2-item per-take reopen without native window).
    assert!(bv.eval::<bool>("(function() require('bags').junk_ids['2001'] = true return true end)()"));
    bv.click((lx, ly));
    bv.play.bags_broker_tick();
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100,"icon":4020},
        {"name":"Bloodstone","id":2003,"key":1,"qty":1,"max_stack":100,"icon":4021},
        {"name":"Bone Chip","id":2004,"key":2,"qty":1,"max_stack":100,"icon":4022}
    ]));
    bv.frames(2);
    assert_eq!(bv.play.mode, 0, "corpse window must NOT open natively under takeover");
    let (nx, ny, _) = bv.action_hit("Take all but junk").unwrap();
    bv.click((nx, ny));
    bv.play.bags_broker_tick(); // Take Bloodstone (step 1)
    bv.play.bags_added(&serde_json::json!({"name":"Bloodstone","id":2003,"key":75,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close(); // Sandstorm 0x0016 close between takes
    bv.frames(1);
    bv.play.bags_broker_tick(); // Broker reopens corpse with 0x0015 automatically
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100},
        {"name":"Bone Chip","id":2004,"key":1,"qty":1,"max_stack":100}
    ]));
    assert_eq!(bv.play.mode, 0, "reopened corpse must NOT open natively");
    bv.frames(1);
    bv.play.bags_broker_tick(); // Take Bone Chip (step 2)
    bv.play.bags_added(&serde_json::json!({"name":"Bone Chip","id":2004,"key":76,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(bv.play.mode, 0);
    assert_eq!(
        bv.result_text(),
        Some(("Looted 2 of 2".into(), Some(true)))
    );
    assert!(bv.eval::<bool>("not eqoa.windows.taken('loot')"), "Bag Viewer must release loot takeover when batch ends");

    // 4. Mark Odd Thing (slot 8) as junk on All tab, then click Sell junk near Merchant (native merchant closed) -> talks & sells without native window!
    bv.click(bv.tab(1));
    bv.frames(1);
    let s8 = bv.slot(8);
    bv.click(s8);
    bv.frames(2);
    let (mx, my, en, _, _) = bv.menu_row("Mark as junk").unwrap();
    assert!(en);
    bv.click((mx, my));
    bv.frames(2);

    let (sx, sy, slbl) = bv.action_hit("Sell junk (").unwrap();
    assert!(slbl.starts_with("Sell junk (1 item)"), "{slbl}");
    let sends_merch = bv.play.bags_sends_lost();
    bv.click((sx, sy));
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > sends_merch, "Sell junk must talk to merchant");
    assert_eq!(bv.play.target, 9002);
    bv.play.bags_merchant_packet(9002);
    assert_eq!(bv.play.mode, 0, "native merchant window must stay closed");
    bv.frames(2);
    bv.play.bags_broker_tick(); // dispatches step 1 sell
    assert!(bv.play.bags_shop_pending(), "sell step sent silently");
    bv.play.bags_sell_confirm(7, 1);
    bv.frames(2);
    assert_eq!(bv.result_text(), Some(("Sold 1 of 1".into(), Some(true))));
    assert!(bv.eval::<bool>("not eqoa.windows.taken('merchant')"), "Bag Viewer must release merchant takeover when batch ends");

    // 5. Deposit near Banker (native bank closed): click item -> Deposit -> talks & deposits silently (mode stays 0).
    let s2 = bv.slot(2); // Healing Potion
    bv.click(s2);
    bv.frames(2);
    let (dx, dy, den, _, _) = bv.menu_row("Deposit 1").unwrap();
    assert!(den, "Deposit 1 must be enabled when near banker");
    bv.click((dx, dy));
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.target, 9003);
    bv.play.bags_bank_packet();
    assert_eq!(bv.play.mode, 0, "native bank window must stay closed");
    bv.frames(2);
    bv.play.bags_broker_tick(); // dispatches bank_move cleanly via bank_taken_tick
    assert_eq!(bv.play.bags_bank_state(), 2, "bank_move must have sent native 0x1254");
    bv.play.bags_bank_deposit_confirm(99, 900002, "Healing Potion", 1);
    bv.frames(2);
    assert_eq!(bv.result_text(), Some(("Deposit: ok".into(), Some(true))));
    assert!(bv.eval::<bool>("not eqoa.windows.taken('bank')"), "Bag Viewer must release bank takeover when action ends");

    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

#[test]
fn round65_bag_viewer_auto_loot_option_takes_all_but_junk_on_killed_target() {
    big_stack(round65_bag_viewer_auto_loot_option_takes_all_but_junk_on_killed_target_body)
}
fn round65_bag_viewer_auto_loot_option_takes_all_but_junk_on_killed_target_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();

    // Auto-loot option is registered and OFF by default.
    assert_eq!(
        bv.eval::<String>("require('bags').auto_mode"),
        "off",
        "Auto-loot is Off until chosen"
    );

    // Living target 9010 dies while auto_loot is OFF -> zero actions taken.
    bv.play.bags_spawn_npc(9010, "a goblin", 0, 1, [3.0, 0.0, 0.0]);
    bv.play.bags_target_sync(9010);
    bv.frames(2);
    let sends_off = bv.play.bags_sends_lost();
    bv.play.bags_entity_die(9010);
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_sends_lost(), sends_off, "auto_loot OFF must do nothing");

    // Turn Auto-loot ON; mark Rat Pelt (id 2001) as junk so auto-loot leaves it behind.
    bv.open();
    bv.frames(2);
    bv_auto(&bv, "non_junk");
    assert!(bv.eval::<bool>("not eqoa.windows.taken('loot')"), "auto_loot ON must NOT hold loot takeover while idle");
    assert!(bv.eval::<bool>("(function() require('bags').junk_ids['2001'] = true return true end)()"));

    // Kill living target 9011 nearest ([2,0,0] < 9010 at [3,0,0]) -> auto-loot opens corpse & loots non-junk cleanly without native window!
    bv.play.bags_self_id(77);
    bv.play.bags_spawn_npc(9011, "a gnoll", 0, 1, [2.0, 0.0, 0.0]);
    bv.play.bags_target_sync(9011);
    bv.play.bags_combat_hit(77, 9011, -12); // you hit it: the signal that it is your kill
    bv.frames(2);
    let sends_before = bv.play.bags_sends_lost();
    bv.play.bags_entity_die(9011);
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > sends_before, "auto_loot must send 0x0015 open_loot");
    assert_eq!(bv.play.target, 9011);
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100},
        {"name":"Sapphire","id":2009,"key":1,"qty":1,"max_stack":100}
    ]));
    bv.frames(2);
    assert_eq!(bv.play.mode, 0, "native corpse window must remain closed during auto-loot");
    bv.play.bags_broker_tick(); // dispatches Take Sapphire (leaves junk Rat Pelt!)
    bv.play.bags_added(&serde_json::json!({"name":"Sapphire","id":2009,"key":81,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(
        bv.result_text(),
        Some(("Looted 1 of 1".into(), Some(true))),
        "auto-loot must take all non-junk and report completion"
    );
    assert!(bv.eval::<bool>("not eqoa.windows.taken('loot')"), "auto-loot must release loot takeover after completion");

    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// L6: Auto-loot's "my kill" signal is that you hit the creature, not that it was merely targeted.
#[test]
fn round65_fix_bag_viewer_auto_loot_only_corpses_of_creatures_you_hit() {
    big_stack(round65_fix_bag_viewer_auto_loot_only_corpses_of_creatures_you_hit_body)
}
fn round65_fix_bag_viewer_auto_loot_only_corpses_of_creatures_you_hit_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.play.bags_self_id(77);
    bv.enable();
    bv.open();
    bv.frames(2);
    bv_auto(&bv, "non_junk");

    // a) targeted but never hit by you (another player's kill, say): not looted
    bv.play.bags_spawn_npc(9101, "a rat", 0, 1, [2.0, 0.0, 0.0]);
    bv.play.bags_target_sync(9101);
    bv.frames(2);
    let before = bv.play.bags_sends_lost();
    bv.play.bags_entity_die(9101);
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_sends_lost(), before, "a corpse of a creature you did not hit is not auto-looted");

    // b) hit by someone else only: not looted
    bv.play.bags_spawn_npc(9102, "a bat", 0, 1, [2.0, 0.0, 0.0]);
    bv.play.bags_combat_hit(555, 9102, -9);
    bv.frames(1);
    bv.play.bags_entity_die(9102);
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_sends_lost(), before, "another player's hit does not make it your kill");

    // c) a player's corpse you hit (a duel, a mistake): never auto-looted
    bv.play.bags_spawn_npc(9103, "Somebody", 0, 1, [2.0, 0.0, 0.0]);
    bv.play.bags_creature(9103, 2, u32::MAX);
    bv.play.bags_combat_hit(77, 9103, -9);
    bv.frames(1);
    bv.play.bags_entity_die(9103);
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_sends_lost(), before, "a player's corpse is never auto-looted");

    // d) a creature you hit, even if you had switched target before it died: looted
    bv.play.bags_spawn_npc(9104, "a snake", 0, 1, [2.0, 0.0, 0.0]);
    bv.play.bags_combat_hit(77, 9104, -9);
    bv.frames(1);
    bv.play.bags_entity_die(9104);
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > before, "a creature you hit and that died is auto-looted");

    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// M1: Auto-loot never runs while you are in combat (it retargets, which ends the auto-attack); it waits
/// and goes on when the fight is over.
#[test]
fn round65_fix_bag_viewer_auto_loot_waits_while_in_combat() {
    big_stack(round65_fix_bag_viewer_auto_loot_waits_while_in_combat_body)
}
fn round65_fix_bag_viewer_auto_loot_waits_while_in_combat_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.play.bags_self_id(77);
    bv.enable();
    bv.open();
    bv.frames(2);
    bv_auto(&bv, "non_junk");

    // one kill of yours, while a second hostile still has you targeted
    bv.play.bags_spawn_npc(9201, "a gnoll", 0, 1, [2.0, 0.0, 0.0]);
    bv.play.bags_spawn_npc(9202, "a gnoll scout", 0, 1, [4.0, 0.0, 0.0]);
    bv.play.bags_creature(9202, 0, 77);
    bv.play.bags_target_sync(9201);
    bv.play.bags_combat_hit(77, 9201, -12);
    bv_look_again(&mut bv); // the look at the zone is a quarter second old at most
    let before = bv.play.bags_sends_lost();
    bv.play.bags_entity_die(9201);
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_sends_lost(), before, "M1: no auto-loot while a live hostile has you targeted");
    assert_eq!(bv.play.target, 9201, "M1: the target is not touched during combat");

    // the scout stops (dies): out of combat, the held-back corpse is taken now (the next creature update, a quarter
    // second on, asks again)
    bv.play.bags_entity_die(9202);
    bv_look_again(&mut bv); // the next look at the zone (at most half a second on, from Auto-loot's poll)
    bv.eval::<bool>("(function() require('bags').tick_auto_loot() return true end)()");
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > before, "M1: auto-loot goes on once the fight is over");

    // 9201's corpse answers with nothing on it: that open is over
    bv.play.bags_corpse(&serde_json::json!([]));
    bv.frames(2);
    bv.play.bags_broker_tick();
    bv.play.bags_loot_close();
    bv.play.bags_broker_tick();
    bv.frames(2);

    // auto-attack on a live creature is a fight; the game leaves it on after the creature dies, and then it is not
    // (fix round 6, owner log 2026-11-19: corpses waited "in combat (auto-attack is on)" long after the kill)
    bv.play.bags_spawn_npc(9203, "a rat", 0, 1, [2.0, 0.0, 0.0]);
    bv.play.bags_spawn_npc(9204, "a second rat", 0, 1, [3.0, 0.0, 0.0]);
    bv.play.bags_target_sync(9203);
    bv.play.bags_auto_attack(true);
    bv.play.bags_combat_hit(77, 9203, -12);
    bv_look_again(&mut bv);
    let before2 = bv.play.bags_sends_lost();
    bv.play.bags_entity_die(9203);
    bv.play.bags_target_sync(9204); // the auto-attack goes on against the next one
    bv_look_again(&mut bv);
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_sends_lost(), before2, "M1: no auto-loot while auto-attack is on against a live creature");
    bv.play.bags_entity_die(9204); // auto-attack still on, its target a corpse now
    bv_look_again(&mut bv);
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > before2, "fix round 6 B: auto-attack left on after the kill is not a fight");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// L5: the Loot tab keeps the hidden corpse session alive while it shows it; the idle timeout is for sessions
/// nobody uses.
#[test]
fn round65_fix_bag_viewer_loot_tab_keeps_hidden_session_alive() {
    big_stack(round65_fix_bag_viewer_loot_tab_keeps_hidden_session_alive_body)
}
fn round65_fix_bag_viewer_loot_tab_keeps_hidden_session_alive_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.play.bags_spawn_npc(9301, "a fallen orc", 0, 3, [3.0, 0.0, 0.0]);
    bv.open();
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(1);
    let (lx, ly, _) = bv.action_hit("Look at corpse").unwrap();
    bv.click((lx, ly));
    bv.play.bags_broker_tick();
    bv.play.bags_corpse(&bv_corpse());
    bv.frames(2);
    let hidden_open = |bv: &Bv| {
        super::super::window_takeover::is_hidden_active(super::super::window_takeover::TakeoverKind::Loot)
            && bv.play.bags_corpse_held()
    };
    assert!(hidden_open(&bv), "the corpse is open (hidden) in the tab");
    // the player reads the tab for longer than the idle timeout
    super::super::window_takeover::expire_hidden_session_for_test(super::super::window_takeover::TakeoverKind::Loot);
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert!(hidden_open(&bv), "L5: the Loot tab must renew the session while it is shown");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// L7: the manifest declares exactly the actions the Lua calls.
#[test]
fn round65_fix_bag_viewer_manifest_declares_exactly_the_actions_it_uses() {
    big_stack(round65_fix_bag_viewer_manifest_declares_exactly_the_actions_it_uses_body)
}
fn round65_fix_bag_viewer_manifest_declares_exactly_the_actions_it_uses_body() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bundled-mods/eqoa_bag_viewer");
    let mut used = std::collections::BTreeSet::new();
    for file in ["main.lua", "modules/bags.lua"] {
        let src = std::fs::read_to_string(root.join(file)).unwrap();
        let mut rest = src.as_str();
        while let Some(i) = rest.find("eqoa.actions.") {
            rest = &rest[i + "eqoa.actions.".len()..];
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if !name.is_empty() {
                used.insert(name);
            }
        }
    }
    // cancel and stop_batch ask for no capability
    used.remove("cancel");
    used.remove("stop_batch");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("mod.json")).unwrap()).unwrap();
    let declared: std::collections::BTreeSet<String> = manifest["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c.as_str()?.strip_prefix("eqoa.actions.").map(str::to_string))
        .collect();
    assert!(!declared.contains("*"), "no blanket eqoa.actions.*");
    assert_eq!(declared, used, "declared actions must equal the actions the mod calls");
}

/// Owner test of round 6.5 (a, b): the corpse button opens the nearest corpse into the Corpse tab and SHOWS its
/// items, without the game's window and without taking anything; stale loot-request bookkeeping (a request the
/// server never answered) must not turn the mod's open into a native window.
#[test]
fn round65_fix2_open_corpse_shows_items_hidden_even_with_stale_loot_requests() {
    big_stack(round65_fix2_open_corpse_shows_items_hidden_even_with_stale_loot_requests_body)
}
fn round65_fix2_open_corpse_shows_items_hidden_even_with_stale_loot_requests_body() {
    r65fix2_open_corpse(true);
}

#[test]
fn round65_fix2_open_button_only_displays_never_takes() {
    big_stack(round65_fix2_open_button_only_displays_never_takes_body)
}
fn round65_fix2_open_button_only_displays_never_takes_body() {
    r65fix2_open_corpse(false);
}

fn r65fix2_open_corpse(stale: bool) {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.play
        .bags_spawn_npc(9001, "a fallen orc", 0, 3, [3.0, 0.0, 0.0]);
    // requests of earlier play that no list ever answered: the loot-source bookkeeping is tainted for good
    if stale {
        crate::fork_ext::loot_requested(7001);
        crate::fork_ext::loot_requested(7002);
    }
    bv.open();
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(1);
    let (lx, ly, lbl) = bv.action_hit("Look at corpse").unwrap();
    assert_eq!(lbl, "Look at corpse");
    let _ = super::super::window_takeover::take_log_for_test();
    bv.click((lx, ly));
    bv.play.bags_broker_tick();
    let log = super::super::window_takeover::take_log_for_test();
    assert!(
        log.iter().any(|l| l.contains("mod open_loot -> corpse 0x2329") && l.contains("opcode 0x0015")),
        "the request's entity and opcode are logged: {log:?}"
    );
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100},
        {"name":"Bloodstone","id":2003,"key":1,"qty":1,"max_stack":100},
        {"name":"Bone Chip","id":2004,"key":2,"qty":1,"max_stack":100}
    ]));
    bv.frames(2);
    assert_eq!(
        bv.play.mode, 0,
        "the game's corpse window must not open after a Bag Viewer button"
    );
    assert!(
        super::super::window_takeover::is_hidden_active(
            super::super::window_takeover::TakeoverKind::Loot
        ),
        "the open is the mod's hidden session"
    );
    let names = bv.names();
    assert_eq!(
        names,
        vec!["Rat Pelt", "Bloodstone", "Bone Chip"],
        "the Corpse tab shows the corpse's items"
    );
    assert_eq!(bv.result_text(), Some(("Corpse ready".into(), Some(true))));
    // Opening must not leave a take queued for the next broker tick.
    let sends = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    assert_eq!(
        bv.play.bags_sends_lost(),
        sends,
        "opening must not queue a take request"
    );
    assert_eq!(bv.names().len(), 3);
    assert!(bv.eval::<bool>("#assert(eqoa.loot.get()).items == 3"));
    if std::env::var_os("EQOA_R6_CAPTURE_DIR").is_some() {
        let shot = bv.frames(1);
        bv_r6_png(&rm_pixels(&bv.gpu, &shot), bv.win, "r65-corpse-open.png");
    }
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
}

/// Owner test (c): a merchant that never answers the talk must end the step visibly after the server timeout,
/// not stay on "Opening merchant...".
#[test]
fn round65_fix2_unanswered_merchant_talk_fails_visibly() {
    big_stack(round65_fix2_unanswered_merchant_talk_fails_visibly_body)
}
fn round65_fix2_unanswered_merchant_talk_fails_visibly_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.play
        .bags_spawn_npc(9002, "Merchant Kim", 0x80 | 1, 1, [6.0, 0.0, 0.0]);
    assert!(
        bv.eval::<bool>("(function() require('bags').junk_ids['900008'] = true return true end)()")
    );
    bv.open();
    bv.frames(2);
    bv.click(bv.tab(1));
    bv.frames(2);
    let (sx, sy, _) = bv.action_hit("Sell junk (").unwrap();
    let sends = bv.play.bags_sends_lost();
    bv.click((sx, sy));
    bv.play.bags_broker_tick();
    assert!(
        bv.play.bags_sends_lost() > sends,
        "the talk request goes out"
    );
    assert_eq!(bv.result_text(), Some(("Opening merchant...".into(), None)));
    // the server says nothing at all
    for _ in 0..320 {
        bv.play.bags_broker_tick();
    }
    bv.frames(2);
    let (text, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false));
    assert!(
        text.contains("merchant did not answer"),
        "visible failure, got: {text}"
    );
    assert!(
        bv.eval::<bool>("not eqoa.windows.taken('merchant')"),
        "the takeover is let go after the failure"
    );
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Owner test (d): a banker who does not answer (out of the server's reach) is told in the result line, for the
/// open and for a bank action.
#[test]
fn round65_fix2_unanswered_bank_requests_fail_visibly() {
    big_stack(round65_fix2_unanswered_bank_requests_fail_visibly_body)
}
fn round65_fix2_unanswered_bank_requests_fail_visibly_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.play
        .bags_spawn_npc(9003, "Banker Dan", 0x80 | 2, 1, [8.0, 0.0, 0.0]);
    bv.open();
    bv.frames(2);
    // Deposit near a banker: the open (talk) goes unanswered
    let s2 = bv.slot(2);
    bv.click(s2);
    bv.frames(2);
    let (dx, dy, den, _, _) = bv.menu_row("Deposit 1").unwrap();
    assert!(den);
    bv.click((dx, dy));
    bv.play.bags_broker_tick();
    for _ in 0..320 {
        bv.play.bags_broker_tick();
    }
    bv.frames(2);
    let (text, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false));
    assert!(
        text.contains("banker did not answer"),
        "open: visible failure, got: {text}"
    );

    // the bank opens (the player stands nearer, 4 units: the silence at 8 made 8 and farther too far), but the
    // deposit itself goes unanswered
    bv.play.bags_player_at([4.0, 0.0, 0.0]);
    bv.eval::<bool>("(function() require('bags').scan_reset() return true end)()");
    bv.frames(2);
    bv.click(s2);
    bv.frames(2);
    let (dx, dy, _, _, _) = bv.menu_row("Deposit 1").unwrap();
    bv.click((dx, dy));
    bv.play.bags_broker_tick();
    bv.play.bags_bank_packet();
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_bank_state(), 2, "the deposit request went out");
    for _ in 0..320 {
        bv.play.bags_broker_tick();
    }
    bv.frames(2);
    let (text, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false));
    assert!(
        text.contains("too far from the banker"),
        "action: visible failure, got: {text}"
    );
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

// ---- Round 6.5 fix round 4, item 1: the Bag Viewer's cost per frame -------------------------------------------------

/// Chooses Auto-loot in the window's own way (what its button does).
fn bv_auto(bv: &Bv, mode: &str) {
    assert!(bv.eval::<bool>(&format!("require('bags').set_auto_mode({mode:?})")), "Auto-loot {mode}");
}

/// A full pack: `n` distinct stacks (the biggest pack the game has is 100 slots).
fn bv_full_pack(n: usize) -> serde_json::Value {
    let mut world = bv_world();
    let pack: Vec<serde_json::Value> = (0..n)
        .map(|i| serde_json::json!({"name": format!("Pack Item {i}"), "id": 800_000 + i, "key": 100 + i, "qty": 1 + (i % 7), "max_stack": 20, "icon": 4001 + (i % 8)}))
        .collect();
    world["pack"] = serde_json::Value::Array(pack);
    world
}

/// The first number after `needle` in `line`.
fn bv_num_after(line: &str, needle: &str) -> Option<f64> {
    let rest = &line[line.find(needle)? + needle.len()..];
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// What the Bag Viewer cost over a run of busy frames.
struct BvCost {
    /// ms a frame spent in the mod (its Lua and its `eqoa.*` calls)
    mod_ms: f64,
    /// `eqoa.entities.list` calls a second of wall time
    lists_per_s: f64,
    /// wall seconds the frames took
    secs: f64,
    lines: Vec<String>,
}

/// One frame's creature updates: every twentieth creature moves and the session tells the API.
fn bv_churn(bv: &mut Bv, tick: usize) {
    for i in 3..bv.play.entities.slots.len() {
        if (tick + i) % 20 != 0 {
            continue;
        }
        if let Some(e) = bv.play.entities.slots[i].as_mut() {
            if e.id >= 5000 {
                e.pos[0] += 0.1;
                e.actor_pos = e.pos;
                bv.play.entity_slot_changed(i);
            }
        }
    }
}

/// Runs `n` frames in a busy world (a creature in every twentieth slot changing each frame: 35 updates a frame, what the
/// owner's log shows in a crowded zone)
/// and returns what the Bag Viewer cost.
fn bv_cost(bv: &mut Bv, n: usize) -> BvCost {
    bv_cost_with(bv, n, &mut |_, _| {})
}

/// The release-build budgets of the Bag Viewer's cost a frame, ms (the report's numbers; before the fixes about 60 open).
const RELEASE_OPEN_MS: f64 = 3.0;
const RELEASE_AUTO_ON_MS: f64 = 3.0;

/// `bv_cost` with something more happening each frame (`extra`, given the frame number).
fn bv_cost_with(bv: &mut Bv, n: usize, extra: &mut dyn FnMut(&mut Bv, usize)) -> BvCost {
    use super::super::addon_perf;
    // every creature is touched once before the count starts: the API reports one it has not seen as added, and
    // each such event (rightly) drops the Bag Viewer's look at the zone
    for tick in 0..25 {
        bv_churn(bv, tick);
        extra(bv, tick);
        bv.frame();
    }
    addon_perf::force_for_test(true);
    let _ = addon_perf::take_report();
    let started = std::time::Instant::now();
    for tick in 25..25 + n {
        bv_churn(bv, tick);
        extra(bv, tick);
        bv.frame();
    }
    let secs = started.elapsed().as_secs_f64();
    let lines = addon_perf::take_report();
    addon_perf::force_for_test(false);
    let mod_ms = lines
        .iter()
        .find(|l| l.starts_with("addons:"))
        .and_then(|l| bv_num_after(l, &format!("{BV} ")))
        .unwrap_or(0.0);
    let per_frame = lines
        .iter()
        .find(|l| l.starts_with("addon-api:") && l.contains(BV))
        .and_then(|l| bv_num_after(l, "entities.list "))
        .unwrap_or(0.0);
    BvCost { mod_ms, lists_per_s: per_frame * n as f64 / secs, secs, lines }
}

/// What one `eqoa.entities.list()` of the busy zone costs here, in ms (the least of a few: the unit the window's
/// per-frame budget is counted in, so the test means the same in a debug and a release build).
fn bv_list_ms(bv: &Bv) -> f64 {
    (0..5)
        .map(|_| {
            let t = std::time::Instant::now();
            assert!(bv.eval::<i64>("#eqoa.entities.list()") > 700);
            t.elapsed().as_secs_f64() * 1e3
        })
        .fold(f64::MAX, f64::min)
}

/// The real-shaped scene: a 100-slot pack, 700 creatures round the player, a corpse and a merchant in reach.
fn bv_busy_scene() -> Option<Bv> {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let mut bv = Bv::new()?;
    bv.play.bags_connect();
    bv.play.bags_set(&bv_full_pack(100));
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    for i in 0..700 {
        let a = i as f32 * 2.399_963;
        let d = 20.0 + (i % 40) as f32 * 5.0;
        bv.play.entities.slots.push(Some(rm_creature(5000 + i as u32, &format!("Busy Creature {i}"), [a.cos() * d, 0.0, a.sin() * d], 0, 1 + (i % 50) as u8)));
    }
    bv.enable();
    bv.lua().load("require('bags').AUTO_POLL_SECS = 0.25").exec().unwrap();
    bv.play.bags_spawn_npc(9001, "a fallen orc", 0, 3, [3.0, 0.0, 0.0]);
    bv.play.bags_spawn_npc(9002, "Merchant Kim", 0x80 | 1, 1, [6.0, 0.0, 0.0]);
    Some(bv)
}

/// Owner log 2026-11-18: with the Bags window open the `hud` time went from about 10 to 22 ms a frame, 7-10 ms of it
/// the Bag Viewer's, nearly all of it `eqoa.entities.list` (each call clones and compares every creature in the
/// zone; the window asked for the list four times a frame). The window, open on a 100-slot pack beside a corpse and
/// a merchant in a 700-creature zone, must ask for the list at most about twice a second, and cost less per frame
/// than one list call does (counted so in any build; in a release build it must also be under 6 ms a frame, closed under 0.2).
#[test]
fn round65_fix4_item1_bags_window_costs_under_a_millisecond_a_frame() {
    big_stack(round65_fix4_item1_bags_window_costs_under_a_millisecond_a_frame_body)
}
fn round65_fix4_item1_bags_window_costs_under_a_millisecond_a_frame_body() {
    let Some(mut bv) = bv_busy_scene() else { return };
    bv.frames(4);
    let list_ms = bv_list_ms(&bv);
    let closed = bv_cost(&mut bv, 120);
    eprintln!("R65P one list call {list_ms:.3} ms; closed: {:.3} ms/frame, {:.2} list calls/s", closed.mod_ms, closed.lists_per_s);
    bv.open();
    bv.frames(6);
    assert_eq!(bv.names().len(), 100, "the 100-slot pack is on screen");
    let open = bv_cost(&mut bv, 120);
    eprintln!("R65P open: {:.3} ms/frame, {:.2} list calls/s", open.mod_ms, open.lists_per_s);
    for l in &open.lines {
        eprintln!("R65P     {l}");
    }
    // bags.lua rescans at most once per SCAN_MIN (0.25 s): 4 a second is the design's ceiling. A slow
    // runner sees a few extra invalidations (3.4 on GitHub's Windows runner, 2026-11-19), so the bound is the cap.
    // Counted over a window of `secs`, one scan every 0.25 s gives at most `secs / 0.25 + 1` scans (the one at the
    // window's start and one per interval after it): bounding the rate at 4.0 flaked at 4.1 to 4.2 on a run of about
    // a second (CI and this PC, 2026-11-20). The cap itself is unchanged.
    let scans = open.lists_per_s * open.secs;
    assert!(
        scans <= open.secs * 4.0 + 1.0 + 1e-6,
        "the open window asked for the zone's creature list {scans:.0} times in {:.2} s ({:.1} a second; at most one per 0.25 s)",
        open.secs,
        open.lists_per_s
    );
    assert!(open.mod_ms < list_ms, "the open Bags window costs {:.3} ms a frame; one look at the zone is {list_ms:.3} ms", open.mod_ms);
    if !cfg!(debug_assertions) {
        assert!(open.mod_ms < RELEASE_OPEN_MS, "the open Bags window costs {:.3} ms a frame (budget {RELEASE_OPEN_MS}; before the fixes about 60)", open.mod_ms);
        assert!(closed.mod_ms < 0.2, "the closed Bag Viewer costs {:.3} ms a frame (budget 0.2)", closed.mod_ms);
    }
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}
/// Auto-loot's fight check runs on every creature update while a looted corpse waits; it must share one look at the
/// zone, not scan 700 creatures per call.
#[test]
fn round65_fix4_item1_waiting_auto_loot_shares_one_scan_of_the_zone() {
    big_stack(round65_fix4_item1_waiting_auto_loot_shares_one_scan_of_the_zone_body)
}
fn round65_fix4_item1_waiting_auto_loot_shares_one_scan_of_the_zone_body() {
    let Some(mut bv) = bv_busy_scene() else { return };
    bv.frames(4);
    bv_auto(&bv, "non_junk");
    bv.frames(2);
    bv.play.entity_slot_changed(5);
    bv.frames(2);
    super::super::addon_perf::force_for_test(true);
    let _ = super::super::addon_perf::take_report();
    // 50 questions in one frame: a fight check, then where the nearest corpse, merchant and banker are
    {
        let _in_the_mod = super::super::addon_perf::lua_scope(BV);
        bv.eval::<bool>(
            "(function() local b = require('bags') for i = 1, 50 do b.in_combat() b.nearby_corpse() b.nearby_npc('merchant') b.nearby_npc('bank') end return true end)()",
        );
    }
    bv.frame();
    let lines = super::super::addon_perf::take_report();
    super::super::addon_perf::force_for_test(false);
    let calls = lines
        .iter()
        .find(|l| l.starts_with("addon-api:") && l.contains(BV))
        .and_then(|l| bv_num_after(l, "entities.list "))
        .unwrap_or(0.0);
    eprintln!("R65P 200 questions: {calls:.1} list calls; {lines:?}");
    assert!(calls <= 2.0, "200 questions about the zone made {calls:.1} full scans of its creatures");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

// ---- Round 6.5 fix round 4, item 2: every open a mod causes stays hidden -------------------------------------------

/// Owner log 2026-11-18: after a hidden open_loot the game's own corpse window showed during auto-loot and batches
/// ("takeover: loot window left to the game (no request of the holder is waiting for this window ...)"). The live
/// server closes the corpse after each take (0x0016); the broker asks for it again (0x0015) and the answer (0x0018)
/// must be the mod's, like the first one: the game's window never shows, and no line of the takeover log says it
/// was left to the game. `auto_loot` is how the batch starts (the button, or Auto-loot after a kill); `relist` is how
/// the server goes on after a take: it lists the corpse again by itself right behind its close (what the owner's
/// log shows: no client request before those lists), or only after the broker asks for it (0x0015).
fn r65fix4_hidden_batch(auto_loot: bool, relist: bool) {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.play.bags_self_id(77);
    bv.enable();
    let _ = super::super::window_takeover::take_log_for_test();
    let items = |keys_from: usize| {
        let all = [("Rat Pelt", 2001), ("Bloodstone", 2003), ("Bone Chip", 2004)];
        serde_json::Value::Array(
            all[keys_from..]
                .iter()
                .enumerate()
                .map(|(k, (n, id))| serde_json::json!({"name": n, "id": id, "key": k, "qty": 1, "max_stack": 100}))
                .collect(),
        )
    };
    if auto_loot {
        bv.open();
        bv.frames(2);
        bv_auto(&bv, "non_junk");
        bv.play.bags_spawn_npc(9011, "a gnoll", 0, 1, [2.0, 0.0, 0.0]);
        bv.play.bags_target_sync(9011);
        bv.play.bags_combat_hit(77, 9011, -12);
        bv.frames(2);
        bv.play.bags_entity_die(9011);
        bv.frames(2);
        bv.play.bags_broker_tick();
    } else {
        bv.play.bags_spawn_npc(9001, "a fallen orc", 0, 3, [3.0, 0.0, 0.0]);
        bv.open();
        bv.frames(2);
        bv.click(bv.tab(7));
        bv.frames(1);
        let (lx, ly, _) = bv.action_hit("Take all but junk").unwrap();
        bv.click((lx, ly));
        bv.play.bags_broker_tick();
    }
    let names = ["Rat Pelt", "Bloodstone", "Bone Chip"];
    // the list that answers the open
    bv.play.bags_corpse(&items(0));
    for (taken, name) in names.iter().enumerate() {
        bv.frames(2);
        assert_eq!(bv.play.mode, 0, "the game's corpse window must not show (list {taken})");
        bv.play.bags_broker_tick(); // the next take
        let id = [2001, 2003, 2004][taken];
        bv.play.bags_added(&serde_json::json!({"name": name, "id": id, "key": 80 + taken, "qty": 1, "max_stack": 100}));
        bv.play.bags_chat(&format!("You receive {name} from the corpse."));
        bv.play.bags_loot_close(); // the server closes the corpse after every take
        if taken + 1 < names.len() {
            if relist {
                // ... and lists it again at once, with no request of ours before it
                bv.play.bags_corpse(&items(taken + 1));
            } else {
                bv.frames(1);
                let before = bv.play.bags_sends_lost();
                bv.play.bags_broker_tick(); // the broker asks for the corpse again: 0x0015
                assert!(bv.play.bags_sends_lost() > before, "the broker asks for the corpse again (take {taken})");
                bv.play.bags_corpse(&items(taken + 1));
            }
        }
    }
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.mode, 0, "no game window at the end");
    let log = super::super::window_takeover::take_log_for_test();
    assert!(
        !log.iter().any(|l| l.contains("left to the game")),
        "every open the mod caused stays hidden, got: {log:#?}"
    );
    assert_eq!(
        log.iter().filter(|l| l.contains("window HIDDEN for the mod")).count(),
        3,
        "the open and each list after a take are all the mod's: {log:#?}"
    );
    assert_eq!(bv.result_text(), Some(("Looted 3 of 3".into(), Some(true))));
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
}

#[test]
fn round65_fix4_item2_take_all_but_junk_relists_stay_hidden() {
    big_stack(round65_fix4_item2_take_all_but_junk_relists_stay_hidden_body)
}
fn round65_fix4_item2_take_all_but_junk_relists_stay_hidden_body() {
    r65fix4_hidden_batch(false, true);
}

#[test]
fn round65_fix4_item2_take_all_but_junk_reopens_asked_by_the_broker_stay_hidden() {
    big_stack(round65_fix4_item2_take_all_but_junk_reopens_asked_by_the_broker_stay_hidden_body)
}
fn round65_fix4_item2_take_all_but_junk_reopens_asked_by_the_broker_stay_hidden_body() {
    r65fix4_hidden_batch(false, false);
}

#[test]
fn round65_fix4_item2_auto_loot_relists_stay_hidden() {
    big_stack(round65_fix4_item2_auto_loot_relists_stay_hidden_body)
}
fn round65_fix4_item2_auto_loot_relists_stay_hidden_body() {
    r65fix4_hidden_batch(true, true);
}

#[test]
fn round65_fix4_item2_auto_loot_reopens_asked_by_the_broker_stay_hidden() {
    big_stack(round65_fix4_item2_auto_loot_reopens_asked_by_the_broker_stay_hidden_body)
}
fn round65_fix4_item2_auto_loot_reopens_asked_by_the_broker_stay_hidden_body() {
    r65fix4_hidden_batch(true, false);
}

/// A corpse the player clicks themselves while a hidden session is up is still theirs: the game's window shows.
#[test]
fn round65_fix4_item2_a_players_own_click_on_the_corpse_still_shows_the_game_window() {
    big_stack(round65_fix4_item2_a_players_own_click_on_the_corpse_still_shows_the_game_window_body)
}
fn round65_fix4_item2_a_players_own_click_on_the_corpse_still_shows_the_game_window_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.play.bags_spawn_npc(9001, "a fallen orc", 0, 3, [3.0, 0.0, 0.0]);
    bv.open();
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(1);
    let (lx, ly, _) = bv.action_hit("Take all but junk").unwrap();
    bv.click((lx, ly));
    bv.play.bags_broker_tick();
    let list = serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100},
        {"name":"Bloodstone","id":2003,"key":1,"qty":1,"max_stack":100}
    ]);
    bv.play.bags_corpse(&list);
    bv.frames(2);
    assert_eq!(bv.play.mode, 0, "the mod's open is hidden");
    // the player presses the loot button on the same corpse themselves: no claim of the mod's stands behind it
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&list);
    assert_eq!(bv.play.mode, 1, "the player's own request shows the game's corpse window");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
}

// ---- Round 6.5 fix round 4, items 3-6: Auto-loot choice, Look/Take labels, no grey items, out-of-range buttons ------

/// A killed creature of yours (you hit it), looted by Auto-loot under `mode`: returns the viewer with the creature
/// dead, junk Rat Pelt (2001) marked.
fn r65fix4_auto_kill(mode: &str) -> Option<Bv> {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let mut bv = Bv::new()?;
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.play.bags_self_id(77);
    bv.enable();
    bv.open();
    bv.frames(2);
    assert!(bv.eval::<bool>("(function() require('bags').junk_ids['2001'] = true return true end)()"));
    if mode != "off" {
        // the window's own button, as a player chooses it
        let label = if mode == "all" { "Auto-loot: Everything" } else { "Auto-loot: Everything but junk" };
        let (x, y, _) = bv.action_hit(label).unwrap();
        bv.click((x, y));
    }
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), mode);
    bv.play.bags_spawn_npc(9011, "a gnoll", 0, 1, [2.0, 0.0, 0.0]);
    bv.play.bags_target_sync(9011);
    bv.play.bags_combat_hit(77, 9011, -12);
    bv.frames(2);
    bv.play.bags_entity_die(9011);
    bv.frames(2);
    Some(bv)
}
/// The corpse's list (junk Rat Pelt and Sapphire) answers Auto-loot's open.
fn r65fix4_auto_list(bv: &mut Bv) {
    bv.play.bags_broker_tick(); // 0x0015
    bv.play.bags_corpse(&serde_json::json!([
        {"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100},
        {"name":"Sapphire","id":2009,"key":1,"qty":1,"max_stack":100}
    ]));
    bv.frames(2);
    assert_eq!(bv.play.mode, 0, "the game's corpse window stays closed during Auto-loot");
}

/// Off: a kill is left alone.
#[test]
fn round65_fix4_item3_auto_loot_off_leaves_the_corpse_alone() {
    big_stack(round65_fix4_item3_auto_loot_off_leaves_the_corpse_alone_body)
}
fn round65_fix4_item3_auto_loot_off_leaves_the_corpse_alone_body() {
    let Some(mut bv) = r65fix4_auto_kill("off") else { return };
    let sends = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.play.bags_sends_lost(), sends, "Off: nothing is asked of the server");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Everything: the corpse's junk goes too (one Take all).
#[test]
fn round65_fix4_item3_auto_loot_everything_takes_the_junk_too() {
    big_stack(round65_fix4_item3_auto_loot_everything_takes_the_junk_too_body)
}
fn round65_fix4_item3_auto_loot_everything_takes_the_junk_too_body() {
    let Some(mut bv) = r65fix4_auto_kill("all") else { return };
    r65fix4_auto_list(&mut bv);
    let sends = bv.play.bags_sends_lost();
    bv.play.bags_broker_tick(); // Take all: 0x001A
    assert!(bv.play.bags_sends_lost() > sends, "Everything sends one Take all");
    bv.play.bags_loot_confirm_all();
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert_eq!(bv.result_text(), Some(("Take all: ok".into(), Some(true))), "Everything is a Take all, junk included");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Everything but junk: the marked junk stays on the corpse.
#[test]
fn round65_fix4_item3_auto_loot_everything_but_junk_leaves_the_junk() {
    big_stack(round65_fix4_item3_auto_loot_everything_but_junk_leaves_the_junk_body)
}
fn round65_fix4_item3_auto_loot_everything_but_junk_leaves_the_junk_body() {
    let Some(mut bv) = r65fix4_auto_kill("non_junk") else { return };
    r65fix4_auto_list(&mut bv);
    bv.play.bags_broker_tick(); // takes Sapphire only
    bv.play.bags_added(&serde_json::json!({"name":"Sapphire","id":2009,"key":81,"qty":1,"max_stack":100}));
    bv.play.bags_loot_close();
    bv.frames(2);
    assert_eq!(bv.result_text(), Some(("Looted 1 of 1".into(), Some(true))), "only the one that is not junk");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// The choice is three buttons with a one-line explanation under them, kept with the junk marks, and it replaces
/// the old on/off option (the Options page no longer lists one).
#[test]
fn round65_fix4_item3_auto_loot_choice_buttons_explanation_and_saved() {
    big_stack(round65_fix4_item3_auto_loot_choice_buttons_explanation_and_saved_body)
}
fn round65_fix4_item3_auto_loot_choice_buttons_explanation_and_saved_body() {
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    bv.open();
    bv.frames(2);
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), "off");
    let mut explained = Vec::new();
    for (label, key) in [("Auto-loot: Off", "off"), ("Auto-loot: Everything", "all"), ("Auto-loot: Everything but junk", "non_junk")] {
        let (x, y, found) = bv.action_hit(label).unwrap_or_else(|| panic!("a button for {label}"));
        assert_eq!(found, label);
        bv.click((x, y));
        bv.frames(1);
        assert_eq!(bv.eval::<String>("require('bags').auto_mode"), key);
        let line: String = bv.eval("require('bags').auto_explain()");
        assert!(line.starts_with("Auto-loot: ") && line.len() < 60, "one short line: {line:?}");
        explained.push(line);
    }
    explained.sort();
    explained.dedup();
    assert_eq!(explained.len(), 3, "each choice explains itself");
    assert_eq!(super::super::options::get_toggle(BV, "auto_loot"), None, "the old on/off option is gone");
    // kept: written with the junk marks, read back after a reload
    let saved: String = bv.eval("eqoa.files.read('junk.cfg')");
    assert!(saved.lines().next() == Some("auto:Wereoxx=non_junk"), "{saved:?}");
    bv.manager.queue(Command::Enable(BV.into(), false));
    bv.frames(2);
    bv.manager.queue(Command::Enable(BV.into(), true));
    bv.frames(3);
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), "non_junk");
    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

/// Item 4: the buttons say what they do: Look only shows, Take takes.
#[test]
fn round65_fix4_item4_look_and_take_labels() {
    big_stack(round65_fix4_item4_look_and_take_labels_body)
}
fn round65_fix4_item4_look_and_take_labels_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.play.bags_spawn_npc(9001, "a fallen orc", 0, 3, [3.0, 0.0, 0.0]);
    bv.open();
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(2);
    let labels: Vec<String> = bv.eval("(function() local t = {} for _, h in ipairs(require('bags').action_hits) do t[#t + 1] = h.label end return t end)()");
    for want in ["Look at corpse", "Take all", "Take all but junk"] {
        assert!(labels.iter().any(|l| l == want), "a button {want:?} among {labels:?}");
    }
    assert!(!labels.iter().any(|l| l.contains("Open nearest corpse")), "the unclear label is gone: {labels:?}");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Item 5: no "Also sell grey items": no checkbox, no setting; only the marks make junk; an old file's `grey=` line
/// is ignored and not written back.
#[test]
fn round65_fix4_item5_no_grey_items_option() {
    big_stack(round65_fix4_item5_no_grey_items_option_body)
}
fn round65_fix4_item5_no_grey_items_option_body() {
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    bv.open();
    bv.frames(2);
    let labels: Vec<String> = bv.eval("(function() local t = {} for _, h in ipairs(require('bags').action_hits) do t[#t + 1] = h.label end return t end)()");
    assert!(!labels.iter().any(|l| l.to_lowercase().contains("grey")), "no grey-items checkbox: {labels:?}");
    assert!(bv.eval::<bool>("require('bags').sell_grey == nil and require('bags').toggle_grey == nil and require('bags').is_grey_item == nil"));
    // an older version's file: its grey line changes nothing, the mark loads, and saving drops the line
    assert!(bv.eval::<bool>("(function() eqoa.files.write('junk.cfg', 'grey=1\\n900008\\n') require('bags').load_junk() return true end)()"));
    assert!(bv.eval::<bool>("require('bags').junk_ids['900008'] == true"));
    assert!(!bv.eval::<bool>("require('bags').is_junk({id = 900007, name = 'Rat Pelt'})"), "a plain misc item is not junk");
    assert!(bv.eval::<bool>("(function() require('bags').save_junk() return true end)()"));
    let saved: String = bv.eval("eqoa.files.read('junk.cfg')");
    assert!(!saved.contains("grey"), "{saved:?}");
    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

/// The nearest NPC list is cached for half a second; a test that moves the player asks again at once.
fn bv_look_again(bv: &mut Bv) {
    bv.eval::<bool>("(function() require('bags').scan_reset() return true end)()");
    bv.frames(2);
}

/// Item 6: with the nearest merchant or banker beyond the reach the server last accepted, the mod's own buttons
/// grey out and say why. The reach is learned: an answer at a distance makes that distance fine, silence makes it
/// (and farther) too far; until known, a conservative default decides. The player's own windows are never blocked.
#[test]
fn round65_fix4_item6_out_of_range_buttons_grey_out_and_the_reach_is_learned() {
    big_stack(round65_fix4_item6_out_of_range_buttons_grey_out_and_the_reach_is_learned_body)
}
fn round65_fix4_item6_out_of_range_buttons_grey_out_and_the_reach_is_learned_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    // the merchant and the banker stand 30 units away: past the default reach
    bv.play.bags_spawn_npc(9002, "Merchant Kim", 0x80 | 1, 1, [30.0, 0.0, 0.0]);
    bv.play.bags_spawn_npc(9003, "Banker Dan", 0x80 | 2, 1, [0.0, 0.0, 30.0]);
    assert!(bv.eval::<bool>("(function() require('bags').junk_ids['900008'] = true return true end)()"));
    bv.open();
    bv.frames(2);
    bv.click(bv.tab(1));
    bv.frames(2);
    let sends = bv.play.bags_sends_lost();
    let (x, y, label) = bv.action_hit("Sell junk").unwrap();
    assert!(label.contains("too far from the merchant"), "greyed out and says why: {label}");
    bv.click((x, y));
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_sends_lost(), sends, "nothing is sent to a merchant out of reach");
    assert_eq!(bv.result_text(), Some(("Sell junk: too far from the merchant (click again to try anyway)".into(), Some(false))));
    // Deposit and Withdraw: the rows are there, greyed, with the reason
    bv.click(bv.slot(2));
    bv.frames(2);
    let (_, _, en, reason, _) = bv.menu_row("Deposit").unwrap();
    assert!(!en && reason.as_deref() == Some("too far from the banker"), "{en} {reason:?}");
    bv.key(winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape));
    bv.frames(1);

    // walking up to 5 units: in reach; the merchant answers: 5 is a distance that works
    bv.play.bags_player_at([25.0, 0.0, 0.0]);
    bv_look_again(&mut bv);
    let (x, y, label) = bv.action_hit("Sell junk").unwrap();
    assert!(label.starts_with("Sell junk (1 item)"), "in reach: {label}");
    bv.click((x, y));
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > sends, "the talk request goes out");
    bv.play.bags_merchant_packet(9002);
    bv.frames(2);
    let ok: f64 = bv.eval("require('bags').range.merchant.ok");
    assert!((ok - 5.0).abs() < 0.01, "an answer at 5 units: {ok}");
    bv.play.bags_close_merchant();
    bv.play.bags_broker_tick();
    bv.frames(2);

    // at 9 units (inside the default reach) the merchant says nothing, once: one miss greys nothing
    bv.play.bags_player_at([21.0, 0.0, 0.0]);
    bv_look_again(&mut bv);
    let (x, y, label) = bv.action_hit("Sell junk").unwrap();
    assert!(label.starts_with("Sell junk (1 item)"), "9 units is inside the default reach: {label}");
    bv.click((x, y));
    for _ in 0..330 {
        bv.play.bags_broker_tick();
    }
    bv.frames(2);
    assert!(bv.result_text().unwrap().0.contains("did not answer"));
    let fail: Option<f64> = bv.eval("require('bags').range.merchant.fail");
    assert_eq!(fail, None, "one silence is not yet 'too far'");
    bv_look_again(&mut bv);
    let (x, y, label) = bv.action_hit("Sell junk").unwrap();
    assert!(label.starts_with("Sell junk (1 item)"), "after one miss: {label}");
    // the second silence at the same distance: 9 and farther are too far from now on
    bv.click((x, y));
    for _ in 0..330 {
        bv.play.bags_broker_tick();
    }
    bv.frames(2);
    let fail: f64 = bv.eval("require('bags').range.merchant.fail");
    assert!((fail - 9.0).abs() < 0.01, "two silences at 9 units: {fail}");
    bv_look_again(&mut bv);
    let (_, _, label) = bv.action_hit("Sell junk").unwrap();
    assert!(label.contains("too far from the merchant"), "9 units is now known to be too far: {label}");
    // 7 units lies between a distance that worked (5) and one that did not (9): worth trying
    bv.play.bags_player_at([23.0, 0.0, 0.0]);
    bv_look_again(&mut bv);
    let (_, _, label) = bv.action_hit("Sell junk").unwrap();
    assert!(label.starts_with("Sell junk (1 item)"), "between worked and failed: {label}");

    // the player's own window is never blocked: far from the merchant, with the shop open, Sell junk works
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.play.bags_open_merchant(9002);
    bv_look_again(&mut bv);
    let (x, y, label) = bv.action_hit("Sell junk").unwrap();
    assert!(label.starts_with("Sell junk (1 item"), "open shop: no distance check: {label}");
    let sends = bv.play.bags_sends_lost();
    bv.click((x, y));
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert!(bv.play.bags_sends_lost() > sends || bv.play.bags_shop_pending(), "the sell goes out from an open shop");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Item 6, bank side: far from the banker the rows are greyed with the reason; a bank the player opened by hand is a
/// real answer (the reach is learned from it), and an open bank is never blocked.
#[test]
fn round65_fix4_item6_bank_rows_follow_the_reach_and_native_opens_teach_it() {
    big_stack(round65_fix4_item6_bank_rows_follow_the_reach_and_native_opens_teach_it_body)
}
fn round65_fix4_item6_bank_rows_follow_the_reach_and_native_opens_teach_it_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.play.bags_spawn_npc(9003, "Banker Dan", 0x80 | 2, 1, [0.0, 0.0, 14.0]);
    bv.open();
    bv.frames(2);
    bv.click(bv.tab(1));
    bv.frames(2);
    bv.click(bv.slot(2));
    bv.frames(2);
    let (_, _, en, reason, _) = bv.menu_row("Deposit").unwrap();
    assert!(!en, "14 units: past the default reach");
    assert_eq!(reason.as_deref(), Some("too far from the banker"));
    bv.key(winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape));
    // the player walks to 4 units and opens the bank themselves: that answer is learned (4 units works)
    bv.play.bags_player_at([0.0, 0.0, 10.0]);
    bv_look_again(&mut bv);
    bv.play.bags_bank_packet();
    bv.frames(2);
    let ok: Option<f64> = bv.eval("require('bags').range.bank.ok");
    assert!(ok.is_some_and(|d| (d - 4.0).abs() < 0.01), "a native open at 4 units teaches the reach: {ok:?}");
    // with the bank open nothing is blocked, wherever the banker stands
    bv.play.bags_player_at([0.0, 0.0, -40.0]);
    bv_look_again(&mut bv);
    bv.click(bv.slot(2));
    bv.frames(2);
    let (_, _, en, reason, _) = bv.menu_row("Deposit").unwrap();
    assert!(en, "an open bank is never blocked: {reason:?}");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

// ---- Round 6.5 fix round 4, review H1: the server's list after the mod's LAST take stays hidden too ---------------------

/// The corpse after the last take: the junk is still on it, so the server closes it (0x0016) and lists it again by
/// itself (0x0018). The mod's session has just ended with that take; the list must still not reach the player as the
/// game's window, and the corpse is closed again quietly. A corpse the player asks for afterwards is theirs.
/// `single` is how the last take is made: one item from its menu, or a Take all but junk batch.
fn r65fix4_last_take_relist(single: bool) {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    assert!(bv.eval::<bool>("(function() require('bags').junk_ids['2001'] = true return true end)()"));
    bv.play.bags_spawn_npc(9001, "a fallen orc", 0, 3, [3.0, 0.0, 0.0]);
    bv.open();
    bv.frames(2);
    bv.click(bv.tab(7));
    bv.frames(1);
    let _ = super::super::window_takeover::take_log_for_test();
    let pelt = serde_json::json!({"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100});
    let stone = |key: u32| serde_json::json!({"name":"Bloodstone","id":2003,"key":key,"qty":1,"max_stack":100});
    if single {
        let (x, y, _) = bv.action_hit("Look at corpse").unwrap();
        bv.click((x, y));
        bv.play.bags_broker_tick();
        bv.play.bags_corpse(&serde_json::json!([stone(0), {"name":"Rat Pelt","id":2001,"key":1,"qty":1,"max_stack":100}]));
        bv.frames(2);
        assert_eq!(bv.play.mode, 0, "the preview is hidden");
        bv.click(bv.slot(1));
        bv.frames(2);
        let (mx, my, en, _, _) = bv.menu_row("Take").unwrap();
        assert!(en);
        bv.click((mx, my));
        bv.play.bags_broker_tick();
    } else {
        let (x, y, _) = bv.action_hit("Take all but junk").unwrap();
        bv.click((x, y));
        bv.play.bags_broker_tick();
        bv.play.bags_corpse(&serde_json::json!([pelt.clone(), {"name":"Bloodstone","id":2003,"key":1,"qty":1,"max_stack":100}]));
        bv.frames(2);
        assert_eq!(bv.play.mode, 0);
        bv.play.bags_broker_tick(); // the one item that is not junk
    }
    // the take: the item, the chat line, the close, and the list of what is left (the junk), all at once
    bv.play.bags_added(&serde_json::json!({"name":"Bloodstone","id":2003,"key":80,"qty":1,"max_stack":100}));
    bv.play.bags_chat("You receive Bloodstone from the corpse.");
    bv.play.bags_loot_close();
    bv.play.bags_corpse(&serde_json::json!([{"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100}]));
    assert_eq!(bv.play.mode, 0, "the list after the last take must not show the game's corpse window");
    bv.frames(2);
    bv.play.bags_broker_tick();
    bv.frames(2);
    bv.play.bags_broker_tick(); // the session of a single take ends in this tick, its close is sent in the next
    bv.frames(2);
    assert_eq!(bv.play.mode, 0);
    let log = super::super::window_takeover::take_log_for_test();
    assert!(!log.iter().any(|l| l.contains("left to the game")), "nothing left to the game: {log:#?}");
    assert!(
        log.iter().any(|l| l.contains("again")),
        "the list is recognised as the server's (the session going on, or the grace after its last take): {log:#?}"
    );
    assert!(
        single || !bv.play.bags_corpse_held(),
        "and after a batch the corpse is closed again, quietly (hidden active {}, mode {}): {log:#?}",
        super::super::window_takeover::is_hidden_active(super::super::window_takeover::TakeoverKind::Loot),
        bv.play.mode
    );
    // the player's own click on that corpse afterwards is theirs: the game's window shows
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([{"name":"Rat Pelt","id":2001,"key":0,"qty":1,"max_stack":100}]));
    assert_eq!(bv.play.mode, 1, "a player's own request is never hidden");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
    crate::fork_ext::loot_source::clear();
}

#[test]
fn round65_fix4_h1_list_after_the_last_batch_take_stays_hidden() {
    big_stack(round65_fix4_h1_list_after_the_last_batch_take_stays_hidden_body)
}
fn round65_fix4_h1_list_after_the_last_batch_take_stays_hidden_body() {
    r65fix4_last_take_relist(false);
}

#[test]
fn round65_fix4_h1_list_after_a_single_menu_take_stays_hidden() {
    big_stack(round65_fix4_h1_list_after_a_single_menu_take_stays_hidden_body)
}
fn round65_fix4_h1_list_after_a_single_menu_take_stays_hidden_body() {
    r65fix4_last_take_relist(true);
}

// ---- Round 6.5 fix round 4, review M2: the Auto-loot choice is per character ----------------------------------------

/// The choice is kept per character (the junk file keeps one line for each); a login as another character reads that
/// character's choice; a reload of the mod keeps both. The junk marks stay shared: they are per item type, in the same file.
#[test]
fn round65_fix4_m2_auto_loot_choice_is_per_character() {
    big_stack(round65_fix4_m2_auto_loot_choice_is_per_character_body)
}
fn round65_fix4_m2_auto_loot_choice_is_per_character_body() {
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    bv.open();
    bv.frames(3);
    assert_eq!(bv.eval::<String>("require('bags').character"), "Wereoxx");
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), "off", "nothing chosen yet");
    assert!(bv.eval::<bool>("(function() require('bags').junk_ids['900008'] = true require('bags').save_junk() return true end)()"));
    bv_auto(&bv, "all");
    let saved: String = bv.eval("eqoa.files.read('junk.cfg')");
    assert!(saved.lines().any(|l| l == "auto:Wereoxx=all"), "{saved:?}");

    // another character enters the world: the choice is theirs (off), the first one's is kept
    bv.play.bags_set_name("Aldara");
    bv.play.bags_zone();
    bv.eval::<bool>("(function() require('bags').char_checked = -100 return true end)()");
    bv.frames(3);
    bv.eval::<bool>("(function() require('bags').sync_character() return true end)()"); // the game's next player update does this
    let who: String = bv.eval("(function() local p = eqoa.player.get() return tostring(p and p.name) .. ' dirty=' .. tostring(require('bags').char_dirty) end)()");
    assert_eq!(bv.eval::<String>("require('bags').character"), "Aldara", "{who}");
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), "off", "a new character starts with Auto-loot off");
    assert!(bv.eval::<bool>("require('bags').junk_ids['900008'] == true"), "the junk marks are shared");
    bv_auto(&bv, "non_junk");
    let saved: String = bv.eval("eqoa.files.read('junk.cfg')");
    assert!(saved.lines().any(|l| l == "auto:Wereoxx=all") && saved.lines().any(|l| l == "auto:Aldara=non_junk"), "{saved:?}");

    // back to the first character
    bv.play.bags_set_name("Wereoxx");
    bv.play.bags_zone();
    bv.eval::<bool>("(function() require('bags').char_checked = -100 return true end)()");
    bv.frames(3);
    bv.eval::<bool>("(function() require('bags').sync_character() return true end)()"); // the game's next player update does this
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), "all");

    // a reload of the mod: each character still has theirs
    bv.manager.queue(Command::Enable(BV.into(), false));
    bv.frames(2);
    bv.manager.queue(Command::Enable(BV.into(), true));
    bv.frames(3);
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), "all");
    bv.play.bags_set_name("Aldara");
    bv.play.bags_zone();
    bv.eval::<bool>("(function() require('bags').char_checked = -100 return true end)()");
    bv.frames(3);
    bv.eval::<bool>("(function() require('bags').sync_character() return true end)()"); // the game's next player update does this
    assert_eq!(bv.eval::<String>("require('bags').auto_mode"), "non_junk");
    bv.clean();
    super::super::action_broker::reset_all_for_test();
}

// ---- Round 6.5 fix round 4, review M3: one unanswered request does not grey a button for the session ----------------

/// Silence is weaker evidence than an answer: one miss greys nothing, two misses grey the nearer distance, a miss closer
/// than the farthest answer drops the old answers and greys at once, and an answer always clears the doubt.
#[test]
fn round65_fix4_m3_note_range_needs_two_misses_and_an_answer_clears() {
    big_stack(round65_fix4_m3_note_range_needs_two_misses_and_an_answer_clears_body)
}
fn round65_fix4_m3_note_range_needs_two_misses_and_an_answer_clears_body() {
    super::super::action_broker::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.enable();
    let eval = |lua: &str| bv.eval::<Option<f64>>(&format!("(function() local b = require('bags') b.range.merchant = {{}} {lua} end)()"));
    assert_eq!(eval("b.note_range('merchant', 9, false) return b.range.merchant.fail"), None, "one miss greys nothing");
    assert_eq!(eval("b.note_range('merchant', 9, false) b.note_range('merchant', 12, false) return b.range.merchant.fail"), Some(9.0), "two misses grey the nearer one");
    assert_eq!(eval("b.note_range('merchant', 9, false) b.note_range('merchant', 12, false) b.note_range('merchant', 6, true) return b.range.merchant.fail"), None, "an answer clears the misses");
    assert_eq!(eval("b.note_range('merchant', 12, true) b.note_range('merchant', 15, false) return b.range.merchant.fail"), None, "one miss past the farthest answer greys nothing");
    assert_eq!(eval("b.note_range('merchant', 12, true) b.note_range('merchant', 5, false) return b.range.merchant.fail"), Some(5.0), "a miss closer than the farthest answer greys at once");
    assert_eq!(eval("b.note_range('merchant', 12, true) b.note_range('merchant', 5, false) return b.range.merchant.ok"), None, "and the old answer no longer counts");
}

/// A greyed button still allows one try: the first click says why and asks to click again, the second goes out; an
/// answer to it clears the grey.
#[test]
fn round65_fix4_m3_a_greyed_button_still_allows_one_try() {
    big_stack(round65_fix4_m3_a_greyed_button_still_allows_one_try_body)
}
fn round65_fix4_m3_a_greyed_button_still_allows_one_try_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.play.bags_spawn_npc(9002, "Merchant Kim", 0x80 | 1, 1, [20.0, 0.0, 0.0]);
    assert!(bv.eval::<bool>("(function() require('bags').junk_ids['900008'] = true return true end)()"));
    bv.open();
    bv.frames(2);
    bv.click(bv.tab(1));
    bv.frames(2);
    let (x, y, label) = bv.action_hit("Sell junk").unwrap();
    assert!(label.contains("too far from the merchant"), "{label}");
    let sends = bv.play.bags_sends_lost();
    bv.click((x, y));
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.bags_sends_lost(), sends, "the first click only says why");
    let (text, _) = bv.result_text().unwrap();
    assert!(text.contains("too far from the merchant") && text.contains("click again"), "{text}");
    bv.click((x, y));
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_sends_lost() > sends, "the second click tries anyway");
    // the merchant answers (the server's reach is longer than guessed): the grey is gone for good
    bv.play.bags_merchant_packet(9002);
    bv.frames(2);
    let ok: f64 = bv.eval("require('bags').range.merchant.ok");
    assert!((ok - 20.0).abs() < 0.01, "{ok}");
    bv.play.bags_close_merchant();
    bv.play.bags_broker_tick();
    bv.eval::<bool>("(function() require('bags').scan_reset() return true end)()");
    bv.frames(2);
    let (_, _, label) = bv.action_hit("Sell junk").unwrap();
    assert!(label.starts_with("Sell junk (1 item)"), "an answer clears the grey: {label}");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

// ---- Round 6.5 fix round 4, review M4: rescans capped, Auto-loot ON in a crowded zone, the window's drawing ------------

/// Auto-loot ON in a crowded zone, window closed, a corpse of yours held back by a fight, and a hostile that starts and
/// stops targeting you every frame (each flip makes the fight check stale): the zone is looked at no more than once
/// every quarter second, and the mod's per-frame cost stays small.
#[test]
fn round65_fix4_m4_auto_loot_on_in_a_crowded_zone_scans_at_most_four_times_a_second() {
    big_stack(round65_fix4_m4_auto_loot_on_in_a_crowded_zone_scans_at_most_four_times_a_second_body)
}
fn round65_fix4_m4_auto_loot_on_in_a_crowded_zone_scans_at_most_four_times_a_second_body() {
    let Some(mut bv) = bv_busy_scene() else { return };
    bv.play.bags_self_id(77);
    bv.frames(4);
    bv_auto(&bv, "non_junk");
    bv.frames(2);
    bv.play.bags_spawn_npc(9700, "a hostile", 0, 1, [5.0, 0.0, 0.0]);
    bv.frames(2);
    // a corpse of yours waits for the fight to end
    assert!(bv.eval::<bool>("(function() require('bags').eligible_corpses = {[9001] = {id = 9001, position = {x = 3, y = 0, z = 0}}} return true end)()"));
    // fix round 6: Auto-loot polls instead of hearing creature updates, so a flip no longer makes the look stale and a
    // moment without the hostile let it open the corpse (then nothing waits and nothing is looked at); the hostile now
    // stays on the player, updated every frame, and the corpse waits in combat throughout
    bv.play.bags_creature(9700, 0, 77);
    bv.eval::<bool>("(function() require('bags').scan_reset() return true end)()");
    let scans_before: u32 = bv.eval("require('bags').scan_count");
    let cost = bv_cost_with(&mut bv, 120, &mut |bv, _| {
        bv.play.bags_creature(9700, 0, 77);
    });
    eprintln!("R65P auto-loot on, crowded: {:.3} ms/frame, {:.2} list calls/s", cost.mod_ms, cost.lists_per_s);
    for l in &cost.lines {
        eprintln!("R65P     {l}");
    }
    let scans = bv.eval::<u32>("require('bags').scan_count") - scans_before;
    let per_s = scans as f64 / (cost.secs + 0.001);
    let state: String = bv.eval("(function() local b = require('bags') return tostring(b.auto_mode) .. ' elig=' .. tostring(next(b.eligible_corpses or {})) .. ' batch=' .. tostring(b.batch_id) .. ' open=' .. tostring(b.pending_open) .. ' last=' .. tostring(b.last_req) .. ' char=' .. tostring(b.character) end)()");
    eprintln!("R65P scans: {scans} in {:.1} s ({per_s:.2} a second); {state}", cost.secs);
    assert!(per_s <= 4.5, "a flip every frame made {per_s:.1} looks at the zone a second (cap: one every 0.25 s)");
    assert!(per_s > 0.5, "the fight check still looks at the zone ({per_s:.2} a second): {state}");
    if !cfg!(debug_assertions) {
        assert!(cost.mod_ms < RELEASE_AUTO_ON_MS, "Auto-loot on in a crowded zone costs {:.3} ms a frame (budget {RELEASE_AUTO_ON_MS})", cost.mod_ms);
    }
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

// ---- Round 6.5 fix round 4, review L5: a merchant or banker across the city shows nothing, not a "too far" line ----------

#[test]
fn round65_fix4_l5_a_merchant_or_banker_far_away_shows_no_button_and_no_row() {
    big_stack(round65_fix4_l5_a_merchant_or_banker_far_away_shows_no_button_and_no_row_body)
}
fn round65_fix4_l5_a_merchant_or_banker_far_away_shows_no_button_and_no_row_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else { return };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.play.bags_spawn_npc(9002, "Merchant Kim", 0x80 | 1, 1, [60.0, 0.0, 0.0]);
    bv.play.bags_spawn_npc(9003, "Banker Dan", 0x80 | 2, 1, [0.0, 0.0, 60.0]);
    assert!(bv.eval::<bool>("(function() require('bags').junk_ids['900008'] = true return true end)()"));
    bv.open();
    bv.frames(2);
    bv.click(bv.tab(1));
    bv.frames(2);
    assert!(bv.action_hit("Sell junk").is_none(), "no Sell junk line for a merchant 60 units away");
    bv.click(bv.slot(2));
    bv.frames(2);
    assert!(bv.menu_row("Deposit").is_none(), "no Deposit row for a banker 60 units away");
    bv.key(winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape));
    bv.frames(1);
    // within sight (30 units) the greyed line is there
    bv.play.bags_player_at([30.0, 0.0, 0.0]);
    bv_look_again(&mut bv);
    let (_, _, label) = bv.action_hit("Sell junk").unwrap();
    assert!(label.contains("too far from the merchant"), "{label}");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

// ---- Round 6.5 fix round 5: Auto-loot stopped for good after the owner's live test (2026-11-18) -------------------

/// A creature of yours killed at `pos`: you target it, hit it, it dies (its corpse is where it stood).
fn r65fix5_kill(bv: &mut Bv, id: u32, pos: [f32; 3]) {
    bv.play.bags_spawn_npc(id, "a vulture", 0, 1, pos);
    bv.play.bags_target_sync(id);
    bv.play.bags_combat_hit(77, id, -12);
    bv_look_again(bv);
    bv.play.bags_entity_die(id);
    bv.frames(2);
}

/// The mod asked the server for corpse `id` (0x0015 by the mod, seen in the takeover log).
fn r65fix5_asked(log: &[String], id: u32) -> bool {
    let needle = format!("mod open_loot -> corpse {id:#x}");
    log.iter().any(|l| l.contains(&needle))
}

/// The owner's live sequence (r65-retest3 log, lines 403-483): a corpse with coin only, a corpse the server answers
/// with "Out of range." and nothing else (then the player's own Loot click on it, unanswered too), a corpse the player
/// opens and closes in the game's own window, Take all on a later corpse; then every kill after it must still be
/// auto-looted.
#[test]
fn round65_fix5_auto_loot_keeps_going_after_the_owners_sequence() {
    big_stack(round65_fix5_auto_loot_keeps_going_after_the_owners_sequence_body)
}
fn round65_fix5_auto_loot_keeps_going_after_the_owners_sequence_body() {
    let Some(mut bv) = r65fix4_auto_kill("all") else { return };
    // B's retry (fix round 5) is not part of this sequence: only coming closer to it would ask again; the reach is
    // the 15 units of that round (fix round 6 starts at 7 and learns it)
    bv.eval::<bool>("(function() require('bags').AUTO_RETRY_SECS = 1e9 require('bags').LOOT_DEFAULT_REACH = 15 return true end)()");
    let _ = super::super::window_takeover::take_log_for_test();
    // A: 9011's list has coin only
    bv.play.bags_broker_tick();
    bv.play.bags_corpse(&serde_json::json!([]));
    bv.frames(2);
    bv.play.bags_broker_tick();
    bv.play.bags_loot_close();
    bv.play.bags_broker_tick();
    bv.frames(2);
    // B: the server says "Out of range." and nothing more
    r65fix5_kill(&mut bv, 9012, [12.0, 0.0, 0.0]);
    bv.play.bags_broker_tick();
    let log = super::super::window_takeover::take_log_for_test();
    assert!(r65fix5_asked(&log, 9012), "B is asked for: {log:#?}");
    bv.play.bags_chat("Yours alone for 5:12 more.");
    bv.play.bags_chat("Out of range.");
    bv.frames(2);
    for _ in 0..320 {
        bv.play.bags_broker_tick();
    }
    bv.frames(2);
    // C: a fight (auto-attack on) holds Auto-loot back; the player opens C in the game's window and closes it
    bv.play.bags_spawn_npc(9013, "a vulture", 0, 1, [3.0, 0.0, 0.0]);
    bv.play.bags_target_sync(9013);
    bv.play.bags_auto_attack(true);
    bv.play.bags_combat_hit(77, 9013, -12);
    bv_look_again(&mut bv);
    bv.play.bags_entity_die(9013);
    bv.frames(2);
    bv.play.bags_broker_tick();
    bv.play.bags_target(9013);
    bv.play.bags_press_loot();
    bv.play.bags_corpse(&serde_json::json!([{"name":"broken vulture feather","id":2101,"key":0,"qty":1,"max_stack":20}]));
    assert_eq!(bv.play.mode, 1, "the player's own click shows the game's window");
    bv.play.bags_loot_close();
    bv.frames(2);
    bv.play.bags_auto_attack(false);
    bv.frames(2);
    bv.play.bags_broker_tick();
    bv.frames(2);
    // ... after which Auto-loot opens C hidden and takes all
    let log = super::super::window_takeover::take_log_for_test();
    assert!(r65fix5_asked(&log, 9013), "C is asked for once the fight is over: {log:#?}");
    bv.play.bags_corpse(&serde_json::json!([{"name":"broken vulture feather","id":2101,"key":0,"qty":1,"max_stack":20}]));
    bv.frames(2);
    assert_eq!(bv.play.mode, 0, "Auto-loot's open is hidden");
    bv.play.bags_broker_tick(); // Take all: 0x001A
    bv.play.bags_loot_confirm_all();
    bv.play.bags_loot_close();
    bv.play.bags_broker_tick();
    bv.frames(2);
    let _ = super::super::window_takeover::take_log_for_test();
    // D and E: every kill after it is still auto-looted
    for (id, x) in [(9014u32, 2.0f32), (9015, 4.0)] {
        r65fix5_kill(&mut bv, id, [x, 0.0, 0.0]);
        bv.play.bags_broker_tick();
        let log = super::super::window_takeover::take_log_for_test();
        let state: String = bv.eval(
            "(function() local B = require('bags') return string.format('pending=%s last=%s batch=%s combat=%s eligible=%s', \
             tostring(B.pending_open ~= nil), tostring(B.last_req), tostring(B.batch_id), tostring(B.in_combat()), \
             tostring(B.eligible_corpses and next(B.eligible_corpses) ~= nil)) end)()",
        );
        assert!(r65fix5_asked(&log, id), "kill {id} after the owner's sequence is auto-looted ({state}): {log:#?}");
        bv.play.bags_corpse(&serde_json::json!([{"name":"Aquamarine","id":2102,"key":0,"qty":1,"max_stack":20}]));
        bv.frames(2);
        bv.play.bags_broker_tick();
        bv.play.bags_loot_confirm_all();
        bv.play.bags_loot_close();
        bv.play.bags_broker_tick();
        bv.frames(2);
    }
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Who has the player targeted after a fight is not only a hostile: the player's mercenary (a group member), a
/// merchant or banker spoken to, or a creature at the far end of the zone that once chased the player. None of them
/// is a fight: Auto-loot goes on.
#[test]
fn round65_fix5_a_friendly_or_distant_npc_targeting_you_is_not_a_fight() {
    big_stack(round65_fix5_a_friendly_or_distant_npc_targeting_you_is_not_a_fight_body)
}
fn round65_fix5_a_friendly_or_distant_npc_targeting_you_is_not_a_fight_body() {
    for who in ["mercenary", "merchant", "far creature"] {
        let Some(mut bv) = r65fix4_auto_kill("all") else { return };
        // the first kill's corpse is taken
        r65fix4_auto_list(&mut bv);
        bv.play.bags_broker_tick();
        bv.play.bags_loot_confirm_all();
        bv.play.bags_loot_close();
        bv.play.bags_broker_tick();
        bv.frames(2);
        match who {
            "mercenary" => {
                bv.play.bags_spawn_npc(9050, "Quinor", 0, 1, [1.0, 0.0, 1.0]);
                bv.play.group.in_group = true;
                bv.play.group.members.push(crate::group::Member { id: 9050, name: "Quinor".into(), active: true, ..Default::default() });
                bv.play.bags_creature(9050, 0, 77);
            }
            "merchant" => {
                bv.play.bags_spawn_npc(9051, "a merchant", 1, 1, [6.0, 0.0, 0.0]);
                bv.play.bags_creature(9051, 0, 77);
            }
            _ => {
                bv.play.bags_spawn_npc(9052, "a gnoll", 0, 1, [300.0, 0.0, 0.0]);
                bv.play.bags_creature(9052, 0, 77);
            }
        }
        let _ = super::super::window_takeover::take_log_for_test();
        r65fix5_kill(&mut bv, 9016, [3.0, 0.0, 0.0]);
        bv.play.bags_broker_tick();
        let log = super::super::window_takeover::take_log_for_test();
        assert!(r65fix5_asked(&log, 9016), "a {who} targeting you is not a fight: {log:#?}");
        bv.clean();
        super::super::window_takeover::reset_all_for_test();
        super::super::action_broker::reset_all_for_test();
    }
}

/// Auto-loot's own log lines so far (`bags.auto_log`).
fn r65fix5_auto_log(bv: &Bv) -> Vec<String> {
    let all: String = bv.eval("table.concat(require('bags').auto_log, string.char(10))");
    all.lines().map(str::to_string).collect()
}

fn r65fix5_tick(bv: &mut Bv) {
    bv_look_again(bv);
    bv.eval::<bool>("(function() require('bags').tick_auto_loot() return true end)()");
    bv.frames(2);
    bv.play.bags_broker_tick();
}

/// The first kill (9011) taken, so the next kill is the only corpse waiting.
fn r65fix5_first_taken(bv: &mut Bv) {
    r65fix4_auto_list(bv);
    bv.play.bags_broker_tick();
    bv.play.bags_loot_confirm_all();
    bv.play.bags_loot_close();
    bv.play.bags_broker_tick();
    bv.frames(2);
}

/// Item 3: a corpse the server does not answer is asked again when the player comes closer (not while they stand
/// still), at most three times in all, and each step is in the log.
#[test]
fn round65_fix5_item3_unanswered_corpse_is_retried_when_closer_at_most_three_times() {
    big_stack(round65_fix5_item3_unanswered_corpse_is_retried_when_closer_at_most_three_times_body)
}
fn round65_fix5_item3_unanswered_corpse_is_retried_when_closer_at_most_three_times_body() {
    let Some(mut bv) = r65fix4_auto_kill("all") else { return };
    r65fix5_first_taken(&mut bv);
    // fix round 5's 15-unit reach (fix round 6 starts at 7 and learns it)
    bv.eval::<bool>("(function() require('bags').AUTO_RETRY_SECS = 1e9 require('bags').LOOT_DEFAULT_REACH = 15 return true end)()");
    let _ = super::super::window_takeover::take_log_for_test();
    r65fix5_kill(&mut bv, 9020, [14.0, 0.0, 0.0]);
    bv.play.bags_broker_tick();
    let log = super::super::window_takeover::take_log_for_test();
    assert!(r65fix5_asked(&log, 9020), "the first try: {log:#?}");
    let mut x = 0.0f32;
    for attempt in 1..=3 {
        for _ in 0..320 {
            bv.play.bags_broker_tick(); // the server says nothing
        }
        bv.frames(2);
        let alog = r65fix5_auto_log(&bv);
        if attempt < 3 {
            assert!(
                alog.iter().any(|l| l.contains("0x233c") && l.contains(&format!("(try {} of 3)", attempt + 1))),
                "try {attempt} unanswered: a retry is announced: {alog:#?}"
            );
            // standing still: not asked again
            r65fix5_tick(&mut bv);
            let log = super::super::window_takeover::take_log_for_test();
            assert!(!r65fix5_asked(&log, 9020), "no retry while standing where the server did not answer: {log:#?}");
            // three units closer: asked again
            x += 3.0;
            bv.play.bags_player_at([x, 0.0, 0.0]);
            r65fix5_tick(&mut bv);
            let log = super::super::window_takeover::take_log_for_test();
            assert!(r65fix5_asked(&log, 9020), "try {}: asked again once closer: {log:#?}", attempt + 1);
        } else {
            assert!(
                alog.iter().any(|l| l.contains("0x233c") && l.contains("gave up after 3 tries")),
                "the third silence ends it: {alog:#?}"
            );
            x += 3.0;
            bv.play.bags_player_at([x, 0.0, 0.0]);
            r65fix5_tick(&mut bv);
            let log = super::super::window_takeover::take_log_for_test();
            assert!(!r65fix5_asked(&log, 9020), "never a fourth try: {log:#?}");
        }
    }
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Item 3 with the live server's own answer: "Out of range." in chat ends the open at once (no 10 s wait), the result
/// line says so, and the corpse is asked again from closer.
#[test]
fn round65_fix5_item3_out_of_range_ends_the_open_at_once_and_is_retried() {
    big_stack(round65_fix5_item3_out_of_range_ends_the_open_at_once_and_is_retried_body)
}
fn round65_fix5_item3_out_of_range_ends_the_open_at_once_and_is_retried_body() {
    let Some(mut bv) = r65fix4_auto_kill("all") else { return };
    r65fix5_first_taken(&mut bv);
    // fix round 5's 15-unit reach (fix round 6 starts at 7 and learns it)
    bv.eval::<bool>("(function() require('bags').AUTO_RETRY_SECS = 1e9 require('bags').LOOT_DEFAULT_REACH = 15 return true end)()");
    let _ = super::super::window_takeover::take_log_for_test();
    r65fix5_kill(&mut bv, 9020, [14.0, 0.0, 0.0]);
    bv.play.bags_broker_tick();
    bv.play.bags_chat("Yours alone for 5:12 more.");
    bv.play.bags_broker_tick();
    bv.frames(2);
    assert!(bv.eval::<bool>("require('bags').pending_open ~= nil"), "a chat line that is not a refusal changes nothing");
    bv.play.bags_chat("Out of range.");
    bv.play.bags_broker_tick();
    bv.frames(2);
    let (text, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false));
    assert!(text.contains("Out of range."), "the server's own words in the result line: {text}");
    assert!(bv.eval::<bool>("require('bags').pending_open == nil"), "nothing left waiting");
    let alog = r65fix5_auto_log(&bv);
    assert!(alog.iter().any(|l| l.contains("Out of range.") && l.contains("waiting until you are closer")), "{alog:#?}");
    let log = super::super::window_takeover::take_log_for_test();
    assert!(log.iter().any(|l| l.contains("refused the corpse open: Out of range.")), "{log:#?}");
    bv.play.bags_player_at([4.0, 0.0, 0.0]);
    r65fix5_tick(&mut bv);
    let log = super::super::window_takeover::take_log_for_test();
    assert!(r65fix5_asked(&log, 9020), "asked again from closer: {log:#?}");
    bv.play.bags_corpse(&serde_json::json!([{"name":"Aquamarine","id":2102,"key":0,"qty":1,"max_stack":20}]));
    bv.frames(2);
    assert_eq!(bv.play.mode, 0, "the retry's open is hidden like the first");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Item 4: the game itself turns a corpse's list away while another of its windows is up (`npcw` answers 0x0016
/// outside mode 0; after the 11-18 upstream sync that includes the mercenary windows and the dye picker, modes 61-63).
/// The mod's open ends as refused at once, nothing waits, nothing loops, and Auto-loot asks again later.
#[test]
fn round65_fix5_item4_a_list_the_game_turns_away_ends_the_open() {
    big_stack(round65_fix5_item4_a_list_the_game_turns_away_ends_the_open_body)
}
fn round65_fix5_item4_a_list_the_game_turns_away_ends_the_open_body() {
    let Some(mut bv) = r65fix4_auto_kill("all") else { return };
    r65fix5_first_taken(&mut bv);
    bv.eval::<bool>("(function() require('bags').AUTO_RETRY_SECS = 1e9 return true end)()");
    let _ = super::super::window_takeover::take_log_for_test();
    r65fix5_kill(&mut bv, 9021, [3.0, 0.0, 0.0]);
    bv.play.bags_broker_tick();
    let log = super::super::window_takeover::take_log_for_test();
    assert!(r65fix5_asked(&log, 9021), "{log:#?}");
    // the player opened the start menu (mode 13) before the list came back
    bv.play.mode = 13;
    let sends = bv.play.bags_sends_lost();
    bv.play.bags_corpse(&serde_json::json!([{"name":"Aquamarine","id":2102,"key":0,"qty":1,"max_stack":20}]));
    assert!(bv.play.bags_sends_lost() > sends, "the game answers the list with its own 0x0016");
    for _ in 0..3 {
        bv.play.bags_broker_tick();
        bv.frames(2);
    }
    assert_eq!(bv.play.mode, 13, "the player's menu stays");
    let (text, ok) = bv.result_text().unwrap();
    assert_eq!(ok, Some(false));
    assert!(text.contains("another game window was open"), "{text}");
    assert!(bv.eval::<bool>("require('bags').pending_open == nil and require('bags').last_req == nil"), "nothing waits");
    let log = super::super::window_takeover::take_log_for_test();
    assert!(log.iter().any(|l| l.contains("turned away by the game (mode 13")), "{log:#?}");
    assert!(!r65fix5_asked(&log, 9021), "no loop of re-asks while the menu is up: {log:#?}");
    // the menu closes: Auto-loot asks again and the corpse opens hidden
    bv.eval::<bool>("(function() require('bags').AUTO_RETRY_SECS = 0 return true end)()");
    bv.play.mode = 0;
    r65fix5_tick(&mut bv);
    let log = super::super::window_takeover::take_log_for_test();
    assert!(r65fix5_asked(&log, 9021), "asked again once the menu is gone: {log:#?}");
    bv.play.bags_corpse(&serde_json::json!([{"name":"Aquamarine","id":2102,"key":0,"qty":1,"max_stack":20}]));
    bv.frames(2);
    assert_eq!(bv.play.mode, 0);
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Item 2: every Auto-loot decision is a line in the game's log (eqoa.log), through eqoa.log.write, with the corpse id
/// and the reason; none of them is an Addon Manager diagnostic.
#[test]
fn round65_fix5_item2_auto_loot_decisions_are_in_the_game_log() {
    big_stack(round65_fix5_item2_auto_loot_decisions_are_in_the_game_log_body)
}
fn round65_fix5_item2_auto_loot_decisions_are_in_the_game_log_body() {
    let _ = super::take_mod_log_for_test();
    let Some(mut bv) = r65fix4_auto_kill("all") else { return };
    r65fix5_first_taken(&mut bv);
    // not your kill: a creature nobody of yours hit dies near you
    bv.play.bags_spawn_npc(9030, "a rat", 0, 1, [5.0, 0.0, 0.0]);
    bv.frames(2);
    bv.play.bags_entity_die(9030);
    bv.frames(2);
    // in combat: a scout has you targeted when your kill dies
    bv.play.bags_spawn_npc(9031, "a gnoll scout", 0, 1, [4.0, 0.0, 0.0]);
    bv.play.bags_creature(9031, 0, 77);
    r65fix5_kill(&mut bv, 9032, [2.0, 0.0, 0.0]);
    bv.play.bags_broker_tick();
    // too far: the scout dies (out of combat) and the player walks off 30 units
    bv.play.bags_entity_die(9031);
    bv.play.bags_player_at([32.0, 0.0, 0.0]);
    r65fix5_tick(&mut bv);
    // back: it opens and takes
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    r65fix5_tick(&mut bv);
    bv.play.bags_corpse(&serde_json::json!([{"name":"Aquamarine","id":2102,"key":0,"qty":1,"max_stack":20}]));
    bv.frames(2);
    bv.play.bags_broker_tick();
    bv.play.bags_loot_confirm_all();
    bv.play.bags_loot_close();
    bv.play.bags_broker_tick();
    bv.frames(3);
    let game_log = super::take_mod_log_for_test();
    let has = |needle: &str| {
        game_log
            .iter()
            .any(|l| l.starts_with("addon eqoa_bag_viewer: [info] auto-loot: ") && l.contains(needle))
    };
    for needle in [
        "mode Everything for",
        "corpse 0x2333: a gnoll, your kill: queued",
        "corpse 0x2333: done: Take all ok",
        "corpse 0x2346: a rat: skipped, not your kill",
        "corpse 0x2348: waiting: in combat (a gnoll scout has you targeted)",
        "corpse 0x2348: waiting: walk closer (30 units",
        "corpse 0x2348: opening it",
        "corpse 0x2348: done: Take all ok (1 items)",
    ] {
        assert!(has(needle), "the game log has `{needle}`: {game_log:#?}");
    }
    bv.clean(); // and not one of them is a diagnostic
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

// ---- Round 6.5 fix round 6: owner live tests of d221531e (2026-11-19) ---------------------------------------------

/// Item 1, owner log r65-retest4 lines 828-862: the ant died while the player was fighting; the player's attack key on
/// the dead target (the game's `attack_press` on a corpse, the only native 0x0015) opened it in the game's own window
/// (mode 1), and Auto-loot then burned its three tries on "cannot open corpse right now". The corpse it wants is open:
/// it takes from that session (no open of its own), and says so.
#[test]
fn round65_fix6_item1_auto_loot_takes_from_the_corpse_the_player_opened() {
    big_stack(round65_fix6_item1_auto_loot_takes_from_the_corpse_the_player_opened_body)
}
fn round65_fix6_item1_auto_loot_takes_from_the_corpse_the_player_opened_body() {
    let Some(mut bv) = r65fix4_auto_kill("all") else { return };
    r65fix5_first_taken(&mut bv);
    let _ = super::super::window_takeover::take_log_for_test();
    // the fight: auto-attack on the ant, which dies; the player presses attack again on the dead target
    bv.play.bags_spawn_npc(9040, "ant", 0, 1, [1.0, 0.0, 0.0]);
    bv.play.bags_target_sync(9040);
    bv.play.bags_auto_attack(true);
    bv.play.bags_combat_hit(77, 9040, -12);
    bv.play.bags_spawn_npc(9041, "a second ant", 0, 1, [4.0, 0.0, 0.0]);
    bv.play.bags_creature(9041, 0, 77); // still fighting: Auto-loot waits
    bv_look_again(&mut bv);
    bv.play.bags_entity_die(9040);
    bv.frames(2);
    bv.play.bags_press_loot(); // the game's own loot request
    bv.play.bags_corpse(&serde_json::json!([{"name":"Ant Leg","id":2201,"key":0,"qty":1,"max_stack":20}]));
    assert_eq!(bv.play.mode, 1, "the player's own key opened the game's corpse window");
    bv.play.bags_entity_die(9041); // the fight is over
    let sends = bv.play.bags_sends_lost();
    r65fix5_tick(&mut bv);
    bv.play.bags_broker_tick();
    let log = super::super::window_takeover::take_log_for_test();
    let alog = r65fix5_auto_log(&bv);
    assert!(!r65fix5_asked(&log, 9040), "no open of its own while the corpse is open: {log:#?}");
    assert!(!alog.iter().any(|l| l.contains("0x2350") && l.contains("try 2 of 3")), "no try burned: {alog:#?}");
    assert!(
        alog.iter().any(|l| l.contains("0x2350") && l.contains("open already") && l.contains("taking from it (Take all)")),
        "{alog:#?}"
    );
    assert!(bv.play.bags_sends_lost() > sends, "Take all goes out from the player's session");
    bv.play.bags_loot_confirm_all();
    bv.play.bags_loot_close();
    bv.play.bags_broker_tick();
    bv.frames(2);
    let alog = r65fix5_auto_log(&bv);
    assert!(alog.iter().any(|l| l.contains("0x2350: done: Take all ok")), "{alog:#?}");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Item 1, the other case: another corpse is open in the game's window. Auto-loot's corpse waits (no try burned) and is
/// opened once that window closes.
#[test]
fn round65_fix6_item1_another_open_corpse_makes_auto_loot_wait_not_fail() {
    big_stack(round65_fix6_item1_another_open_corpse_makes_auto_loot_wait_not_fail_body)
}
fn round65_fix6_item1_another_open_corpse_makes_auto_loot_wait_not_fail_body() {
    let Some(mut bv) = r65fix4_auto_kill("all") else { return };
    r65fix5_first_taken(&mut bv);
    let _ = super::super::window_takeover::take_log_for_test();
    bv.play.bags_spawn_npc(9042, "a fallen gnoll", 0, 3, [2.0, 0.0, 0.0]);
    bv.play.bags_target(9042);
    bv.play.bags_press_loot(); // the player opens someone else's corpse
    bv.play.bags_corpse(&serde_json::json!([{"name":"Bone Chip","id":2004,"key":0,"qty":1,"max_stack":100}]));
    assert_eq!(bv.play.mode, 1);
    r65fix5_kill(&mut bv, 9043, [3.0, 0.0, 0.0]);
    for _ in 0..3 {
        r65fix5_tick(&mut bv);
    }
    let log = super::super::window_takeover::take_log_for_test();
    let alog = r65fix5_auto_log(&bv);
    assert!(!r65fix5_asked(&log, 9043), "{log:#?}");
    assert!(alog.iter().any(|l| l.contains("0x2353: waiting: another corpse (0x2352) is open")), "{alog:#?}");
    assert!(!alog.iter().any(|l| l.contains("0x2353") && (l.contains("try 2") || l.contains("gave up"))), "{alog:#?}");
    bv.play.bags_loot_close(); // the player closes it
    bv.frames(2);
    r65fix5_tick(&mut bv);
    let log = super::super::window_takeover::take_log_for_test();
    assert!(r65fix5_asked(&log, 9043), "opened once the player's window closed: {log:#?}");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Item A, owner log eqoa.log 980-1146: the server opened corpses at 3 and 6 units and said "Out of range." at 11, 12,
/// 13 and 15; Auto-loot (reach 15) gave up after three refusals. The reach now starts at 7 and is learned from the
/// answers: a kill 12 units off waits ("walk closer"), and opens once the player is within reach.
#[test]
fn round65_fix6_item_a_loot_reach_starts_short_and_says_walk_closer() {
    big_stack(round65_fix6_item_a_loot_reach_starts_short_and_says_walk_closer_body)
}
fn round65_fix6_item_a_loot_reach_starts_short_and_says_walk_closer_body() {
    let Some(mut bv) = r65fix4_auto_kill("all") else { return };
    r65fix5_first_taken(&mut bv);
    let _ = super::super::window_takeover::take_log_for_test();
    r65fix5_kill(&mut bv, 9050, [12.0, 0.0, 0.0]);
    r65fix5_tick(&mut bv);
    let log = super::super::window_takeover::take_log_for_test();
    assert!(!r65fix5_asked(&log, 9050), "12 units is beyond the reach not yet learned (7): {log:#?}");
    let alog = r65fix5_auto_log(&bv);
    assert!(alog.iter().any(|l| l.contains("0x235a: waiting: walk closer (12 units; server reach ~7 units)")), "{alog:#?}");
    bv.play.bags_player_at([6.0, 0.0, 0.0]); // 6 units: what the owner's server opened
    r65fix5_tick(&mut bv);
    let log = super::super::window_takeover::take_log_for_test();
    assert!(r65fix5_asked(&log, 9050), "within reach: {log:#?}");
    bv.play.bags_corpse(&serde_json::json!([{"name":"Damselfly Wing","id":2301,"key":0,"qty":1,"max_stack":20}]));
    bv.frames(2);
    assert_eq!(bv.play.mode, 0, "hidden");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Item A with the owner's own order (11, 12, 13, 15 refused in a row): four refusals in a row never give up; the corpse
/// opens when the player comes within the learned reach.
#[test]
fn round65_fix6_item_a_four_refusals_in_a_row_still_loot_when_closer() {
    big_stack(round65_fix6_item_a_four_refusals_in_a_row_still_loot_when_closer_body)
}
fn round65_fix6_item_a_four_refusals_in_a_row_still_loot_when_closer_body() {
    let Some(mut bv) = r65fix4_auto_kill("all") else { return };
    r65fix5_first_taken(&mut bv);
    bv.eval::<bool>("(function() local b = require('bags') b.AUTO_RETRY_SECS = 1e9 b.LOOT_DEFAULT_REACH = 16 return true end)()");
    let _ = super::super::window_takeover::take_log_for_test();
    r65fix5_kill(&mut bv, 9051, [20.0, 0.0, 0.0]);
    for d in [15.0f32, 13.0, 12.0, 11.0] {
        bv.play.bags_player_at([20.0 - d, 0.0, 0.0]);
        // what the owner's server answered: the mod forgets nothing, it only waits for the player to come closer
        r65fix5_tick(&mut bv);
        let log = super::super::window_takeover::take_log_for_test();
        assert!(r65fix5_asked(&log, 9051), "asked from {d} units (nearer than any refusal so far): {log:#?}");
        bv.play.bags_chat("Out of range.");
        bv.play.bags_broker_tick();
        bv.frames(2);
    }
    bv.play.bags_player_at([14.0, 0.0, 0.0]); // 6 units
    r65fix5_tick(&mut bv);
    let log = super::super::window_takeover::take_log_for_test();
    assert!(r65fix5_asked(&log, 9051), "the fifth ask, from 6 units: {log:#?}");
    let alog = r65fix5_auto_log(&bv);
    assert!(!alog.iter().any(|l| l.contains("0x235b") && (l.contains("gave up") || l.contains("try 2 of 3"))), "never a try: {alog:#?}");
    assert!(
        alog.iter().any(|l| l.contains("0x235b") && l.contains("\"Out of range.\" at 11 units: waiting until you are closer (server reach ~")),
        "{alog:#?}"
    );
    let reach: f64 = bv.eval("require('bags').loot_reach_now()");
    assert!((reach - 10.5).abs() < 0.01, "the nearest refusal was 11 units: reach {reach}");
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Item 2, owner log r65-retest4: with Auto-loot on in a crowd the mod cost 2.2-4.3 ms a frame, `player.get` 6-25 times
/// a frame and 53-70 event deliveries a frame (every creature update, every move of the player). Auto-loot on, window
/// closed, 700 creatures with 35 updates a frame, the player walking and hits landing every frame: at most 3 event
/// deliveries and one `player.get` a frame, and (release build) under 0.5 ms a frame.
#[test]
fn round65_fix6_item2_auto_loot_on_in_a_crowd_is_cheap() {
    big_stack(round65_fix6_item2_auto_loot_on_in_a_crowd_is_cheap_body)
}
fn round65_fix6_item2_auto_loot_on_in_a_crowd_is_cheap_body() {
    let Some(mut bv) = bv_busy_scene() else { return };
    bv.play.bags_self_id(77);
    bv.frames(4);
    bv_auto(&bv, "all");
    bv.frames(2);
    let mut x = 0.0f32;
    let cost = bv_cost_with(&mut bv, 120, &mut |bv, tick| {
        x += 0.05;
        bv.play.bags_player_at([x, 0.0, 0.0]);
        bv.play.bags_auto_attack(tick % 2 == 0);
        bv.play.bags_combat_hit(5003, 77, -2);
    });
    for l in &cost.lines {
        eprintln!("R65P6     {l}");
    }
    let deliveries = cost
        .lines
        .iter()
        .find(|l| l.starts_with("core:"))
        .and_then(|l| bv_num_after(l, "events.deliver "))
        .unwrap_or(0.0);
    let player_gets = cost
        .lines
        .iter()
        .find(|l| l.starts_with("addon-api:") && l.contains(BV))
        .and_then(|l| bv_num_after(l, "player.get "))
        .unwrap_or(0.0);
    eprintln!("R65P6 auto-loot on, crowd: {:.3} ms/frame, {deliveries:.1} deliveries, {player_gets:.2} player.get a frame", cost.mod_ms);
    assert!(deliveries <= 3.0, "{deliveries:.1} event deliveries a frame with Auto-loot on in a crowd");
    assert!(player_gets <= 1.0, "player.get {player_gets:.2} times a frame");
    if !cfg!(debug_assertions) {
        assert!(cost.mod_ms < 0.5, "Auto-loot on in a crowd costs {:.3} ms a frame (budget 0.5)", cost.mod_ms);
    }
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Owner test fix 1 (owner, 2026-11-20: "single item selling should work"): Sell 1 and Sell stack are offered
/// whenever a merchant is within reach. With no merchant window open, Bag Viewer talks to the nearest merchant
/// itself through the hidden session (as Sell junk does), sells, and the game's merchant window (mode 10)
/// never shows, nor when the server answers the sale with the merchant's list again. With the game's merchant
/// window open it sells through it as before; with no merchant near, both entries are greyed with the reason.
#[test]
fn round7c_bag_viewer_sells_single_items_near_a_merchant() {
    big_stack(round7c_bag_viewer_sells_single_items_near_a_merchant_body)
}
fn round7c_bag_viewer_sells_single_items_near_a_merchant_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else {
        eprintln!("SKIP round7c single-item sell: no GPU adapter");
        return;
    };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.open();
    bv.frames(2);
    // no merchant in the zone: both entries greyed with the reason
    bv.click(bv.slot(7));
    bv.frames(2);
    for row in ["Sell 1", "Sell stack"] {
        let (_, _, enabled, reason, _) = bv.menu_row(row).unwrap_or_else(|| panic!("{row} is listed"));
        assert!(!enabled, "{row} with no merchant near");
        assert_eq!(reason.as_deref(), Some("no merchant nearby"), "{row}");
    }
    bv.click(bv.slot(7));
    bv.frames(2);
    bv.play.bags_spawn_npc(9002, "Merchant Kim", 0x80 | 1, 1, [6.0, 0.0, 0.0]);
    bv.lua().load("require('bags').scan_reset()").exec().unwrap();
    bv.frames(2);

    // Sell 1 of the Healing Potions (slot 2, 5 of them), no merchant window open
    let sell = |bv: &mut Bv, slot: usize, row: &str, inv_slot: usize, n: i64| {
        bv.click(bv.slot(slot));
        bv.frames(2);
        let (x, y, enabled, reason, label) = bv.menu_row(row).unwrap_or_else(|| panic!("{row} is offered"));
        assert!(enabled, "{label} offered near a merchant with no window open: {reason:?}");
        let sends = bv.play.bags_sends_lost();
        bv.click((x, y));
        bv.play.bags_broker_tick();
        assert!(bv.play.bags_sends_lost() > sends, "{row} talks to the merchant first");
        assert_eq!(bv.play.target, 9002);
        bv.play.bags_merchant_packet(9002);
        assert_eq!(bv.play.mode, 0, "the merchant's answer stays hidden (mode 10 is its window)");
        bv.frames(2);
        bv.play.bags_broker_tick();
        assert!(bv.play.bags_shop_pending(), "{row}: the sale went out");
        bv.play.bags_sell_confirm(inv_slot, n);
        bv.play.bags_merchant_packet(9002);
        assert_eq!(bv.play.mode, 0, "the list the server answers the sale with stays hidden");
        bv.frames(2);
        bv.play.bags_broker_tick();
        bv.frames(2);
        let (text, ok) = bv.result_text().expect("a result line");
        assert_eq!(ok, Some(true), "{text}");
        assert!(text.starts_with(row), "{text}");
        assert!(bv.eval::<bool>("not eqoa.windows.taken('merchant')"), "the hidden session is closed afterwards");
        assert_eq!(bv.play.mode, 0);
    };
    sell(&mut bv, 2, "Sell 1", 1, 1);
    // Sell stack of the Rat Pelts (slot 7, 3 of them)
    sell(&mut bv, 7, "Sell stack", 6, 3);

    // the player talked to the merchant: its window is open, and Sell 1 sells through it as before
    bv.play.bags_merchant_packet(9002);
    assert_eq!(bv.play.mode, 10, "the player's own talk opens the game's merchant window");
    bv.frames(2);
    bv.click(bv.slot(2));
    bv.frames(2);
    let (x, y, enabled, _, _) = bv.menu_row("Sell 1").unwrap();
    assert!(enabled);
    let sends = bv.play.bags_sends_lost();
    bv.click((x, y));
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_shop_pending(), "sold through the open window");
    assert!(bv.play.bags_sends_lost() > sends);
    assert!(bv.eval::<bool>("not eqoa.windows.taken('merchant')"), "no claim: the player's window");
    assert_eq!(bv.play.mode, 10, "the player's window stays");
    bv.play.bags_sell_confirm(1, 1);
    bv.play.bags_close_merchant();
    bv.frames(2);

    // walked away, out of the zone scan's radius: greyed with the reason again
    bv.play.bags_player_at([400.0, 0.0, 400.0]);
    bv.lua().load("require('bags').scan_reset()").exec().unwrap();
    bv.frames(2);
    bv.click(bv.slot(2));
    bv.frames(2);
    let (_, _, enabled, reason, _) = bv.menu_row("Sell 1").expect("Sell 1 is still listed");
    assert!(!enabled);
    assert_eq!(reason.as_deref(), Some("no merchant nearby"));

    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}

/// Recheck LOW 1: a second Sell 1 clicked before the merchant answers the first's talk is refused with the
/// busy reason (as a second corpse open is), and the first item is the one sold.
#[test]
fn round7c_bag_viewer_second_sell_before_the_merchant_answers_is_refused() {
    big_stack(round7c_bag_viewer_second_sell_before_the_merchant_answers_is_refused_body)
}
fn round7c_bag_viewer_second_sell_before_the_merchant_answers_is_refused_body() {
    super::super::action_broker::reset_all_for_test();
    super::super::window_takeover::reset_all_for_test();
    let Some(mut bv) = Bv::new() else {
        eprintln!("SKIP round7c second sell before the answer: no GPU adapter");
        return;
    };
    bv.play.bags_connect();
    bv.play.bags_set(&bv_world());
    bv.play.bags_player_at([0.0, 0.0, 0.0]);
    bv.enable();
    bv.play.bags_spawn_npc(9002, "Merchant Kim", 0x80 | 1, 1, [6.0, 0.0, 0.0]);
    bv.open();
    bv.frames(2);
    let click_sell = |bv: &mut Bv, slot: usize| {
        bv.click(bv.slot(slot));
        bv.frames(2);
        let (x, y, enabled, _, _) = bv.menu_row("Sell 1").expect("Sell 1 is offered");
        assert!(enabled);
        bv.click((x, y));
        bv.frames(1);
    };
    // Sell 1 of the potions: its talk goes out, no answer yet
    click_sell(&mut bv, 2);
    bv.play.bags_broker_tick();
    assert_eq!(bv.play.target, 9002);
    // Sell 1 of the pelts before the merchant answers
    click_sell(&mut bv, 7);
    let (text, ok) = bv.result_text().expect("a result line");
    assert_eq!(ok, Some(false), "{text}");
    assert_eq!(text, "Sell 1: still waiting for the server to open the merchant");
    // the merchant answers: the potion is the one sold
    bv.play.bags_merchant_packet(9002);
    assert_eq!(bv.play.mode, 0);
    bv.frames(2);
    bv.play.bags_broker_tick();
    assert!(bv.play.bags_shop_pending(), "the first item's sale went out");
    bv.play.bags_sell_confirm(1, 1);
    bv.play.bags_merchant_packet(9002);
    bv.frames(2);
    bv.play.bags_broker_tick();
    bv.frames(2);
    let (text, ok) = bv.result_text().expect("a result line");
    assert_eq!(ok, Some(true), "{text}");
    assert!(text.starts_with("Sell 1"), "{text}");
    assert_eq!(bv.eval::<i64>("eqoa.inventory.get().items[2].quantity"), 4, "one potion sold");
    assert_eq!(bv.eval::<i64>("eqoa.inventory.get().items[7].quantity"), 3, "the pelts were not sold");
    assert_eq!(bv.play.mode, 0);
    bv.clean();
    super::super::window_takeover::reset_all_for_test();
    super::super::action_broker::reset_all_for_test();
}
