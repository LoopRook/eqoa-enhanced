//! Performance round 2 (entity deltas): the cheaper ways of doing a thing give exactly what the old way gave.
use super::native_core::{changed_fields, json_len};
use serde_json::{Value as Json, json};

/// A small deterministic generator, so the corpus is the same every run.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn string(&mut self) -> String {
        const PIECES: [&str; 12] = ["a", "Z", " ", "\"", "\\", "\n", "\t", "\u{1}", "\u{1f}", "\u{7f}", "é", "😀"];
        (0..self.pick(8)).map(|_| PIECES[self.pick(12) as usize]).collect()
    }
    fn json(&mut self, depth: u32) -> Json {
        match self.pick(if depth > 3 { 7 } else { 9 }) {
            0 => Json::Null,
            1 => json!(self.pick(2) == 0),
            2 => json!(self.next()),
            3 => json!(-(self.next() as i64)),
            4 => json!(f64::from_bits(self.next() << 31 ^ self.next()).clamp(-1e300, 1e300)),
            5 => json!((self.next() as f32 / 977.0) as f64),
            6 => Json::String(self.string()),
            7 => Json::Array((0..self.pick(5)).map(|_| self.json(depth + 1)).collect()),
            _ => Json::Object((0..self.pick(5)).map(|_| (self.string(), self.json(depth + 1))).collect()),
        }
    }
}

/// What the length used to come from: writing the whole value out.
fn written(v: &Json) -> usize {
    serde_json::to_vec(v).unwrap().len()
}

/// A creature record as the API makes them.
fn creature(x: f32) -> Json {
    json!({"world_id":1,"entity_type":2,"is_creature":true,"corpse":false,"search_flags":0,"id":7001,"name":"a \"fine\" rat\\é","level":7,
        "position":{"x":x,"y":1.5f32,"z":-3.25f32},"actor_position":{"x":x,"y":1.5f32,"z":-3.25f32},"hp_fraction":0.5f32,"power":null,"kind":"npc",
        "class_code":0,"target_id":null,"status_flags":0,"poisoned":false,"diseased":false,"disposition":null,"disposition_name":null,
        "ref":{"$ref":"entity","key":"7001","generation":1,"incarnation":9}})
}

#[test]
fn json_len_is_the_length_serde_json_writes() {
    let fixed = [
        json!(null), json!(true), json!(false), json!(0), json!(-1), json!(u64::MAX), json!(i64::MIN), json!(0.1), json!(-0.0), json!(1e21),
        json!(1e-7), json!(123456789.123456789), json!(f64::MAX), json!(f64::MIN_POSITIVE), json!(0.15f32), json!(""), json!("a\"b\\c\n\r\t\u{8}\u{c}\u{0}\u{1f}\u{7f}é😀"),
        json!([]), json!({}), json!([[], {}, [null]]), json!({"": 1, "a\"": [1, 2, {"b": "c"}]}), creature(12.3456789), json!({"before": creature(1.0), "after": creature(2.0), "changed": ["position"]}),
    ];
    for v in &fixed {
        assert_eq!(json_len(v), written(v), "{v}");
    }
    let mut g = Lcg(7);
    for _ in 0..4000 {
        let v = g.json(0);
        assert_eq!(json_len(&v), written(&v), "{v}");
    }
    // the control: a count that does not know the escapes is wrong on a string that has them
    let naive = |s: &str| s.len() + 2;
    let escaped = json!("a\"b\\c\n");
    assert_ne!(naive("a\"b\\c\n"), written(&escaped), "the control must differ, or the test cannot tell");
    assert_eq!(json_len(&escaped), written(&escaped));
}

/// The old walk: every key of either side, sorted, kept where the two values differ.
fn changed_fields_old(before: &Json, after: &Json) -> Vec<String> {
    if before.is_array() || after.is_array() {
        return vec!["records".into()];
    }
    let mut keys: Vec<&String> = Vec::new();
    for value in [before, after] {
        if let Some(o) = value.as_object() {
            keys.extend(o.keys());
        }
    }
    keys.sort_unstable();
    keys.dedup();
    keys.into_iter().filter(|k| before[*k] != after[*k]).cloned().collect()
}

#[test]
fn changed_fields_names_what_the_old_walk_named() {
    let a = creature(1.0);
    let mut b = creature(2.0);
    b["hp_fraction"] = json!(0.25);
    b["extra"] = json!(1);
    let mut only_before = creature(1.0);
    only_before["gone"] = json!(5);
    only_before["gone_null"] = Json::Null;
    for (x, y) in [
        (&a, &b), (&b, &a), (&a, &a), (&Json::Null, &a), (&a, &Json::Null), (&only_before, &a), (&a, &only_before), (&json!([1]), &a), (&a, &json!([])),
        (&json!("s"), &json!({"k": null})), (&json!({"k": null}), &json!({})),
    ] {
        assert_eq!(changed_fields(x, y), changed_fields_old(x, y), "{x} -> {y}");
    }
    assert_eq!(changed_fields(&a, &b), ["actor_position", "extra", "hp_fraction", "position"]);
    let mut g = Lcg(11);
    for _ in 0..3000 {
        let (x, y) = (g.json(2), if g.pick(3) == 0 { g.json(2) } else { Json::Null });
        let y = if y.is_null() { x.clone() } else { y };
        assert_eq!(changed_fields(&x, &y), changed_fields_old(&x, &y), "{x} -> {y}");
        // and one key changed on a copy
        if let Json::Object(mut o) = x.clone() {
            if let Some(k) = o.keys().next().cloned() {
                o.insert(k, json!("changed"));
            }
            let z = Json::Object(o);
            assert_eq!(changed_fields(&x, &z), changed_fields_old(&x, &z), "{x} -> {z}");
        }
    }
}
