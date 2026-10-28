//! Native map search. Recorded spawns and current server observations remain
//! separate layers; neither is a promise that a creature is alive.
use std::collections::BTreeMap;
use std::{cell::RefCell, rc::Rc};
use vi::ui::{Raster, Vui, wide};

#[derive(Clone, Debug)]
pub struct Live {
    pub name: String,
    pub world: i32,
    pub at: [f32; 2],
    pub level: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub id: u32,
    pub name: String,
    pub world: Option<i32>,
    pub at: Option<[f32; 2]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Waypoint(i32),
    Npc(String),
    Group(u32),
    Place(usize),
}

#[derive(Clone, Debug)]
struct Spot {
    name: String,
    world: i32,
    at: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct ResultRow {
    pub name: String,
    pub detail: String,
    pub target: Target,
    pub at: Option<[f32; 2]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Npcs,
    Group,
    Places,
}

#[derive(Clone, Debug)]
enum Action {
    Open,
    Focus,
    Stop,
    Tab(Tab),
    Choose(Target),
    Previous,
    Next,
    Close,
    TrackWaypoint,
}

/// Appearance tokens, independent of search, input and navigation state.
pub mod skin {
    pub use super::super::theme::{GROUP, KNOWN, LIVE, TEXT};
}

#[derive(Default)]
pub struct Search {
    pub open: bool,
    pub toolbar_top: Option<i32>,
    pub hover: Option<[f32; 2]>,
    waypoint: Option<(i32, [f32; 2])>,
    pub query: String,
    pub tab: Tab,
    pub guide: Option<Target>,
    known: Vec<Spot>,
    places: Vec<Spot>,
    live: Vec<Live>,
    group: Vec<Member>,
    loaded: bool,
    world: i32,
    player: [f32; 2],
    selected: usize,
    offset: usize,
    hits: Vec<([i32; 4], Action)>,
    cached: RefCell<Option<(String, Tab, Rc<Vec<ResultRow>>)>>,
}

fn mini_search_rect(m: [f32; 4]) -> [i32; 4] {
    [
        m[2] as i32 - 20,
        m[3] as i32 + 2,
        m[2] as i32,
        m[3] as i32 + 22,
    ]
}
fn valid(at: [f32; 2]) -> bool {
    at.iter().all(|x| x.is_finite() && x.abs() < 10_000_000.0)
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Forge's parallel world/pos arrays use [x,y,z], not map [x,y]. Bad
/// records are ignored instead of silently guiding players to (0,0).
pub fn imported(s: &str) -> Vec<(String, i32, [f32; 2])> {
    let Ok(serde_json::Value::Object(db)) = serde_json::from_str(s) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, row) in db {
        let (Some(worlds), Some(positions)) = (row["world"].as_array(), row["pos"].as_array())
        else {
            continue;
        };
        for (w, p) in worlds.iter().zip(positions) {
            let (Some(w), Some(x), Some(z)) = (
                w.as_i64().and_then(|w| i32::try_from(w).ok()),
                p[0].as_f64(),
                p[2].as_f64(),
            ) else {
                continue;
            };
            let at = [x as f32, z as f32];
            if w >= 0 && valid(at) && !name.trim().is_empty() {
                out.push((name.clone(), w, at));
            }
        }
    }
    out
}

impl Search {
    pub fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = std::env::var_os("EQOA_MAP_DATABASE")
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    std::env::current_exe()
                        .ok()
                        .and_then(|p| p.parent().map(|p| p.join("map-entities.json")))
                });
            if let Some(p) = path.filter(|p| p.is_file()) {
                match std::fs::read_to_string(&p) {
                    Ok(s) => {
                        self.known = imported(&s)
                            .into_iter()
                            .map(|(name, world, at)| Spot { name, world, at })
                            .collect();
                        crate::say(&format!(
                            "Map: {} recorded NPC locations loaded",
                            self.known.len()
                        ));
                    }
                    Err(e) => crate::say(&format!("Map database {}: {e}", p.display())),
                }
            }
        }
        for c in super::markers_data::CAMPS {
            self.places.push(Spot {
                name: format!("{} (XP camp)", c.name),
                world: c.world,
                at: c.at,
            });
        }
        for c in super::markers_data::COACHES {
            self.places.push(Spot {
                name: format!("{} - {} (coach)", c.town, c.coachman),
                world: c.world,
                at: c.at,
            });
        }
    }

    pub fn update(&mut self, world: i32, player: [f32; 2], live: Vec<Live>, group: Vec<Member>) {
        self.ensure_loaded();
        let live: Vec<Live> = live.into_iter().filter(|n| valid(n.at)).collect();
        // Searching holds the player's movement. Keep a stable result order
        // while nearby NPCs walk; the guide itself still reads their current
        // positions every frame. Names/counts/levels, zoning, player movement,
        // or any roster/position validity update invalidate the appropriate list.
        let cached_tab = self.cached.get_mut().as_ref().map_or(self.tab, |c| c.1);
        let changed = self.world != world
            || self.player != player
            || match cached_tab {
                Tab::Npcs => {
                    self.live.len() != live.len()
                        || self.live.iter().zip(&live).any(|(a, b)| {
                            a.name != b.name || a.world != b.world || a.level != b.level
                        })
                }
                Tab::Group => self.group != group,
                Tab::Places => false,
            };
        if changed {
            *self.cached.get_mut() = None;
        }
        self.world = world;
        self.player = player;
        self.live = live;
        self.group = group;
        if let Some(Target::Group(id)) = self.guide.as_ref() {
            if !self.group.iter().any(|m| m.id == *id) {
                self.guide = None;
            }
        }
    }

    pub fn set_waypoint(&mut self, world: i32, at: Option<[f32; 2]>) {
        let next = at.filter(|p| valid(*p)).map(|at| (world, at));
        if self.waypoint != next {
            self.waypoint = next;
            *self.cached.get_mut() = None;
        }
        if matches!(self.guide,Some(Target::Waypoint(w)) if w==world) && next.is_none() {
            self.guide = None;
        }
    }
    pub fn track_waypoint(&mut self) -> bool {
        if let Some((world, _)) = self.waypoint.filter(|(w, _)| *w == self.world) {
            self.guide = Some(Target::Waypoint(world));
            true
        } else {
            false
        }
    }
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.selected = 0;
        self.offset = 0;
    }
    pub fn hide(&mut self) {
        self.open = false;
        self.hits.clear();
    }
    pub fn stop(&mut self) {
        self.guide = None;
    }
    fn nearest(&self, name: &str) -> Option<[f32; 2]> {
        self.known
            .iter()
            .filter(|s| s.world == self.world && s.name.eq_ignore_ascii_case(name))
            .map(|s| s.at)
            .min_by(|a, b| distance(*a, self.player).total_cmp(&distance(*b, self.player)))
    }
    pub fn destination(&self) -> Option<([f32; 2], bool)> {
        match self.guide.as_ref()? {
            Target::Waypoint(w) => self
                .waypoint
                .filter(|(world, _)| world == w && *w == self.world)
                .map(|(_, at)| (at, false)),
            Target::Npc(name) => self
                .live
                .iter()
                .filter(|n| n.world == self.world && n.name.eq_ignore_ascii_case(name))
                .min_by(|a, b| distance(a.at, self.player).total_cmp(&distance(b.at, self.player)))
                .map(|n| (n.at, false))
                .or_else(|| self.nearest(name).map(|at| (at, true))),
            Target::Group(id) => self
                .group
                .iter()
                .find(|m| m.id == *id && m.world == Some(self.world))
                .and_then(|m| m.at.filter(|a| valid(*a)))
                .map(|at| (at, false)),
            Target::Place(i) => self
                .places
                .get(*i)
                .filter(|p| p.world == self.world)
                .map(|p| (p.at, true)),
        }
    }
    pub fn status(&self) -> Option<String> {
        let t = self.guide.as_ref()?;
        let name = match t {
            Target::Waypoint(_) => "Personal waypoint".into(),
            Target::Npc(n) => n.clone(),
            Target::Group(id) => self.group.iter().find(|m| m.id == *id)?.name.clone(),
            Target::Place(i) => self.places.get(*i)?.name.clone(),
        };
        Some(match self.destination() {
            Some((at, known)) => format!(
                "{} {:.0}: {name}",
                if known {
                    "Known"
                } else if matches!(t, Target::Waypoint(_)) {
                    "Waypoint"
                } else if matches!(t, Target::Group(_)) {
                    "Group"
                } else {
                    "Live"
                },
                distance(at, self.player)
            ),
            None => format!("Unavailable here: {name}"),
        })
    }
    pub fn results(&self) -> Rc<Vec<ResultRow>> {
        if let Some((query, tab, rows)) = self.cached.borrow().as_ref() {
            if query == &self.query && *tab == self.tab {
                return Rc::clone(rows);
            }
        }
        let rows = Rc::new(self.build_results());
        *self.cached.borrow_mut() = Some((self.query.clone(), self.tab, Rc::clone(&rows)));
        rows
    }

    fn build_results(&self) -> Vec<ResultRow> {
        let words: Vec<String> = self
            .query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let matches = |n: &str| {
            let n = n.to_lowercase();
            words.iter().all(|w| n.contains(w))
        };
        let mut out = Vec::new();
        match self.tab {
            Tab::Npcs => {
                // One pass over the database, rather than a database scan per
                // result. The search popup must not stall the game loop.
                let mut names = BTreeMap::<String, (String, Option<[f32; 2]>, usize, Option<(u8,u8)>)>::new();
                for s in self.known.iter().filter(|s| matches(&s.name)) {
                    let row = names
                        .entry(s.name.to_lowercase())
                        .or_insert_with(|| (s.name.clone(), None, 0, None));
                    if s.world == self.world
                        && row.1.is_none_or(|at| {
                            distance(s.at, self.player) < distance(at, self.player)
                        })
                    {
                        row.1 = Some(s.at);
                    }
                }
                for n in self
                    .live
                    .iter()
                    .filter(|n| n.world == self.world && matches(&n.name))
                {
                    let row = names
                        .entry(n.name.to_lowercase())
                        .or_insert_with(|| (n.name.clone(), None, 0, None));
                    if row.2 == 0 || row.1.is_none_or(|at| distance(n.at,self.player)<distance(at,self.player)) {
                        row.1 = Some(n.at);
                    }
                    row.2 += 1;
                    if n.level > 0 {
                        row.3=Some(row.3.map_or((n.level,n.level),|(lo,hi)|(lo.min(n.level),hi.max(n.level))));
                    }
                }
                for (name, at, count, levels) in names.into_values() {
                    let detail = if count > 0 {
                        let level=levels.map_or_else(||"level unknown".into(),|(lo,hi)|if lo==hi{format!("level {lo}")}else{format!("levels {lo}-{hi}")});
                        format!("Spawned nearby: {count}; {level}")
                    } else if at.is_some() {
                        "Not detected (range/spawn unknown)".into()
                    } else {
                        "Other world; not detected".into()
                    };
                    out.push(ResultRow {
                        name: name.clone(),
                        detail,
                        target: Target::Npc(name),
                        at,
                    });
                }
            }
            Tab::Group => {
                for m in self.group.iter().filter(|m| matches(&m.name)) {
                    let at = m.at.filter(|p| valid(*p) && m.world == Some(self.world));
                    let detail = if at.is_some() {
                        "Group position"
                    } else if m.world.is_some_and(|w| w != self.world) {
                        "In another world"
                    } else {
                        "Position unavailable"
                    }
                    .into();
                    out.push(ResultRow {
                        name: m.name.clone(),
                        detail,
                        target: Target::Group(m.id),
                        at,
                    });
                }
            }
            Tab::Places => {
                if let Some((w, at)) = self
                    .waypoint
                    .filter(|(w, _)| *w == self.world && matches("Personal waypoint"))
                {
                    out.push(ResultRow {
                        name: "Personal waypoint".into(),
                        detail: "Your map pin / select to guide".into(),
                        target: Target::Waypoint(w),
                        at: Some(at),
                    });
                }
                for (i, p) in self
                    .places
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| matches(&p.name))
                {
                    out.push(ResultRow {
                        name: p.name.clone(),
                        detail: if p.world == self.world {
                            "Known location"
                        } else {
                            "In another world"
                        }
                        .into(),
                        target: Target::Place(i),
                        at: (p.world == self.world).then_some(p.at),
                    });
                }
            }
        }
        out.sort_by(|a, b| {
            a.at.is_none()
                .cmp(&b.at.is_none())
                .then_with(|| {
                    distance(a.at.unwrap_or(self.player), self.player)
                        .total_cmp(&distance(b.at.unwrap_or(self.player), self.player))
                })
                .then_with(|| a.name.cmp(&b.name))
        });
        out
    }
    pub fn key(&mut self, name: &str, text: Option<&str>) {
        match name {
            "escape" => self.open = false,
            "backspace" => {
                self.query.pop();
                self.selected = 0;
                self.offset = 0;
            }
            "tab" => {
                self.tab = match self.tab {
                    Tab::Npcs => Tab::Group,
                    Tab::Group => Tab::Places,
                    Tab::Places => Tab::Npcs,
                };
                self.selected = 0;
                self.offset = 0;
            }
            "up" => {
                self.selected = self.selected.saturating_sub(1);
                self.offset = self.offset.min(self.selected);
            }
            "down" => {
                self.selected = (self.selected + 1).min(self.results().len().saturating_sub(1));
                self.offset = self.offset.max(self.selected.saturating_sub(5));
            }
            "enter" => {
                if let Some(r) = self.results().get(self.selected).filter(|r| r.at.is_some()) {
                    self.guide = Some(r.target.clone());
                    self.open = false;
                }
            }
            _ => {
                if let Some(t) = text {
                    for c in t.chars().filter(|c| !c.is_control()) {
                        if self.query.chars().count() < 80 {
                            self.query.push(c);
                        }
                    }
                    self.selected = 0;
                    self.offset = 0;
                }
            }
        }
    }
    pub fn pointer(&mut self, p: [f32; 2], right: bool) -> bool {
        let action = self
            .hits
            .iter()
            .rev()
            .find(|(r, _)| {
                p[0] >= r[0] as f32
                    && p[0] <= r[2] as f32
                    && p[1] >= r[1] as f32
                    && p[1] <= r[3] as f32
            })
            .map(|(_, a)| a.clone());
        if right && self.open {
            self.open = false;
            return true;
        }
        if let Some(a) = action {
            match a {
                Action::Open => self.toggle(),
                Action::Focus => {}
                Action::Stop => self.stop(),
                Action::TrackWaypoint => {
                    self.track_waypoint();
                }
                Action::Close => self.open = false,
                Action::Tab(t) => {
                    self.tab = t;
                    self.selected = 0;
                    self.offset = 0;
                }
                Action::Choose(t) => {
                    self.guide = Some(t);
                    self.open = false;
                }
                Action::Previous => {
                    self.offset = self.offset.saturating_sub(6);
                    self.selected = self.offset;
                }
                Action::Next => {
                    self.offset = (self.offset + 6).min(self.results().len().saturating_sub(1));
                    self.selected = self.offset;
                }
            }
            return true;
        }
        if self.open {
            self.open = false;
            return true;
        }
        false
    }

    /// Called with the native layer in centered frame coordinates. Search is
    /// temporary; the only persistent navigation text is one minimap line.
    pub fn draw(
        &mut self,
        r: &mut dyn Raster,
        _vui: &Vui,
        bounds: [[f32; 2]; 2],
        mini: Option<[f32; 4]>,
    ) {
        use super::theme as t;
        self.hits.clear();
        let [[left, top], [right, bottom]] = bounds;
        let bind = |id: &str| {
            crate::settings::with_binds(|b| {
                b.key(crate::binds::row_named(id).unwrap())
                    .map(crate::binds::display)
                    .unwrap_or_else(|| "Unbound".into())
            })
        };
        let (x, y) = mini
            .map(|m| {
                let b = mini_search_rect(m);
                (b[0], b[1])
            })
            .unwrap_or((left as i32 + 8, self.toolbar_top.unwrap_or(top as i32 + 34)));
        let title = if mini.is_some() {
            String::new()
        } else {
            format!("Search  {}", bind("map_search"))
        };
        let width = if mini.is_some() {
            20
        } else {
            r.string_width(&wide(&title)) + 16
        };
        self.button(
            r,
            [x, y, x + width, y + 20],
            &title,
            Action::Open,
            self.open,
        );
        if mini.is_some() {
            t::magnifier(r, [x, y, x + 20, y + 20], t::TEXT);
            if !self.open
                && self.hover.is_some_and(|p| {
                    inside(p, [x as f32, y as f32, (x + 20) as f32, (y + 20) as f32])
                })
            {
                let text = format!("Search / {}", bind("map_search"));
                let w = r.string_width(&wide(&text)) + 12;
                let b = [(x - w - 4).max(left as i32 + 2), y, x - 4, y + 20];
                t::panel(r, b);
                t::text(r, &text, [b[0] + 6, b[1] + 3], b[2] - b[0] - 12, t::TEXT);
            }
        }
        if self.guide.is_some() {
            let title = if mini.is_some() {
                "X".into()
            } else {
                format!("Stop  {}", bind("map_stop"))
            };
            let stop = if mini.is_some() {
                [x - 23, y, x - 3, y + 20]
            } else {
                [
                    x + width + 3,
                    y,
                    x + width + 10 + r.string_width(&wide(&title)) + 12,
                    y + 20,
                ]
            };
            self.button(r, stop, &title, Action::Stop, false);
        }
        if mini.is_none() && self.waypoint.is_some_and(|(w, _)| w == self.world) {
            let bx = self.hits.last().map_or(x + width + 4, |h| h.0[2] + 4);
            self.button(
                r,
                [bx, y, bx + 110, y + 20],
                "Track waypoint",
                Action::TrackWaypoint,
                matches!(self.guide, Some(Target::Waypoint(_))),
            );
        }
        if let Some(m) = mini {
            if let Some(s) = self.status().filter(|_| {
                self.hover
                    .is_some_and(|p| inside(p, [m[0], m[1], m[2], m[3] + 22.0]))
            }) {
                let b = [m[0] as i32, y + 23, right as i32 - 3, y + 42];
                t::fill(r, b, t::SURFACE);
                t::text(r, &s, [b[0] + 4, b[1] + 3], b[2] - b[0] - 8, t::TEXT);
            }
        }
        if !self.open {
            return;
        }
        let w = 252.min((right - left) as i32 - 16);
        let h = 300.min((bottom - top) as i32 - 70);
        let px = mini
            .map_or(left as i32 + 8, |m| m[0] as i32 - w - 6)
            .max(left as i32 + 8);
        let py = mini
            .map_or(y + 24, |m| m[1] as i32)
            .min(bottom as i32 - h - 8)
            .max(top as i32 + 8);
        let panel = [px, py, px + w, py + h];
        t::panel(r, panel);
        self.hits.push((panel, Action::Focus));
        t::text(r, "FIND ON MAP", [px + 10, py + 10], w - 40, t::MUTED);
        self.button(
            r,
            [px + w - 25, py + 5, px + w - 5, py + 25],
            "X",
            Action::Close,
            false,
        );
        let input = [px + 8, py + 32, px + w - 8, py + 56];
        t::fill(r, input, t::INPUT);
        t::frame(r, input, t::ACCENT);
        let query = if self.query.is_empty() {
            match self.tab {
                Tab::Npcs => "NPC name...",
                Tab::Group => "Group member...",
                Tab::Places => "Camp or coach...",
            }
            .into()
        } else {
            format!("{}_", self.query)
        };
        t::text(
            r,
            &query,
            [px + 15, py + 37],
            w - 30,
            if self.query.is_empty() {
                t::MUTED
            } else {
                t::TEXT
            },
        );
        for (i, (name, tab)) in [
            ("NPCs", Tab::Npcs),
            ("Group", Tab::Group),
            ("Places", Tab::Places),
        ]
        .into_iter()
        .enumerate()
        {
            let a = px + 8 + i as i32 * (w - 16) / 3;
            let b = [a, py + 64, a + (w - 16) / 3 - 2, py + 85];
            t::text(
                r,
                name,
                [a + 5, py + 67],
                b[2] - a - 10,
                if self.tab == tab { t::TEXT } else { t::MUTED },
            );
            if self.tab == tab {
                t::fill(r, [a, b[3] - 2, b[2], b[3]], t::ACCENT);
            }
            self.hits.push((b, Action::Tab(tab)));
        }
        let results = self.results();
        self.selected = self.selected.min(results.len().saturating_sub(1));
        self.offset = self.offset.min(self.selected);
        let rows = ((h - 132) / 28).clamp(1, 6) as usize;
        for (i, row) in results.iter().enumerate().skip(self.offset).take(rows) {
            let yy = py + 94 + (i - self.offset) as i32 * 28;
            let b = [px + 6, yy - 2, px + w - 6, yy + 25];
            let selected = i == self.selected;
            if selected {
                t::fill(r, b, t::SELECTED);
                t::fill(r, [b[0], b[1], b[0] + 2, b[3]], t::ACCENT);
            }
            t::text(
                r,
                &row.name,
                [px + 13, yy],
                w - 26,
                if row.at.is_none() { t::MUTED } else { t::TEXT },
            );
            t::text(r, &row.detail, [px + 13, yy + 13], w - 26, t::MUTED);
            if row.at.is_some() {
                self.hits.push((b, Action::Choose(row.target.clone())));
            }
        }
        if results.is_empty() {
            t::text(
                r,
                if self.tab == Tab::Group {
                    "No group members"
                } else {
                    "No matching locations"
                },
                [px + 13, py + 108],
                w - 26,
                t::MUTED,
            );
        }
        let foot = py + h - 34;
        t::fill(r, [px + 8, foot - 7, px + w - 8, foot - 6], t::BORDER);
        t::text(
            r,
            &format!("{} matches / Enter guides", results.len()),
            [px + 10, foot - 2],
            w - 20,
            t::MUTED,
        );
        self.button(
            r,
            [px + 8, foot + 12, px + 61, py + h - 3],
            "Prev",
            Action::Previous,
            false,
        );
        self.button(
            r,
            [px + w - 61, foot + 12, px + w - 8, py + h - 3],
            "Next",
            Action::Next,
            false,
        );
    }
    fn button(&mut self, r: &mut dyn Raster, b: [i32; 4], s: &str, a: Action, active: bool) {
        super::theme::control(r, b, active);
        super::theme::text(r, s, [b[0] + 6, b[1] + 3], b[2] - b[0] - 12, skin::TEXT);
        self.hits.push((b, a));
    }

    /// Destination and live matches coexist. Coordinates and clipping here
    /// are frame coordinates; callers supply their actual world projection.
    pub fn draw_guidance(
        &self,
        ui: &mut render::ui::Ui,
        white: usize,
        to: impl Fn([f32; 2]) -> [f32; 2],
        clip: [f32; 4],
        rim: bool,
    ) {
        let Some(t) = &self.guide else { return };
        let center = to(self.player);
        if let Some((at, known)) = self.destination() {
            let at = if rim {
                super::mini::to_rim(
                    center,
                    to(at),
                    [clip[0] + 5.0, clip[1] + 5.0, clip[2] - 5.0, clip[3] - 5.0],
                )
            } else {
                to(at)
            };
            let color = if matches!(t, Target::Waypoint(_)) {
                super::WAYPOINT
            } else if matches!(t, Target::Group(_)) {
                skin::GROUP
            } else {
                skin::KNOWN
            };
            for (a, b) in
                super::markers::dashes(center, at, clip, 5.0, if known { 4.0 } else { 0.0 })
            {
                super::markers::segment(ui, white, a, b, 0.7, color);
            }
            dot(ui, white, at, 4.0, color);
        }
        if let Target::Npc(name) = t {
            for s in self
                .known
                .iter()
                .filter(|s| s.world == self.world && s.name.eq_ignore_ascii_case(name))
            {
                let p = to(s.at);
                if inside(p, clip) {
                    dot(ui, white, p, 3.0, skin::KNOWN);
                }
            }
            for n in self
                .live
                .iter()
                .filter(|n| n.world == self.world && n.name.eq_ignore_ascii_case(name))
            {
                let p = to(n.at);
                if inside(p, clip) {
                    dot(ui, white, p, 5.0, skin::LIVE);
                }
            }
        }
    }
}
fn inside(p: [f32; 2], r: [f32; 4]) -> bool {
    p[0] >= r[0] && p[0] <= r[2] && p[1] >= r[1] && p[1] <= r[3]
}
fn dot(ui: &mut render::ui::Ui, white: usize, p: [f32; 2], radius: f32, color: [f32; 4]) {
    let [x, y] = p;
    for (a, b) in [
        ([x - radius, y], [x, y - radius]),
        ([x, y - radius], [x + radius, y]),
        ([x + radius, y], [x, y + radius]),
        ([x, y + radius], [x - radius, y]),
    ] {
        super::markers::segment(ui, white, a, b, 0.8, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn known() -> Search {
        let mut s = Search {
            loaded: true,
            ..Default::default()
        };
        s.known = imported(r#"{"Rat":{"world":[0,0,2],"pos":[[10,500,20],[100,0,200],[1,0,2]]}}"#)
            .into_iter()
            .map(|(name, world, at)| Spot { name, world, at })
            .collect();
        s.guide = Some(Target::Npc("Rat".into()));
        s
    }
    #[test]
    fn map_refinement_minimap_search_uses_a_compact_hit_target() {
        let b = mini_search_rect([474.0, 50.0, 586.0, 162.0]);
        assert_eq!((b[2] - b[0], b[3] - b[1]), (20, 20));
        assert!(b[2] <= 586 && b[0] >= 474);
    }
    #[test]
    fn map_refinement_presence_changes_with_observation_without_claiming_absence() {
        let mut s = known();
        s.query = "Rat".into();
        let row = s.results();
        assert!(row[0].detail.contains("Not detected"));
        s.update(
            0,
            [0.0; 2],
            vec![Live {
                name: "Rat".into(),
                world: 0,
                at: [3.0, 4.0],
                level: 7,
            }],
            vec![],
        );
        assert!(s.results()[0].detail.contains("Spawned nearby"));
        s.update(0, [0.0; 2], vec![], vec![]);
        assert!(s.results()[0].detail.contains("Not detected"));
        assert!(!s.results()[0].detail.contains("Down"));
    }
    #[test]
    fn map_refinement_confirmed_spawn_guides_to_the_live_position() {
        let mut s = known();
        s.update(
            0,
            [0.0; 2],
            vec![Live {
                name: "Rat".into(),
                world: 0,
                at: [3.0, 4.0],
                level: 7,
            }],
            vec![],
        );
        assert_eq!(s.destination(), Some(([3.0, 4.0], false)));
    }

    #[test]
    fn forge_database_preserves_world_and_xz_not_height() {
        let db = imported(r#"{"Rat":{"world":[0,2],"pos":[[10,500,20],[30,600,40]]}}"#);
        assert_eq!(
            db,
            vec![
                ("Rat".into(), 0, [10.0, 20.0]),
                ("Rat".into(), 2, [30.0, 40.0])
            ]
        );
    }
    #[test]
    fn malformed_records_never_become_origin_destinations() {
        let db =
            imported(r#"{"Bad":{"world":[0],"pos":[[3]]},"Good":{"world":[0],"pos":[[1,2,3]]}}"#);
        assert_eq!(db, vec![("Good".into(), 0, [1.0, 3.0])]);
        assert!(imported("broken").is_empty());
    }
    #[test]
    fn live_match_is_preferred_and_recorded_fallback_stays_in_this_world() {
        let mut s = known();
        s.update(
            0,
            [0.0, 0.0],
            vec![Live {
                name: "Rat".into(),
                world: 0,
                at: [3.0, 4.0],
                level: 1,
            }],
            vec![],
        );
        assert_eq!(s.destination(), Some(([3.0, 4.0], false)));
        assert!(s.results()[0].detail.contains("Spawned nearby: 1"));
        s.update(0, [99.0, 199.0], vec![], vec![]);
        assert_eq!(s.destination(), Some(([100.0, 200.0], true)));
        s.update(3, [0.0, 0.0], vec![], vec![]);
        assert_eq!(s.destination(), None);
    }
    #[test]
    fn group_loses_position_without_guiding_to_zero_and_departure_clears() {
        let mut s = Search {
            loaded: true,
            ..Default::default()
        };
        s.guide = Some(Target::Group(7));
        let mut m = Member {
            id: 7,
            name: "Friend".into(),
            world: Some(0),
            at: Some([12.0, 34.0]),
        };
        s.update(0, [0.0, 0.0], vec![], vec![m.clone()]);
        assert_eq!(s.destination(), Some(([12.0, 34.0], false)));
        m.at = None;
        s.update(0, [0.0, 0.0], vec![], vec![m.clone()]);
        assert!(s.destination().is_none());
        assert!(s.guide.is_some());
        m.at = Some([1.0, 2.0]);
        m.world = Some(2);
        s.update(0, [0.0, 0.0], vec![], vec![m]);
        assert!(s.destination().is_none());
        s.update(0, [0.0, 0.0], vec![], vec![]);
        assert!(s.guide.is_none());
    }
    #[test]
    fn typing_and_selection_close_only_the_temporary_panel() {
        let mut s = known();
        s.open = true;
        s.key("x", Some("ra"));
        assert_eq!(s.query, "ra");
        assert_eq!(s.results().len(), 1);
        s.key("enter", None);
        assert!(!s.open);
        assert_eq!(s.guide, Some(Target::Npc("Rat".into())));
        s.toggle();
        s.key("escape", None);
        assert!(!s.open);
        assert!(s.guide.is_some());
        s.stop();
        assert!(s.guide.is_none());
    }
    #[test]
    fn search_controls_fit_and_results_consume_clicks_before_the_game() {
        use crate::pointer::testing::{NullRaster, mid, vui};
        let mut s = known();
        s.open = true;
        s.draw(
            &mut NullRaster,
            &vui(),
            [[0.0, 0.0], [640.0, 448.0]],
            Some([474.0, 50.0, 586.0, 162.0]),
        );
        assert!(
            s.hits
                .iter()
                .all(|(r, _)| r[0] >= 0 && r[1] >= 0 && r[2] <= 640 && r[3] <= 448)
        );
        let pick = s
            .hits
            .iter()
            .find(|(_, a)| matches!(a, Action::Choose(_)))
            .unwrap()
            .0;
        assert!(s.pointer(mid(pick), false));
        assert!(!s.open);
        assert!(s.guide.is_some());
        assert!(!s.pointer([10.0, 400.0], false));
        s.open = true;
        assert!(s.pointer([10.0, 400.0], false));
        assert!(!s.open);
    }
    #[test]
    fn map_actions_rebind_and_persist_while_queries_remain_text() {
        use crate::binds::{Binds, KeyEdge, Sent};
        let mut b = Binds::default();
        b.bind_key(crate::binds::row_named("map_search").unwrap(), "v");
        b.bind_key(crate::binds::row_named("map_stop").unwrap(), "o");
        let b = Binds::from_text(&b.to_text());
        assert!(
            matches!(KeyEdge::default().key("v",true,false,false,&b),Some(Sent::Key(k)) if k=="f2")
        );
        assert!(
            matches!(KeyEdge::default().key("o",true,false,false,&b),Some(Sent::Key(k)) if k=="f4")
        );
        assert!(
            KeyEdge::default()
                .key("f2", true, false, false, &b)
                .is_none()
        );
        assert!(
            matches!(KeyEdge::default().key("v",true,false,true,&b),Some(Sent::Key(k)) if k=="v")
        );
    }
    #[test]
    fn search_results_refresh_after_zoning_roster_updates_and_despawns() {
        let mut s = known();
        s.query = "rat".into();
        s.update(
            0,
            [0.0, 0.0],
            vec![Live {
                name: "Rat".into(),
                world: 0,
                at: [3.0, 4.0],
                level: 1,
            }],
            vec![],
        );
        assert!(s.results()[0].detail.contains("Spawned nearby: 1"));
        s.update(0, [0.0, 0.0], vec![], vec![]);
        assert!(
            !s.results()[0].detail.contains("live"),
            "despawn must remove live status from cached results"
        );
        s.update(4, [0.0, 0.0], vec![], vec![]);
        assert!(
            s.results()[0].at.is_none(),
            "a cached same-world position must not survive zoning"
        );
        s.tab = Tab::Group;
        s.query.clear();
        s.update(
            0,
            [0.0, 0.0],
            vec![],
            vec![Member {
                id: 7,
                name: "Friend".into(),
                world: Some(0),
                at: Some([10.0, 20.0]),
            }],
        );
        assert!(s.results()[0].at.is_some());
        s.update(
            0,
            [0.0, 0.0],
            vec![],
            vec![Member {
                id: 7,
                name: "Friend".into(),
                world: None,
                at: None,
            }],
        );
        assert!(s.results()[0].at.is_none());
        s.update(0, [0.0, 0.0], vec![], vec![]);
        assert!(s.results().is_empty());
    }
    #[test]
    fn waypoint_guidance_updates_and_never_points_into_the_wrong_world() {
        let mut s = Search {
            loaded: true,
            ..Default::default()
        };
        s.set_waypoint(0, Some([10.0, 20.0]));
        assert!(s.track_waypoint());
        assert_eq!(s.destination(), Some(([10.0, 20.0], false)));
        s.set_waypoint(0, Some([30.0, 40.0]));
        assert_eq!(s.destination(), Some(([30.0, 40.0], false)));
        s.update(2, [0.0, 0.0], vec![], vec![]);
        s.set_waypoint(2, None);
        assert!(s.destination().is_none());
        assert!(s.guide.is_some());
        s.update(0, [0.0, 0.0], vec![], vec![]);
        s.set_waypoint(0, Some([30.0, 40.0]));
        assert!(s.destination().is_some());
        s.set_waypoint(0, None);
        assert!(
            s.guide.is_none(),
            "arrival or pin deletion clears only its guide"
        );
    }
}
