import contextlib, io, json, sys, tempfile, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import api_tiers as T


FROZEN = [
    "eqoa.items", "eqoa.items.find", "eqoa.items.get", "eqoa.items.Item", "eqoa.items.Item.name",
    "eqoa.items.Item.stack", "eqoa.items.slots", "eqoa.items.slots.head",
    "eqoa.raid", "eqoa.raid.get", "eqoa.raid.Raid", "eqoa.raid.Raid.size",
    "eqoa.actions", "eqoa.actions.cast", "eqoa.actions.jump", "eqoa.actions.RequestId",
    "eqoa.actions.ActionResult", "eqoa.actions.ActionResult.ok", "eqoa.actions.ActionResult.detail",
    "eqoa.events", "eqoa.events.on", "eqoa.events.combat.hit", "eqoa.events.combat.hit.data.amount",
    "eqoa.events.combat.hit.data.source", "eqoa.events.raid.changed", "eqoa.events.raid.changed.data.members",
    "eqoa.events.render", "eqoa.events.world.entered", "eqoa.events.chat.message",
    "eqoa.imgui.Begin", "ImGui.Begin", "ImGui.Text", "ImDrawList.AddLine", "ImDrawList.AddRect",
    "ImGui.GetWindowDrawList", "frontiers_forge.player", "frontiers_forge.player.get", "UiForge.Log", "bit.band",
]


def make_mod(root, name, files):
    d = root / name
    for rel, text in files.items():
        p = d / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8")


def tiers_for(mods, frozen=FROZEN):
    """Build tiers from {mod name: {file: lua source}}."""
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        bundled = tmp / "bundled-mods"
        bundled.mkdir()
        for name, files in mods.items():
            make_mod(bundled, name, files)
        frozen_path = tmp / "frozen.json"
        frozen_path.write_text(json.dumps({"api": "eqoa/1", "schema_version": 1, "supported": frozen}),
                               encoding="utf-8")
        doc, t = T.build(frozen_path, bundled)
        return doc, t.tier


class Tokenizer(unittest.TestCase):
    def test_comments_and_strings_are_not_code(self):
        toks = T.tokenize('-- eqoa.items.find()\nlocal s = "eqoa.items.get" --[[ eqoa.raid.get() ]] x = 1\n'
                          '--[==[ eqoa.actions.cast() ]==]\nlocal l = [[eqoa.actions.jump]]\n')
        names = [t[1] for t in toks if t[0] == "name"]
        self.assertNotIn("eqoa", names)
        self.assertEqual([t[1] for t in toks if t[0] == "str"], ["eqoa.items.get", "eqoa.actions.jump"])


class FunctionsAndAliases(unittest.TestCase):
    def test_direct_call_is_proven_and_unused_is_provisional(self):
        _, tier = tiers_for({"m": {"main.lua": "local x = eqoa.items.find(1)\n"}})
        self.assertEqual(tier["eqoa.items.find"], "proven")
        self.assertEqual(tier["eqoa.items.get"], "provisional")

    def test_negative_control_commented_call_is_not_use(self):
        _, tier = tiers_for({"m": {"main.lua": "-- eqoa.items.find(1)\nlocal s = 'eqoa.items.find'\n"}})
        self.assertEqual(tier["eqoa.items.find"], "provisional")

    def test_module_alias(self):
        _, tier = tiers_for({"m": {"main.lua": "local items = eqoa.items\nlocal x = items.find(1)\n"}})
        self.assertEqual(tier["eqoa.items.find"], "proven")
        self.assertEqual(tier["eqoa.items.get"], "provisional")

    def test_function_alias(self):
        _, tier = tiers_for({"m": {"main.lua": "local find = eqoa.items.find\nlocal x = find(1)\n"}})
        self.assertEqual(tier["eqoa.items.find"], "proven")

    def test_alias_through_alias(self):
        _, tier = tiers_for({"m": {"main.lua": "local e = eqoa\nlocal i = e.items\ni.get(1)\n"}})
        self.assertEqual(tier["eqoa.items.get"], "proven")

    def test_ambiguous_alias_is_not_resolved(self):
        # `f` is also a parameter somewhere in the file: the call may be to anything
        src = "local i = eqoa.items\nlocal function g(i) return i.find(1) end\n"
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.items.find"], "provisional")
        # negative control: without the second binding the same call is credited through the alias
        src = "local i = eqoa.items\nlocal function g() return i.find(1) end\n"
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.items.find"], "proven")

    def test_alias_with_fallback_is_not_resolved(self):
        # `w` may be eqoa.raid or something else: w.get() is not credited to eqoa.raid.get
        _, tier = tiers_for({"m": {"main.lua": "local w = eqoa.raid and eqoa.raid\nw.get()\n"}})
        self.assertEqual(tier["eqoa.raid.get"], "provisional")
        _, tier = tiers_for({"m": {"main.lua": "local w = eqoa.raid\nw.get()\n"}})
        self.assertEqual(tier["eqoa.raid.get"], "proven")

    def test_call_result_is_not_an_alias(self):
        _, tier = tiers_for({"m": {"main.lua": "local r = eqoa.raid.get()\nr.find(1)\n"}})
        self.assertEqual(tier["eqoa.raid.get"], "proven")
        self.assertEqual(tier["eqoa.items.find"], "provisional")

    def test_dynamic_action_is_provisional(self):
        _, tier = tiers_for({"m": {"main.lua": "eqoa.actions[name](1)\n"}})
        self.assertEqual(tier["eqoa.actions.cast"], "provisional")
        self.assertEqual(tier["eqoa.actions.jump"], "provisional")

    def test_bracket_string_path(self):
        _, tier = tiers_for({"m": {"main.lua": 'eqoa.actions["cast"](1)\n'}})
        self.assertEqual(tier["eqoa.actions.cast"], "proven")

    def test_only_bundled_mods_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            make_mod(tmp / "bundled-mods", "a", {"main.lua": "eqoa.items.find(1)\n"})
            make_mod(tmp / "mods", "b", {"main.lua": "eqoa.items.get(1)\n"})
            fp = tmp / "f.json"
            fp.write_text(json.dumps({"supported": FROZEN}), encoding="utf-8")
            _, t = T.build(fp, tmp / "bundled-mods")
            self.assertEqual(t.tier["eqoa.items.get"], "provisional")

    def test_compat_names_are_left_out_of_the_counts(self):
        doc, tier = tiers_for({"m": {"main.lua": "frontiers_forge = nil\nUiForge.Log('x')\n"}})
        self.assertEqual(tier["UiForge.Log"], "compat")
        self.assertEqual(tier["frontiers_forge.player.get"], "compat")
        self.assertEqual(tier["bit.band"], "compat")
        self.assertEqual(doc["counts"]["compat"], 4)
        self.assertEqual(doc["counts"]["total"], doc["counts"]["compat"] + doc["counts"]["proven"]
                         + doc["counts"]["provisional"])
        self.assertTrue(set(doc["compat"]).isdisjoint(doc["provisional"]))

    def test_constants_need_a_direct_reference(self):
        _, tier = tiers_for({"m": {"main.lua": "local x = eqoa.items.slots.head\n"}})
        self.assertEqual(tier["eqoa.items.slots.head"], "proven")
        _, tier = tiers_for({"m": {"main.lua": "for k in pairs(eqoa.items.slots) do end\n"}})
        self.assertEqual(tier["eqoa.items.slots.head"], "provisional")

    def test_globals_and_imgui_alias(self):
        src = "local ui = eqoa.imgui\nui.Begin('x')\nImGui.Text('y')\n"
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.imgui.Begin"], "proven")
        self.assertEqual(tier["ImGui.Text"], "proven")
        self.assertEqual(tier["ImGui.Begin"], "provisional")


