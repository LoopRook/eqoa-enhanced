// Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
//! The Forge item modules (`frontiers_forge.inventory`, `.item`, `.bank`, `.loot`, `.merchant`,
//! `.gems`) over the same core as `eqoa.inventory` and friends. Forge's shapes and quirks are kept
//! here: its 1-based positions, the tab enum 0..3, the dense `bank.GetItems`, the item objects with
//! live methods and properties (a handle in `.record` stands in for the record's memory offset), the
//! `SLOT`/`STAT`/`ATTACK` names. What Forge reads from a window's private memory (the item page's
//! tab and selection, the merchant window's mode and selection) comes from the two read-only
//! accessors the owner approved (`ItemMenu::read_state`, `Merchant::read_state`) through
//! `Container.view` and `MerchantState`; only the button hint, which needs `windows.format_hint`,
//! says it is not built (`DEFERRED`) and returns nothing.
//! Forge's transactions (buy, sell, loot, bank moves) are not built in this round.
use super::{host::Host, native::error};
use mlua::{Function, Lua, MultiValue, Result, Table, Value};

/// Forge functions whose text is not built yet (the button hint is `windows.format_hint`'s).
pub(super) const DEFERRED: &[&str] = &["frontiers_forge.merchant.GetButtonHint"];

const ITEM: &str = r#"
local eqoa, unsupported = ...
local Item = {}

Item.SLOT_NONE = 0xFFFFFFFF
Item.SLOT = {
    NONE = Item.SLOT_NONE, HEAD = 1, ROBE = 2, EAR = 3, NECK = 4, TORSO = 5, FOREARM = 6, TWO_FOREARM = 7,
    RING = 8, WAIST = 9, LEGS = 10, FEET = 11, PRIMARY = 12, SHIELD = 13, SECONDARY = 14, TWO_HAND = 15,
    BOW = 16, THROWN = 17, HELD = 18, GLOVES = 19, FISHING = 20, BAIT = 21, WEAPON_CRAFT = 22,
    ARMOR_CRAFT = 23, TAILORING = 24, JEWEL_CRAFT = 25, CARPENTRY = 26, ALCHEMY = 27,
}
local SLOT_NAME = {}
for name, id in pairs(Item.SLOT) do SLOT_NAME[id] = name end
Item.STAT = {
    STR = 0, STA = 1, AGI = 2, DEX = 3, WIS = 4, INT = 5, CHA = 6, HP = 8, POW = 10, HOT = 12, POT = 13,
    AC = 14, POISON_RESIST = 22, DISEASE_RESIST = 23, FIRE_RESIST = 24, COLD_RESIST = 25,
    LIGHTNING_RESIST = 26, ARCANE_RESIST = 27, FISH = 28,
}
local STAT_NAME = {}
for name, id in pairs(Item.STAT) do STAT_NAME[id] = name end
Item.ATTACK = {
    NONE = 0, ONE_HAND_SLASH = 1, TWO_HAND_SLASH = 2, ONE_HAND_BLUNT = 3, TWO_HAND_BLUNT = 4,
    ONE_HAND_PIERCE = 5, TWO_HAND_PIERCE = 6, BOW = 7, ONE_HAND_CROSSBOW = 8, TWO_HAND_CROSSBOW = 9,
    THROWN = 10,
}
local ATTACK_NAME = {}
for name, id in pairs(Item.ATTACK) do ATTACK_NAME[id] = name end

-- a record is an item object (its .record) or the handle itself
local function handle(record)
    if type(record) == "table" then return record.record end
    return record
end
local function fetch(record)
    local ref = handle(record)
    if ref == nil then return nil end
    local ok, item = pcall(eqoa.items.get, ref)
    if ok then return item end
    return nil
end
local function field(record, key, default)
    local item = fetch(record)
    if item == nil then return default end
    local v = item[key]
    if v == nil then return default end
    return v
end

