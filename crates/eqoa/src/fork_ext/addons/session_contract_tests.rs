// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The session contract across rounds (integration review 2026-11-25, P4): what a session's `data` holds, kind by
//! kind. A field a kind's `data` carries is declared on `eqoa.windows.SessionData`; reading a declared field a kind does
//! not carry is nil and silent; a name nothing declares is still `unknown_symbol`. The spec's table is checked against the
//! list the code declares from, so the two cannot drift.
use super::{
    host::Host,
    native_a6::SESSION_DATA,
    native_events, window_sessions as ws,
    window_takeover::TakeoverKind,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// A sample value for a `data` field, shaped as the client builds it (so nested records are guarded as they are in play).
fn sample(field: &str) -> Value {
    match field {
        "talked_to" => json!({"id": 7, "name": "Guard"}),
        "offer" => json!({"seq": 1, "name": "Sword", "list": "inventory", "key": 3}),
        "inventory" => json!({"open": true, "slot": 1, "name": "Kel", "stats": [1, 2, 3, 4, 5, 6, 7], "records": []}),
        "spells" => json!({"open": true, "slot": 1, "name": "Kel", "slots": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], "spells": []}),
        "entries" | "links" | "catch" | "slots" | "options" => json!([]),
        "available" | "own_out" | "waiting" | "alpha" | "open" | "no_dye" | "casting" | "ready" | "list" => json!(true),
        "token" | "current" | "perfect_from" | "perfect_to" | "fair_from" | "fair_to" => json!(5),
        _ => json!("x"),
    }
}

fn data_for(fields: &str) -> Value {
    let mut o = serde_json::Map::new();
    for f in fields.split_whitespace() {
        o.insert(f.to_string(), sample(f));
    }
    Value::Object(o)
}

fn union() -> BTreeSet<&'static str> {
    SESSION_DATA.iter().flat_map(|(_, f)| f.split_whitespace()).collect()
}

fn host() -> Host {
    Host::with_test_capabilities(&["eqoa.windows.session", "eqoa.events.on"]).unwrap()
}

/// For every kind: each field the table gives it reads, each other declared field reads nil, and neither says anything
/// to the log. A name nobody declares still does. Negative control (run red, see the report): with the A2 and A11 names
/// left out of `SESSION_DATA_ALL`, `data.options` on a `text_dialog` raised `SessionData.options [unknown_symbol]`.
#[test]
fn session_data_every_kinds_fields_read_without_a_diagnostic() {
    ws::reset_for_test();
    let h = host();
    let vm = native_events::bus(&h.lua).borrow().vm;
    let every: BTreeSet<&str> = union();
    let mut kinds: Vec<(&str, &str)> = SESSION_DATA.to_vec();
    // kinds that carry no `data`: their table is empty, and every declared name still reads nil
    for none in ["loot", "trade", "friend_request", "map", "blacksmith"] {
        kinds.push((none, ""));
    }
    for (kind, fields) in &kinds {
        let k = TakeoverKind::parse(kind).unwrap();
        let s = ws::start_dialog_session(vm, "t", k, if fields.is_empty() { Value::Null } else { data_for(fields) });
        let have: BTreeSet<&str> = fields.split_whitespace().collect();
        let mut lua = format!("local s = assert(eqoa.windows.session('{kind}'), '{kind}')\nassert(s.ref == '{}')\n", s.id);
        for f in &every {
            if have.contains(f) {
                lua.push_str(&format!("assert(s.data.{f} ~= nil, '{kind}: {f} is carried')\n"));
            } else {
                lua.push_str(&format!("assert(s.data.{f} == nil, '{kind}: {f} is not carried')\n"));
            }
        }
        h.execute(&lua).unwrap();
        let logs = h.drain_logs().unwrap();
        assert!(logs.iter().all(|l| !l.contains("unknown_symbol")), "{kind}: a documented field was diagnosed: {logs:?}");
    }
    // an undocumented name is still diagnosed, on a kind with data and on one without
    for kind in ["response_dialog", "loot"] {
        // a name once diagnosed is not told twice: one name for each kind
        let name = format!("not_a_field_{kind}");
        h.execute(&format!("assert(eqoa.windows.session('{kind}').data.{name} == nil)")).unwrap();
        let logs = h.drain_logs().unwrap();
        assert!(
            logs.iter().any(|l| l.contains(&format!("SessionData.{name}")) && l.contains("unknown_symbol")),
            "{kind}: an undeclared name is diagnosed: {logs:?}"
        );
    }
}