class Events(unittest.TestCase):
    def test_literal_subscription(self):
        _, tier = tiers_for({"m": {"main.lua": 'eqoa.events.on("combat.hit", function(e) end)\n'}})
        self.assertEqual(tier["eqoa.events.combat.hit"], "proven")
        self.assertEqual(tier["eqoa.events.raid.changed"], "provisional")

    def test_event_names_in_a_comment_or_unrelated_string_do_not_count(self):
        src = '-- eqoa.events.on("combat.hit", f)\nlocal x = "combat.hit"\n'
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.events.combat.hit"], "provisional")

    def test_wrapper_function(self):
        src = ('local function listen(name, fn)\n  local t = eqoa.events.on(name, fn)\nend\n'
               'listen("raid.changed", function(e) end)\n')
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.events.raid.changed"], "proven")

    def test_wrapper_called_on_a_global_of_another_file_is_not_resolved(self):
        # `M` is a global here, not the result of require: the call does not resolve to the definition
        a = 'M = {}\nfunction M.listen(name, fn) eqoa.events.on(name, fn) end\n'
        b = 'M.listen("render", function() end)\n'
        _, tier = tiers_for({"m": {"a.lua": a, "b.lua": b}})
        self.assertEqual(tier["eqoa.events.render"], "provisional")

    def test_loop_over_names(self):
        src = 'for _, name in ipairs({"world.entered", "render"}) do eqoa.events.on(name, function() end) end\n'
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.events.world.entered"], "proven")
        self.assertEqual(tier["eqoa.events.render"], "proven")

    def test_computed_name_is_not_guessed(self):
        src = 'local n = "combat" .. ".hit"\neqoa.events.on(n, function() end)\n'
        doc, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.events.combat.hit"], "provisional")

    def test_wrapper_must_resolve_to_a_definition(self):
        # a different function that shares the wrapper's last name, in another file, does not subscribe
        a = 'local function listen(name, fn) eqoa.events.on(name, fn) end\nlisten("render", function() end)\n'
        b = 'local O = {}\nfunction O.listen(name, fn) return name end\nO.listen("world.entered", function() end)\n'
        _, tier = tiers_for({"m": {"a.lua": a, "b.lua": b}})
        self.assertEqual(tier["eqoa.events.render"], "proven")
        self.assertEqual(tier["eqoa.events.world.entered"], "provisional")

    def test_wrapper_called_through_require_resolves(self):
        a = 'local M = {}\nfunction M.listen(name, fn) eqoa.events.on(name, fn) end\nreturn M\n'
        b = 'local H = require("helpers")\nH.listen("world.entered", function() end)\n'
        _, tier = tiers_for({"m": {"helpers.lua": a, "main.lua": b}})
        self.assertEqual(tier["eqoa.events.world.entered"], "proven")
        # negative control: the same call on a receiver that was not required from the wrapper's file
        c = 'local H = {}\nH.listen("world.entered", function() end)\n'
        _, tier = tiers_for({"m": {"helpers.lua": a, "main.lua": c}})
        self.assertEqual(tier["eqoa.events.world.entered"], "provisional")


