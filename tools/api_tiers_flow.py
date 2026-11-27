#!/usr/bin/env python3
"""Provenance analysis for tools/api_tiers.py: which record fields and event data fields does a bundled mod
really read, from a value it really received?

A read `x.field` counts for record R only when `x` is bound to an R in that scope:

  * a local or loop variable assigned from a call to a function that returns R (RETURNS), or from a field of
    an R that the mod received (the field types are read from the Rust, see load_nested);
  * a handler's event parameter (`e.data.field`, `e.data.after.field`, or a local alias of those);
  * a function parameter that every call site in the same mod fills with such a value (found by repeating
    the pass until nothing changes), when the function is defined once in the mod and never used as a plain
    value (a callback this scan cannot follow).

Anything the scan cannot bind stays unproven. Path segments of an `eqoa.<module>.<fn>` chain are never
field reads. Evidence for a name comes back with the file and line, so `tools/api_tiers.py --why` can show it.
"""
import re
from pathlib import Path

import api_tiers as T

ROOT = Path(__file__).resolve().parent.parent
RUST_DIR = ROOT / "crates/eqoa/src/fork_ext/addons"

# ---------------------------------------------------------------------------------------------------------
# What a function returns. The spec's function tables are prose, and the Rust builds most records through
# helpers that take the path as a runtime string, so this table is kept by hand. A test requires that every
# name in it exists in the frozen record, and that every record type of the frozen record is either listed
# here (as a producer's result), reachable through a field type read from the Rust, an event payload (below),
# or named in NO_PRODUCER with a reason. A record nothing in this scan can produce stays provisional.

