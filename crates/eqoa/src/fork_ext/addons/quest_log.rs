// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see LICENSE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Retail journal titles, in the packed zero-based order used by Forge.
use super::host::{Host, Snapshot};
use mlua::{Result, Table, Value};
use std::{cell::Cell, rc::Rc};

pub(crate) const MAX_QUESTS: usize = 8;

#[derive(Clone, Copy)]
pub(crate) struct JournalSnapshot {
    pub available: bool,
    pub count: u8,
    pub titles: [[u16; 128]; MAX_QUESTS],
}

impl Default for JournalSnapshot {
    fn default() -> Self {
        Self {
            available: false,
            count: 0,
            titles: [[0; 128]; MAX_QUESTS],
        }
    }
}

impl JournalSnapshot {
    pub fn from_titles(in_game: bool, titles: &[Vec<u16>]) -> Self {
        let mut journal = Self::default();
        if !in_game || titles.len() > MAX_QUESTS {
            return journal;
        }
        journal.count = titles.len() as u8;
        journal.available = true;
        for (out, title) in journal.titles.iter_mut().zip(titles) {
            let len = title.len().min(127);
            out[..len].copy_from_slice(&title[..len]);
        }
        journal
    }
}

fn count(snapshot: Snapshot) -> usize {
    if snapshot.journal.available && snapshot.journal.count as usize <= MAX_QUESTS {
        snapshot.journal.count as usize
    } else {
        0
    }
}

fn quest(lua: &mlua::Lua, state: Rc<Cell<Snapshot>>, index: i32) -> Result<Option<Table>> {
    if index < 0 || index as usize >= count(state.get()) {
        return Ok(None);
    }
    let object = lua.create_table()?;
    object.set(
        "GetName",
        lua.create_function(move |_, _: Table| {
            let snapshot = state.get();
            if index as usize >= count(snapshot) {
                return Ok(String::new());
            }
            let title = &snapshot.journal.titles[index as usize];
            let end = title.iter().position(|c| *c == 0).unwrap_or(title.len());
            Ok(String::from_utf16_lossy(&title[..end]))
        })?,
    )?;
    Ok(Some(object))
}

pub(super) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    let module = lua.create_table()?;
    module.set("max_quests", MAX_QUESTS)?;
    let state = host.snapshot.clone();
    module.set(
        "GetCount",
        lua.create_function(move |_, ()| Ok(count(state.get())))?,
    )?;
    let state = host.snapshot.clone();
    module.set(
        "GetQuestByIndex",
        lua.create_function(move |lua, index: i32| quest(lua, state.clone(), index))?,
    )?;
    let state = host.snapshot.clone();
    module.set(
        "Quests",
        lua.create_function(move |lua, ()| {
            let mut index = -1;
            let state = state.clone();
            lua.create_function_mut(move |lua, ()| {
                index += 1;
                Ok(mlua::MultiValue::from_vec(
                    match quest(lua, state.clone(), index)? {
                        Some(q) => vec![Value::Integer(index as i64), Value::Table(q)],
                        None => vec![Value::Nil],
                    },
                ))
            })
        })?,
    )?;
    host.register_host_module("frontiers_forge.quest_log", Value::Table(module))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forge_review_journal_available_before_first_hp_packet() {
        let raw = Snapshot {
            in_game: true,
            journal: JournalSnapshot::from_titles(true, &["Bat Eyes".encode_utf16().collect()]),
            ..Snapshot::default()
        };
        let managed = raw.managed();
        assert!(
            !managed.in_game,
            "other modules retain their existing health gate"
        );
        assert_eq!(count(managed), 1, "journal must not require an HP receipt");
        assert_eq!(
            count(
                Snapshot {
                    in_game: false,
                    ..raw
                }
                .managed()
            ),
            0
        );
    }
}