FIELDS = FROZEN + [
    "eqoa.camera.get", "eqoa.camera.Camera", "eqoa.camera.Camera.distance", "eqoa.camera.Camera.position",
    "eqoa.bank.get", "eqoa.merchant.get", "eqoa.items.Container", "eqoa.items.Container.items",
    "eqoa.items.Container.merchant", "eqoa.items.MerchantState", "eqoa.items.MerchantState.mode",
    "eqoa.items.Item.range",
    "eqoa.windows.confirm", "eqoa.windows.Confirm", "eqoa.windows.Confirm.id",
    "eqoa.events.confirm.closed", "eqoa.events.confirm.closed.data.confirm",
    "eqoa.events.state.reset", "eqoa.events.state.reset.data.initial",
    "eqoa.events.batch.progress", "eqoa.actions.BatchResult", "eqoa.actions.BatchResult.ok",
    "eqoa.actions.BatchResult.total", "eqoa.actions.BatchId", "eqoa.actions.batch",
    "eqoa.events.Event", "eqoa.events.Event.name", "eqoa.events.Event.data",
    "eqoa.events.auction.changed", "eqoa.events.auction.changed.data.open",
    "eqoa.events.bank.changed", "eqoa.events.bank.changed.data.after",
    "eqoa.entities.list", "eqoa.entities.Entity", "eqoa.entities.Entity.name", "eqoa.entities.Entity.kind",
    "eqoa.Error", "eqoa.Error.message",
    "eqoa.events.chat.message", "eqoa.events.chat.message.data.text", "eqoa.chat.ChatMessage",
    "eqoa.chat.ChatMessage.text",
]