RETURNS = {
    "eqoa.client.get": "eqoa.client.Client",
    "eqoa.client.display": "eqoa.client.DisplayMetrics",
    "eqoa.player.get": "eqoa.player.Player",
    "eqoa.player.stats": "eqoa.player.Stats",
    "eqoa.player.training": "eqoa.player.Training",
    "eqoa.player.breath": "eqoa.player.Breath",
    "eqoa.entities.list": "eqoa.entities.Entity",
    "eqoa.entities.get": "eqoa.entities.Entity",
    "eqoa.entities.find": "eqoa.entities.Entity",
    "eqoa.entities.named": "eqoa.entities.Entity",
    "eqoa.target.get": "eqoa.entities.Entity",
    "eqoa.target.info": "eqoa.target.TargetInfo",
    "eqoa.abilities.list": "eqoa.abilities.Ability",
    "eqoa.abilities.get": "eqoa.abilities.Ability",
    "eqoa.abilities.find": "eqoa.abilities.Ability",
    "eqoa.abilities.in_range": "eqoa.abilities.RangeResult",
    "eqoa.hotbar.list": "eqoa.hotbar.Hotbar",
    "eqoa.cooldowns.get": "eqoa.cooldowns.Cooldown",
    "eqoa.effects.list": "eqoa.effects.Effect",
    "eqoa.effects.buffs": "eqoa.effects.Buff",
    "eqoa.effects.pet": "eqoa.effects.PetEffect",
    "eqoa.casts.list": "eqoa.casts.Cast",
    "eqoa.casts.get": "eqoa.casts.Cast",
    "eqoa.quests.goals": "eqoa.quests.QuestGoals",
    "eqoa.quests.list": "eqoa.quests.Quest",
    "eqoa.quests.get": "eqoa.quests.Quest",
    "eqoa.group.get": "eqoa.group.Group",
    "eqoa.group.raid_roles": "eqoa.group.RaidRole",
    "eqoa.combat.record": "eqoa.combat.CombatRecord",
    "eqoa.combat.get_counters": "eqoa.combat.Counters",
    "eqoa.chat.history": "eqoa.chat.ChatMessage",
    "eqoa.chat.tabs": "eqoa.chat.ChatTabs",
    "eqoa.hud.frame": "eqoa.hud.Frame",
    "eqoa.hud.layout": "eqoa.hud.Layout",
    "eqoa.options.toggle": "eqoa.options.Toggle",
    "eqoa.options.button": "eqoa.options.Button",
    "eqoa.input.get": "eqoa.input.InputState",
    "eqoa.input.pointer": "eqoa.input.Pointer",
    "eqoa.icons.resource": "eqoa.textures.TextureInfo",
    "eqoa.icons.ui": "eqoa.textures.TextureInfo",
    "eqoa.icons.class": "eqoa.textures.TextureInfo",
    "eqoa.icons.disposition": "eqoa.textures.TextureInfo",
    "eqoa.icons.info": "eqoa.icons.SurfaceInfo",
    "eqoa.textures.load": "eqoa.textures.TextureInfo",
    "eqoa.textures.rgba": "eqoa.textures.TextureInfo",
    "eqoa.fonts.load": "eqoa.fonts.FontInfo",
    "eqoa.fonts.default": "eqoa.fonts.FontInfo",
    "eqoa.files.list": "eqoa.files.FileEntry",
    "eqoa.inventory.get": "eqoa.items.Container",
    "eqoa.bank.get": "eqoa.items.Container",
    "eqoa.loot.get": "eqoa.items.Container",
    "eqoa.merchant.get": "eqoa.items.Container",
    "eqoa.items.get": "eqoa.items.Item",
    "eqoa.items.can_use": "eqoa.items.CanUse",
    "eqoa.items.slot_info": "eqoa.items.SlotInfo",
    "eqoa.gems.get": "eqoa.gems.Gem",
    "eqoa.mastery.get": "eqoa.mastery.Mastery",
    "eqoa.mastery.skills": "eqoa.mastery.MasterySkill",
    "eqoa.mastery.selected": "eqoa.mastery.MasteryNode",
    "eqoa.windows.get": "eqoa.windows.Windows",
    "eqoa.windows.find": "eqoa.windows.Window",
    "eqoa.windows.get_window": "eqoa.windows.Window",
    "eqoa.windows.parse_hint": "eqoa.windows.ButtonHint",
    "eqoa.windows.confirm": "eqoa.windows.Confirm",
    "eqoa.windows.session": "eqoa.windows.Session",
    "eqoa.windows.open_session": "eqoa.windows.Session",
    "eqoa.raid.get": "eqoa.raid.RaidState",
    "eqoa.raid.ready_check": "eqoa.raid.ReadyCheckSession",
    "eqoa.raid.loot_rolls": "eqoa.raid.LootRollSession",
    "eqoa.social.friends": "eqoa.social.Friend",
    "eqoa.social.ignored": "eqoa.social.Ignored",
    "eqoa.social.friend_request": "eqoa.social.FriendRequest",
    "eqoa.guild.get": "eqoa.guild.Guild",
    "eqoa.guild.invite": "eqoa.guild.GuildInvite",
    "eqoa.who.get": "eqoa.who.WhoState",
    "eqoa.lfg.get": "eqoa.lfg.LfgState",
    "eqoa.map.get": "eqoa.map.MapState",
    "eqoa.map.markers": "eqoa.map.Marker",
    "eqoa.map.waypoint": "eqoa.map.Waypoint",
    "eqoa.camera.get": "eqoa.camera.Camera",
    "eqoa.camera.project": "eqoa.camera.Projection",
    "eqoa.camera.project_entity": "eqoa.camera.Projection",
    "eqoa.capabilities.get": "eqoa.capabilities.Capability",
    "eqoa.capabilities.list": "eqoa.capabilities.Capability",
    "eqoa.factions.list": "eqoa.factions.FactionPage",
    "eqoa.merc.get": "eqoa.merc.State",
    "eqoa.dye.get": "eqoa.dye.State",
    "eqoa.fishing.get": "eqoa.fishing.State",
    "eqoa.trade.get": "eqoa.trade.State",
    "eqoa.duel.get": "eqoa.duel.State",
    "eqoa.blacksmith.get": "eqoa.blacksmith.State",
    "eqoa.craft.get": "eqoa.craft.State",
    "eqoa.mail.get": "eqoa.mail.State",
    "eqoa.auction.get": "eqoa.auction.State",
    "eqoa.who.request": "eqoa.actions.RequestId",
    "eqoa.lfg.search": "eqoa.actions.RequestId",
    "eqoa.lfg.register": "eqoa.actions.RequestId",
    "eqoa.lfg.unregister": "eqoa.actions.RequestId",
    "eqoa.actions.batch": "eqoa.actions.BatchId",
    "eqoa.actions.status": "eqoa.actions.ActionResult",
    "eqoa.actions.batch_status": "eqoa.actions.BatchResult",
}
# every other eqoa.actions.<lowercase name> returns a RequestId
ACTION_RETURN = "eqoa.actions.RequestId"
ACTION_NOT_REQUEST = {"stop_batch"}
ERROR = "eqoa.Error"
EVENT_RECORD = "eqoa.events.Event"  # the handler parameter of every event

