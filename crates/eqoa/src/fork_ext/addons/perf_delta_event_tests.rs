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

/// What `a` and `b` (two envelopes of one VM) differ in at `paths`, as a handler would read them: the
/// first mismatch, or None. Values by value; references by equality and by what they resolve to now;
/// tables by what `pairs` and `ipairs` give, in order; every table by its write error.
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
            local function shown(v)
                local t = type(v)
                if t == 'table' or t == 'userdata' then return t end
                return tostring(v)
            end
            -- what pairs and ipairs give, in order
            local function sig(t)
                local out = {}
                for k, v in pairs(t) do out[#out + 1] = 'p' .. tostring(k) .. ':' .. shown(v) end
                for i, v in ipairs(t) do out[#out + 1] = 'i' .. i .. ':' .. shown(v) end
                return table.concat(out, ',')
            end
            -- what a reference gives back now (the world holds one creature, key 7001, incarnation 9)
            local function resolved(r)
                local rec, err = eqoa.entities.get(r)
                return (rec and (rec.name .. '@' .. tostring(rec.position.x)) or 'nil') .. '/' .. (err and err.code or 'ok')
            end
            local function write(t)
                local ok, err = pcall(function() t.fresh = 1 end)
                if ok then t.fresh = nil end
                return tostring(ok) .. ':' .. (ok and '' or tostring(err):match('^[^\n]*'))
            end
            for _, k in ipairs({'name', 'seq', 'time', 'tick', 'frame', 'generation'}) do
                if a[k] ~= b[k] then return k end
            end
            for _, path in ipairs(list) do
                local x, y = at(a, path), at(b, path)
                if type(x) ~= type(y) then return 'type ' .. path end
                if type(x) == 'userdata' then
                    if x ~= y then return 'ref ' .. path end
                    if resolved(x) ~= resolved(y) then return 'resolve ' .. path end
                elseif type(x) == 'table' then
                    if sig(x) ~= sig(y) then return 'iterate ' .. path end
                    if write(x) ~= write(y) then return 'write ' .. path end
                    -- identity: reading again gives the same table, as an eager build does
                    if not rawequal(x, at(a, path)) then return 'identity ' .. path end
                elseif x ~= y then
                    return path
                end
            end
            for _, which in ipairs({'envelope', 'data'}) do
                local x, y = a, b
                if which == 'data' then x, y = a.data, b.data end
                local ok, err = pcall(function() x.fresh = 1 end)
                local ok2, err2 = pcall(function() y.fresh = 1 end)
                if ok or ok2 then return 'writable ' .. which end
                err, err2 = tostring(err):match('^[^\n]*'), tostring(err2):match('^[^\n]*')
                if err ~= err2 then return 'error text: ' .. err .. ' / ' .. err2 end
                if sig(x) ~= sig(y) then return 'iterates ' .. which end
                if getmetatable(x) ~= getmetatable(y) then return 'metatable ' .. which end
            end
            return nil
        "#,
        )
        .into_function()
        .unwrap();
    f.call((a.clone(), b.clone(), paths.to_vec())).unwrap()
}

fn creature(name: &str, x: f64, incarnation: u64) -> Json {
    json!({"world_id":1,"entity_type":2,"is_creature":true,"corpse":false,"search_flags":0,"id":7001,"name":name,"level":7,
        "position":{"x":x,"y":1.5,"z":-3.25},"actor_position":{"x":x,"y":1.5,"z":-3.25},"hp_fraction":0.5,"power":null,"kind":"npc",
        "class_code":0,"target_id":null,"status_flags":0,"poisoned":false,"diseased":false,"disposition":null,"disposition_name":null,
        "ref":{"$ref":"entity","key":"7001","generation":1,"incarnation":incarnation}})
}