class Records(unittest.TestCase):
    def check(self, src, **files):
        mods = {"m": {"main.lua": src, **files}}
        return tiers_for(mods, FIELDS)[1]

    def test_read_from_a_producer_result_is_proven(self):
        tier = self.check("local i = eqoa.items.get(1)\nlocal n = i.name\n")
        self.assertEqual(tier["eqoa.items.Item.name"], "proven")
        self.assertEqual(tier["eqoa.items.Item.stack"], "provisional")
        self.assertEqual(tier["eqoa.items.Item"], "proven")

    def test_same_name_on_something_else_is_not_a_read(self):
        # the reviewer's `S.distance` case: a settings table, and eqoa.camera.get never called
        tier = self.check("local S = { distance = 5 }\nlocal d = S.distance\nlocal c = eqoa.items.get(1)\n")
        self.assertEqual(tier["eqoa.camera.Camera.distance"], "provisional")
        self.assertEqual(tier["eqoa.camera.Camera"], "provisional")
        # positive control: the same read from a real camera record
        tier = self.check("local c = eqoa.camera.get()\nlocal d = c.distance\n")
        self.assertEqual(tier["eqoa.camera.Camera.distance"], "proven")
        self.assertEqual(tier["eqoa.camera.Camera.position"], "provisional")

    def test_negative_control_range_on_a_local_table(self):
        # the reviewer's `M.range`: a mod calls eqoa.bank.get and reads .items, and has its own M.range
        src = ("local M = {}\nM.range = 3\nlocal r = M.range\n"
               "local c = eqoa.bank.get()\nfor _, it in ipairs(c.items) do local nm = it.name end\n")
        tier = self.check(src)
        self.assertEqual(tier["eqoa.items.Item.range"], "provisional")
        self.assertEqual(tier["eqoa.items.Item.name"], "proven")
        self.assertEqual(tier["eqoa.items.Container.items"], "proven")
        # positive control
        tier = self.check("local c = eqoa.bank.get()\nfor _, it in ipairs(c.items) do local r = it.range end\n")
        self.assertEqual(tier["eqoa.items.Item.range"], "proven")

    def test_path_segments_are_not_field_reads(self):
        # `.merchant` and `.confirm` here are parts of eqoa.merchant.get / eqoa.windows.confirm
        src = ("local c = eqoa.bank.get()\nlocal m = eqoa.merchant.get()\nlocal q = eqoa.windows.confirm()\n"
               "local ok = c ~= nil\n")
        tier = self.check(src)
        self.assertEqual(tier["eqoa.items.Container.merchant"], "provisional")
        self.assertEqual(tier["eqoa.windows.Confirm.id"], "provisional")
        # the record is received but only tested for nil: a record with fields is proven by a field read
        self.assertEqual(tier["eqoa.windows.Confirm"], "provisional")
        # positive control: reading the field of the received record
        tier = self.check("local c = eqoa.bank.get()\nlocal x = c.merchant\n")
        self.assertEqual(tier["eqoa.items.Container.merchant"], "proven")

    def test_a_nested_record_is_reached_through_its_proven_parent_field(self):
        tier = self.check("local c = eqoa.bank.get()\nlocal mode = c.merchant.mode\n")
        self.assertEqual(tier["eqoa.items.MerchantState.mode"], "proven")
        self.assertEqual(tier["eqoa.items.MerchantState"], "proven")
        # negative control: a field of that name on something not reached from a Container
        tier = self.check("local c = eqoa.bank.get()\nlocal t = {}\nlocal mode = t.mode\n")
        self.assertEqual(tier["eqoa.items.MerchantState.mode"], "provisional")

    def test_scope_a_shadowing_local_is_not_the_record(self):
        src = ("local c = eqoa.bank.get()\nlocal a = c.items\n"
               "do local c = {}\nlocal x = c.merchant end\n")
        tier = self.check(src)
        self.assertEqual(tier["eqoa.items.Container.items"], "proven")
        self.assertEqual(tier["eqoa.items.Container.merchant"], "provisional")

    def test_a_reassigned_variable_is_no_longer_the_record(self):
        tier = self.check("local c = eqoa.bank.get()\nc = {}\nlocal x = c.items\n")
        self.assertEqual(tier["eqoa.items.Container.items"], "provisional")

    def test_function_parameter_filled_at_every_call_site(self):
        src = ("local function show(c) return c.items end\nshow(eqoa.bank.get())\n"
               "local b = eqoa.merchant.get()\nshow(b)\n")
        self.assertEqual(self.check(src)["eqoa.items.Container.items"], "proven")
        # negative control: one call site passes something that is not a Container
        src += "show({})\n"
        self.assertEqual(self.check(src)["eqoa.items.Container.items"], "provisional")
        # negative control: the function is also passed around as a value (a callback the scan cannot follow)
        src = "local function show(c) return c.items end\nshow(eqoa.bank.get())\nlater(show)\n"
        self.assertEqual(self.check(src)["eqoa.items.Container.items"], "provisional")

    def test_a_name_defined_in_two_files_is_resolved_only_when_the_call_says_which(self):
        a = "local function show(c) return c.items end\nshow(eqoa.bank.get())\n"
        b = "local function show(c) return c end\n"
        # a plain call resolves to the definition in its own file
        tier = tiers_for({"m": {"a.lua": a, "b.lua": b}}, FIELDS)[1]
        self.assertEqual(tier["eqoa.items.Container.items"], "proven")
        # negative control: a call on an unknown receiver cannot be tied to either definition
        a = "function M.show(c) return c.items end\nX.show(eqoa.bank.get())\n"
        b = "function N.show(c) return c end\n"
        tier = tiers_for({"m": {"a.lua": a, "b.lua": b}}, FIELDS)[1]
        self.assertEqual(tier["eqoa.items.Container.items"], "provisional")
        # the same call through a module the mod required resolves
        a = "local M = {}\nfunction M.show(c) return c.items end\nreturn M\n"
        b = 'local A = require("a")\nA.show(eqoa.bank.get())\n'
        c = "local function show(c) return c end\n"
        tier = tiers_for({"m": {"a.lua": a, "main.lua": b, "c.lua": c}}, FIELDS)[1]
        self.assertEqual(tier["eqoa.items.Container.items"], "proven")

    def test_function_that_returns_a_record(self):
        src = "local function get() return eqoa.bank.get() end\nlocal c = get()\nlocal x = c.items\n"
        self.assertEqual(self.check(src)["eqoa.items.Container.items"], "proven")
        # negative control: one return is something else
        src = ("local function get(a) if a then return {} end return eqoa.bank.get() end\n"
               "local c = get()\nlocal x = c.items\n")
        self.assertEqual(self.check(src)["eqoa.items.Container.items"], "provisional")

    def test_and_or_idioms_and_pcall(self):
        tier = self.check("local ok = true\nlocal c = ok and eqoa.bank.get() or nil\nlocal x = c.items\n")
        self.assertEqual(tier["eqoa.items.Container.items"], "proven")
        # negative control: the fallback is something else
        tier = self.check("local ok = true\nlocal c = ok and eqoa.bank.get() or other\nlocal x = c.items\n")
        self.assertEqual(tier["eqoa.items.Container.items"], "provisional")
        tier = self.check("local c = eqoa.bank and eqoa.bank.get and eqoa.bank.get()\nlocal x = c.items\n")
        self.assertEqual(tier["eqoa.items.Container.items"], "proven")
        tier = self.check("local c = eqoa.bank and eqoa.bank.get and other.get()\nlocal x = c.items\n")
        self.assertEqual(tier["eqoa.items.Container.items"], "provisional")
        tier = self.check("local ok, c = pcall(eqoa.bank.get)\nlocal x = c.items\n")
        self.assertEqual(tier["eqoa.items.Container.items"], "proven")
        tier = self.check("local ok, c = pcall(other.get)\nlocal x = c.items\n")
        self.assertEqual(tier["eqoa.items.Container.items"], "provisional")

    def test_loop_variable_over_a_list_of_records(self):
        src = "for _, e in ipairs(eqoa.entities.list()) do local n = e.name end\n"
        self.assertEqual(self.check(src)["eqoa.entities.Entity.name"], "proven")
        src = "for _, e in ipairs(other.list()) do local n = e.name end\n"
        self.assertEqual(self.check(src)["eqoa.entities.Entity.name"], "provisional")

    def test_error_result_of_an_api_call(self):
        src = "local r, err = eqoa.actions.cast(1)\nlocal m = err.message\n"
        tier = self.check(src)
        self.assertEqual(tier["eqoa.Error.message"], "proven")
        src = "local r, err = something(1)\nlocal m = err.message\n"
        self.assertEqual(self.check(src)["eqoa.Error.message"], "provisional")

    def test_fieldless_type_needs_a_received_value(self):
        tier = self.check("local id = eqoa.actions.batch({})\n")
        self.assertEqual(tier["eqoa.actions.BatchId"], "proven")
        tier = self.check("local id = other.batch({})\n")
        self.assertEqual(tier["eqoa.actions.BatchId"], "provisional")

    def test_container_is_proven_with_any_proven_child(self):
        _, tier = tiers_for({"m": {"main.lua": "eqoa.items.find(1)\n"}})
        self.assertEqual(tier["eqoa.items"], "proven")
        self.assertEqual(tier["eqoa.raid"], "provisional")

    def test_draw_list_methods_need_a_variable_bound_from_a_getter(self):
        src = "local dl = ImGui.GetWindowDrawList()\ndl:AddLine(1, 2)\n"
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["ImDrawList.AddLine"], "proven")
        self.assertEqual(tier["ImDrawList.AddRect"], "provisional")
        # negative controls: a method call on something else, in a file that does get a draw list
        src = "local dl = ImGui.GetWindowDrawList()\nlocal x = {}\nx:AddRect(1, 2)\n"
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["ImDrawList.AddRect"], "provisional")
        _, tier = tiers_for({"m": {"main.lua": "x:AddLine(1, 2)\n"}})
        self.assertEqual(tier["ImDrawList.AddLine"], "provisional")

    def test_draw_list_through_a_parameter(self):
        src = "local function draw(dl) dl:AddLine(1, 2) end\ndraw(ImGui.GetWindowDrawList())\n"
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["ImDrawList.AddLine"], "proven")


