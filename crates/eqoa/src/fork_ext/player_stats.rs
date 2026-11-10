// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Fork-owned player stats computation for the native Stats Box and `eqoa.player.stats()`.
use net::dump::{ItemList, Stats};
use serde_json::{Value as Json, json};

/// The rows, two columns of eight: a stat's word (`sStatWords`) and its id.
/// The seven trainable stats and the armour class; the maximum hit points
/// (HPMax, the channel's 8), the maximum power (POW, 10) and the resists in
/// the Summary's order (fire, cold, lightning, arcane, poison, disease).
pub const ROWS: [[(i32, i32); 8]; 2] = [
    [(226, 0), (227, 1), (228, 2), (229, 3), (230, 4), (231, 5), (232, 6), (242, 14)],
    [(234, 8), (496, 10), (447, 24), (451, 25), (450, 26), (452, 27), (448, 22), (449, 23)],
];

/// How the effects leave a stat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Same,
    Raised,
    Lowered,
}

impl Shift {
    pub fn as_str(&self) -> &'static str {
        match self {
            Shift::Same => "same",
            Shift::Raised => "raised",
            Shift::Lowered => "lowered",
        }
    }
}

/// A row: its stat's word, its value, and what it is without the effects:
/// between the base's and the worn items' alone and that with the masteries'
/// (none for POW, whose effects' share cannot be told).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub word: i32,
    pub value: i32,
    pub without: Option<(i32, i32)>,
}

impl Row {
    pub fn shift(&self) -> Shift {
        match self.without {
            Some((_, high)) if self.value > high => Shift::Raised,
            Some((low, _)) if self.value < low => Shift::Lowered,
            _ => Shift::Same,
        }
    }

    pub fn color(&self) -> [f32; 4] {
        match self.shift() {
            Shift::Same => [0.96, 0.95, 0.9, 1.0],
            Shift::Raised => [0.45, 0.78, 1.0, 1.0],
            Shift::Lowered => [1.0, 0.45, 0.4, 1.0],
        }
    }

    pub fn stat_name(stat: i32) -> &'static str {
        match stat {
            0 => "strength",
            1 => "stamina",
            2 => "agility",
            3 => "dexterity",
            4 => "wisdom",
            5 => "intelligence",
            6 => "charisma",
            14 => "armor",
            8 => "hp_max",
            10 => "power",
            24 => "fire",
            25 => "cold",
            26 => "lightning",
            27 => "arcane",
            22 => "poison",
            23 => "disease",
            _ => "unknown",
        }
    }

    pub fn to_json(&self, stat: i32) -> Json {
        let name = Self::stat_name(stat);
        let shift = self.shift();
        let (min, max) = match self.without {
            Some((l, h)) => (Some(l), Some(h)),
            None => (None, None),
        };
        let base = max.unwrap_or(self.value);
        let change = match shift {
            Shift::Same => 0,
            Shift::Raised => self.value - max.unwrap_or(self.value),
            Shift::Lowered => self.value - min.unwrap_or(self.value),
        };
        json!({
            "name": name,
            "word": self.word,
            "stat_id": stat,
            "current": self.value,
            "base": base,
            "change": change,
            "effect_change": change,
            "shift": shift.as_str(),
            "min": min,
            "max": max,
            "without_min": min,
            "without_max": max,
        })
    }
}

/// What the box shows: `ROWS`' rows, column by column.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub rows: [[Row; 8]; 2],
}

impl View {
    pub fn to_json(&self) -> Json {
        let mut cols = Vec::with_capacity(2);
        let mut named = serde_json::Map::new();
        let mut resists = serde_json::Map::new();

        for (col_idx, rows) in self.rows.iter().enumerate() {
            let mut col_json = Vec::with_capacity(8);
            for (row_idx, row) in rows.iter().enumerate() {
                let (_, stat) = ROWS[col_idx][row_idx];
                let item = row.to_json(stat);
                let name = Row::stat_name(stat);
                if matches!(stat, 22..=27) {
                    resists.insert(name.to_string(), item.clone());
                }
                named.insert(name.to_string(), item.clone());
                col_json.push(item);
            }
            cols.push(col_json);
        }

        let mut flat_rows = Vec::with_capacity(16);
        for col in &cols {
            flat_rows.extend(col.clone());
        }
        named.insert("rows".to_string(), json!(flat_rows));
        named.insert("columns".to_string(), json!(cols));
        named.insert("resists".to_string(), Json::Object(resists));
        Json::Object(named)
    }
}