/// The ten shapes: the entity events with all 21 entity fields (one of them with a reference to a
/// creature that is gone), a group with nested members and references, a map, the player, a target
/// change (two references), a hotbar (arrays), and two events with plain data.
fn shapes() -> Vec<(&'static str, Json)> {
    let eref = json!({"$ref":"entity","key":"7001","generation":1,"incarnation":9});
    let gone = json!({"$ref":"entity","key":"7001","generation":1,"incarnation":8});
    vec![
        ("entity.changed", json!({"ref": eref, "before": creature("Rat", 1.0, 9), "after": creature("Rat", 5.0, 9), "changed": ["actor_position", "position"]})),
        ("entity.added", json!({"ref": eref, "before": null, "after": creature("Fox", 3.0, 9), "changed": ["id", "name"]})),
        ("entity.removed", json!({"ref": gone, "before": creature("Fox", 3.0, 8), "after": null, "changed": []})),
        ("group.changed", json!({"before": {"in_group": false, "members": [], "raid": [], "invites": []},
            "after": {"in_group": true, "self_leader": true, "members": [{"id": 1, "name": "A", "active": true, "hp_fraction": 0.5, "position": {"x": 1.0, "y": 2.0, "z": 3.0}, "entity": eref}], "raid": [], "invites": [{"id": 4, "name": "B"}]},
            "changed": ["in_group", "members"]})),
        ("map.changed", json!({"before": {"zone": "a", "revision": 1}, "after": {"zone": "b", "revision": 2, "markers": [1, 2.5, "x"]}, "changed": ["map", "markers"]})),
        ("player.changed", json!({"before": {"id": 1, "hp": {"current": 10, "maximum": 20, "base": 20}}, "after": {"id": 1, "hp": {"current": 5, "maximum": 20, "base": 20}}, "changed": ["hp"]})),
        ("target.changed", json!({"before": gone, "after": eref})),
        ("hotbar.changed", json!({"before": [], "after": [{"index": 1, "kind": "abilities", "slots": [{"index": 1, "kind": "empty"}]}], "changed": ["records"]})),
        ("state.reset", json!({"in_world": true, "initial": false})),
        ("cooldown.started", json!({"ability_id": "3", "display_key": 2, "lockout": 4.5, "ability": null})),
    ]
}