class EventData(unittest.TestCase):
    def check(self, src, **files):
        return tiers_for({"m": {"main.lua": src, **files}}, FIELDS)[1]

    def test_a_read_by_the_handler_of_that_event_proves_the_field(self):
        # state.reset + `j.initial` (the reviewer's case): the handler ignores its event
        src = ('local j = {}\nlocal x = j.initial\n'
               'eqoa.events.on("state.reset", function() scan = true end)\n')
        self.assertEqual(self.check(src)["eqoa.events.state.reset.data.initial"], "provisional")
        # positive control
        src = 'eqoa.events.on("state.reset", function(e) if e.data.initial then scan = true end end)\n'
        self.assertEqual(self.check(src)["eqoa.events.state.reset.data.initial"], "proven")

    def test_path_segment_of_an_api_call_is_not_a_data_field(self):
        # the reviewer's `.confirm()`: eqoa.windows.confirm() in a mod that listens to confirm.closed
        src = ('eqoa.events.on("confirm.closed", function(e) done = true end)\n'
               'local q = eqoa.windows.confirm()\n')
        tier = self.check(src)
        self.assertEqual(tier["eqoa.events.confirm.closed"], "proven")
        self.assertEqual(tier["eqoa.events.confirm.closed.data.confirm"], "provisional")
        src = 'eqoa.events.on("confirm.closed", function(e) done = e.data.confirm end)\n'
        self.assertEqual(self.check(src)["eqoa.events.confirm.closed.data.confirm"], "proven")

    def test_a_read_in_a_handler_of_another_event_does_not_count(self):
        src = ('eqoa.events.on("auction.changed", function() redraw() end)\n'
               'eqoa.events.on("bank.changed", function(e) local o = e.data.open end)\n')
        self.assertEqual(self.check(src)["eqoa.events.auction.changed.data.open"], "provisional")

    def test_data_passed_to_a_function_and_read_there(self):
        # batch.progress: the helper reads `total`; the `.ok` in the same file belongs to another table
        src = ('local function on_progress(d) return d.total end\n'
               'eqoa.events.on("batch.progress", function(e) on_progress(e.data) end)\n'
               'local B = { ok = true }\nlocal flag = B.ok\n')
        tier = self.check(src)
        self.assertEqual(tier["eqoa.actions.BatchResult.total"], "proven")
        self.assertEqual(tier["eqoa.actions.BatchResult.ok"], "provisional")
        # negative control: the helper is also called with something else
        src += "on_progress({})\n"
        self.assertEqual(self.check(src)["eqoa.actions.BatchResult.total"], "provisional")

    def test_handler_given_by_name_and_through_a_wrapper(self):
        src = ('local function on_reset(e) return e.data.initial end\n'
               'eqoa.events.on("state.reset", on_reset)\n')
        self.assertEqual(self.check(src)["eqoa.events.state.reset.data.initial"], "proven")
        src = ('local function listen(name, fn) eqoa.events.on(name, fn) end\n'
               'listen("state.reset", function(e) return e.data.initial end)\n')
        self.assertEqual(self.check(src)["eqoa.events.state.reset.data.initial"], "proven")

    def test_event_record_fields_and_before_after(self):
        src = ('eqoa.events.on("bank.changed", function(e)\n'
               '  local n = e.name\n  local a = e.data.after\n  local i = a.items\nend)\n')
        tier = self.check(src)
        self.assertEqual(tier["eqoa.events.Event.name"], "proven")
        self.assertEqual(tier["eqoa.events.bank.changed.data.after"], "proven")
        self.assertEqual(tier["eqoa.items.Container.items"], "proven")
        # `.name` on a variable that is not an event
        tier = self.check('eqoa.events.on("bank.changed", function(e) end)\nlocal t = {}\nlocal n = t.name\n')
        self.assertEqual(tier["eqoa.events.Event.name"], "provisional")

    def test_event_whose_data_is_a_record_proves_both_names(self):
        src = 'eqoa.events.on("chat.message", function(e) local t = e.data.text end)'+chr(10)
        tier = self.check(src)
        self.assertEqual(tier["eqoa.events.chat.message.data.text"], "proven")
        self.assertEqual(tier["eqoa.chat.ChatMessage.text"], "proven")
        # negative control: the same read in the handler of another event
        src = 'eqoa.events.on("state.reset", function(e) local t = e.data.text end)'+chr(10)
        tier = self.check(src)
        self.assertEqual(tier["eqoa.events.chat.message.data.text"], "provisional")
        self.assertEqual(tier["eqoa.chat.ChatMessage.text"], "provisional")

    def test_the_handler_parameter_is_scoped_to_its_function(self):
        src = ('eqoa.events.on("state.reset", function(e) return 1 end)\n'
               'local function other(e) return e.data.initial end\n')
        self.assertEqual(self.check(src)["eqoa.events.state.reset.data.initial"], "provisional")