# What an event's `data` is: a record itself, or a table with before/after of a record.
EVENT_SELF = {
    "chat.message": "eqoa.chat.ChatMessage",
    "combat.hit": "eqoa.combat.CombatHit",
    "action.result": "eqoa.actions.ActionResult",
    "batch.progress": "eqoa.actions.BatchResult",
    "batch.result": "eqoa.actions.BatchResult",
}
EVENT_BEFORE_AFTER = {
    "player.changed": "eqoa.player.Player",
    "entity.added": "eqoa.entities.Entity",
    "entity.changed": "eqoa.entities.Entity",
    "entity.removed": "eqoa.entities.Entity",
    "group.changed": "eqoa.group.Group",
    "abilities.changed": "eqoa.abilities.Ability",
    "hotbar.changed": "eqoa.hotbar.Hotbar",
    "effects.changed": "eqoa.effects.Effect",
    "buffs.changed": "eqoa.effects.Buff",
    "quests.goals_changed": "eqoa.quests.QuestGoals",
    "inventory.changed": "eqoa.items.Container",
    "bank.changed": "eqoa.items.Container",
    "loot.changed": "eqoa.items.Container",
    "merchant.changed": "eqoa.items.Container",
    "guild.changed": "eqoa.guild.Guild",
    "lfg.changed": "eqoa.lfg.LfgState",
    "friends.changed": "eqoa.social.Friend",
    "ignored.changed": "eqoa.social.Ignored",
    "mastery.changed": "eqoa.mastery.Mastery",
    "camera.changed": "eqoa.camera.Camera",
    "windows.changed": "eqoa.windows.Windows",
    "map.changed": "eqoa.map.MapState",
    "trade.changed": "eqoa.trade.State",
    "duel.changed": "eqoa.duel.State",
    "blacksmith.changed": "eqoa.blacksmith.State",
}
# Field types the Rust's nested_type table does not list.
NESTED_EXTRA = {
    ("eqoa.map.MapState", "waypoint"): "eqoa.map.Waypoint",
    ("eqoa.map.MapState", "bounds"): "eqoa.map.Bounds",
    ("eqoa.windows.Session", "data"): "eqoa.windows.SessionData",
}
# Record types no function or event in this table produces: provisional, whatever a mod reads by name.
NO_PRODUCER = {
    "eqoa.ChatHistory": "only a capability name; no function returns it",
    "eqoa.Error": "typed from the second result of an API call",
    "eqoa.combat.CombatRow": "reached through CombatRecord.rows (Rust table)",
}
DRAW_LIST_GETTERS = {f"{root}.{name}" for root in ("ImGui", "eqoa.imgui") for name in T.DRAWLIST_SOURCES}
LUA_LIBS = {"table", "string", "math", "os", "io", "coroutine", "debug", "utf8"}
ROOTS = ("eqoa", "ImGui", "ImDrawList", "ImGuiCond", "ImGuiWindowFlags", "bit", "UiForge", "frontiers_forge")


def load_nested(rust_dir=RUST_DIR):
    """{(record, field): record} from the Rust's `nested_type` match arms ('_' stands for any record)."""
    out = {}
    pat = re.compile(
        r'\(\s*((?:"[^"]+"|_)(?:\s*\|\s*"[^"]+")*)\s*,\s*("[^"]+"(?:\s*\|\s*"[^"]+")*)\s*,?\s*\)\s*=>\s*Some\("([^"]+)"\)',
        re.S)
    found = 0
    for name in ("native.rs", "native_a11.rs"):
        text = (Path(rust_dir) / name).read_text(encoding="utf-8", errors="replace")
        start = text.index("fn nested_type")
        brace = text.index("{", start)
        depth, end = 0, brace
        while end < len(text):
            if text[end] == "{":
                depth += 1
            elif text[end] == "}":
                depth -= 1
                if depth == 0:
                    break
            end += 1
        body = text[start:end + 1]
        for m in pat.finditer(body):
            paths = [a or b for a, b in re.findall(r'"([^"]+)"|(_)', m.group(1))]
            keys = re.findall(r'"([^"]+)"', m.group(2))
            for p in paths:
                for k in keys:
                    out[(p, k)] = m.group(3)
                    found += 1
    if found < 100:
        raise RuntimeError(f"nested_type: parsed only {found} field types from the Rust; the pattern is stale")
    out.update(NESTED_EXTRA)
    return out


def event_data_record(event):
    return EVENT_SELF.get(event)


# ---------------------------------------------------------------------------------------------------------
# Values: None (unknown) | ("t", Record, why) | ("p", path tuple) | ("ev", event, why) | ("ed", event, why)
#         | ("dl", why) a draw list | ("mod", stem) the result of require("stem")

BINARY = {"and", "+", "-", "*", "/", "%", "^", "..", "==", "~=", "<", ">", "<=", ">=", "//", "<<", ">>"}


def group_end(toks, i):
    """Index of the bracket closing the one at toks[i]."""
    pairs = {"(": ")", "{": "}", "[": "]"}
    open_, close = toks[i][1], pairs[toks[i][1]]
    depth = 0
    for j in range(i, len(toks)):
        if toks[j][0] == "op":
            if toks[j][1] == open_:
                depth += 1
            elif toks[j][1] == close:
                depth -= 1
                if depth == 0:
                    return j
    return len(toks) - 1


def split_args(toks, open_index):
    """Start indexes of the top-level arguments of the call whose `(` is at open_index."""
    end = group_end(toks, open_index)
    starts, depth, expect = [], 0, True
    for j in range(open_index + 1, end):
        t = toks[j]
        if expect:
            starts.append(j)
            expect = False
        if t[0] == "op" and t[1] in "({[":
            depth += 1
        elif t[0] == "op" and t[1] in ")}]":
            depth -= 1
        elif t[0] == "name" and t[1] == "function":
            pass
        elif t[0] == "op" and t[1] == "," and depth == 0:
            expect = True
    return starts, end