/// The same shapes with every number and every name moved (references keep their kind and key, so
/// they still name the creature; their incarnation moves with the numbers).
fn moved(v: &Json) -> Json {
    match v {
        Json::Number(n) => json!(n.as_f64().unwrap() + 1000.0),
        Json::Array(items) => Json::Array(items.iter().map(moved).collect()),
        Json::Object(fields) => Json::Object(
            fields
                .iter()
                .map(|(k, child)| {
                    let child = match (k.as_str(), child) {
                        ("name" | "zone", Json::String(s)) => Json::String(format!("{s}!")),
                        _ => moved(child),
                    };
                    (k.clone(), child)
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

/// `v` with every flag turned over (for the control: it differs from `v` in a shape that has no number or name).
fn negated(v: &Json) -> Json {
    match v {
        Json::Bool(b) => Json::Bool(!b),
        Json::Array(items) => Json::Array(items.iter().map(negated).collect()),
        Json::Object(fields) => Json::Object(fields.iter().map(|(k, child)| (k.clone(), negated(child))).collect()),
        other => other.clone(),
    }
}

/// A VM with two handlers for every shape: `first` and `second` keep the first event each handler
/// got (read nothing), `latest` the newest.
fn vm(names: &[&str]) -> Host {
    let h = Host::new().unwrap();
    let list = names.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(",");
    h.execute(&format!(
        "first, second, latest = {{}}, {{}}, {{}}; for _, n in ipairs({{{list}}}) do \
         eqoa.events.on(n, function(e) first[n] = first[n] or e; latest[n] = e end); \
         eqoa.events.on(n, function(e) second[n] = second[n] or e end) end"
    ))
    .unwrap();
    let _ = h.drain_logs();
    h
}

fn emit_all(events: &[(&'static str, Json)]) -> Vec<Event> {
    for (name, data) in events {
        CORE.with(|c| c.borrow_mut().emit(name, data.clone()));
    }
    events.iter().map(|(name, _)| CORE.with(|c| c.borrow().events.iter().rev().find(|e| e.name == *name).cloned()).unwrap()).collect()
}

/// A world with the one creature the references name (key 7001, incarnation 9), generation 1.
fn world(name: &str, x: f64) {
    CORE.with(|c| {
        let mut c = c.borrow_mut();
        *c = Core::default();
        c.generation = 1;
        c.view.in_world = true;
        c.view.entities = vec![creature(name, x, 9)];
    });
}

fn table(h: &Host, global: &str) -> mlua::Table {
    h.lua.globals().get(global).unwrap()
}

/// Every event shape, through two handlers of one VM and one handler of a second VM, read only after
/// the creature has changed and a newer event of each kind has come: each gives what the old eager
/// build of its own event gave (values, references by equality and by what they resolve to, `pairs`
/// and `ipairs`, the same table on a second read, read-only errors at every level).
#[test]
fn delivered_events_read_as_the_eager_build_reads_for_every_shape() {
    world("Rat", 1.0);
    let first = shapes();
    let names: Vec<&str> = first.iter().map(|(n, _)| *n).collect();
    let (h, h2) = (vm(&names), vm(&names));
    let e1 = emit_all(&first);
    native_events::dispatch(&h).unwrap();
    native_events::dispatch(&h2).unwrap();
    // nothing has been read; the creature changes in the world, and a newer event of each kind arrives
    world("Changed", 555.0);
    let second: Vec<(&'static str, Json)> = first.iter().map(|(n, d)| (*n, moved(d))).collect();
    let e2 = emit_all(&second);
    native_events::dispatch(&h).unwrap();
    native_events::dispatch(&h2).unwrap();
    let _ = (h.drain_logs(), h2.drain_logs());
    let mut checked = 0;
    for (which, host) in [("vm 1", &h), ("vm 2", &h2)] {
        for (i, name) in names.iter().enumerate() {
            let mut list = Vec::new();
            paths(&first[i].1, "", &mut list);
            let mut list2 = Vec::new();
            paths(&second[i].1, "", &mut list2);
            // each handler's first event: its emit-time values, though the world and the newer event moved on
            for slot in ["first", "second"] {
                let got: mlua::Table = table(host, slot).get(*name).unwrap();
                let old = eager(host, &e1[i]);
                assert_eq!(mismatch(host, &got, &old, &list), None, "{which} {slot} {name}: kept across a frame");
            }
            let got: mlua::Table = table(host, "latest").get(*name).unwrap();
            let old = eager(host, &e2[i]);
            assert_eq!(mismatch(host, &got, &old, &list2), None, "{which} latest {name}");
            let distinct: bool = host.lua.load(format!("return not rawequal(first['{name}'], second['{name}'])")).eval().unwrap();
            assert!(distinct, "{which} {name}: each handler has its own event");
            checked += list.len() + list2.len();
            if which == "vm 1" && *name == "entity.changed" {
                let logs = host.drain_logs().unwrap();
                assert!(logs.iter().any(|l| l.contains("zz_not_a_field")), "a missing field is diagnosed: {logs:?}");
            }
        }
    }
    assert!(checked > 600, "{checked} places read");
    // the controls: the comparison can fail. Against another event's data it does, for every shape; and
    // against the same event with the reference one incarnation on, it reports the reference.
    for (i, name) in names.iter().enumerate() {
        let mut list = Vec::new();
        paths(&first[i].1, "", &mut list);
        let got: mlua::Table = table(&h, "first").get(*name).unwrap();
        let mut other = e1[i].clone();
        other.data = std::sync::Arc::new(negated(&moved(&first[i].1)));
        assert!(mismatch(&h, &got, &eager(&h, &other), &list).is_some(), "{name}: the comparison must be able to fail");
    }
    let got: mlua::Table = table(&h, "first").get("entity.changed").unwrap();
    let mut other = e1[0].clone();
    let mut data = (*other.data).clone();
    data["ref"]["incarnation"] = json!(10);
    other.data = std::sync::Arc::new(data);
    let mut list = Vec::new();
    paths(&first[0].1, "", &mut list);
    assert_eq!(mismatch(&h, &got, &eager(&h, &other), &list), Some("ref ref".into()), "a reference to another incarnation is told apart");
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
