// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Fork-owned player stats computation for the native Stats Box and `eqoa.player.stats()`.
use net::dump::{ItemList, Stats};
use serde_json::{Value as Json, json};
use vi::ui::wide;

/// The rows, two columns of eight: a stat's word (`sStatWords`) and its id.
/// The seven trainable stats and the armour class; the maximum hit points
/// (HPMax, the channel's 8), the maximum power (POW, 10) and the resists in
/// the Summary's order (fire, cold, lightning, arcane, poison, disease).
pub const ROWS: [[(i32, i32); 8]; 2] = [
    [
        (226, 0),
        (227, 1),
        (228, 2),
        (229, 3),
        (230, 4),
        (231, 5),
        (232, 6),
        (242, 14),
    ],
    [
        (234, 8),
        (496, 10),
        (447, 24),
        (451, 25),
        (450, 26),
        (452, 27),
        (448, 22),
        (449, 23),
    ],
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
        let short_name = match stat {
            0 => "Str",
            1 => "Sta",
            2 => "Agi",
            3 => "Dex",
            4 => "Wis",
            5 => "Int",
            6 => "Cha",
            14 => "AC",
            8 => "HPMax",
            10 => "POW",
            24 => "FR",
            25 => "CR",
            26 => "LR",
            27 => "AR",
            22 => "PR",
            23 => "DR",
            _ => "???",
        };
        json!({
            "name": name,
            "short": short_name,
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
                if stat == 8 {
                    named.insert("max_hp".to_string(), item.clone());
                } else if stat == 10 {
                    named.insert("max_power".to_string(), item.clone());
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
        let cap = |v: i32| {
            if i < 7 && s.p44 > 0 {
                v.min(crate::trainer::stat_cap(s.p44, s.bonuses, i as i32))
            } else {
                v
            }
        };
        (low[i], high[i]) = (cap(gear.min(gear + m)), cap(gear.max(gear + m)));
    }
    let mut out = View::default();
    for (col, rows) in ROWS.iter().enumerate() {
        for (k, &(word, stat)) in rows.iter().enumerate() {
            let derived = |v: &[i32; 29]| crate::trainer::Data::derived(v, stat);
            let without = (stat != 10).then(|| (derived(&low), derived(&high)));
            out.rows[col][k] = Row {
                word,
                value: derived(&s.stats.stat),
                without,
            };
        }
    }
    Some(out)
}

pub const FRAME: [i32; 4] = [200, 54, 348, 186];
const BORDER: [f32; 2] = [7., 9.];
const PAD: [f32; 2] = [3., 2.];
const GAP: f32 = 6.;
const LEADING: f32 = 0.08;
const SCALE: f32 = 0.6;
const ROOM: &str = "8888";
/// Where the rows go inside the frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rows {
    /// each column's labels' left edge and its values' right edge
    pub lefts: [f32; 2],
    pub rights: [f32; 2],
    /// the first row's top and the step to the next
    pub top: f32,
    pub step: f32,
    /// the words' scale, of the font's
    pub scale: f32,
}

/// The rows' place inside the frame for a font `font_h` high whose two
/// columns are `wide[c]` across at its own size (the widest label, a space
/// and `ROOM`): the largest scale up to `SCALE` at which eight rows fit the
/// frame's height and both columns its width with `GAP` between them, the
/// columns at the frame's two sides (the room left over between them).
pub fn layout(font_h: f32, wide: [f32; 2]) -> Rows {
    let (x0, y0, x1, y1) = (
        FRAME[0] as f32,
        FRAME[1] as f32,
        FRAME[2] as f32,
        FRAME[3] as f32,
    );
    let (left, right) = (x0 + BORDER[0] + PAD[0], x1 - BORDER[0] - PAD[0]);
    let (top, bottom) = (y0 + BORDER[1] + PAD[1], y1 - BORDER[1] - PAD[1]);
    let font_h = font_h.max(1.0);
    let by_height = (bottom - top) / (8.0 * font_h * (1.0 + LEADING));
    let by_width = (right - left - GAP) / (wide[0] + wide[1]).max(1.0);
    let scale = SCALE.min(by_height).min(by_width);
    let step = (bottom - top) / 8.0;
    Rows {
        lefts: [left, right - wide[1] * scale],
        rights: [left + wide[0] * scale, right],
        top: top + (step - font_h * scale) / 2.0,
        step,
        scale,
    }
}

/// Each column's width at the font's own size by `width` (a string's, as the
/// font measures it): its widest label, a space and `ROOM`.
pub fn widths(labels: &[[Vec<u16>; 8]; 2], width: impl Fn(&[u16]) -> f32) -> [f32; 2] {
    let room = width(&wide(&format!(" {ROOM}")));
    std::array::from_fn(|c| labels[c].iter().map(|l| width(l)).fold(0.0, f32::max) + room)
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
        v.rows[0][0] = Row {
            word: 226,
            value: 120,
            without: Some((100, 100)),
        };
        let j = v.to_json();
        assert_eq!(j["rows"].as_array().unwrap().len(), 16);
        assert_eq!(j["strength"]["current"], 120);
        assert_eq!(j["strength"]["base"], 100);
        assert_eq!(j["strength"]["shift"], "raised");
        assert_eq!(j["strength"]["change"], 20);
    }

    #[test]
    fn stat_shift_detects_raised_and_lowered() {
        let raised = Row {
            word: 226,
            value: 110,
            without: Some((100, 100)),
        };
        assert_eq!(raised.shift(), Shift::Raised);

        let lowered = Row {
            word: 226,
            value: 90,
            without: Some((100, 100)),
        };
        assert_eq!(lowered.shift(), Shift::Lowered);

        let same = Row {
            word: 226,
            value: 100,
            without: Some((100, 100)),
        };
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
    use net::dump::{Item, ItemStat, NetItem};

    /// A character's channel: every base stat 50, the totals as the server
    /// counts them from the base, the worn items and `fx` (the effects' share
    /// by stat), the resists' raw 10, 300 base hit points and 200 power.
    fn channel(worn: &[(i64, i64)], fx: &[(usize, i32)]) -> Stats {
        let mut s = Stats::default();
        for i in 0..7 {
            s.base[i] = 50;
        }
        s.base[8] = 300;
        s.base[10] = 200;
        for i in 22..28 {
            s.base[i] = 10;
        }
        s.stat = s.base;
        for &(id, v) in worn {
            s.stat[id as usize] += v as i32;
        }
        for &(i, v) in fx {
            s.stat[i] += v;
        }
        s
    }

    /// An inventory of one worn item giving `stats`, and one carried (not
    /// worn) giving the same, which counts for nothing.
    fn gear(stats: &[(i64, i64)]) -> ItemList {
        let it = |slot| NetItem {
            equip_slot: slot,
            item: Item {
                stats: stats
                    .iter()
                    .map(|&(id, value)| ItemStat { id, value })
                    .collect(),
                ..Default::default()
            },
            ..Default::default()
        };
        ItemList {
            items: vec![it(3), it(-1)],
            ..Default::default()
        }
    }

    fn of(v: &View, stat: i32) -> Row {
        let k = ROWS
            .iter()
            .flatten()
            .position(|&(_, s)| s == stat)
            .expect("a row");
        v.rows[k / 8][k % 8]
    }

    /// Worn items alone leave every value white; a buff of 10 strength is
    /// blue and a debuff of 5 agility red, by the amount the channel moved;
    /// the resists derive as the Summary's (a seventh of wisdom and 40 on).
    /// The control: counting no worn items, the item's strength would read as
    /// an effect's.
    #[test]
    fn an_effect_is_marked_and_gear_is_not() {
        let inv = gear(&[(0, 7), (14, 40), (24, 5)]);
        let src = |s| Source {
            stats: s,
            inv: &inv,
            mastery: &[0; 29],
            bonuses: &[0; 7],
            p44: 0,
        };
        let plain = channel(&[(0, 7), (14, 40), (24, 5)], &[]);
        let v = view(&src(&plain)).expect("a view");
        assert!(
            v.rows.iter().flatten().all(|r| r.shift() == Shift::Same),
            "gear is no effect: {v:?}"
        );
        assert_eq!((of(&v, 0).value, of(&v, 14).value), (57, 40));
        assert_eq!(
            of(&v, 24).value,
            50 / 7 + 15 + 40,
            "fire: a seventh of wisdom, 40 and the raw 15"
        );
        let buffed = channel(&[(0, 7), (14, 40), (24, 5)], &[(0, 10), (2, -5)]);
        let v = view(&src(&buffed)).expect("a view");
        assert_eq!(
            (of(&v, 0).value, of(&v, 0).without, of(&v, 0).shift()),
            (67, Some((57, 57)), Shift::Raised)
        );
        assert_eq!(
            (of(&v, 2).value, of(&v, 2).shift(), of(&v, 2).color()),
            (45, Shift::Lowered, [1.0, 0.45, 0.4, 1.0])
        );
        assert_eq!(of(&v, 1).shift(), Shift::Same, "stamina untouched");
        // wisdom raised: every resist with it (its seventh), each marked
        let wise = channel(&[], &[(4, 14)]);
        let v = view(&Source {
            stats: &wise,
            inv: &ItemList::default(),
            mastery: &[0; 29],
            bonuses: &[0; 7],
            p44: 0,
        })
        .expect("a view");
        assert!(
            [24, 25, 26, 27, 22, 23]
                .iter()
                .all(|&s| of(&v, s).shift() == Shift::Raised
                    && Some(of(&v, s).value - 2) == of(&v, s).without.map(|w| w.1))
        );
        // the control: the worn item not counted
        let v = view(&Source {
            stats: &plain,
            inv: &ItemList::default(),
            mastery: &[0; 29],
            bonuses: &[0; 7],
            p44: 0,
        })
        .expect("a view");
        assert_eq!(of(&v, 0).shift(), Shift::Raised, "the control must fail");
    }

    /// A stat at its cap: a buff past it moves nothing and is not marked
    /// (the server keeps the seven within `GetP44Total`), one from below it
    /// is; a mastery's share counts as the base's. The control: with no cap
    /// the capped stat would read as lowered.
    #[test]
    fn the_cap_and_the_masteries_count_as_the_base() {
        let inv = gear(&[(0, 30)]);
        // 50 base + 30 worn = 80, the cap 70 (60 and a mastery's 10)
        let mut bonuses = [0; 7];
        bonuses[0] = 10;
        let capped = channel(&[(0, 20)], &[]);
        let src = |s, p44| Source {
            stats: s,
            inv: &inv,
            mastery: &[0; 29],
            bonuses: &bonuses,
            p44,
        };
        let v = view(&src(&capped, 60)).expect("a view");
        assert_eq!(
            (of(&v, 0).value, of(&v, 0).without, of(&v, 0).shift()),
            (70, Some((70, 70)), Shift::Same)
        );
        // the control: no cap
        assert_eq!(
            view(&src(&capped, 0)).expect("a view").rows[0][0].shift(),
            Shift::Lowered,
            "the control must fail"
        );
        // a mastery's 5 stamina (slot 1) is the base's, whether the server
        // counts it on top (go-eqoa-server) or in its base (as Sandstorm its
        // hit points'); 6 more is an effect's
        let mut m = [0; 29];
        m[1] = 5;
        let none = ItemList::default();
        let with = |s: &Stats, m: &[i64; 29]| {
            view(&Source {
                stats: s,
                inv: &none,
                mastery: m,
                bonuses: &[0; 7],
                p44: 0,
            })
            .expect("a view")
        };
        let on_top = channel(&[], &[(1, 5)]);
        assert_eq!(of(&with(&on_top, &m), 1).shift(), Shift::Same);
        let mut in_base = channel(&[], &[(1, 5)]);
        in_base.base[1] += 5;
        assert_eq!(
            of(&with(&in_base, &m), 1).shift(),
            Shift::Same,
            "counted in the base"
        );
        let buffed = channel(&[], &[(1, 11)]);
        assert_eq!(of(&with(&buffed, &m), 1).shift(), Shift::Raised);
        assert_eq!(
            of(&with(&on_top, &[0; 29]), 1).shift(),
            Shift::Raised,
            "the control: without the mastery, an effect's"
        );
        // POW is never marked: worn stats move the server's maximum power off
        // its base too
        let mut pow = channel(&[], &[(10, 37)]);
        pow.stat[9] = 0;
        assert_eq!(
            (
                of(&with(&pow, &[0; 29]), 10).value,
                of(&with(&pow, &[0; 29]), 10).shift()
            ),
            (237, Shift::Same)
        );
        assert_eq!(
            of(&with(&channel(&[], &[(8, 37)]), &[0; 29]), 8).shift(),
            Shift::Raised,
            "the control: HPMax's own share is"
        );
        // nothing before the channel
        assert!(
            view(&Source {
                stats: &Stats::default(),
                inv: &none,
                mastery: &m,
                bonuses: &[0; 7],
                p44: 0
            })
            .is_none()
        );
        // a mastery that takes away (an alchemist's Pow -100 kind): the band
        // the other way round
        let mut minus = [0; 29];
        minus[14] = -20;
        let hp = channel(&[], &[(8, -20)]);
        assert_eq!(
            of(&with(&hp, &minus), 8).shift(),
            Shift::Same,
            "{:?}",
            of(&with(&hp, &minus), 8)
        );
    }

    /// The frame clears the HUD's left side (the player's gauges, the
    /// effects' bar, the group's bars) and the right's (the minimap, the
    /// tracker, the target's details), and the rows fit it: eight a column,
    /// two columns side by side. The control: a frame from x 190 would be on
    /// the group's bars.
    #[test]
    fn it_fits_between_the_huds_pieces() {
        let f = FRAME;
        assert!(f[0] >= 194 + 4, "right of the group's bars (to 194)");
        assert!(f[1] >= 15 + 34 + 4, "below the effects' bar (to 49)");
        assert!(f[3] < 208, "above the crosshair and the death notice");
        assert!(
            f[2] < crate::tracker::COLUMN[0] && f[2] < crate::map::mini::FRAME[0],
            "left of the right side's pieces"
        );
        // a font 21 high with columns 86 and 113 across (ArialUI's English)
        let l = layout(21.0, [86.0, 113.0]);
        assert!(
            l.lefts[0] > f[0] as f32
                && l.rights[1] < f[2] as f32
                && l.rights[0] + GAP <= l.lefts[1] + 1e-3,
            "{l:?}"
        );
        assert!(
            l.top >= f[1] as f32 && l.top + 7.0 * l.step + 21.0 * l.scale <= f[3] as f32,
            "eight rows inside: {l:?}"
        );
        assert!(l.scale <= SCALE && 21.0 * l.scale < l.step);
        // a wider language's words: smaller, still inside; a small font: the cap
        let w = layout(21.0, [140.0, 160.0]);
        assert!(
            w.scale < l.scale
                && w.lefts[0] >= l.lefts[0]
                && w.rights[0] + GAP <= w.lefts[1] + 1e-3
                && w.rights[1] <= l.rights[1] + 1e-3,
            "{w:?}"
        );
        assert_eq!(layout(8.0, [20.0, 20.0]).scale, SCALE, "never past SCALE");
        let reaching = [190, f[1], f[2], f[3]];
        assert!(reaching[0] < 194, "the control: on the bars");
    }

    /// The disc's own font (SKIPs without run/eqoa.iso): each column's widest
    /// label and a four-digit value fit its column with a space between them.
    #[test]
    fn the_widest_row_fits_its_column() {
        let iso_path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../run/eqoa.iso");
        let Ok(mut disc) = iso::Iso::open(&iso_path) else {
            eprintln!("SKIP: no run/eqoa.iso");
            return;
        };
        let id = vi::ui::font_id() as i32;
        let c = esf::Container::open(disc.read("ARIALUI.CSF").expect("ARIALUI.CSF"))
            .expect("a container");
        let font = (0..c.nodes.len())
            .filter(|&i| c.nodes[i].ty == 0x7000)
            .find_map(|i| esf::font::read(&c, i).filter(|f| f.id == id))
            .expect("ArialUI");
        // the game's words for the rows (WORDS.TXT's English column)
        let words = [
            ["Str", "Sta", "Agi", "Dex", "Wis", "Int", "Cha", "AC"],
            ["HPMax", "POW", "FR", "CR", "LR", "AR", "PR", "DR"],
        ];
        let labels: [[Vec<u16>; 8]; 2] =
            std::array::from_fn(|c| std::array::from_fn(|k| wide(words[c][k])));
        let width = |s: &[u16]| font.width(&String::from_utf16_lossy(s)) as f32;
        let l = layout(font.height as f32, widths(&labels, width));
        eprintln!(
            "ArialUI {} high: scale {}, rows {} apart",
            font.height, l.scale, l.step
        );
        assert!(l.scale >= 0.5, "readable: {l:?}");
        for col in 0..2 {
            let label = words[col].iter().map(|w| font.width(w)).max().unwrap() as f32 * l.scale;
            let value = font.width(ROOM) as f32 * l.scale;
            let room = l.rights[col] - l.lefts[col];
            assert!(
                label + value + font.width(" ") as f32 * l.scale <= room + 1e-3,
                "column {col}: {label} + {value} in {room}"
            );
        }
        assert!(
            l.rights[0] + GAP <= l.lefts[1] + 1e-3,
            "the columns apart: {l:?}"
        );
        // the control: a five-digit value with HPMax would not fit its column
        assert!(
            (font.width("HPMax") + font.width(" 88888")) as f32 * l.scale
                > l.rights[1] - l.lefts[1] + 1e-3,
            "the control must fail"
        );
    }
}