/// What the box reads off the client: the stat channel, the inventory (its
/// worn items), the masteries' stats and stat-cap bonuses
/// (`VIClientPart4EC58`, the dump's and 0x1402's) and the stat cap
/// (`m_player.m_p44`, none when 0).
pub struct Source<'a> {
    pub stats: &'a Stats,
    pub inv: &'a ItemList,
    pub mastery: &'a [i64; 29],
    pub bonuses: &'a [i64; 7],
    pub p44: i32,
}

/// The rows as they stand, or none before the stat channel has come (no
/// maximum hit points yet).
pub fn view(s: &Source) -> Option<View> {
    if s.stats.stat[8] <= 0 {
        return None;
    }
    // each stat without the effects: the base and the worn items, and that
    // with the masteries, the seven within their cap
    let (mut low, mut high) = ([0i32; 29], [0i32; 29]);
    for i in 0..29 {
        let gear = crate::infotext::stat_total(&s.stats.base, s.inv, i as i64) as i32;
        let m = crate::trainer::mastery_stat(s.mastery, i as i32);
        let cap = |v: i32| if i < 7 && s.p44 > 0 { v.min(crate::trainer::stat_cap(s.p44, s.bonuses, i as i32)) } else { v };
        (low[i], high[i]) = (cap(gear.min(gear + m)), cap(gear.max(gear + m)));
    }
    let mut out = View::default();
    for (col, rows) in ROWS.iter().enumerate() {
        for (k, &(word, stat)) in rows.iter().enumerate() {
            let derived = |v: &[i32; 29]| crate::trainer::Data::derived(v, stat);
            let without = (stat != 10).then(|| (derived(&low), derived(&high)));
            out.rows[col][k] = Row { word, value: derived(&s.stats.stat), without };
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_order_matches_legacy_statbox() {
        assert_eq!(ROWS[0].len(), 8);
        assert_eq!(ROWS[1].len(), 8);
        assert_eq!(ROWS[0][0], (226, 0)); // Str
        assert_eq!(ROWS[0][7], (242, 14)); // AC
        assert_eq!(ROWS[1][0], (234, 8)); // HPMax
        assert_eq!(ROWS[1][1], (496, 10)); // POW
        assert_eq!(ROWS[1][2], (447, 24)); // Fire
        assert_eq!(ROWS[1][7], (449, 23)); // Disease
    }

    #[test]
    fn json_representation_contains_both_array_and_keys() {
        let mut v = View::default();
        v.rows[0][0] = Row { word: 226, value: 120, without: Some((100, 100)) };
        let j = v.to_json();
        assert_eq!(j["rows"].as_array().unwrap().len(), 16);
        assert_eq!(j["strength"]["current"], 120);
        assert_eq!(j["strength"]["base"], 100);
        assert_eq!(j["strength"]["shift"], "raised");
        assert_eq!(j["strength"]["change"], 20);
    }

    #[test]
    fn stat_shift_detects_raised_and_lowered() {
        let raised = Row { word: 226, value: 110, without: Some((100, 100)) };
        assert_eq!(raised.shift(), Shift::Raised);

        let lowered = Row { word: 226, value: 90, without: Some((100, 100)) };
        assert_eq!(lowered.shift(), Shift::Lowered);

        let same = Row { word: 226, value: 100, without: Some((100, 100)) };
        assert_eq!(same.shift(), Shift::Same);
    }

    #[test]
    fn derived_resists_apply_wisdom_formula() {
        let mut stats = Stats::default();
        stats.stat[8] = 500; // HPMax
        stats.stat[4] = 70; // 70 Wisdom -> 40 + 70/7 = 50 base resist
        let inv = ItemList::default();
        let mastery = [0i64; 29];
        let bonuses = [0i64; 7];
        let src = Source {
            stats: &stats,
            inv: &inv,
            mastery: &mastery,
            bonuses: &bonuses,
            p44: 0,
        };
        let view = view(&src).unwrap();
        // Fire resist row is ROWS[1][2]
        let fire = view.rows[1][2];
        assert_eq!(fire.value, 50);
    }
}
