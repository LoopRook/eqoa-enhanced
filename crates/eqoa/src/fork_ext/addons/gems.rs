// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! `eqoa.gems`: the crafting gems the game's players know by name. Static shipped metadata (name,
//! craft, rarity, stat), not anything the server sends; a server's own gem names stay unknown.
use super::{
    native::{bind, error, record},
    native_state::uint,
};
use mlua::{Function, Lua, MultiValue, Result, Value};
use serde_json::json;

/// (name, type, rarity, stat): the 80 entries of the pinned Forge lookup, keys exact (case-sensitive).
const GEMS: [(&str, &str, &str, &str); 80] = [
    ("Achroite", "Armorsmithing/Tailoring", "Common", "AR"),
    ("Amblygonite", "Armorsmithing/Tailoring", "Common", "PR"),
    ("Apatite", "Armorsmithing/Tailoring", "Common", "WIS"),
    ("Dravite", "Armorsmithing/Tailoring", "Common", "LR"),
    ("Idocrase", "Armorsmithing/Tailoring", "Common", "CHA"),
    ("Indicolite", "Armorsmithing/Tailoring", "Common", "CR"),
    ("Leucite", "Armorsmithing/Tailoring", "Common", "INT"),
    ("Pectolite", "Armorsmithing/Tailoring", "Common", "AGI"),
    ("Rhodizite", "Armorsmithing/Tailoring", "Common", "STA"),
    ("Rubellite", "Armorsmithing/Tailoring", "Common", "FR"),
    ("Schorl", "Armorsmithing/Tailoring", "Common", "DR"),
    ("Sinhalite", "Armorsmithing/Tailoring", "Common", "STR"),
    ("Zoisite", "Armorsmithing/Tailoring", "Common", "DEX"),
    ("Enstatite", "Armorsmithing", "Uncommon", "STR STA"),
    ("Lazulite", "Armorsmithing", "Uncommon", "AGI, STA"),
    ("Takish Ivory", "Armorsmithing", "Uncommon", "DEX, AGI"),
    ("Marr's Blessed Alloy", "Armorsmithing", "Uncommon", "WIS, STA"),
    ("Takish Ivory Studs", "Tailoring", "Uncommon", "INT, AGI"),
    ("Rallosian Studs", "Tailoring", "Uncommon", "DEX, STA"),
    ("Marr's Blessed Studs", "Tailoring", "Uncommon", "WIS, CHA"),
    ("Tunare Hair Thread", "Tailoring", "Uncommon", "DEX, WIS"),
    ("Mana Infused Alloy", "Armorsmithing", "Rare", "POW"),
    ("Nephrite", "Armorsmithing", "Rare", "HP, POW"),
    ("Titan Alloy", "Armorsmithing", "Rare", "HP"),
    ("Ancient Treant Sap", "Tailoring", "Rare", "HP, PWR MAX"),
    ("Mana Infused Thread", "Tailoring", "Rare", "POW"),
    ("Titan Alloy Studs", "Tailoring", "Rare", "HP"),
    ("Heartsblood Alloy", "Armorsmithing", "Ultra Rare", "HP, STA, AR"),
    ("Mindflow Crystals", "Armorsmithing", "Ultra Rare", "POW, WIS, AR"),
    ("Heartsblood Alloy Studs", "Tailoring", "Ultra Rare", "HP, STA, AR"),
    ("Mindflow Thread", "Tailoring", "Ultra Rare", "POW, WIS, AR"),
    ("Ametrine", "Weaponsmithing", "Common", "DEX"),
    ("Bloodstone", "Weaponsmithing", "Common", "STR"),
    ("Citrine", "Weaponsmithing", "Common", "STA"),
    ("Erollite", "Weaponsmithing", "Common", "CHA"),
    ("Heliodore", "Weaponsmithing", "Common", "WIS"),
    ("Moonstone", "Weaponsmithing", "Common", "INT"),
    ("Tsavorite", "Weaponsmithing", "Common", "AGI"),
    ("Ethereal Mists", "Weaponsmithing", "Rare", "Power DoT"),
    ("Magnesium Alloy", "Weaponsmithing", "Rare", "Fire Proc"),
    ("Malign Alloy", "Weaponsmithing", "Rare", "Disease Proc"),
    ("Rogue Metal", "Weaponsmithing", "Rare", "Poison Proc"),
    ("Volatile Mana Crystals", "Weaponsmithing", "Rare", "Arcane DoT"),
    ("Negative Energy Stone", "Weaponsmithing", "Ultra Rare", "Mana Tap"),
    ("Viscious Vampire Blood", "Weaponsmithing", "Ultra Rare", "Blood Tap"),
    ("Dervish Ice Crystals", "Weaponsmithing", "Uncommon", "Cold Proc"),
    ("Ectoplasmic Crystals", "Weaponsmithing", "Uncommon", "Cursed Proc"),
    ("Intellect Spores", "Weaponsmithing", "Uncommon", "Arcane Power Dmg"),
    ("Amber", "Jewelcrafting", "Common", "STR"),
    ("Aquamarine", "Jewelcrafting", "Common", "PWR MINOR"),
    ("Black Bloodstone", "Jewelcrafting", "Common", "STA"),
    ("Carnelian", "Jewelcrafting", "Common", "AGI"),
    ("Cat's Eye Agate", "Jewelcrafting", "Common", "CHA"),
    ("Clear Quartz", "Jewelcrafting", "Common", "LR"),
    ("Hematite", "Jewelcrafting", "Common", "FR"),
    ("Jasper", "Jewelcrafting", "Common", "WIS"),
    ("Lapis Lazuli", "Jewelcrafting", "Common", "DR"),
    ("Malachite", "Jewelcrafting", "Common", "PR"),
    ("Onyx", "Jewelcrafting", "Common", "DEX"),
    ("Pyrite", "Jewelcrafting", "Common", "AC"),
    ("Star Rose Quartz", "Jewelcrafting", "Common", "INT"),
    ("Tourmaline", "Jewelcrafting", "Common", "HP MINOR"),
    ("Turquoise", "Jewelcrafting", "Common", "CR"),
    ("Wolf's Eye Agate", "Jewelcrafting", "Common", "AR"),
    ("Diamond", "Jewelcrafting", "Rare", "CHA, HP, AC"),
    ("Emerald", "Jewelcrafting", "Rare", "PWR MAJOR"),
    ("Fire Emerald", "Jewelcrafting", "Rare", "DEX, PWR, FR"),
    ("Ruby", "Jewelcrafting", "Rare", "HP MAJOR"),
    ("Sapphire", "Jewelcrafting", "Rare", "HP, PWR MAJOR"),
    ("Star Ruby", "Jewelcrafting", "Rare", "STR, PR, HP"),
    ("Black Sapphire", "Jewelcrafting", "Ultra Rare", "STA, HP, PWR, AR"),
    ("Blue Diamond", "Jewelcrafting", "Ultra Rare", "HP, PWR, AC"),
    ("Black Pearl", "Jewelcrafting", "Uncommon", "DEX, CHA"),
    ("Fire Opal", "Jewelcrafting", "Uncommon", "AGI, WIS"),
    ("Jacinth", "Jewelcrafting", "Uncommon", "STR, INT"),
    ("Jade", "Jewelcrafting", "Uncommon", "HP, PWR MINOR"),
    ("Opal", "Jewelcrafting", "Uncommon", "STA, CHA"),
    ("Pearl", "Jewelcrafting", "Uncommon", "PR, DR"),
    ("Peridot", "Jewelcrafting", "Uncommon", "FR, CR"),
    ("Topaz", "Jewelcrafting", "Uncommon", "LR, AR"),
];