function Item.GetSlotId(record) return field(record, "wear_slot", 0) end
function Item.GetName(record) return field(record, "name", "") end
function Item.GetDescription(record) return field(record, "description", "") end
function Item.GetAmount(record) return field(record, "quantity", 0) end
function Item.GetLevelReq(record) return field(record, "level_required", 0) end
function Item.GetRange(record) return field(record, "range", 0) end
function Item.GetMaxStack(record) return field(record, "max_stack", 0) end
function Item.GetMaxHp(record) return field(record, "maximum_hp", 0) end
function Item.GetDur(record) return field(record, "durability", 0) end
function Item.GetCurrentHp(record) return field(record, "current_hp", 0) end
function Item.GetItemId(record) return tonumber(field(record, "id", "0")) or 0 end
function Item.GetValue(record) return field(record, "value", 0) end
function Item.GetDamage(record) return field(record, "damage", 0) end
function Item.GetAttackType(record) return field(record, "attack_type", 0) end
function Item.GetAttackTypeName(attack_type) return ATTACK_NAME[attack_type] end
function Item.GetSlotName(slot_id) return SLOT_NAME[slot_id] end
function Item.IsEquippable(record) return field(record, "equippable", false) end
function Item.IsNoTrade(record) return field(record, "no_trade", false) end
function Item.IsLore(record) return field(record, "lore", false) end
function Item.IsNoRent(record) return field(record, "no_rent", false) end
function Item.GetClassMask(record) return field(record, "class_mask", 0) end
function Item.GetRaceMask(record) return field(record, "race_mask", 0) end
function Item.GetInventoryIndex(record) return field(record, "inventory_key", 0) end
function Item.GetIconRef(record) return field(record, "icon", 0) end

function Item.GetStatCount(record)
    local stats = field(record, "stats", nil)
    return stats and #stats or 0
