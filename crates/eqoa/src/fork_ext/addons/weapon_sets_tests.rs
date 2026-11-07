// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
use super::host::Host;

#[test]
fn weapon_sets_api_is_unavailable_without_native_data() {
    let host = Host::new().unwrap();
    host.execute("assert(eqoa and eqoa.WeaponSets, 'missing native API'); local w=eqoa.WeaponSets; assert(not w.IsAvailable()); assert(w.GetCount()==0); assert(w.GetSelected()==nil); assert(not w.Use(0)); assert(w.GetIcon(0)==nil)").unwrap();
}

#[test]
fn weapon_sets_api_returns_native_icons_selection_and_availability() {
    use super::{host::Snapshot, weapon_sets::WeaponSetsSnapshot};
    let host = Host::new().unwrap();
    let mut gear = crate::gear::Gear::default();
    gear.set(2, "Bow".into(), 7, u32::MAX);
    let inv = net::dump::ItemList {
        items: vec![net::dump::NetItem {
            item: net::dump::Item {
                id: 7,
                attack_type: 7,
                ..Default::default()
            },
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut menus = crate::hudmenu::HudMenus::default();
    menus.menus[3].sel = 4;
    let native = WeaponSetsSnapshot::from_native(&gear, &inv, &menus, true, &Default::default(), 0);
    host.snapshot.set(Snapshot {
        weapon_sets: native,
        ..Default::default()
    });
    let icon = host.lua.create_table().unwrap();
    icon.set(
        "GetTexture",
        host.lua
            .create_function(|_, id: i32| Ok((id, 22, 22)))
            .unwrap(),
    )
    .unwrap();
    host.lua
        .set_named_registry_value("__forge_icon_mod", icon)
        .unwrap();
    host.lua
        .globals()
        .set("expected", gear.icon(2, &inv))
        .unwrap();
    host.execute("local w=eqoa.WeaponSets; assert(w.IsAvailable());assert(w.GetCount()==5);assert(w.GetSelected()==4);assert(w.IsEmpty(0));assert(not w.IsEmpty(2));assert(w.GetIcon(2)==expected);assert(w.GetIcon(0)==nil);assert(w.GetIcon(-1)==nil);assert(w.GetIcon(5)==nil);assert(w.GetName(2)=='Bow');assert(not w.Use(0));assert(not w.Use(5));assert(w.Use(2))").unwrap();
    assert_eq!(super::weapon_sets::take_pending(), vec![2]);
    assert!(
        !WeaponSetsSnapshot::from_native(&gear, &inv, &menus, false, &Default::default(), 0)
            .available
    );
    gear.on = false;
    assert!(
        !WeaponSetsSnapshot::from_native(&gear, &inv, &menus, true, &Default::default(), 0)
            .available
    );
}

// Execute the actual bundled window function with its real slot renderer. The
// small ImGui facade records windows, highlights, textures and button presses.
fn wereoxxs_bar(enabled: bool, native: bool) {
    let lua = mlua::Lua::new();
    lua.load(r#"
        local draw = {}
        function draw:AddImage(...) images=images+1 end
        Paint={Color=function(c) return c end, Rect=function() end,
            Border=function(_,x,y,w,h,c) if c==bar_configs[4].color_selected then highlights=highlights+1 end end,
            HitRegion=function() return false end,
            HitButton=function() return click end}
        ImVec2={new=function(x,y) return {x=x,y=y} end}
        ImGui={GetWindowDrawList=function() return draw end,
            GetCursorScreenPos=function() return 0,0 end,
            SetCursorScreenPos=function() end, Dummy=function() end,
            Begin=function(name) windows_seen[#windows_seen+1]=name;return true end, End=function() end}
        require=function(name) if name=='paint' then return Paint end end
        AbilityBar={GetSelectedBarIndex=function() return 0 end,
            GetSelectedSlotIndex=function() return 0 end,GetSlotCount=function() return 1 end}
        BuildSlot=function() return {empty=true} end
        TargetRangePoints=function() end; Player={GetCurrentPwr=function() return 0 end}
        GcdFraction=function() return 0 end; PushWindowFont=function() end
        PopWindowFont=function() end; LockFlags=function(x) return x end
        GridEdit=function() end; ApplyWindowFont=function() end
        windows_seen={}; images=0;highlights=0;uses=0;click=true
    "#).exec().unwrap();
    let bars = lua
        .load(include_str!(
            "../../../../../bundled-mods/wereoxxs_ui/modules/ability_bars.lua"
        ))
        .eval::<mlua::Table>()
        .unwrap();
    lua.globals().set("AbilityBars", bars).unwrap();
    lua.load("bar_configs={};for i=1,4 do bar_configs[i]=AbilityBars.DefaultBarConfig(i-1);bar_configs[i].enabled=(i<4) end").exec().unwrap();
    lua.globals().set("enabled", enabled).unwrap();
    lua.load("bar_configs[4].enabled=enabled").exec().unwrap();
    if native {
        lua.load(
            r#"eqoa={WeaponSets={IsAvailable=function() return true end,
            GetCount=function() return 5 end,GetSelected=function() return 2 end,
            IsEmpty=function(i) return i==1 end,GetName=function() return '' end,
            GetIcon=function(i) if i~=1 then return 'texture',22,22 end end,
            GetBadge=function() end,Use=function(i) uses=uses+1;return true end}}"#,
        )
        .exec()
        .unwrap();
    }
    let source = include_str!("../../../../../bundled-mods/wereoxxs_ui/wereoxxs_ui.lua");
    let start = source.find("local function DrawAbilityBarWindows").unwrap();
    let end = source[start..]
        .find("local function DrawCastingBar")
        .unwrap()
        + start;
    lua.load(format!("{}\nDrawAbilityBarWindows(0)", &source[start..end]))
        .exec()
        .unwrap();
    let windows: mlua::Table = lua.globals().get("windows_seen").unwrap();
    assert_eq!(windows.raw_len(), 3 + usize::from(enabled && native));
    assert_eq!(
        lua.globals().get::<usize>("images").unwrap(),
        if enabled && native { 4 } else { 0 }
    );
    assert_eq!(
        lua.globals().get::<usize>("highlights").unwrap(),
        usize::from(enabled && native)
    );
    assert_eq!(
        lua.globals().get::<usize>("uses").unwrap(),
        if enabled && native { 4 } else { 0 }
    );
}

#[test]
fn weapon_sets_wereoxxs_draws_fourth_bar_and_clicks_populated_slots() {
    wereoxxs_bar(true, true);
}
#[test]
fn weapon_sets_wereoxxs_toggle_off_draws_nothing() {
    wereoxxs_bar(false, true);
}
#[test]
fn weapon_sets_wereoxxs_without_eqoa_preserves_original_bars() {
    wereoxxs_bar(true, false);
}

#[test]
fn review_regression_weapon_sets_uses_native_bare_hands_word() {
    use super::{host::Snapshot, weapon_sets::WeaponSetsSnapshot};
    let host = Host::new().unwrap();
    let mut gear = crate::gear::Gear::default();
    gear.on = true;
    let mut lines = vec!["x"; 159];
    lines[158] = "server bare hands";
    let words = vi::strtab::StringTable::parse(lines.join("\n").as_bytes()).unwrap();
    let native = WeaponSetsSnapshot::from_native(
        &gear,
        &Default::default(),
        &Default::default(),
        true,
        &words,
        0,
    );
    host.snapshot.set(Snapshot {
        weapon_sets: native,
        ..Default::default()
    });
    host.execute("assert(eqoa.WeaponSets.GetName(4)=='server bare hands')")
        .unwrap();
}