pub(crate) fn lookup(name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    GEMS.iter().find(|g| g.0 == name).map(|g| (g.1, g.2, g.3))
}

fn name_arg(lua: &Lua, symbol: &str, v: &Value) -> std::result::Result<String, MultiValue> {
    match v {
        Value::String(s) => Ok(s.to_string_lossy()),
        _ => Err(error(lua, symbol, "invalid_argument", "name must be a string")
            .unwrap_or_default()),
    }
}

/// `eqoa.gems.format`'s text: the formatter's (or `name x amount`), then the gem's facts.
pub(crate) fn format_text(name: &str, amount: u64, first: Option<String>) -> String {
    let mut text = first.unwrap_or_else(|| format!("{name} x{amount}"));
    if let Some((kind, rarity, stat)) = lookup(name) {
        text.push_str(&format!(" ({kind}, {rarity}, {stat})"));
    }
    text
}

pub(crate) fn install(lua: &Lua) -> Result<()> {
    bind(
        lua,
        "gems",
        "get",
        lua.create_function(|lua, name: Value| {
            let name = match name_arg(lua, "eqoa.gems.get", &name) {
                Ok(n) => n,
                Err(e) => return Ok(e),
            };
            match lookup(&name) {
                Some((kind, rarity, stat)) => lua.pack_multi((
                    record(lua, "eqoa.gems.Gem", &json!({"type":kind,"rarity":rarity,"stat":stat}))?,
                    Value::Nil,
                )),
                None => lua.pack_multi((Value::Nil, Value::Nil)),
            }
        })?,
    )?;
    bind(
        lua,
        "gems",
        "is_gem",
        lua.create_function(|lua, name: Value| {
            let name = match name_arg(lua, "eqoa.gems.is_gem", &name) {
                Ok(n) => n,
                Err(e) => return Ok(e),
            };
            lua.pack_multi((lookup(&name).is_some(), Value::Nil))
        })?,
    )?;
    bind(
        lua,
        "gems",
        "format",
        lua.create_function(|lua, (name, amount, formatter): (Value, Value, Option<Function>)| {
            let name = match name_arg(lua, "eqoa.gems.format", &name) {
                Ok(n) => n,
                Err(e) => return Ok(e),
            };
            let Some(amount) = uint(&amount, u32::MAX as u64) else {
                return error(lua, "eqoa.gems.format", "invalid_argument", "amount must be a nonnegative int");
            };
            let first = match formatter {
                Some(f) => match f.call::<String>((name.as_str(), amount)) {
                    Ok(text) => Some(text),
                    Err(e) => {
                        return error(
                            lua,
                            "eqoa.gems.format",
                            "failed",
                            &format!("the formatter failed: {e}"),
                        );
                    }
                },
                None => None,
            };
            lua.pack_multi((format_text(&name, amount, first), Value::Nil))
        })?,
    )?;
    Ok(())
}