class Flow:
    """One pass of the analysis over every file of one mod."""

    def __init__(self, files, frozen, nested, wrappers, stems, param_types, defs, escaped, ret_types=None):
        self.files = files                  # [(path, tokens)]
        self.frozen = frozen
        self.nested = nested
        self.wrappers = wrappers            # {path: {chain text or last segment: name parameter index}}
        self.stems = stems                  # {module stem: path} (only stems that name exactly one file)
        self.param_types = param_types      # {(file, function name, index): value}
        self.defs = defs                    # {function name: {file: number of definitions there}}
        self.escaped = set(escaped)         # names used as plain values somewhere
        self.ret_types = ret_types or {}    # {(file, function name): value}: what a local function returns
        self.ret_obs = {}                   # (file, function name) -> [value or None] of each `return`
        self.reads = {}                     # ("rec", R, f) | ("ev", E, f) | ("dl", m) -> evidence
        self.received = {}                  # record -> evidence
        self.events = {}                    # event name -> evidence, subscribed through a resolved wrapper
        self.obs = {}                       # (file, function name, index) -> [value or None]
        self.where = ""

    # -- helpers ---------------------------------------------------------------------------------------
    def note(self, line):
        return f"{self.where}:{line}"

    def receive(self, record, why, line):
        self.received.setdefault(record, f"{self.note(line)}: {why}")

    def lookup(self, name):
        for scope in reversed(self.scopes):
            if name in scope:
                return True, scope[name]
        return False, None

    def assign(self, name, value):
        for scope in reversed(self.scopes):
            if name in scope:
                scope[name] = value
                return
        self.scopes[0][name] = value

    # -- expressions -----------------------------------------------------------------------------------
    def chain(self, i):
        """Evaluate the chain starting at toks[i]. Returns (value, index after the chain, info)."""
        toks = self.toks
        n = len(toks)
        info = {"api": False, "name": None, "seg_count": 0, "called": False, "head": None}
        t = toks[i]
        value = None
        j = i + 1
        if t[0] == "name" and t[1] not in T.KEYWORDS:
            found, value = self.lookup(t[1])
            if not found:
                value = ("p", (t[1],)) if t[1] in ROOTS else None
            info["name"] = t[1]
            info["head"] = t[1]
        elif t[0] == "op" and t[1] in "({":
            j = group_end(toks, i) + 1
            value = None
        else:
            return None, i + 1, info
        while j < n:
            u = toks[j]
            if u[0] == "op" and u[1] == "." and j + 1 < n and toks[j + 1][0] == "name":
                value = self.field(value, toks[j + 1][1], toks[j + 1][2])
                info.update(name=toks[j + 1][1], seg_count=info["seg_count"] + 1, called=False, api=False)
                j += 2
            elif u[0] == "op" and u[1] == "[":
                end = group_end(toks, j)
                if end == j + 2 and toks[j + 1][0] == "str":
                    value = self.field(value, toks[j + 1][1], u[2])
                    info["name"] = toks[j + 1][1]
                else:
                    value = self.index(value)
                    info["name"] = None
                info.update(seg_count=info["seg_count"] + 1, called=False, api=False)
                j = end + 1
            elif u[0] == "op" and u[1] == ":" and j + 1 < n and toks[j + 1][0] == "name":
                method = toks[j + 1][1]
                k = j + 2
                result = None
                if k < n and (toks[k][1] == "(" and toks[k][0] == "op"):
                    self.method_call(value, method, k, toks[j + 1][2])
                    j = group_end(toks, k) + 1
                    info.update(name=method, seg_count=info["seg_count"] + 1, called=True, api=False, head=None)
                    result = self.local_return(("dotted",), info, toks[j - 1][2])
                else:
                    j = k
                value = result
                info.update(name=method, seg_count=info["seg_count"] + 1, called=True, api=False)
            elif u[0] == "op" and u[1] == "(":
                value, j, info = self.call(value, j, info, u[2])
            elif u[0] == "str" or (u[0] == "op" and u[1] == "{" and j == i + 1 + info["seg_count"] * 2
                                   and info["name"] is not None):
                # f"text" / f{...}: a call with one argument
                if u[0] == "op":
                    j = group_end(toks, j) + 1
                else:
                    j += 1
                value, info = self.after_call(value, info, u[2], [])
            else:
                break
        return value, j, info

    def field(self, value, name, line):
        if value is None:
            return None
        kind = value[0]
        if kind == "p":
            return ("p", value[1] + (name,))
        if kind == "mod":
            return ("modfn", value[1], name)
        if kind == "t":
            record, why = value[1], value[2]
            key = f"{record}.{name}"
            if key in self.frozen:
                self.reads.setdefault(("rec", record, name), f"{self.note(line)}: .{name} of {why}")
            if len(value) > 3 and f"eqoa.events.{value[3]}.data.{name}" in self.frozen:
                # the data of an event that is itself a record (chat.message, combat.hit, ...)
                self.reads.setdefault(("ev", value[3], name), f"{self.note(line)}: .{name} of {why}")
            child = self.nested.get((record, name)) or self.nested.get(("_", name))
            if child:
                self.receive(child, f"{why}.{name}", line)
                return ("t", child, f"{why}.{name}")
            return None
        if kind == "ev":
            event, why = value[1], value[2]
            key = f"eqoa.events.Event.{name}"
            if key in self.frozen:
                self.reads.setdefault(("rec", EVENT_RECORD, name), f"{self.note(line)}: .{name} of {why}")
            if name == "data":
                record = event_data_record(event)
                if record:
                    self.receive(record, f"data of {why}", line)
                    return ("t", record, f"data of {why}", event)
                return ("ed", event, f"data of {why}")
            return None
        if kind == "ed":
            event, why = value[1], value[2]
            if f"eqoa.events.{event}.data.{name}" in self.frozen:
                self.reads.setdefault(("ev", event, name), f"{self.note(line)}: .{name} of {why}")
            record = EVENT_BEFORE_AFTER.get(event)
            if record and name in ("before", "after"):
                self.receive(record, f"{name} of {why}", line)
                return ("t", record, f"{name} of {why}")
            return None
        return None

    def index(self, value):
        # a list of records and one record share a type here: x[i] has the type of x
        return value if value is not None and value[0] in ("t", "ed") else None

    # -- calls -----------------------------------------------------------------------------------------
    def arg_values(self, toks_starts):
        out = []
        for s in toks_starts:
            if s < len(self.toks) and (self.toks[s][0] == "name" and self.toks[s][1] not in T.KEYWORDS):
                v, _, _ = self.chain(s)
                out.append(v)
            else:
                out.append(None)
        return out

    def call(self, value, j, info, line):
        toks = self.toks
        starts, end = split_args(toks, j)
        info = dict(info, called=True)
        self.callsite(value, info, starts, line)
        args = self.arg_values(starts)
        v, info = self.after_call(value, info, line, args)
        if v is None:
            v = self.local_return(value, info, line)
        return v, end + 1, info

    def resolve(self, value, info):
        """The (file, name) of the one definition a call refers to, or None. A call of a name this mod
        defines that cannot be tied to one definition poisons the name: its parameters stay unproven."""
        name = info.get("name")
        places = self.defs.get(name)
        if name is None or not places or info.get("head") in LUA_LIBS:
            return None
        key = None
        total = sum(places.values())
        if value is None and info.get("seg_count") == 0:
            if places.get(self.path) == 1:
                key = (self.path, name)
            elif total == 1:
                key = (next(iter(places)), name)
        elif value is not None and value[0] == "modfn":
            where = self.stems.get(value[1])
            if places.get(where) == 1:
                key = (where, name)
        elif total == 1:
            key = (next(iter(places)), name)
        if key is None:
            self.escaped.add(name)
        return key

    def callsite(self, value, info, starts, line):
        """Events subscribed, handler parameters, and the arguments a local function is called with."""
        toks = self.toks
        path = value[1] if value is not None and value[0] == "p" else None
        name = info["name"]
        event_arg = None
        if path == ("eqoa", "events", "on") and starts and toks[starts[0]][0] == "str":
            event_arg = (toks[starts[0]][1], 1)
        elif path is None and name is not None and starts:
            idx = self.wrapper_index(value, info)
            if idx is not None and idx < len(starts) and toks[starts[idx]][0] == "str":
                event_arg = (toks[starts[idx]][1], idx + 1)
                self.events.setdefault(toks[starts[idx]][1], f"{self.note(line)}: through {name}()")
        if event_arg is not None:
            event, handler_at = event_arg
            if handler_at < len(starts):
                s = starts[handler_at]
                st = toks[s]
                why = f"handler parameter of {event}"
                ev_value = ("ev", event, why)
                self.receive(EVENT_RECORD, why, line)
                if st[0] == "name" and st[1] == "function":
                    self.literal_types[s] = ev_value
                elif st[0] == "name" and st[1] not in T.KEYWORDS:
                    segs, after = T.read_chain(toks, s)
                    if after < len(toks) and toks[after][1] in (",", ")"):
                        self.handler_refs.add(s)
                        hv, _, hinfo = self.chain(s)
                        key = self.resolve(hv, hinfo)
                        if key is not None:
                            self.obs.setdefault(key + (0,), []).append(ev_value)
            return
        # an ordinary function: what each argument is
        if (value is None or value[0] == "modfn") and name is not None:
            key = self.resolve(value, info)
            if key is not None:
                for idx, v in enumerate(self.arg_values(starts)):
                    self.obs.setdefault(key + (idx,), []).append(v)

    def method_call(self, receiver, method, k, line):
        toks = self.toks
        starts, _ = split_args(toks, k)
        if receiver is not None and receiver[0] == "dl":
            self.reads.setdefault(("dl", method), f"{self.note(line)}: :{method}() on {receiver[1]}")
            return
        key = self.resolve(("dotted",), {"name": method, "seg_count": 1, "head": None})
        if key is not None:
            values = [receiver] + self.arg_values(starts)
            for idx, v in enumerate(values):
                self.obs.setdefault(key + (idx,), []).append(v)

    def wrapper_index(self, value, info):
        """The parameter index of the event name when this call resolves to a wrapper definition."""
        name = info["name"]
        if value is None and info["seg_count"] == 0:
            return self.wrappers.get(self.path, {}).get(name)
        if value is not None and value[0] == "modfn":
            return self.wrappers.get(self.stems.get(value[1]), {}).get(value[2])
        return None

    def local_return(self, value, info, line):
        """What a call of a local function returns (every `return` of its one definition agrees)."""
        name = info.get("name")
        if name is None or info.get("head") in LUA_LIBS or name not in self.defs:
            return None
        if value is not None and value[0] not in ("modfn", "dotted"):
            return None
        key = self.resolve(value, info)
        got = self.ret_types.get(key) if key is not None else None
        if got is None:
            return None
        self.receive(got[1], f"returned by {name}(): {got[2]}", line)
        return ("t", got[1], f"{name}() returning {got[2]}")

    def after_call(self, value, info, line, args):
        path = value[1] if value is not None and value[0] == "p" else None
        info = dict(info, called=True)
        if path is None:
            return None, dict(info, api=False)
        joined = ".".join(path)
        if path[0] == "eqoa":
            info["api"] = True
        if joined in DRAW_LIST_GETTERS:
            return ("dl", f"{joined}() at {self.note(line)}"), info
        if path == ("require",):
            return None, info
        record = RETURNS.get(joined)
        if record is None and len(path) == 3 and path[:2] == ("eqoa", "actions") and path[2][:1].islower() \
                and path[2] not in ACTION_NOT_REQUEST:
            record = ACTION_RETURN
        if record and joined != "eqoa.events.on":
            why = f"{joined}() at {self.note(line)}"
            self.receive(record, why, line)
            return ("t", record, why), info
        return None, info

    # -- statements ------------------------------------------------------------------------------------
    def expr_value(self, i):
        """The value of the expression starting at toks[i], and the index after its first operand."""
        toks = self.toks
        if i >= len(toks):
            return None, i, {}
        t = toks[i]
        if t[0] == "name" and t[1] == "require" and i + 3 < len(toks) and toks[i + 1][1] == "(" \
                and toks[i + 2][0] == "str":
            stem = toks[i + 2][1].split(".")[-1]
            return (("mod", stem) if stem in self.stems else None), i + 4, {}
        if t[0] == "name" and t[1] not in T.KEYWORDS:
            value, after, info = self.chain(i)
            nxt = toks[after] if after < len(toks) else ("op", ";", 0)
            if nxt[0] == "name" and nxt[1] == "and" and after + 1 < len(toks) and T.is_name(toks[after + 1]):
                # `cond and CHAIN` / `cond and CHAIN or nil`: the value is CHAIN's (or falsy); a run of guards
                # (`eqoa.x and eqoa.x.get and eqoa.x.get()`) is the last one's
                value, after, info = self.chain(after + 1)
                nxt = toks[after] if after < len(toks) else ("op", ";", 0)
                while nxt[0] == "name" and nxt[1] == "and" and after + 1 < len(toks) and T.is_name(toks[after + 1]):
                    value, after, info = self.chain(after + 1)
                    nxt = toks[after] if after < len(toks) else ("op", ";", 0)
                if nxt[0] == "name" and nxt[1] == "or":
                    tail = toks[after + 1] if after + 1 < len(toks) else ("op", ";", 0)
                    if tail[1] not in ("{", "nil", "false"):
                        value = None
                elif (nxt[0] == "name" and nxt[1] in BINARY) or (nxt[0] == "op" and nxt[1] in BINARY):
                    value = None
                return value, after, info
            if nxt[0] == "name" and nxt[1] == "or":
                tail = toks[after + 1] if after + 1 < len(toks) else ("op", ";", 0)
                if not (tail[1] in ("{", "nil", "false") and tail[0] in ("op", "name")):
                    value = None
            elif (nxt[0] == "name" and nxt[1] in BINARY) or (nxt[0] == "op" and nxt[1] in BINARY) \
                    or nxt[0] == "op" and nxt[1] == "#":
                value = None
            return value, after, info
        return None, i + 1, {}

    def local_statement(self, i):
        toks = self.toks
        n = len(toks)
        names, j = [], i + 1
        while j < n and T.is_name(toks[j]):
            names.append(toks[j][1])
            j += 1
            if j < n and toks[j][1] == "<":
                j += 3
            if j < n and toks[j][1] == "," and toks[j][0] == "op":
                j += 1
            else:
                break
        values = [None] * len(names)
        if j < n and toks[j][1] == "=" and toks[j][0] == "op" and names:
            k = j + 1
            if k + 2 < n and toks[k][1] == "pcall" and toks[k + 1][1] == "(" and T.is_name(toks[k + 2]) \
                    and len(names) == 2:
                fn, _, _ = self.chain(k + 2)
                if fn is not None and fn[0] == "p" and ".".join(fn[1]) in RETURNS:
                    record = RETURNS[".".join(fn[1])]
                    why = f"pcall({'.'.join(fn[1])}) at {self.note(toks[i][2])}"
                    self.receive(record, why, toks[i][2])
                    self.scopes[-1][names[0]] = None
                    self.scopes[-1][names[1]] = ("t", record, why)
                    return j
            first, after, info = self.expr_value(k)
            if len(names) == 1:
                values = [first]
            else:
                exprs = [first]
                cur = after
                single_call = info.get("called")
                while len(exprs) < len(names) and cur < n and toks[cur][1] == "," and toks[cur][0] == "op":
                    v, cur, _ = self.expr_value(cur + 1)
                    exprs.append(v)
                    single_call = False
                if len(exprs) == len(names):
                    values = exprs
                elif single_call:
                    values = [first] + [None] * (len(names) - 1)
                    if info.get("api") and len(names) >= 2:
                        values[1] = ("t", ERROR, "the error of an API call")
                        self.receive(ERROR, f"error result at {self.note(toks[i][2])}", toks[i][2])
        for name, value in zip(names, values):
            self.scopes[-1][name] = value
        return j if j > i else i + 1

    def for_header(self, i):
        toks = self.toks
        n = len(toks)
        j = i + 1
        names = []
        while j < n and toks[j][1] not in ("in", "=") and not (toks[j][0] == "name" and toks[j][1] == "do"):
            if T.is_name(toks[j]):
                names.append(toks[j][1])
            j += 1
        scope = {name: None for name in names}
        do_index = j
        depth = 0
        while do_index < n:
            t = toks[do_index]
            if t[0] == "op" and t[1] in "({[":
                depth += 1
            elif t[0] == "op" and t[1] in ")}]":
                depth -= 1
            elif t[0] == "name" and t[1] == "do" and depth == 0:
                break
            do_index += 1
        if j < n and toks[j][1] == "in" and len(names) >= 2:
            k = j + 1
            if k + 2 < n and toks[k][0] == "name" and toks[k][1] in ("ipairs", "pairs") and toks[k + 1][1] == "(" \
                    and T.is_name(toks[k + 2]):
                v, _, _ = self.chain(k + 2)
                if v is not None and v[0] in ("t", "ed"):
                    scope[names[1]] = v
        self.loop_scopes[do_index] = scope
        return j

    def function_head(self, i):
        """Parameters of the function literal at toks[i]: (function name or None, [parameter names])."""
        toks = self.toks
        n = len(toks)
        name, colon = None, False
        j = i + 1
        if j < n and T.is_name(toks[j]):
            segs, after = T.read_chain(toks, j)
            name = segs[-1]
            # `function a:b(` leaves the chain at `a`; the colon form follows
            if after < n and toks[after][1] == ":" and after + 1 < n:
                name = toks[after + 1][1]
                colon = True
                after += 2
            j = after
        elif i >= 2 and toks[i - 1][1] == "=" and toks[i - 1][0] == "op" and T.is_name(toks[i - 2]):
            name = toks[i - 2][1]
        params = []
        while j < n and toks[j][1] != "(":
            j += 1
        close = group_end(toks, j) if j < n else j
        j += 1
        while j < close:
            if T.is_name(toks[j]):
                params.append(toks[j][1])
            j += 1
        if colon:
            params.insert(0, "self")
        return name, params, close

    def run(self):
        for path, toks in self.files:
            self.path = path
            self.where = path.name
            self.toks = toks
            self.scopes = [{}]
            self.blocks = []
            self.ctx = ["B"]
            self.loop_scopes = {}
            self.literal_types = {}
            self.handler_refs = set()
            self.walk()

    def push(self, scope, function=None):
        self.scopes.append(scope)
        self.blocks.append(function)
        self.ctx.append("B")

    def current_function(self):
        for entry in reversed(self.blocks):
            if entry is not None:
                return entry[0]
        return None

    def pop(self):
        if self.blocks:
            self.blocks.pop()
            self.scopes.pop()
            while self.ctx and self.ctx.pop() != "B":
                pass

    def walk(self):
        toks = self.toks
        n = len(toks)
        i = 0
        while i < n:
            t = toks[i]
            if t[0] == "name":
                w = t[1]
                if w == "local":
                    if i + 1 < n and toks[i + 1][1] == "function" and i + 2 < n and T.is_name(toks[i + 2]):
                        self.scopes[-1][toks[i + 2][1]] = None
                        i += 1
                        continue
                    i = self.local_statement(i)
                    continue
                if w == "function":
                    name, params, close = self.function_head(i)
                    scope = {}
                    for idx, p in enumerate(params):
                        value = None
                        if idx == 0 and i in self.literal_types:
                            value = self.literal_types[i]
                        elif name is not None and self.defs.get(name, {}).get(self.path) == 1 \
                                and name not in self.escaped:
                            value = self.param_types.get((self.path, name, idx))
                        scope[p] = value
                    self.push(scope, (name,))
                    i = close + 1  # the parameter list is not a call
                    continue
                if w == "for":
                    i = max(self.for_header(i), i + 1)
                    continue
                if w == "do":
                    self.push(self.loop_scopes.pop(i, {}))
                    i += 1
                    continue
                if w in ("if", "repeat"):
                    self.push({})
                    i += 1
                    continue
                if w in ("else", "elseif"):
                    if self.blocks:
                        self.scopes[-1].clear()
                    i += 1
                    continue
                if w in ("end", "until"):
                    self.pop()
                    i += 1
                    continue
                if w == "return":
                    fname = self.current_function()
                    if fname is not None:
                        v, _, _ = self.expr_value(i + 1)
                        self.ret_obs.setdefault((self.path, fname), []).append(v)
                    i += 1
                    continue
                if w in T.KEYWORDS:
                    i += 1
                    continue
                if i > 0 and toks[i - 1][0] == "op" and toks[i - 1][1] in (".", ":"):
                    i += 1
                    continue
                value, after, info = self.chain(i)
                # a name used as a plain value (not called, not a field read) may be a callback
                if after < n and not info.get("called") and info.get("name") and i not in self.handler_refs \
                        and not (toks[after][1] == "=" and toks[after][0] == "op") \
                        and not (i > 0 and toks[i - 1][1] == "function"):
                    self.escaped.add(info["name"])
                if self.ctx[-1] == "T":
                    pass
                elif after == i + 1 and after < n and toks[after][0] == "op" and toks[after][1] == "=":
                    v, _, _ = self.expr_value(after + 1)
                    self.assign(w, v)
                elif after == i + 1 and after < n and toks[after][1] == "," and toks[after][0] == "op":
                    k = after
                    names = [w]
                    while k < n and toks[k][1] == "," and k + 1 < n and T.is_name(toks[k + 1]):
                        names.append(toks[k + 1][1])
                        k += 2
                    if k < n and toks[k][1] == "=" and toks[k][0] == "op":
                        for name in names:
                            self.assign(name, None)
                i += 1
                continue
            if t[0] == "op" and t[1] == "{":
                self.ctx.append("T")
            elif t[0] == "op" and t[1] == "}":
                if self.ctx and self.ctx[-1] == "T":
                    self.ctx.pop()
            i += 1


