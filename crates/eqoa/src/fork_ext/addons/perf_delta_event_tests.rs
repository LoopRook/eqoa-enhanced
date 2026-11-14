//! Performance round 2 (entity deltas): what a handler gets from a delivered event is what an eagerly
//! materialised event gives, for every event shape.
use super::{
    host::Host,
    native,
    native_core::{CORE, Core, Event},
    native_events, record_raw,
};
use serde_json::{Value as Json, json};

/// Every path to a value in `v` (`after/position/x`, array places by number), the object paths too,
/// and one name that is no field at each object.
fn paths(v: &Json, at: &str, out: &mut Vec<String>) {
    match v {
        Json::Object(fields) if !fields.contains_key("$ref") => {
            for (k, child) in fields {
                let path = if at.is_empty() { k.clone() } else { format!("{at}/{k}") };
                out.push(path.clone());
                paths(child, &path, out);
            }
            out.push(if at.is_empty() { "zz_not_a_field".into() } else { format!("{at}/zz_not_a_field") });
        }
        Json::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                let path = format!("{at}/{}", i + 1);
                out.push(path.clone());
                paths(child, &path, out);
            }
        }
        _ => {}
    }
}

/// The event as the old delivery made it: its data built whole, then behind the guard.
fn eager(h: &Host, e: &Event) -> mlua::Table {
    let data = native::record(&h.lua, &format!("eqoa.events.{}.data", e.name), &e.data).unwrap();
    let table = record_raw::envelope_table(&h.lua, &e.name, [e.time, e.tick as f64, e.frame as f64, e.generation as f64], e.seq as f64, data).unwrap();
    native_events::readonly_envelope(&h.lua, table).unwrap()
}

/// What `a` and `b` (two envelopes) differ in at `paths`, as a handler would read them: the first
/// mismatch, or None.
fn mismatch(h: &Host, a: &mlua::Table, b: &mlua::Table, paths: &[String]) -> Option<String> {
    let f: mlua::Function = h
        .lua
        .load(
            r#"
            local a, b, list = ...
            local function at(root, path)
                local x = root.data
                for seg in path:gmatch('[^/]+') do
                    if type(x) ~= 'table' then return nil end
                    x = x[tonumber(seg) or seg]
                end
                return x
            end
            for _, k in ipairs({'name', 'seq', 'time', 'tick', 'frame', 'generation'}) do
                if a[k] ~= b[k] then return k end
            end
            for _, path in ipairs(list) do
                local x, y = at(a, path), at(b, path)
                local same
                if type(x) == 'userdata' or type(x) == 'table' then same = type(y) == type(x) else same = (x == y) end
                if not same then return path end
            end
            -- identity: reading again gives the same table, as an eager build does
            for _, path in ipairs(list) do
                local x = at(a, path)
                if type(x) == 'table' and not rawequal(x, at(a, path)) then return 'identity ' .. path end
            end
            -- read-only with the same error, and nothing to iterate, as before
            for _, which in ipairs({'envelope', 'data'}) do
                local x, y = a, b
                if which == 'data' then x, y = a.data, b.data end
                local ok, err = pcall(function() x.fresh = 1 end)
                local ok2, err2 = pcall(function() y.fresh = 1 end)
                if ok or ok2 then return 'writable ' .. which end
                err, err2 = tostring(err):match('^[^\n]*'), tostring(err2):match('^[^\n]*')
                if err ~= err2 then return 'error text: ' .. tostring(err) .. ' / ' .. tostring(err2) end
                if next(x) ~= nil then return 'iterates ' .. which end
                if getmetatable(x) ~= getmetatable(y) then return 'metatable ' .. which end
            end
            return nil
        "#,
        )
        .into_function()
        .unwrap();
    f.call((a.clone(), b.clone(), paths.to_vec())).unwrap()
}