class Reviewer(unittest.TestCase):
    """The names the independent review found proven without a real use stay provisional."""

    RECORDS = [
        "eqoa.camera.Camera", "eqoa.capabilities.Capability", "eqoa.files.FileEntry", "eqoa.group.RaidRole",
        "eqoa.group.RaidMember", "eqoa.group.Invite", "eqoa.hotbar.Hotbar", "eqoa.hotbar.HotbarSlot",
        "eqoa.items.MasterLoot", "eqoa.items.MasterLootMember", "eqoa.items.ContainerView", "eqoa.items.Tab",
        "eqoa.items.MerchantState", "eqoa.items.MerchantOption", "eqoa.player.Breath", "eqoa.player.Resists",
        "eqoa.player.StatRange", "eqoa.hud.GroupMemberLayout", "eqoa.hud.LayoutPieces",
        "eqoa.hud.SinglePanelLayout", "eqoa.input.Pad", "eqoa.input.RawAxes", "eqoa.windows.Window",
        "eqoa.windows.Dialog", "eqoa.windows.MenuRow", "eqoa.windows.ButtonHint", "eqoa.windows.Confirm",
    ]
    NAMES = [
        "eqoa.items.Container.merchant", "eqoa.hud.Layout.pet", "eqoa.items.Item.range",
        "eqoa.blacksmith.Item.worn", "eqoa.player.StatItem.base", "eqoa.player.Vital.base",
        "eqoa.mastery.Mastery.stats", "eqoa.events.Event.name", "eqoa.events.Event.seq", "eqoa.events.Event.tick",
        "eqoa.events.auction.changed.data.open", "eqoa.events.batch.progress.data.ok",
        "eqoa.events.batch.progress.data.reason", "eqoa.events.confirm.closed.data.confirm",
        "eqoa.events.entity.added.data.changed", "eqoa.events.state.reset.data.initial",
        "eqoa.events.world.leaving.data.world_id",
        "eqoa.windows.Windows.active", "eqoa.windows.Windows.mode", "eqoa.windows.Windows.windows",
        "eqoa.windows.Session.focused", "eqoa.windows.Session.revision",
    ] + ["eqoa.windows.SessionData." + f for f in
         "available current id inventory list message open state target title".split()]

    # Names a bundled mod really reads now (the Bag Viewer's Merchant tab, 2026-11-27: `eqoa.merchant.get().merchant.options`
    # and `.pending`), so they are proven for a real use. They stay exempt only while that is so (checked below).
    USED_SINCE_REVIEW = [
        "eqoa.items.Container.merchant", "eqoa.items.MerchantState", "eqoa.items.MerchantState.options",
        "eqoa.items.MerchantState.pending",
    ]

    def violations(self, provisional, frozen):
        families = [n for n in frozen if any(n == r or n.startswith(r + ".") for r in self.RECORDS)]
        return [n for n in families + self.NAMES if n not in provisional and n not in self.USED_SINCE_REVIEW]

    def test_every_reviewed_name_is_provisional_in_the_committed_file(self):
        doc = json.loads(T.TIERS.read_text(encoding="utf-8"))
        frozen = json.loads(T.FROZEN.read_text(encoding="utf-8"))["supported"]
        self.assertGreater(len([n for n in frozen if any(n == r or n.startswith(r + ".") for r in self.RECORDS)]), 100)
        self.assertEqual(self.violations(set(doc["provisional"]), frozen), [])
        # the names are real frozen names (the list does not rot)
        self.assertEqual([n for n in self.NAMES if n not in frozen], [])
        # the exemption does not rot: each exempt name is proven now, by a real read
        self.assertEqual([n for n in self.USED_SINCE_REVIEW if n in set(doc["provisional"])], [])

    def test_negative_control_a_reviewed_name_that_is_proven_is_caught(self):
        doc = json.loads(T.TIERS.read_text(encoding="utf-8"))
        frozen = json.loads(T.FROZEN.read_text(encoding="utf-8"))["supported"]
        provisional = set(doc["provisional"]) - {"eqoa.camera.Camera.distance", "eqoa.windows.SessionData.state"}
        self.assertEqual(sorted(self.violations(provisional, frozen)),
                         ["eqoa.camera.Camera.distance", "eqoa.windows.SessionData.state"])