def analyze_mod(files, frozen, nested, wrappers, stems):
    """files: [(path, tokens)] of one mod. Returns the last pass's Flow."""
    defs = {}
    for path, toks in files:
        for i, t in enumerate(toks):
            if t[0] == "name" and t[1] == "function":
                probe = Flow([], frozen, nested, wrappers, stems, {}, {}, set())
                probe.toks = toks
                name, _, _ = probe.function_head(i)
                if name:
                    defs.setdefault(name, {})
                    defs[name][path] = defs[name].get(path, 0) + 1
    param_types, escaped, ret_types = {}, set(), {}
    flow = None
    for _ in range(8):
        flow = Flow(files, frozen, nested, wrappers, stems, param_types, defs, escaped, ret_types)
        flow.run()
        new_ret = {}
        for key, seen in flow.ret_obs.items():
            if defs.get(key[1], {}).get(key[0]) == 1 and key[1] not in flow.escaped \
                    and all(v is not None and v[0] == "t" for v in seen)                     and len({(v[1], v[3] if len(v) > 3 else None) for v in seen}) == 1:
                new_ret[key] = seen[0]
        new = {}
        for (path, name, idx), seen in flow.obs.items():
            if defs.get(name, {}).get(path) != 1 or name in flow.escaped:
                continue
            if all(v is not None and v[0] in ("t", "ev", "ed", "dl") for v in seen):
                kinds = {(v[0], v[1], v[3] if len(v) > 3 else None) for v in seen}
                if len(kinds) == 1:
                    new[(path, name, idx)] = seen[0]
        escaped = flow.escaped
        same = new == param_types and {k: v[1] for k, v in new_ret.items()} == {k: v[1] for k, v in ret_types.items()}
        param_types, ret_types = new, new_ret
        if same:
            break
    return flow