#[test]
fn delivered_events_read_as_the_eager_build_reads_for_every_shape() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let ent = |name: &str, x: f64| {
        json!({"world_id":1,"entity_type":2,"is_creature":true,"corpse":false,"search_flags":0,"id":7001,"name":name,"level":7,
            "position":{"x":x,"y":1.5,"z":-3.25},"actor_position":{"x":x,"y":1.5,"z":-3.25},"hp_fraction":0.5,"power":null,"kind":"npc",
            "class_code":0,"target_id":null,"status_flags":0,"poisoned":false,"diseased":false,"disposition":null,"disposition_name":null,
            "ref":{"$ref":"entity","key":"7001","generation":1,"incarnation":9}})
    };
    let eref = json!({"$ref":"entity","key":"7001","generation":1,"incarnation":9});
    let events: Vec<(&str, Json)> = vec![
        ("entity.changed", json!({"ref": eref, "before": ent("Rat", 1.0), "after": ent("Rat", 5.0), "changed": ["actor_position", "position"]})),
        ("entity.added", json!({"ref": eref, "before": null, "after": ent("Fox", 3.0), "changed": ["id", "name"]})),
        ("entity.removed", json!({"ref": eref, "before": ent("Fox", 3.0), "after": null, "changed": []})),
        ("group.changed", json!({"before": {"in_group": false, "members": [], "raid": [], "invites": []},
            "after": {"in_group": true, "self_leader": true, "members": [{"id": 1, "name": "A", "active": true, "hp_fraction": 0.5, "position": {"x": 1.0, "y": 2.0, "z": 3.0}, "entity": eref}], "raid": [], "invites": [{"id": 4, "name": "B"}]},
            "changed": ["in_group", "members"]})),
        ("map.changed", json!({"before": {"zone": "a", "revision": 1}, "after": {"zone": "b", "revision": 2, "markers": [1, 2.5, "x"]}, "changed": ["map", "markers"]})),
        ("player.changed", json!({"before": {"id": 1, "hp": {"current": 10, "maximum": 20, "base": 20}}, "after": {"id": 1, "hp": {"current": 5, "maximum": 20, "base": 20}}, "changed": ["hp"]})),
        ("target.changed", json!({"before": null, "after": eref})),
        ("hotbar.changed", json!({"before": [], "after": [{"index": 1, "kind": "abilities", "slots": [{"index": 1, "kind": "empty"}]}], "changed": ["records"]})),
        ("state.reset", json!({"in_world": true, "initial": false})),
        ("cooldown.started", json!({"ability_id": "3", "display_key": 2, "lockout": 4.5, "ability": null})),
    ];
    let h = Host::new().unwrap();
    let names = events.iter().map(|(n, _)| format!("'{n}'")).collect::<Vec<_>>().join(",");
    h.execute(&format!("got = {{}}; for _, n in ipairs({{{names}}}) do eqoa.events.on(n, function(e) got[n] = e end) end")).unwrap();
    let _ = h.drain_logs();
    for (name, data) in &events {
        CORE.with(|c| c.borrow_mut().emit(name, data.clone()));
    }
    native_events::dispatch(&h).unwrap();
    let delivered: mlua::Table = h.lua.globals().get("got").unwrap();
    let mut checked = 0;
    for (name, data) in &events {
        let event = CORE.with(|c| c.borrow().events.iter().rev().find(|e| e.name == *name).cloned()).unwrap();
        let new: mlua::Table = delivered.get(*name).unwrap();
        let old = eager(&h, &event);
        let mut list = Vec::new();
        paths(data, "", &mut list);
        let _ = h.drain_logs();
        assert_eq!(mismatch(&h, &new, &old, &list), None, "{name}");
        let new_logs = h.drain_logs().unwrap();
        assert!(new_logs.iter().any(|l| l.contains("zz_not_a_field")) || !list.iter().any(|p| p.ends_with("zz_not_a_field")), "{name}: a missing field is diagnosed: {new_logs:?}");
        checked += list.len();
        // the control: the same comparison against a different event's data finds the difference
        let mut other = event.clone();
        other.data = std::sync::Arc::new(match &*event.data {
            Json::Object(o) if o.contains_key("after") => json!({"ref": null, "before": o["before"], "after": {"id": 99, "name": "other"}, "changed": []}),
            _ => json!({"in_world": false, "initial": true, "ability_id": "9", "ability": null, "lockout": 1.0, "display_key": 1, "zone": "z"}),
        });
        let different = eager(&h, &other);
        assert!(mismatch(&h, &new, &different, &list).is_some(), "{name}: the comparison must be able to fail");
        let _ = h.drain_logs();
    }
    assert!(checked > 150, "{checked} places read");
}

/// An event no handler of the VM listens for is skipped cheaply, but still counts against the frame's
/// delivery budget (256 a frame): a handler's event waiting behind 256 others is delivered next frame,
/// as before. (Control, run by hand: counting after the skip instead of before it makes this fail.)
#[test]
fn an_event_nobody_listens_for_still_spends_the_delivery_budget() {
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let h = Host::new().unwrap();
    h.execute("heard = 0; eqoa.events.on('world.entered', function() heard = heard + 1 end)").unwrap();
    let _ = h.drain_logs();
    let heard = || h.lua.globals().get::<i64>("heard").unwrap();
    h.begin_iteration();
    for _ in 0..256 {
        CORE.with(|c| c.borrow_mut().emit("state.reset", json!({"in_world": false, "initial": false})));
    }
    CORE.with(|c| c.borrow_mut().emit("world.entered", json!({"world_id": 1, "initial": false})));
    native_events::dispatch(&h).unwrap();
    native_events::dispatch(&h).unwrap();
    assert_eq!(heard(), 0, "the budget went on the 256 before it");
    h.end_iteration();
    native_events::dispatch(&h).unwrap();
    assert_eq!(heard(), 1, "and it is delivered in the next frame");
}