/// The nested records are guarded too: a field a mercenary's `inventory` has reads; one it has not is diagnosed.
#[test]
fn session_data_nested_records_are_guarded_as_their_own_shapes() {
    ws::reset_for_test();
    let h = host();
    let vm = native_events::bus(&h.lua).borrow().vm;
    ws::start_dialog_session(vm, "t", TakeoverKind::TextDialog, data_for("text talked_to"));
    ws::start_dialog_session(vm, "t", TakeoverKind::MercItems, data_for("available own_out waiting inventory"));
    h.execute("local d = eqoa.windows.session('text_dialog').data; assert(d.talked_to.id == 7 and d.talked_to.name == 'Guard')").unwrap();
    h.execute("local d = eqoa.windows.session('mercenary_items').data; assert(d.inventory.name == 'Kel' and d.inventory.max_hp == nil)").unwrap();
    assert!(h.drain_logs().unwrap().iter().all(|l| !l.contains("unknown_symbol")));
    h.execute("assert(eqoa.windows.session('text_dialog').data.talked_to.rank == nil)").unwrap();
    assert!(h.drain_logs().unwrap().iter().any(|l| l.contains("TalkedTo.rank") && l.contains("unknown_symbol")));
}

fn spec() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/modding/EQOA-API-SPEC.md");
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

/// The spec's table of `data` by kind (between the two marker comments), one `(kind, fields)` per kind named.
fn spec_table(spec: &str) -> Vec<(String, BTreeSet<String>)> {
    let from = spec.find("<!-- session-data-table -->").expect("the spec has the session data table");
    let to = spec.find("<!-- /session-data-table -->").expect("the table ends");
    let mut rows = Vec::new();
    for l in spec[from..to].lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = l.trim().trim_matches('|').split(" | ").map(str::trim).collect();
        let fields: BTreeSet<String> = cells[1].split(',').map(|f| f.trim().trim_matches('`').to_string()).collect();
        // one row may name several kinds that share their fields
        for kind in cells[0].split(',') {
            rows.push((kind.trim().trim_matches('`').to_string(), fields.clone()));
        }
    }
    rows
}

fn table_problems(spec: &str) -> Vec<String> {
    let rows = spec_table(spec);
    let mut problems = Vec::new();
    for (kind, fields) in SESSION_DATA {
        let want: BTreeSet<String> = fields.split_whitespace().map(String::from).collect();
        match rows.iter().find(|(k, _)| k == kind) {
            None => problems.push(format!("the spec's table has no row for {kind}")),
            Some((_, got)) if *got != want => problems.push(format!("{kind}: the spec says {got:?}, the code declares {want:?}")),
            _ => {}
        }
    }
    for (kind, _) in &rows {
        if !SESSION_DATA.iter().any(|(k, _)| k == kind) {
            problems.push(format!("the spec's table names {kind}, which the code does not declare"));
        }
    }
    problems
}

#[test]
fn session_data_table_in_the_spec_is_the_one_the_code_declares() {
    let problems = table_problems(&spec());
    assert!(problems.is_empty(), "{problems:?}");
}

/// Negative control kept as a test: a field dropped from, or added to, a spec row is caught.
#[test]
fn session_data_table_a_changed_spec_row_is_caught() {
    let s = spec();
    let dropped = s.replacen("| `response_dialog` | `text`, `options`, `talked_to` |", "| `response_dialog` | `text`, `talked_to` |", 1);
    assert_ne!(dropped, s, "the response_dialog row is in the spec");
    assert!(table_problems(&dropped).iter().any(|p| p.contains("response_dialog")));
    let extra = s.replacen("| `start_menu` | `entries` |", "| `start_menu` | `entries`, `title` |", 1);
    assert_ne!(extra, s, "the start_menu row is in the spec");
    assert!(table_problems(&extra).iter().any(|p| p.contains("start_menu")));
}