class Output(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.frozen = self.root / "frozen.json"
        self.frozen.write_text(json.dumps({"api": "eqoa/1", "schema_version": 1, "supported": FROZEN}),
                               encoding="utf-8")
        self.bundled = self.root / "bundled-mods"
        make_mod(self.bundled, "m", {"main.lua": "eqoa.items.find(1)\n"})
        self.tiers, self.md = self.root / "tiers.json", self.root / "PROVISIONAL.md"

    def run_main(self, *extra):
        argv = ["--frozen", str(self.frozen), "--bundled", str(self.bundled), "--tiers", str(self.tiers),
                "--md", str(self.md), *extra]
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = T.main(argv)
        return code, out.getvalue(), err.getvalue()

    def test_file_shape_and_determinism(self):
        self.assertEqual(self.run_main()[0], 0)
        first = self.tiers.read_bytes()
        self.assertEqual(self.run_main()[0], 0)
        self.assertEqual(first, self.tiers.read_bytes())
        self.assertTrue(first.endswith(b"\n") and b"\r" not in first)
        doc = json.loads(first)
        self.assertEqual(doc["api"], "eqoa/1")
        self.assertEqual(doc["schema_version"], 1)
        self.assertEqual(doc["provisional"], sorted(doc["provisional"]))
        self.assertEqual(len(set(doc["provisional"])), len(doc["provisional"]))
        self.assertTrue(set(doc["provisional"]) <= set(FROZEN))
        self.assertNotIn("eqoa.items.find", doc["provisional"])
        self.assertIn("eqoa.items.get", doc["provisional"])
        self.assertEqual(doc["compat"], sorted(["UiForge.Log", "bit.band", "frontiers_forge.player",
                                                "frontiers_forge.player.get"]))
        md = self.md.read_text(encoding="utf-8")
        self.assertIn("## raid", md)
        self.assertNotIn("`eqoa.items.find`", md)
        self.assertNotIn("—", md)

    def test_check_passes_on_fresh_files_and_fails_after_a_mod_change(self):
        self.run_main()
        self.assertEqual(self.run_main("--check")[0], 0)
        # a mod starts using another name: the committed file is out of date
        make_mod(self.bundled, "m", {"main.lua": "eqoa.items.find(1)\neqoa.items.get(2)\n"})
        code, _, err = self.run_main("--check")
        self.assertEqual(code, 1)
        self.assertIn("eqoa.items.get", err)

    def test_check_fails_when_the_markdown_is_stale(self):
        self.run_main()
        self.md.write_text("stale\n", encoding="utf-8")
        self.assertEqual(self.run_main("--check")[0], 1)

    def test_check_fails_when_the_tiers_file_is_missing(self):
        self.assertEqual(self.run_main("--check")[0], 1)

    def test_demotion_is_refused_unless_allowed(self):
        self.run_main()
        # the only mod stops using eqoa.items.find
        make_mod(self.bundled, "m", {"main.lua": "-- nothing\n"})
        code, _, err = self.run_main("--check")
        self.assertEqual(code, 1)
        self.assertIn("proven to provisional", err)
        self.assertIn("eqoa.items.find", err)
        before = self.tiers.read_bytes()
        code, _, err = self.run_main()
        self.assertEqual(code, 1)
        self.assertEqual(before, self.tiers.read_bytes())  # nothing written
        self.assertIn("--allow-demote", err)
        code, _, _ = self.run_main("--allow-demote")
        self.assertEqual(code, 0)
        self.assertIn("eqoa.items.find", json.loads(self.tiers.read_text(encoding="utf-8"))["provisional"])
        self.assertEqual(self.run_main("--check")[0], 0)

    def test_graduation_needs_no_flag(self):
        self.run_main()
        make_mod(self.bundled, "m", {"main.lua": "eqoa.items.find(1)\neqoa.items.get(1)\n"})
        self.assertEqual(self.run_main()[0], 0)
        self.assertNotIn("eqoa.items.get", json.loads(self.tiers.read_text(encoding="utf-8"))["provisional"])

    def test_a_name_new_to_the_frozen_record_counts_as_a_demotion(self):
        self.run_main()
        self.frozen.write_text(json.dumps({"api": "eqoa/1", "schema_version": 1,
                                           "supported": FROZEN + ["eqoa.items.brand_new"]}), encoding="utf-8")
        code, _, err = self.run_main("--check")
        self.assertEqual(code, 1)
        self.assertIn("eqoa.items.brand_new", err)


class Committed(unittest.TestCase):
    """The committed files match the committed mods (what CI checks)."""

    def test_committed_tiers_are_current(self):
        code = None
        err = io.StringIO()
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(err):
            code = T.main(["--check"])
        self.assertEqual(code, 0, err.getvalue())

    def test_actions_split_matches_the_brief(self):
        doc = json.loads(T.TIERS.read_text(encoding="utf-8"))
        acts = doc["counts"]["by_kind"]["action"]
        self.assertEqual(acts["proven"] + acts["provisional"], 132)
        provisional_actions = [n for n in doc["provisional"] if n.startswith("eqoa.actions.") and n.count(".") == 2
                               and n.split(".")[2][0].islower()]
        self.assertEqual(len(provisional_actions), acts["provisional"])


SPEC = T.ROOT / "docs/modding/EQOA-API-SPEC.md"


def provisional_actions(doc):
    return sorted(n.split(".")[2] for n in doc["provisional"]
                  if n.startswith("eqoa.actions.") and n.count(".") == 2 and n.split(".")[2][0].islower())


def spec_problems(spec_text, prov):
    """Every provisional action is marked on a line that describes it, and no line carries the
    whole-row mark unless every action in its first cell is provisional."""
    import re
    lines = spec_text.replace("\r\n", "\n").split("\n")
    out = []
    for name in prov:
        described = [l for l in lines if re.search(r"eqoa\.actions\.%s\b" % name, l)]
        if not any("*Provisional" in l or "*(provisional)*" in l for l in described):
            out.append(f"{name}: no line that describes it is marked provisional")
    for l in lines:
        if l.startswith("| `eqoa.actions.") and "*Provisional.*" in l:
            cells = re.split(r"(?<!\\)\|", l)
            names = re.findall(r"eqoa\.actions\.([a-z_0-9]+)", cells[1])
            wrong = [n for n in names if n not in prov]
            if wrong:
                out.append(f"{wrong}: marked provisional but proven")
    return out


class SpecMarks(unittest.TestCase):
    def setUp(self):
        self.doc = json.loads(T.TIERS.read_text(encoding="utf-8"))
        self.prov = provisional_actions(self.doc)
        self.spec = SPEC.read_text(encoding="utf-8")

    def test_every_provisional_action_is_marked_in_the_spec(self):
        self.assertEqual(spec_problems(self.spec, self.prov), [])

    def test_negative_control_unmarked_action_is_caught(self):
        stripped = self.spec.replace("*Provisional.* ", "")
        self.assertTrue(spec_problems(stripped, self.prov))

    def test_negative_control_a_proven_action_marked_is_caught(self):
        self.assertNotIn("cast", self.prov)
        bad = self.spec.replace("| `eqoa.actions.cast(ability)` | ", "| `eqoa.actions.cast(ability)` | *Provisional.* ", 1)
        self.assertNotEqual(bad, self.spec)
        self.assertTrue(spec_problems(bad, self.prov))

    def test_wording_has_no_em_dash_and_names_both_words(self):
        for page in ("VERSIONING.md", "PROVISIONAL.md"):
            text = (T.ROOT / "docs/modding" / page).read_text(encoding="utf-8")
            self.assertNotIn("—", text, page)
            self.assertIn("provisional", text.lower())
        # docs_tests.rs lets only the limits table say "tested": the spec's new paragraph must not
        para = next(l for l in self.spec.splitlines() if l.startswith("- **Proven and provisional.**"))
        self.assertNotRegex(para, r"\btested\b")
        self.assertIn("## Proven and provisional names",
                      (T.ROOT / "docs/modding/VERSIONING.md").read_text(encoding="utf-8"))


import api_tiers_flow as FL


class ProducerTables(unittest.TestCase):
    """The hand tables of api_tiers_flow stay in step with the frozen record and the Rust."""

    @classmethod
    def setUpClass(cls):
        cls.frozen = json.loads(T.FROZEN.read_text(encoding="utf-8"))["supported"]
        cls.names = set(cls.frozen)
        cls.nested = FL.load_nested()

    def test_every_producer_and_record_is_a_frozen_name(self):
        self.assertEqual([k for k in FL.RETURNS if k not in self.names], [])
        self.assertEqual([v for v in FL.RETURNS.values() if v not in self.names], [])
        for table in (FL.EVENT_SELF, FL.EVENT_BEFORE_AFTER):
            self.assertEqual([e for e in table if "eqoa.events." + e not in self.names], [])
            self.assertEqual([r for r in table.values() if r not in self.names], [])
        self.assertEqual([r for r in self.nested.values() if r not in self.names], [])

    def test_every_record_type_has_a_producer_or_a_reason(self):
        covered = (set(self.nested.values()) | set(FL.RETURNS.values()) | set(FL.EVENT_SELF.values())
                   | set(FL.EVENT_BEFORE_AFTER.values()) | {FL.ERROR, FL.ACTION_RETURN, FL.EVENT_RECORD}
                   | set(FL.NO_PRODUCER))
        records = [n for n in self.frozen if T.classify(n, self.names)[0] in ("record", "type")]
        self.assertGreater(len(records), 150)
        self.assertEqual([r for r in records if r not in covered], [])
        self.assertEqual([r for r in FL.NO_PRODUCER if r not in self.names], [])

    def test_negative_control_a_record_nobody_produces_is_caught(self):
        covered = set(self.nested.values()) | set(FL.RETURNS.values())
        self.assertIn("eqoa.camera.Camera", covered)
        self.assertNotIn("eqoa.nothing.Made", covered)
        self.assertNotIn("eqoa.ChatHistory", covered)

    def test_nested_types_match_the_rust_table(self):
        self.assertGreater(len(self.nested), 140)
        self.assertEqual(self.nested[("eqoa.items.Container", "merchant")], "eqoa.items.MerchantState")
        self.assertEqual(self.nested[("eqoa.windows.Window", "dialog")], "eqoa.windows.Dialog")
        self.assertEqual(self.nested[("_", "position")], "eqoa.Vec3")
        # every `Some("eqoa...")` the Rust's nested_type returns was read
        import re
        for name in ("native.rs", "native_a11.rs"):
            text = (FL.RUST_DIR / name).read_text(encoding="utf-8", errors="replace")
            start = text.index("fn nested_type")
            end = text.index("\n}\n", start)
            rust = set(re.findall(r'=> Some\("(eqoa\.[A-Za-z.]+)"\)', text[start:end]))
            self.assertTrue(rust)
            self.assertEqual(sorted(r for r in rust if r not in set(self.nested.values())), [], name)

    def test_negative_control_a_stale_pattern_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            for name in ("native.rs", "native_a11.rs"):
                (Path(tmp) / name).write_text("fn nested_type() { }\n", encoding="utf-8")
            with self.assertRaises(RuntimeError):
                FL.load_nested(tmp)


class Reports(unittest.TestCase):
    def test_report_writes_nothing_and_why_shows_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            tiers, md = Path(tmp) / "t.json", Path(tmp) / "p.md"
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                self.assertEqual(T.main(["--report", "--tiers", str(tiers), "--md", str(md)]), 0)
            self.assertFalse(tiers.exists() or md.exists())
            self.assertIn('"proven"', out.getvalue())
            # negative control: the plain run does write
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(T.main(["--tiers", str(tiers), "--md", str(md)]), 0)
            self.assertTrue(tiers.exists() and md.exists())

    def test_why_names_file_line_and_binding_for_each_kind(self):
        doc = json.loads(T.TIERS.read_text(encoding="utf-8"))
        provisional = set(doc["provisional"])
        frozen = json.loads(T.FROZEN.read_text(encoding="utf-8"))["supported"]
        names = set(frozen)

        def why(name):
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                T.main(["--why", name])
            return out.getvalue()

        for kind, pick in (("record_field", lambda n: T.classify(n, names)[0] == "record_field"),
                           ("event_field", lambda n: T.classify(n, names)[0] == "event_field"),
                           ("drawlist_method", lambda n: T.classify(n, names)[0] == "drawlist_method"),
                           ("type", lambda n: T.classify(n, names)[0] == "type"),
                           ("event", lambda n: T.classify(n, names)[0] == "event")):
            name = next(n for n in sorted(frozen) if pick(n) and n not in provisional)
            text = why(name)
            self.assertIn(": proven", text.splitlines()[0], name)
            self.assertRegex(text, r"\.lua:\d+", name)
        # a provisional name prints no evidence line
        self.assertEqual(len(why(next(iter(sorted(provisional)))).splitlines()), 1)


class StatusProse(unittest.TestCase):
    def test_every_prose_mention_of_a_provisional_action_is_marked(self):
        import re
        doc = json.loads(T.TIERS.read_text(encoding="utf-8"))
        prov = {n.split(".")[2] for n in doc["provisional"]
                if n.startswith("eqoa.actions.") and n.count(".") == 2 and n.split(".")[2][0].islower()}
        spec = SPEC.read_text(encoding="utf-8").replace("\r\n", "\n").split("\n")
        start = next(i for i, l in enumerate(spec) if l.startswith("## 8."))
        unmarked = []
        for i, line in enumerate(spec[start:], start):
            # lines about what a call needs (its capability) or the permission table name actions without
            # describing them; a line that describes one is marked
            if "capabilit" in line or "grant" in line or line.startswith(("4. ", "6. ")) or "alias" in line \
                    or "Mods & Addons" in line or line.startswith("| Guild Say"):
                continue
            for m in re.finditer(r"eqoa\.actions\.([a-z_0-9]+)", line):
                if m.group(1) in prov and "rovisional" not in line:
                    unmarked.append((i + 1, m.group(1)))
        self.assertEqual(unmarked, [])

    def test_negative_control_an_unmarked_mention_is_caught(self):
        import re
        line = "- `eqoa.actions.status` does too."
        self.assertTrue(re.search(r"eqoa\.actions\.status", line) and "rovisional" not in line)


if __name__ == "__main__":
    unittest.main()
