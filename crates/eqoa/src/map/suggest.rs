//! In-map marker suggestion form; the existing community transport owns sending.
use super::{community, theme};
use vi::ui::Raster;
#[derive(Clone, Debug)]
pub enum Outcome {
    Submit { name: String, note: String },
    Cancel,
}
#[derive(Default)]
pub struct Form {
    pub open: bool,
    pub name: String,
    pub note: String,
    focus: usize,
    spot: Option<community::Spot>,
    error: Option<String>,
    outcome: Option<Outcome>,
    hits: Vec<([i32; 4], usize)>,
}
impl Form {
    pub fn show(&mut self, spot: community::Spot, name: String) {
        *self = Self {
            open: true,
            name,
            spot: Some(spot),
            ..Default::default()
        };
    }
    pub fn cancel(&mut self) {
        if self.open {
            self.open = false;
            self.outcome = Some(Outcome::Cancel);
            self.hits.clear();
        }
    }
    pub fn take(&mut self) -> Option<Outcome> {
        self.outcome.take()
    }
    fn submit(&mut self) {
        let name = community::clip(&self.name, community::NAME_MAX);
        if name.chars().count() < community::NAME_MIN {
            self.error = Some("Name needs at least 2 characters.".into());
            self.focus = 0;
            return;
        }
        self.open = false;
        self.hits.clear();
        self.outcome = Some(Outcome::Submit {
            name,
            note: community::clip(&self.note, community::NOTE_MAX),
        });
    }
    pub fn key(&mut self, key: &str, text: Option<&str>) {
        if !self.open {
            return;
        }
        match key {
            "escape" => self.cancel(),
            "tab" => self.focus = (self.focus + 1) % 4,
            "up" => self.focus = self.focus.saturating_sub(1),
            "down" => self.focus = (self.focus + 1).min(3),
            "enter" => match self.focus {
                0 => self.focus = 1,
                1 | 2 => self.submit(),
                _ => self.cancel(),
            },
            "backspace" => {
                if self.focus == 0 {
                    self.name.pop();
                } else if self.focus == 1 {
                    self.note.pop();
                }
            }
            _ => {
                if let Some(text) = text {
                    let (s, max) = if self.focus == 0 {
                        (&mut self.name, community::NAME_MAX)
                    } else if self.focus == 1 {
                        (&mut self.note, community::NOTE_MAX)
                    } else {
                        return;
                    };
                    for c in text.chars().filter(|c| !c.is_control()) {
                        if s.chars().count() < max {
                            s.push(c);
                        }
                    }
                    self.error = None;
                }
            }
        }
    }
    pub fn pointer(&mut self, p: [f32; 2], right: bool) -> bool {
        if !self.open {
            return false;
        }
        if right {
            self.cancel();
            return true;
        }
        if let Some((_, action)) = self.hits.iter().rev().find(|(r, _)| {
            p[0] >= r[0] as f32 && p[0] <= r[2] as f32 && p[1] >= r[1] as f32 && p[1] <= r[3] as f32
        }) {
            self.focus = *action;
            match self.focus {
                2 => self.submit(),
                3 => self.cancel(),
                _ => {}
            }
        }
        true
    }
    pub fn draw(&mut self, r: &mut dyn Raster, bounds: [[f32; 2]; 2]) {
        self.hits.clear();
        if !self.open {
            return;
        }
        let w = 300.min((bounds[1][0] - bounds[0][0]) as i32 - 20);
        let h = 204.min((bounds[1][1] - bounds[0][1]) as i32 - 20);
        let x = ((bounds[0][0] + bounds[1][0] - w as f32) * 0.5) as i32;
        let y = ((bounds[0][1] + bounds[1][1] - h as f32) * 0.5) as i32;
        theme::panel(r, [x, y, x + w, y + h]);
        theme::fill(r, [x, y, x + w, y + 2], theme::ACCENT);
        theme::text(
            r,
            "SUGGEST A COMMUNITY MARKER",
            [x + 10, y + 12],
            w - 20,
            theme::TEXT,
        );
        if let Some(s) = self.spot {
            theme::text(
                r,
                &format!("World {}  /  {:.0}, {:.0}", s.world, s.at[0], s.at[1]),
                [x + 10, y + 29],
                w - 20,
                theme::MUTED,
            );
        }
        for (i, (title, text, top, height)) in [
            ("Name", self.name.as_str(), 48, 24),
            ("Note (optional)", self.note.as_str(), 92, 48),
        ]
        .into_iter()
        .enumerate()
        {
            theme::text(r, title, [x + 10, y + top], w - 20, theme::MUTED);
            let b = [x + 10, y + top + 15, x + w - 10, y + top + 15 + height];
            theme::fill(r, b, theme::INPUT);
            theme::frame(
                r,
                b,
                if self.focus == i {
                    theme::ACCENT
                } else {
                    theme::BORDER
                },
            );
            let shown = format!("{text}{}", if self.focus == i { "_" } else { "" });
            let mut lines = vec![String::new()];
            for ch in shown.chars() {
                let last = lines.last_mut().unwrap();
                if !last.is_empty()
                    && r.string_width(&vi::ui::wide(&format!("{last}{ch}"))) > w - 32
                {
                    lines.push(String::new());
                }
                lines.last_mut().unwrap().push(ch);
            }
            let count = ((height - 8) / 14).max(1) as usize;
            let offset = if self.focus == i {
                lines.len().saturating_sub(count)
            } else {
                0
            };
            for (n, line) in lines.iter().skip(offset).take(count).enumerate() {
                theme::text(
                    r,
                    line,
                    [b[0] + 6, b[1] + 5 + n as i32 * 14],
                    w - 32,
                    theme::TEXT,
                );
            }
            self.hits.push((b, i));
        }
        let yy = y + h - 28;
        for (action, label, left) in [(3, "Cancel", x + w - 158), (2, "Submit", x + w - 80)] {
            let b = [left, yy, left + 70, yy + 20];
            theme::control(r, b, self.focus == action || action == 2);
            theme::text(r, label, [left + 8, yy + 4], 54, theme::TEXT);
            self.hits.push((b, action));
        }
        if let Some(e) = &self.error {
            theme::text(r, e, [x + 10, yy - 15], w - 20, theme::KNOWN);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_name_stays_in_map_and_cancel_never_submits() {
        let mut f = Form::default();
        f.show(
            community::Spot {
                world: 0,
                at: [10.0, 20.0],
            },
            "x".into(),
        );
        f.key("enter", None);
        f.key("enter", None);
        assert!(f.open);
        assert!(f.take().is_none());
        f.key("escape", None);
        assert!(matches!(f.take(), Some(Outcome::Cancel)));
    }
    #[test]
    fn name_and_note_are_one_form_and_submit_together() {
        let mut f = Form::default();
        f.show(
            community::Spot {
                world: 2,
                at: [10.0, 20.0],
            },
            "Merchant".into(),
        );
        f.key("enter", None);
        f.key("n", Some("Near the gate"));
        f.key("enter", None);
        assert!(!f.open);
        assert!(
            matches!(f.take(),Some(Outcome::Submit{name,note}) if name=="Merchant"&&note=="Near the gate")
        );
        assert!(f.take().is_none());
    }
}
