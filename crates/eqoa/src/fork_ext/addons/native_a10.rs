// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! Round A10: mail and the auction for a mod (spec section 7, "Mail and auction"): the `eqoa.mail.get` and
//! `eqoa.auction.get` reads and the actions `eqoa.actions.mail_*` and `auction_*`, over three windows a mod can open
//! for itself (`window_sessions::OWN_KINDS`): `mail_inbox`, `mail_compose` and `auction_search`. Plain data and
//! bindings only: `fork_ext/a10_play.rs` reads the game's gram client and auction state into the snapshots kept
//! here and carries the actions out through the game's own request builders (`gram.rs`, `auction.rs`).
//!
//! What a result means: an action completes when the client has sent the request (`detail.sent`), never when the
//! server has answered. The answer is read afterwards from `eqoa.mail.get()` (its `status`) or `eqoa.auction.get()`
//! (its `pending`). One request is out at a time, as the game has it; an action that finds one still out is refused
//! `busy`, and nothing is ever sent twice. Deleting mail, sending mail that carries tunar or an item, a bid, a buyout
//! and putting an item up for sale each go through the client-drawn "Are you sure?" (`confirm.rs`), starting on No:
//! a mod cannot answer it.
use super::{
    action_broker::{ActionKind, enqueue_lua_with},
    host::Host,
    native::{bind, describe, error, record_cached, require_capability},
};
use mlua::{Result, Value};
use serde_json::{Value as Json, json};
use std::cell::{Cell, RefCell};

pub(crate) const ROUND: u8 = 8;

/// The names of the new actions, as capabilities (`eqoa.actions.<name>`).
pub(crate) const NAMES: [&str; 13] = [
    "mail_refresh",
    "mail_read",
    "mail_delete",
    "mail_take",
    "mail_send",
    "auction_search",
    "auction_page",
    "auction_my_lots",
    "auction_my_bids",
    "auction_bidders",
    "auction_bid",
    "auction_buyout",
    "auction_sell",
];

/// What a mail carries along (an item row of `eqoa.mail.get().pack`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MailItem {
    pub slot: usize,
    pub id: i64,
    pub count: i64,
}

/// An auction search's form, as the game's own fields (`auctionwnd.rs`, `select`). `None` is the form's own default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SearchArgs {
    pub name: String,
    pub seller: String,
    pub class: Option<u8>,
    pub race: Option<u8>,
    pub min_level: Option<i32>,
    pub max_level: Option<i32>,
    pub min_price: Option<i32>,
    pub max_price: Option<i32>,
    pub wear: Option<i32>,
    pub attack: Option<i32>,
}

/// The actions of round A10, carried by the broker as `ActionKind::A10`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum A10Action {
    MailRefresh,
    MailRead { id: u32 },
    MailDelete { id: u32 },
    MailTake { id: u32 },
    MailSend { to: String, subject: String, body: String, tunar: u32, item: Option<MailItem> },
    AuctionSearch(SearchArgs),
    /// `forward` true is the next page.
    AuctionPage { forward: bool },
    AuctionMyLots,
    AuctionMyBids,
    AuctionBidders { lot: i64 },
    AuctionBid { lot: i64, price: i32, count: i32 },
    AuctionBuyout { lot: i64 },
    /// A bank item put up: `slot` is the 0-based row (a `sellable` entry's `slot` less one), `id` that row's item.
    AuctionSell { slot: usize, id: i64, count: i32, price: i32, buyout: i32, hours: i32 },
}

impl A10Action {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::MailRefresh => "mail_refresh",
            Self::MailRead { .. } => "mail_read",
            Self::MailDelete { .. } => "mail_delete",
            Self::MailTake { .. } => "mail_take",
            Self::MailSend { .. } => "mail_send",
            Self::AuctionSearch(_) => "auction_search",
            Self::AuctionPage { .. } => "auction_page",
            Self::AuctionMyLots => "auction_my_lots",
            Self::AuctionMyBids => "auction_my_bids",
            Self::AuctionBidders { .. } => "auction_bidders",
            Self::AuctionBid { .. } => "auction_bid",
            Self::AuctionBuyout { .. } => "auction_buyout",
            Self::AuctionSell { .. } => "auction_sell",
        }
    }
}

/// A snapshot a window's reads come from, with a revision that moves only when its content does.
#[derive(Default)]
struct Slot {
    state: RefCell<Option<Json>>,
    revision: Cell<u64>,
}

impl Slot {
    fn publish(&self, mut state: Json) {
        let mut s = self.state.borrow_mut();
        if let Some(old) = s.as_ref() {
            let mut same = old.clone();
            same["revision"] = Json::Null;
            let mut now = state.clone();
            now["revision"] = Json::Null;
            if same == now {
                return;
            }
        }
        self.revision.set(self.revision.get() + 1);
        state["revision"] = json!(self.revision.get());
        *s = Some(state);
    }
}