end
function Item.GetStats(record)
    local out = {}
    for _, s in ipairs(field(record, "stats", {})) do
        out[#out + 1] = { id = s.id, name = STAT_NAME[s.id], value = s.value }
    end
    return out
end
function Item.GetStat(record, stat_id)
    for _, stat in ipairs(Item.GetStats(record)) do
        if stat.id == stat_id then return stat.value end
    end
    return 0
end
-- the paperdoll's own (0-based) index, nil when the item is not worn
function Item.GetEquippedSlotIndex(record)
    local slot = field(record, "equipped_slot", nil)
    if slot == nil then return nil end
    return slot - 1
end
function Item.GetEquippedStatus(record)
    local item = fetch(record)
    if item == nil or item.wear_slot == 0 then return "Empty" end
    if item.equipped then return "Equipped" end
    return "Not Equipped"
end

local function in_mask(mask, id) return math.floor(mask / 2 ^ id) % 2 == 1 end
function Item.IsUsableByClass(record, class_id)
    if class_id == nil or class_id < 0 or class_id > 14 then return false end
    return in_mask(Item.GetClassMask(record), class_id)
end
function Item.IsUsableByRace(record, race_id)
    if race_id == nil or race_id < 0 or race_id > 9 then return false end
    return in_mask(Item.GetRaceMask(record), race_id)
end
function Item.GetAllowedClasses(record)
    local ids, mask = {}, Item.GetClassMask(record)
    for id = 0, 14 do if in_mask(mask, id) then ids[#ids + 1] = id end end
    return ids
end
function Item.GetAllowedRaces(record)
    local ids, mask = {}, Item.GetRaceMask(record)
    for id = 0, 9 do if in_mask(mask, id) then ids[#ids + 1] = id end end
    return ids
end
function Item.GetInspectInfo(record)
    local slot = Item.GetSlotId(record)
    local attack = Item.GetAttackType(record)
    return {
        item_id = Item.GetItemId(record), name = Item.GetName(record), description = Item.GetDescription(record),
        icon_ref = Item.GetIconRef(record), amount = Item.GetAmount(record), max_stack = Item.GetMaxStack(record),
        value = Item.GetValue(record), slot = slot, slot_name = SLOT_NAME[slot], equippable = Item.IsEquippable(record),
        level_req = Item.GetLevelReq(record), current_hp = Item.GetCurrentHp(record), max_hp = Item.GetMaxHp(record),
        dur = Item.GetDur(record), damage = Item.GetDamage(record), attack_type = attack,
        attack_type_name = ATTACK_NAME[attack], range = Item.GetRange(record), stats = Item.GetStats(record),
        no_trade = Item.IsNoTrade(record), no_rent = Item.IsNoRent(record), lore = Item.IsLore(record),
        class_mask = Item.GetClassMask(record), race_mask = Item.GetRaceMask(record),
        allowed_classes = Item.GetAllowedClasses(record), allowed_races = Item.GetAllowedRaces(record),
        equipped_status = Item.GetEquippedStatus(record), equipped_slot = Item.GetEquippedSlotIndex(record),
        inventory_index = Item.GetInventoryIndex(record),
    }
end

local methods = {}
for _, name in ipairs({
    "GetSlotId", "GetName", "GetDescription", "GetAmount", "GetLevelReq", "GetRange", "GetMaxStack", "GetMaxHp",
    "GetDur", "GetIconRef", "GetEquippedStatus", "GetClassMask", "GetRaceMask", "GetAllowedClasses",
    "GetAllowedRaces", "GetItemId", "GetValue", "GetCurrentHp", "GetDamage", "GetAttackType", "IsEquippable",
    "IsNoTrade", "IsLore", "IsNoRent", "GetStatCount", "GetStats", "GetInventoryIndex", "GetEquippedSlotIndex",
    "GetInspectInfo",
}) do
    local f = Item[name]
    methods[name] = function(self) return f(self.record) end
end
function methods:GetSlotName() return Item.GetSlotName(Item.GetSlotId(self.record)) end
function methods:GetStat(stat_id) return Item.GetStat(self.record, stat_id) end
function methods:IsUsableByClass(class_id) return Item.IsUsableByClass(self.record, class_id) end
function methods:IsUsableByRace(race_id) return Item.IsUsableByRace(self.record, race_id) end
-- as item.lua:539: the record has an item (slot id not 0) and still the name this object was made with; a handle that no longer resolves reads as an empty record
function methods:IsValid()
    return fetch(self.record) ~= nil and Item.GetSlotId(self.record) ~= 0 and Item.GetName(self.record) == self._name
end

local properties = {
    slot = Item.GetSlotId, name = Item.GetName, description = Item.GetDescription, amount = Item.GetAmount,
    level_req = Item.GetLevelReq, range = Item.GetRange, max_stack = Item.GetMaxStack, max_hp = Item.GetMaxHp,
    dur = Item.GetDur, icon_ref = Item.GetIconRef, equipped_status = Item.GetEquippedStatus,
    class_mask = Item.GetClassMask, race_mask = Item.GetRaceMask, allowed_classes = Item.GetAllowedClasses,
    allowed_races = Item.GetAllowedRaces, item_id = Item.GetItemId, value = Item.GetValue,
    current_hp = Item.GetCurrentHp, damage = Item.GetDamage, attack_type = Item.GetAttackType,
    stat_count = Item.GetStatCount, stats = Item.GetStats, no_trade = Item.IsNoTrade, lore = Item.IsLore,
    no_rent = Item.IsNoRent, equippable = Item.IsEquippable, inventory_index = Item.GetInventoryIndex,
    equipped_slot = Item.GetEquippedSlotIndex,
    slot_name = function(record) return Item.GetSlotName(Item.GetSlotId(record)) end,
    equipped = function(record) return Item.GetEquippedStatus(record) == "Equipped" end,
}
local item_mt = {
    __index = function(self, key)
        local method = methods[key]
        if method ~= nil then return method end
        local property = properties[key]
        if property ~= nil then return property(rawget(self, "record")) end
        return nil
    end,
}
-- an object over an ItemRef (the record offset of Forge's memory reads)
function Item.new(record)
    return setmetatable({ record = record, _name = Item.GetName(record) }, item_mt)
end
return Item
"#;

const INVENTORY: &str = r#"
local eqoa, unsupported, Item = ...
local Inventory = {}
Inventory.max_slots = 40

local SLOT_LABEL = {
    [1] = "Head", [2] = "Robe", [3] = "Earring", [4] = "Neck", [5] = "Torso", [6] = "Bracelet",
    [7] = "2 Forearm", [8] = "Ring", [9] = "Waist", [10] = "Legs", [11] = "Boots", [12] = "Weapon",
    [13] = "Shield", [14] = "Weapon", [15] = "Weapon", [16] = "Weapon", [17] = "Weapon", [18] = "Weapon",
    [19] = "Gloves",
}
local ORDER = {
    Weapon = 1, Shield = 2, Head = 3, Robe = 4, Torso = 5, Waist = 6, Gloves = 7, ["2 Forearm"] = 8,
    Bracelet = 9, Legs = 10, Boots = 11, Neck = 12, Earring = 13, Ring = 14,
}
function Inventory.GetSlotLabelAndOrder(slot_id)
    local base = SLOT_LABEL[slot_id]
    if not base then return ("Slot " .. tostring(slot_id)), 9999 end
    return base, ORDER[base] or 999
end

local function container()
    local c = eqoa.inventory.get()
    return c
end
function Inventory.GetTunar()
    local c = container()
    return c and c.money or 0
end
function Inventory.InventoryUsed()
    local c = container()
    return c and c.used or 0
end
function Inventory.SlotsUsed() return Inventory.InventoryUsed() end
function Inventory.SlotsRemaining()
    local used = Inventory.SlotsUsed()
    if used < 0 then used = 0 end
    if used > 40 then used = 40 end
    return 40 - used
end
function Inventory.IsFull() return Inventory.SlotsRemaining() == 0 end
function Inventory.GetIconRef(slot)
    if slot < 1 or slot > 40 then return nil end
    local c = container()
    local item = c and c.items[slot]
    if item == nil then return nil end
    return item.icon or 0
end
function Inventory.GetItem(slot)
    if slot < 1 or slot > 40 then return nil end
    local c = container()
    local item = c and c.items[slot]
    if item == nil or item.name == "" then return nil end
    local object = Item.new(item.ref)
    object.idx = slot
    return object
end
function Inventory.GetItems()
    local items = {}
    local used = Inventory.SlotsUsed()
    if used > 40 then used = 40 end
    for i = 1, used do
        local item = Inventory.GetItem(i)
        if item ~= nil then items[#items + 1] = item end
    end
    return items
end

-- the item page's tabs (the enum is Forge's 0..3; labels are the game's own words)
Inventory.Tab = { magic = 0, equipment = 1, tradeskill = 2, misc = 3 }
local TAB_STRING_ID = { [0] = 0x1B5, [1] = 0x1B6, [2] = 0x5C1, [3] = 0x1B7 }
function Inventory.GetTabLabel(tab)
    local id = TAB_STRING_ID[tab]
    if id == nil then return nil end
    return (eqoa.text.localized(id))
end
-- the item page: open while the start menu shows it; the tab is Forge's 0..3
local TAB_KEYS = { [0] = "usable", [1] = "equipment", [2] = "charged", [3] = "misc" }
local function page()
    local c = container()
    return c and c.view or nil
end
function Inventory.IsWindowOpen() return page() ~= nil end
function Inventory.GetOpenTab()
    local view = page()
    return view and view.tab - 1 or nil
end
function Inventory.GetTabs()
    local view = page()
    if view == nil then return nil end
    local tabs = {}
    for i, tab in ipairs(view.tabs) do tabs[i] = { tab = tab.index - 1, label = tab.label } end
    return tabs
end
-- the stacks under a tab (the open one when none is named), in the page's order, as item objects
function Inventory.GetTabList(tab)
    local c = container()
    if c == nil then return nil end
    if tab == nil then
        if c.view == nil then return nil end
        tab = c.view.tab - 1
    end
    local key = TAB_KEYS[tab]
    if key == nil then return nil end
    local list = {}
    for _, item in ipairs(c.items) do
        for _, k in ipairs(item.tabs) do
            if k == key then
                local object = Item.new(item.ref)
                object.idx = item.index
                list[#list + 1] = object
                break
            end
        end
    end
    return list
end
-- the lit row's stack: its position in the pack (not its row in the tab)
function Inventory.GetSelectedSlot()
    local view = page()
    if view == nil or view.selected == nil then return nil end
    local item = eqoa.items.get(view.selected)
    return item and item.index or nil
end
function Inventory.GetSelectedItem()
    local slot = Inventory.GetSelectedSlot()
    return slot and Inventory.GetItem(slot) or nil
end
return Inventory
"#;

const BANK: &str = r#"
local eqoa = ...
local Bank = {}
function Bank.SlotsUsed()
    local c = eqoa.bank.get()
    return c and c.used or 0
end
function Bank.GetItems()
    local items = {}
    local c = eqoa.bank.get()
    if c == nil then return items end
    for i, item in ipairs(c.items) do
        if item.name ~= "" then
            items[#items + 1] = {
                idx = i, slot = item.wear_slot, level = item.level_required, name = item.name, amount = item.quantity,
            }
        end
    end
    return items
end
return Bank
"#;

const LOOT: &str = r#"
local eqoa, unsupported, Item = ...
local Loot = {}
Loot.max_slots = 40
function Loot.IsLootWindowOpen() return eqoa.loot.get() ~= nil end
function Loot.GetCount()
    local c = eqoa.loot.get()
    return c and math.min(c.used, 40) or 0
end
function Loot.GetIconRef(index)
    if index < 1 or index > 40 then return nil end
    local c = eqoa.loot.get()
    local item = c and c.items[index]
    if item == nil then return nil end
    return item.icon or 0
end
function Loot.GetItem(index)
    if index < 1 or index > 40 then return nil end
    local c = eqoa.loot.get()
    local item = c and c.items[index]
    if item == nil or item.name == "" then return nil end
    local object = Item.new(item.ref)
    object.idx = index
    return object
end
function Loot.GetItems()
    local items = {}
    for i = 1, Loot.GetCount() do
        local item = Loot.GetItem(i)
        if item ~= nil then items[#items + 1] = item end
    end
    return items
end
function Loot.Items()
    local items = Loot.GetItems()
    local i = 0
    return function()
        i = i + 1
        local item = items[i]
        if item == nil then return nil end
        return item.idx, item
    end
end
return Loot
"#;

const MERCHANT: &str = r#"
local eqoa, unsupported, Item = ...
local Merchant = {}
Merchant.max_slots = 40
Merchant.Mode = { menu = 0, sell = 1, buy = 2 }
Merchant.Option = { buy = 0, sell = 1, exit = 2 }
local OPTION_STRING_ID = { [0] = 0x3F, [1] = 0x3E, [2] = 0xA1 }

function Merchant.IsOpen() return eqoa.merchant.get() ~= nil end
function Merchant.GetEntityId()
    local c = eqoa.merchant.get()
    return c and c.merchant.entity_id or nil
end
function Merchant.GetGreeting()
    local c = eqoa.merchant.get()
    return c and c.merchant.greeting or nil
end
function Merchant.GetOptionLabel(option)
    local id = OPTION_STRING_ID[option]
    if id == nil then return nil end
    return (eqoa.text.localized(id))
end
function Merchant.GetItemCount()
    local c = eqoa.merchant.get()
    return c and math.min(c.used, 40) or 0
end
function Merchant.GetItem(index)
    if index < 1 or index > Merchant.GetItemCount() then return nil end
    local object = Item.new(eqoa.merchant.get().items[index].ref)
    object.idx = index
    return object
end
function Merchant.GetItems()
    local items = {}
    for i = 1, Merchant.GetItemCount() do
        local item = Merchant.GetItem(i)
        if item ~= nil then items[#items + 1] = item end
    end
    return items
end
local function window()
    local c = eqoa.merchant.get()
    return c, c and c.merchant or nil
end
function Merchant.GetMode()
    local _, state = window()
    return state and Merchant.Mode[state.mode] or nil
end
function Merchant.GetSelectedOption()
    local _, state = window()
    return state and state.selected_option and Merchant.Option[state.selected_option] or nil
end
function Merchant.GetOptions()
    local _, state = window()
    if state == nil then return nil end
    local options = {}
    for i, option in ipairs(state.options) do
        options[i] = { option = Merchant.Option[option.key], label = option.label, selected = option.selected }
    end
    return options
end
-- the lit row, 1-based: a ware when buying, a pack position when selling
function Merchant.GetSelectedItemIndex()
    local c = eqoa.merchant.get()
    return c and c.view and c.view.selected_row or nil
end
function Merchant.GetSelectedItem()
    local c = eqoa.merchant.get()
    if c == nil or c.view == nil or c.view.selected == nil then return nil end
    local object = Item.new(c.view.selected)
    object.idx = c.view.selected_row
    return object
end
Merchant.GetButtonHint = function() return unsupported("frontiers_forge.merchant.GetButtonHint") end
return Merchant
"#;

const GEMS: &str = r#"
local eqoa = ...
local Gems = {}
function Gems.IsGem(name) return name and eqoa.gems.is_gem(name) end
function Gems.Get(name)
    if not name then return nil end
    return (eqoa.gems.get(name))
end
-- "Name (amt) - Stat - Rarity"; the reference called a Get that does not exist (Gems.get): this one calls the real one
function Gems.Format(name, amount, fmt_name_amt_fn)
    local g = Gems.Get(name)
    if not g then return nil end
    local display_name
    if fmt_name_amt_fn then
        display_name = fmt_name_amt_fn(name, amount)
    else
        display_name = name
        if amount and amount > 1 then display_name = ("%s (%d)"):format(name, amount) end
    end
    return ("%s - %s - %s"):format(display_name, g.stat or "", g.rarity or "")
end
return Gems
"#;

/// The functions each module's table carries (the real module is built by the Lua source above on the
/// first call or the first read of anything else, inside the calling script's own instruction budget:
/// building them while the host is made would use up the budget a test or a loader still has).
const NAMES: [(&str, &str); 6] = [
    (
        "item",
        "GetSlotId GetName GetDescription GetAmount GetLevelReq GetRange GetMaxStack GetMaxHp GetDur GetCurrentHp GetItemId GetValue GetDamage GetAttackType GetAttackTypeName GetSlotName IsEquippable IsNoTrade IsLore IsNoRent GetClassMask GetRaceMask GetInventoryIndex GetIconRef GetStatCount GetStats GetStat GetEquippedSlotIndex GetEquippedStatus IsUsableByClass IsUsableByRace GetAllowedClasses GetAllowedRaces GetInspectInfo new",
    ),
    (
        "inventory",
        "GetSlotLabelAndOrder GetTunar InventoryUsed SlotsUsed SlotsRemaining IsFull GetIconRef GetItem GetItems GetTabLabel IsWindowOpen GetOpenTab GetTabs GetTabList GetSelectedSlot GetSelectedItem",
    ),
    ("bank", "SlotsUsed GetItems"),
    ("loot", "IsLootWindowOpen GetCount GetIconRef GetItem GetItems Items"),
    (
        "merchant",
        "IsOpen GetEntityId GetGreeting GetOptionLabel GetItemCount GetItem GetItems GetMode GetSelectedOption GetOptions GetButtonHint GetSelectedItemIndex GetSelectedItem",
    ),
    ("gems", "IsGem Get Format"),
];

fn source(short: &str) -> &'static str {
    match short {
        "item" => ITEM,
        "inventory" => INVENTORY,
        "bank" => BANK,
        "loot" => LOOT,
        "merchant" => MERCHANT,
        _ => GEMS,
    }
}

/// The built module `short` (built once per VM and kept in the registry).
fn real(lua: &Lua, short: &str) -> Result<Table> {
    let key = format!("eqoa_forge_{short}");
    if let Ok(t) = lua.named_registry_value::<Table>(&key) {
        return Ok(t);
    }
    let eqoa: Value = lua.globals().get("eqoa")?;
    let unsupported: Function = lua.named_registry_value("eqoa_forge_unsupported")?;
    let mut args = vec![eqoa, Value::Function(unsupported)];
    if matches!(short, "inventory" | "loot" | "merchant") {
        args.push(Value::Table(real(lua, "item")?));
    }
    let module: Table = lua
        .load(source(short))
        .set_name(format!("=frontiers_forge.{short}"))
        .call(mlua::MultiValue::from_iter(args))?;
    lua.set_named_registry_value(&key, module.clone())?;
    Ok(module)
}

pub(super) fn install(host: &Host) -> Result<()> {
    let lua = &host.lua;
    let unsupported = lua.create_function(|lua, symbol: String| {
        error(
            lua,
            &symbol,
            "not_yet",
            "needs the item or merchant window's private selection state; see the round 5 report",
        )?;
        Ok(Value::Nil)
    })?;
    lua.set_named_registry_value("eqoa_forge_unsupported", unsupported)?;
    for (short, names) in NAMES {
        let proxy = lua.create_table()?;
        for name in names.split_whitespace() {
            proxy.set(
                name,
                lua.create_function(move |lua, args: MultiValue| {
                    let f: Function = real(lua, short)?.get(name)?;
                    f.call::<MultiValue>(args)
                })?,
            )?;
        }
        // everything else (the constants, `max_slots`) reads from the built module
        let meta = lua.create_table()?;
        meta.set(
            "__index",
            lua.create_function(move |lua, (_, key): (Table, Value)| {
                real(lua, short)?.get::<Value>(key)
            })?,
        )?;
        proxy.set_metatable(Some(meta));
        host.register_host_module(format!("frontiers_forge.{short}"), Value::Table(proxy))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each function the built modules carry is on the proxy, so none is missing from `NAMES`.
    #[test]
    fn every_function_of_each_forge_item_module_is_listed() {
        let host = Host::new().unwrap();
        for (short, names) in NAMES {
            let listed: std::collections::BTreeSet<&str> = names.split_whitespace().collect();
            let built = real(&host.lua, short).unwrap();
            for pair in built.pairs::<String, Value>() {
                let (key, value) = pair.unwrap();
                if matches!(value, Value::Function(_)) {
                    assert!(listed.contains(key.as_str()), "{short}.{key} is not in NAMES");
                }
            }
            for name in &listed {
                assert!(built.contains_key(*name).unwrap(), "{short}.{name} listed but not built");
            }
        }
    }
}
