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

    def test_wrapper_called_from_another_file_of_the_mod(self):
        a = 'M = {}\nfunction M.listen(name, fn) eqoa.events.on(name, fn) end\n'
        b = 'M.listen("render", function() end)\n'
        _, tier = tiers_for({"m": {"a.lua": a, "b.lua": b}})
        self.assertEqual(tier["eqoa.events.render"], "proven")

    def test_loop_over_names(self):
        src = 'for _, name in ipairs({"world.entered", "render"}) do eqoa.events.on(name, function() end) end\n'
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.events.world.entered"], "proven")
        self.assertEqual(tier["eqoa.events.render"], "proven")

    def test_computed_name_is_not_guessed(self):
        src = 'local n = "combat" .. ".hit"\neqoa.events.on(n, function() end)\n'
        doc, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.events.combat.hit"], "provisional")

    def test_event_data_field_needs_event_and_a_read_in_the_subscribing_file(self):
        src = 'eqoa.events.on("combat.hit", function(e) local a = e.data.amount end)\n'
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.events.combat.hit.data.amount"], "proven")
        self.assertEqual(tier["eqoa.events.combat.hit.data.source"], "provisional")

    def test_event_data_field_read_without_a_subscription_is_not_proven(self):
        # a mod reads `.members` but never subscribes to raid.changed
        _, tier = tiers_for({"m": {"main.lua": "local m = x.members\n"}})
        self.assertEqual(tier["eqoa.events.raid.changed.data.members"], "provisional")

    def test_event_data_field_read_in_another_file_is_not_proven(self):
        a = 'eqoa.events.on("combat.hit", function(e) helper(e.data) end)\n'
        b = 'function helper(d) return d.amount end\n'
        _, tier = tiers_for({"m": {"a.lua": a, "b.lua": b}})
        self.assertEqual(tier["eqoa.events.combat.hit.data.amount"], "provisional")


class Records(unittest.TestCase):
    def test_field_needs_a_proven_function_of_the_module_and_a_read(self):
        src = "local i = eqoa.items.get(1)\nlocal n = i.name\n"
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["eqoa.items.Item.name"], "proven")
        self.assertEqual(tier["eqoa.items.Item.stack"], "provisional")
        self.assertEqual(tier["eqoa.items.Item"], "proven")

    def test_field_read_without_using_the_module_stays_provisional(self):
        _, tier = tiers_for({"m": {"main.lua": "local n = thing.size\neqoa.items.get(1)\n"}})
        self.assertEqual(tier["eqoa.raid.Raid.size"], "provisional")

    def test_negative_control_module_with_no_proven_function(self):
        # reading .size is not enough: no bundled mod calls anything of eqoa.raid
        _, tier = tiers_for({"m": {"main.lua": "local n = thing.size\n"}})
        self.assertEqual(tier["eqoa.raid.Raid.size"], "provisional")
        self.assertEqual(tier["eqoa.raid"], "provisional")

    def test_method_style_read_counts(self):
        _, tier = tiers_for({"m": {"main.lua": "eqoa.items.get(1)\nlocal ok = r:stack()\n"}})
        self.assertEqual(tier["eqoa.items.Item.stack"], "proven")

    def test_record_type_is_proven_when_any_field_is(self):
        _, tier = tiers_for({"m": {"main.lua": "eqoa.actions.cast(1)\nlocal ok = r.ok\n"}})
        self.assertEqual(tier["eqoa.actions.ActionResult.ok"], "proven")
        self.assertEqual(tier["eqoa.actions.ActionResult"], "proven")
        self.assertEqual(tier["eqoa.actions.ActionResult.detail"], "provisional")
        # a type with no fields is not shown to be used
        self.assertEqual(tier["eqoa.actions.RequestId"], "provisional")

    def test_container_is_proven_with_any_proven_child(self):
        _, tier = tiers_for({"m": {"main.lua": "eqoa.items.find(1)\n"}})
        self.assertEqual(tier["eqoa.items"], "proven")
        self.assertEqual(tier["eqoa.raid"], "provisional")

    def test_draw_list_methods_need_a_draw_list(self):
        src = "local dl = ImGui.GetWindowDrawList()\ndl:AddLine(1, 2)\n"
        _, tier = tiers_for({"m": {"main.lua": src}})
        self.assertEqual(tier["ImDrawList.AddLine"], "proven")
        self.assertEqual(tier["ImDrawList.AddRect"], "provisional")
        _, tier = tiers_for({"m": {"main.lua": "x:AddLine(1, 2)\n"}})
        self.assertEqual(tier["ImDrawList.AddLine"], "provisional")


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
        self.assertIn("## Proven and provisional names",
                      (T.ROOT / "docs/modding/VERSIONING.md").read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