thread_local! {
    static MAIL: Slot = Slot::default();
    static AUCTION: Slot = Slot::default();
}

/// The client's reading of the gram client for the open mod mail window.
pub(crate) fn publish_mail(state: Json) {
    MAIL.with(|s| s.publish(state));
}

pub(crate) fn clear_mail() {
    MAIL.with(|s| *s.state.borrow_mut() = None);
}

pub(crate) fn current_mail() -> Option<Json> {
    MAIL.with(|s| s.state.borrow().clone())
}

/// The client's reading of the auction state for the open mod auction window.
pub(crate) fn publish_auction(state: Json) {
    AUCTION.with(|s| s.publish(state));
}

pub(crate) fn clear_auction() {
    AUCTION.with(|s| *s.state.borrow_mut() = None);
}

pub(crate) fn current_auction() -> Option<Json> {
    AUCTION.with(|s| s.state.borrow().clone())
}

fn int_arg(v: &Value) -> Option<i64> {
    match v {
        Value::Integer(n) => Some(*n),
        Value::Number(n) if n.fract() == 0.0 && n.abs() < 1e12 => Some(*n as i64),
        _ => None,
    }
}

type Refusal = (&'static str, String);

fn bad(msg: &str) -> Refusal {
    ("invalid_argument", msg.into())
}

fn field_int(t: &mlua::Table, key: &str, lo: i64, hi: i64) -> std::result::Result<Option<i64>, Refusal> {
    match t.get::<Value>(key).unwrap_or(Value::Nil) {
        Value::Nil => Ok(None),
        v => match int_arg(&v) {
            Some(n) if (lo..=hi).contains(&n) => Ok(Some(n)),
            Some(_) => Err(("out_of_range", format!("{key} must be from {lo} to {hi}"))),
            None => Err(bad(&format!("{key} must be a whole number"))),
        },
    }
}

fn field_text(t: &mlua::Table, key: &str, max_chars: usize) -> std::result::Result<Option<String>, Refusal> {
    match t.get::<Value>(key).unwrap_or(Value::Nil) {
        Value::Nil => Ok(None),
        Value::String(s) => {
            let s = s.to_str().map_err(|_| bad(&format!("{key} must be UTF-8 text")))?.to_string();
            if s.chars().any(|c| c.is_control() && c != '\n') {
                return Err(bad(&format!("{key} holds a control character")));
            }
            if s.chars().count() > max_chars {
                return Err(("out_of_range", format!("{key} is longer than {max_chars} characters")));
            }
            Ok(Some(s))
        }
        _ => Err(bad(&format!("{key} must be text"))),
    }
}

fn id_arg(v: &Value, what: &str, max: i64) -> std::result::Result<i64, Refusal> {
    match int_arg(v) {
        Some(n) if (0..=max).contains(&n) => Ok(n),
        _ => Err(bad(&format!("{what} must be the id of an entry of the window's list"))),
    }
}

/// `{slot = n, id = n, count = n?}`: `slot` is 1-based in Lua (the first row of the list is 1).
fn item_pick(t: &mlua::Table) -> std::result::Result<(usize, i64, Option<i64>), Refusal> {
    let slot = field_int(t, "slot", 1, 100_000)?;
    let id = field_int(t, "id", 0, i64::MAX)?;
    let count = field_int(t, "count", 1, 1_000_000)?;
    match (slot, id) {
        (Some(slot), Some(id)) => Ok((slot as usize - 1, id, count)),
        _ => Err(bad("an item is {slot = a row of the list (1 or more), id = that row's id, count = how many}")),
    }
}

fn parse_send(arg: &Value) -> std::result::Result<A10Action, Refusal> {
    let Value::Table(t) = arg else {
        return Err(bad("pass {to = .., subject = .., body = .., tunar = .., item = {slot = .., id = .., count = ..}}"));
    };
    let to = field_text(t, "to", 512)?.unwrap_or_default();
    let subject = field_text(t, "subject", 32)?.unwrap_or_default();
    if subject.contains('
') {
        return Err(bad("the subject is one line"));
    }
    let body = field_text(t, "body", 512)?.unwrap_or_default();
    let tunar = field_int(t, "tunar", 0, i64::from(u32::MAX))?.unwrap_or(0) as u32;
    let item = match t.get::<Value>("item").unwrap_or(Value::Nil) {
        Value::Nil => None,
        Value::Table(it) => {
            let (slot, id, count) = item_pick(&it)?;
            Some(MailItem { slot, id, count: count.unwrap_or(1) })
        }
        _ => return Err(bad("item must be {slot = .., id = .., count = ..}")),
    };
    Ok(A10Action::MailSend { to, subject, body, tunar, item })
}

fn parse_search(arg: &Value) -> std::result::Result<A10Action, Refusal> {
    let t = match arg {
        Value::Nil => return Ok(A10Action::AuctionSearch(SearchArgs::default())),
        Value::Table(t) => t,
        _ => return Err(bad("pass nothing or {name = .., seller = .., class = .., race = .., min_level = .., ...}")),
    };
    let int = |key: &str, lo: i64, hi: i64| field_int(t, key, lo, hi).map(|v| v.map(|n| n as i32));
    Ok(A10Action::AuctionSearch(SearchArgs {
        name: field_text(t, "name", 63)?.unwrap_or_default(),
        seller: field_text(t, "seller", 23)?.unwrap_or_default(),
        class: field_int(t, "class", 0, 31)?.map(|n| n as u8),
        race: field_int(t, "race", 0, 31)?.map(|n| n as u8),
        min_level: int("min_level", 1, 255)?,
        max_level: int("max_level", 1, 255)?,
        min_price: int("min_price", 0, i64::from(i32::MAX))?,
        max_price: int("max_price", 0, i64::from(i32::MAX))?,
        wear: int("wear", -1, 63)?,
        attack: int("attack", 0, 63)?,
    }))
}

fn parse_sell(arg: &Value) -> std::result::Result<A10Action, Refusal> {
    let Value::Table(t) = arg else {
        return Err(bad("pass {slot = .., id = .., count = .., price = .., buyout = .., hours = ..}"));
    };
    let (slot, id, count) = item_pick(t)?;
    let need = |key: &str, lo: i64, hi: i64| {
        field_int(t, key, lo, hi)?.map(|n| n as i32).ok_or_else(|| bad(&format!("{key} is needed")))
    };
    Ok(A10Action::AuctionSell {
        slot,
        id,
        count: count.map_or(1, |n| n.min(i64::from(i32::MAX)) as i32),
        price: need("price", 0, i64::from(i32::MAX))?,
        buyout: need("buyout", 0, i64::from(i32::MAX))?,
        hours: need("hours", 1, 10_000)?,
    })
}

pub(crate) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    for (path, fields) in [
        (
            "eqoa.mail.State",
            "revision offered status count truncated new entries message pack carries_offer postage max_items tunar names delivered refusal",
        ),
        ("eqoa.mail.Entry", "id from subject date read carries"),
        ("eqoa.mail.Message", "id from subject date text to tunar items returned"),
        ("eqoa.mail.PackItem", "slot id name quantity"),
        (
            "eqoa.auction.State",
            "revision offered mode pending page results minutes lots bids bidders_lot bidders sellable tunar banked uncertain",
        ),
        ("eqoa.auction.Lot", "lot name quantity seller min_bid buyout minutes_left high_bid leader bids next_bid mine"),
        ("eqoa.auction.Bidder", "name bid"),
        ("eqoa.auction.Own", "lot name quantity buyout minutes_left"),
        ("eqoa.auction.Bid", "lot price count escrow"),
        ("eqoa.auction.Sellable", "slot id name quantity"),
    ] {
        describe(lua, path, "supported", Some(ROUND), "");
        for field in fields.split_whitespace() {
            describe(lua, &format!("{path}.{field}"), "supported", Some(ROUND), "");
        }
    }

    bind(
        lua,
        "mail",
        "get",
        lua.create_function(|lua, args: mlua::MultiValue| {
            const SYMBOL: &str = "eqoa.mail.get";
            if let Err((code, msg)) = require_capability(lua, SYMBOL, SYMBOL) {
                return error(lua, SYMBOL, code, &msg);
            }
            if !args.is_empty() {
                return error(lua, SYMBOL, "invalid_argument", "this function takes no arguments");
            }
            match current_mail() {
                // not open yet (the client reads the state at its next step) or no mod holds a mail window
                None => lua.pack_multi((Value::Nil, Value::Nil)),
                Some(state) => lua.pack_multi((record_cached(lua, "mail.get", "eqoa.mail.State", &state)?, Value::Nil)),
            }
        })?,
    )?;
    bind(
        lua,
        "auction",
        "get",
        lua.create_function(|lua, args: mlua::MultiValue| {
            const SYMBOL: &str = "eqoa.auction.get";
            if let Err((code, msg)) = require_capability(lua, SYMBOL, SYMBOL) {
                return error(lua, SYMBOL, code, &msg);
            }
            if !args.is_empty() {
                return error(lua, SYMBOL, "invalid_argument", "this function takes no arguments");
            }
            match current_auction() {
                None => lua.pack_multi((Value::Nil, Value::Nil)),
                Some(state) => {
                    lua.pack_multi((record_cached(lua, "auction.get", "eqoa.auction.State", &state)?, Value::Nil))
                }
            }
        })?,
    )?;

    // no argument
    for (name, action) in [
        ("mail_refresh", A10Action::MailRefresh),
        ("auction_my_lots", A10Action::AuctionMyLots),
        ("auction_my_bids", A10Action::AuctionMyBids),
    ] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, args: mlua::MultiValue| {
                let symbol = format!("eqoa.actions.{name}");
                enqueue_lua_with(lua, &symbol, || {
                    if !args.is_empty() {
                        return Err(bad("this action takes no arguments"));
                    }
                    Ok(ActionKind::A10(action.clone()))
                })
            })?,
        )?;
    }
    // one id
    for (name, make) in [
        ("mail_read", (|id| A10Action::MailRead { id: id as u32 }) as fn(i64) -> A10Action),
        ("mail_delete", |id| A10Action::MailDelete { id: id as u32 }),
        ("mail_take", |id| A10Action::MailTake { id: id as u32 }),
        ("auction_bidders", |id| A10Action::AuctionBidders { lot: id }),
        ("auction_buyout", |id| A10Action::AuctionBuyout { lot: id }),
    ] {
        bind(
            lua,
            "actions",
            name,
            lua.create_function(move |lua, id: Value| {
                let symbol = format!("eqoa.actions.{name}");
                enqueue_lua_with(lua, &symbol, || {
                    // a mail's id is a whole u32 (the game sends it as such); a lot's is an i32
                    let (what, max) = if name.starts_with("mail") {
                        ("a mail id (an eqoa.mail.get() entry's id)", i64::from(u32::MAX))
                    } else {
                        ("a lot (an eqoa.auction.get() lot's id)", i64::from(i32::MAX))
                    };
                    Ok(ActionKind::A10(make(id_arg(&id, what, max)?)))
                })
            })?,
        )?;
    }
    bind(
        lua,
        "actions",
        "mail_send",
        lua.create_function(|lua, arg: Value| {
            enqueue_lua_with(lua, "eqoa.actions.mail_send", || Ok(ActionKind::A10(parse_send(&arg)?)))
        })?,
    )?;
    bind(
        lua,
        "actions",
        "auction_search",
        lua.create_function(|lua, arg: Value| {
            enqueue_lua_with(lua, "eqoa.actions.auction_search", || Ok(ActionKind::A10(parse_search(&arg)?)))
        })?,
    )?;
    bind(
        lua,
        "actions",
        "auction_page",
        lua.create_function(|lua, forward: Value| {
            enqueue_lua_with(lua, "eqoa.actions.auction_page", || match forward {
                Value::Boolean(b) => Ok(ActionKind::A10(A10Action::AuctionPage { forward: b })),
                _ => Err(bad("pass true for the next page or false for the one before")),
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "auction_bid",
        lua.create_function(|lua, arg: Value| {
            enqueue_lua_with(lua, "eqoa.actions.auction_bid", || {
                let Value::Table(t) = &arg else {
                    return Err(bad("pass {lot = .., price = .., count = ..}"));
                };
                let lot = field_int(t, "lot", 0, i64::from(i32::MAX))?.ok_or_else(|| bad("lot is needed"))?;
                let price = field_int(t, "price", 1, i64::from(i32::MAX))?.ok_or_else(|| bad("price is needed"))? as i32;
                let count = field_int(t, "count", 1, 100_000)?.unwrap_or(1) as i32;
                Ok(ActionKind::A10(A10Action::AuctionBid { lot, price, count }))
            })
        })?,
    )?;
    bind(
        lua,
        "actions",
        "auction_sell",
        lua.create_function(|lua, arg: Value| {
            enqueue_lua_with(lua, "eqoa.actions.auction_sell", || Ok(ActionKind::A10(parse_sell(&arg)?)))
        })?,
    )?;
    Ok(())
}

/// The names that need no line of their own in "can act on its own": every one is read-like, a request the game's
/// own window makes by looking, or it asks the player first (`runtime::asks_first_label`).
pub(crate) fn is_a10_action(kind: &str) -> bool {
    NAMES.contains(&kind)
}

/// The honest Mods & Addons line for what a mod can do without asking: plain mail goes out without the question
/// (only a mail that carries tunar or an item, and every delete, bid, buyout and sale, asks the player first).
pub(crate) fn mail_label(caps: &[String]) -> Option<String> {
    let mut steps: Vec<&str> = Vec::new();
    for (name, words) in [
        ("mail_send", "send plain mail"),
        ("mail_take", "take what a mail carries"),
        ("auction_search", "search the auction"),
        ("auction_my_lots", "look at your lots"),
        ("auction_my_bids", "look at your bids"),
    ] {
        if super::native::capability_grants(caps, &format!("eqoa.actions.{name}")) && !steps.contains(&words) {
            steps.push(words);
        }
    }
    (!steps.is_empty()).then(|| format!("Mail and auction: can {}", steps.join(", ")))
}
