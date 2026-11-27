#!/usr/bin/env python3
"""Sort the frozen eqoa/1 names into proven and provisional (docs/modding/VERSIONING.md).

  python tools/api_tiers.py            write docs/modding/eqoa-1-tiers.json and docs/modding/PROVISIONAL.md
                                       (refuses to demote a name unless --allow-demote is given)
  python tools/api_tiers.py --check    fail when the committed files differ from a fresh run, or when a name
                                       would move from proven to provisional (unless --allow-demote)

A name is **proven** when a mod bundled with the client (bundled-mods/*) uses it, so it has been exercised in
real play. It is **provisional** when only automated tests cover it. The rule is deliberately conservative:
when the scan cannot show that a bundled mod uses a name, the name is provisional. Calling an untested name
"proven" is the mistake this scheme exists to avoid.

The FrontiersForge layer (frontiers_forge.*, UiForge.*, bit.*) is not part of the eqoa/1 promise; those names
get the tier value `compat` and are not counted as proven or provisional.

What the scan can see (a Lua tokenizer, not a Lua interpreter):
  * a chain such as `eqoa.items.find`, also through a local alias (`local items = eqoa.items` then
    `items.find(`, or `local m = eqoa.map.get` then `m()`). A local name counts as an alias only when every
    place the file binds that name (local, assignment, parameter, loop variable, table field) binds the same
    path; any other binding of the name in the file makes it ambiguous and it is not resolved;
  * events: `eqoa.events.on("name", ...)` (also through a local wrapper function that passes its first
    parameter on, and a `for _, name in ipairs({"a","b"}) do eqoa.events.on(name, ...)` loop);
  * fields: `.field`, `:field` and `["field"]` reads in the Lua files of the mod.
What it cannot see (so those names stay provisional): names built at run time (`eqoa.actions[name]`,
`"chat." .. kind`), values passed through several helper layers, and any name only mentioned in a comment.
"""
import argparse, json, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FROZEN = ROOT / "docs/modding/eqoa-1-frozen.json"
TIERS = ROOT / "docs/modding/eqoa-1-tiers.json"
PROVISIONAL_MD = ROOT / "docs/modding/PROVISIONAL.md"
BUNDLED = ROOT / "bundled-mods"

RULE = (
    "A name is proven when a bundled mod uses it (a call or reference, a subscribed event, a field it reads); "
    "every other frozen name is provisional; the FrontiersForge layer is compat; names not listed are proven."
)

COMPAT_ROOTS = ("frontiers_forge", "UiForge", "bit")
GLOBAL_ROOTS = ("ImGui", "ImDrawList", "ImGuiCond", "ImGuiWindowFlags")
EVENT_FUNCTIONS = {"on", "off", "after"}
# Capitalised names that are functions, not record types.
FUNCTION_MODULES = {"eqoa.imgui", "eqoa.WeaponSets"}
DRAWLIST_SOURCES = ("GetWindowDrawList", "GetBackgroundDrawList", "GetForegroundDrawList")

# ---------------------------------------------------------------------------------------------------------
# A small Lua tokenizer: (kind, text, line); kinds are name, str, num, op. Comments are dropped.

_LONG = re.compile(r"\[(=*)\[")
_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
_NUM = re.compile(r"0[xX][0-9a-fA-F.]+(?:[pP][+-]?\d+)?|\d+\.?\d*(?:[eE][+-]?\d+)?|\.\d+(?:[eE][+-]?\d+)?")
_OPS3 = {"..."}
_OPS2 = {"..", "==", "~=", "<=", ">=", "::", "//", "<<", ">>"}


def tokenize(src):
    toks, i, n, line = [], 0, len(src), 1
    while i < n:
        c = src[i]
        if c == "\n":
            line += 1
            i += 1
        elif c in " \t\r":
            i += 1
        elif src.startswith("--", i):
            m = _LONG.match(src, i + 2)
            if m:
                close = "]" + m.group(1) + "]"
                j = src.find(close, m.end())
                j = n if j < 0 else j + len(close)
                line += src.count("\n", i, j)
                i = j
            else:
                j = src.find("\n", i)
                i = n if j < 0 else j
        elif c in "\"'":
            j, buf = i + 1, []
            while j < n and src[j] != c and src[j] != "\n":
                if src[j] == "\\" and j + 1 < n:
                    buf.append(src[j + 1])
                    j += 2
                else:
                    buf.append(src[j])
                    j += 1
            toks.append(("str", "".join(buf), line))
            i = j + 1
        elif c == "[" and _LONG.match(src, i):
            m = _LONG.match(src, i)
            close = "]" + m.group(1) + "]"
            j = src.find(close, m.end())
            end = n if j < 0 else j + len(close)
            toks.append(("str", src[m.end():(n if j < 0 else j)], line))
            line += src.count("\n", i, end)
            i = end
        else:
            m = _NAME.match(src, i)
            if m:
                toks.append(("name", m.group(), line))
                i = m.end()
                continue
            m = _NUM.match(src, i)
            if m and (c.isdigit() or c == "."):
                toks.append(("num", m.group(), line))
                i = m.end()
                continue
            for op in (src[i:i + 3], src[i:i + 2]):
                if op in _OPS3 or op in _OPS2:
                    toks.append(("op", op, line))
                    i += len(op)
                    break
            else:
                toks.append(("op", c, line))
                i += 1
    return toks


OPENERS = {"function", "if", "do", "repeat"}
CLOSERS = {"end", "until"}
KEYWORDS = {
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in", "local",
    "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
}


def is_name(t):
    return t[0] == "name" and t[1] not in KEYWORDS


def block_end(toks, i):
    """Index of the `end`/`until` closing the block whose opener is at toks[i]."""
    depth = 0
    for j in range(i, len(toks)):
        k = toks[j]
        if k[0] != "name":
            continue
        if k[1] in OPENERS:
            depth += 1
        elif k[1] in CLOSERS:
            depth -= 1
            if depth == 0:
                return j
    return len(toks) - 1


def read_chain(toks, i):
    """A name chain `a.b.c` (also `a["b"]`) starting at toks[i]: (segments, index after the chain)."""
    segs = [toks[i][1]]
    j = i + 1
    while j < len(toks):
        if toks[j][1] == "." and toks[j][0] == "op" and j + 1 < len(toks) and toks[j + 1][0] == "name":
            segs.append(toks[j + 1][1])
            j += 2
        elif (toks[j][1] == "[" and toks[j][0] == "op" and j + 2 < len(toks) and toks[j + 1][0] == "str"
              and toks[j + 2][1] == "]" and toks[j + 2][0] == "op"):
            segs.append(toks[j + 1][1])
            j += 3
        else:
            break
    return segs, j


# ---------------------------------------------------------------------------------------------------------
# Binding sites and aliases (per file).

# After `local x = a.b.c` the chain is an alias only when the statement ends there.
_NOT_ALIAS_NEXT = {"(", "[", ":", "{", "or", "and", "+", "-", "*", "/", "%", "^", "..", "==", "~=", "<", ">",
                   "<=", ">=", "//", "<<", ">>", "#", "="}


def find_bindings(toks):
    """name -> list of bound paths (a tuple of segments) or None for a binding that is not a plain alias."""
    binds = {}

    def add(name, value):
        binds.setdefault(name, []).append(value)

    n = len(toks)
    i = 0
    while i < n:
        k = toks[i]
        if k == ("name", "local", k[2]) and i + 1 < n:
            if toks[i + 1][1] == "function" and i + 2 < n and is_name(toks[i + 2]):
                add(toks[i + 2][1], None)
                i += 1  # the `function` token is read next, so its parameters are bound too
                continue
            names, j = [], i + 1
            while j < n and is_name(toks[j]):
                names.append(toks[j][1])
                j += 1
                if j < n and toks[j][1] == "<":  # attribs: <const>
                    j += 3
                if j < n and toks[j][1] == ",":
                    j += 1
                else:
                    break
            if j < n and toks[j][1] == "=" and toks[j][0] == "op" and names:
                j += 1
                value = None
                if len(names) == 1 and j < n and is_name(toks[j]):
                    segs, after = read_chain(toks, j)
                    nxt = toks[after] if after < n else ("op", ";", 0)
                    ok = not (nxt[0] == "str" or (nxt[0] == "op" and nxt[1] in _NOT_ALIAS_NEXT)
                              or (nxt[0] == "name" and nxt[1] in ("or", "and")))
                    value = tuple(segs) if ok else None
                for name in names:
                    add(name, value)
                i = j
                continue
            for name in names:
                add(name, None)
            i = j
            continue
        if k[0] == "name" and k[1] == "function":
            j = i + 1
            if j < n and is_name(toks[j]) and not (j + 1 < n and toks[j + 1][1] in ".:"):
                add(toks[j][1], None)
            while j < n and toks[j][1] != "(":
                j += 1
            j += 1
            while j < n and toks[j][1] != ")":
                if is_name(toks[j]):
                    add(toks[j][1], None)
                j += 1
            i = j
            continue
        if k[0] == "name" and k[1] == "for":
            j = i + 1
            while j < n and toks[j][1] not in ("in", "="):
                if is_name(toks[j]):
                    add(toks[j][1], None)
                j += 1
            i = j
            continue
        # a plain `name = ...` (assignment or table field): the name is bound to something we do not track
        if (is_name(k) and i + 1 < n and toks[i + 1] [1] == "=" and toks[i + 1][0] == "op"
                and not (i > 0 and toks[i - 1][1] in (".", ":") and toks[i - 1][0] == "op")):
            add(k[1], None)
        i += 1
    return binds


def resolve_aliases(binds):
    """Alias name -> path tuple, for names whose every binding is the same path starting at eqoa or a global."""
    out = {}
    for name, values in binds.items():
        if not values or any(v is None for v in values) or len(set(values)) != 1:
            continue
        out[name] = values[0]
    # resolve chains of aliases (local a = eqoa.x ; local b = a.y)
    for _ in range(4):
        changed = False
        for name, path in list(out.items()):
            head = path[0]
            if head == name:
                del out[name]
                changed = True
            elif head in out:
                out[name] = out[head] + path[1:]
                changed = True
        if not changed:
            break
    return {k: v for k, v in out.items() if v[0] == "eqoa" or v[0] in GLOBAL_ROOTS or v[0] in COMPAT_ROOTS}


# ---------------------------------------------------------------------------------------------------------
# What one bundled mod uses.


class ModUse:
    def __init__(self, name):
        self.name = name
        self.paths = {}           # every chain referenced (full chains and their prefixes) -> (file, line)
        self.events = {}          # event name -> evidence: subscribed to (directly, in a loop, or via a wrapper)
        self.unresolved = []      # (file, line) of events.on calls whose name could not be read
        self.reads = {}           # ("rec", R, f) | ("ev", E, f) | ("dl", method) -> evidence (api_tiers_flow)
        self.received = {}        # record -> evidence: a value of that record was really received


def lua_files(mod_dir):
    return sorted(p for p in mod_dir.rglob("*.lua") if p.is_file())


def find_wrappers(toks, aliases):
    """Names of local functions that pass one of their parameters to eqoa.events.on as the event name:
    name -> index of that parameter."""
    out = {}
    n = len(toks)
    for i, t in enumerate(toks):
        if not (t[0] == "name" and t[1] == "function"):
            continue
        # the function's name: `local function f`, `function a.b.f`, `f = function`, `a.f = function`
        fname = None
        j = i + 1
        if j < n and is_name(toks[j]):
            segs, after = read_chain(toks, j)
            fname = segs[-1]
            params_at = after
        else:
            params_at = i + 1
            if i >= 2 and toks[i - 1][1] == "=" and is_name(toks[i - 2]):
                fname = toks[i - 2][1]
        if fname is None or params_at >= n or toks[params_at][1] != "(":
            continue
        params, k = [], params_at + 1
        while k < n and toks[k][1] != ")":
            if is_name(toks[k]):
                params.append(toks[k][1])
            k += 1
        end = block_end(toks, i)
        for m in range(k, end):
            if is_name(toks[m]) and (m == 0 or toks[m - 1][1] not in (".", ":")):
                segs, after = read_chain(toks, m)
                path = expand(segs, aliases)
                if path == ("eqoa", "events", "on") and after + 2 < n and toks[after][1] == "(" \
                        and is_name(toks[after + 1]) and toks[after + 1][1] in params:
                    out[fname] = params.index(toks[after + 1][1])
    return out


def expand(segs, aliases):
    head = segs[0]
    if head in aliases:
        return aliases[head] + tuple(segs[1:])
    return tuple(segs)


def loop_strings(toks, i, var):
    """Strings of `for _, var in ipairs({...}) do` for the nearest enclosing loop that binds `var`."""
    for j in range(i, -1, -1):
        if toks[j][0] == "name" and toks[j][1] == "for":
            k = j + 1
            names = []
            while k < len(toks) and toks[k][1] != "in" and toks[k][1] != "=" and toks[k][1] != "do":
                if is_name(toks[k]):
                    names.append(toks[k][1])
                k += 1
            if var not in names or k >= len(toks) or toks[k][1] != "in":
                continue
            if j + 1 < len(toks) and block_end(toks, _do_index(toks, j)) < i:
                continue  # that loop already closed before the call
            strings, depth = [], 0
            while k < len(toks) and toks[k][1] != "do":
                if toks[k][0] == "str" and depth > 0:
                    strings.append(toks[k][1])
                if toks[k][0] == "op" and toks[k][1] == "{":
                    depth += 1
                elif toks[k][0] == "op" and toks[k][1] == "}":
                    depth -= 1
                k += 1
            return strings
    return []


def _do_index(toks, for_index):
    k = for_index
    while k < len(toks) and toks[k][1] != "do":
        k += 1
    return k


def scan_file(path, mod):
    toks = tokenize(path.read_text(encoding="utf-8", errors="replace"))
    aliases = resolve_aliases(find_bindings(toks))
    n = len(toks)
    wrappers = find_wrappers(toks, aliases)
    i = 0
    while i < n:
        t = toks[i]
        if not is_name(t) or (i > 0 and toks[i - 1][0] == "op" and toks[i - 1][1] in (".", ":")):
            i += 1
            continue
        segs, after = read_chain(toks, i)
        full = expand(segs, aliases)
        if full[0] == "eqoa" or full[0] in GLOBAL_ROOTS or full[0] in COMPAT_ROOTS:
            for k in range(1, len(full) + 1):
                mod.paths.setdefault(".".join(full[:k]), (path, t[2]))
        if full == ("eqoa", "events", "on") and after < n and toks[after][1] == "(":
            arg = toks[after + 1] if after + 1 < n else None
            if arg is not None and arg[0] == "str":
                mod.events.setdefault(arg[1], f"{path.name}:{arg[2]}")
            elif arg is not None and is_name(arg):
                names = loop_strings(toks, i, arg[1])
                if names:
                    for name in names:
                        mod.events.setdefault(name, f"{path.name}:{arg[2]}: in a loop over names")
                elif arg[1] not in {p for p in _params_of_enclosing(toks, i)}:
                    mod.unresolved.append((path, arg[2]))
            else:
                mod.unresolved.append((path, t[2]))
        i = max(after, i + 1)
    return toks, wrappers


def _params_of_enclosing(toks, i):
    """Parameters of the innermost function whose body contains toks[i]."""
    best = None
    for j in range(i, -1, -1):
        if toks[j][0] == "name" and toks[j][1] == "function":
            end = block_end(toks, j)
            if end >= i:
                k = j + 1
                while k < len(toks) and toks[k][1] != "(":
                    k += 1
                params = []
                k += 1
                while k < len(toks) and toks[k][1] != ")":
                    if is_name(toks[k]):
                        params.append(toks[k][1])
                    k += 1
                best = params
                break
    return best or []


def scan_mod(mod_dir, frozen, nested):
    import api_tiers_flow as flow
    mod = ModUse(mod_dir.name)
    files, wrappers = [], {}
    for f in lua_files(mod_dir):
        toks, wrap = scan_file(f, mod)
        files.append((f, toks))
        wrappers[f] = wrap
    stems = {}
    for f, _ in files:
        stems.setdefault(f.stem, []).append(f)
    stems = {stem: paths[0] for stem, paths in stems.items() if len(paths) == 1}
    result = flow.analyze_mod(files, frozen, nested, wrappers, stems)
    mod.reads = result.reads
    mod.received = result.received
    for event, why in result.events.items():
        mod.events.setdefault(event, why)
    return mod


def scan_bundled(frozen, bundled=BUNDLED):
    import api_tiers_flow as flow
    nested = flow.load_nested()
    return [scan_mod(d, set(frozen), nested) for d in sorted(p for p in bundled.iterdir() if p.is_dir())]


# ---------------------------------------------------------------------------------------------------------
# Classification of the frozen names and the tiers.


def classify(name, names):
    """(kind, anchor): anchor is the owning event / record / module name for the kinds that need one."""
    p = name.split(".")
    if p[0] in COMPAT_ROOTS:
        return "compat", None
    if p[0] in GLOBAL_ROOTS:
        return ("drawlist_method" if p[0] == "ImDrawList" else "function"), None
    if p[0] != "eqoa":
        return "function", None
    if len(p) <= 2:
        # eqoa.api, eqoa.revision are values; eqoa.<module> and eqoa.<Record> are containers
        if len(p) == 2 and p[1] in ("api", "revision"):
            return "function", None
        return ("record" if len(p) == 2 and p[1] in ("Color", "Vec2", "Vec3", "Error", "ChatHistory") else "module"), None
    if p[1] == "events":
        if len(p) == 3 and p[2] in EVENT_FUNCTIONS:
            return "function", None
        if p[2] == "Event":
            return ("record" if len(p) == 3 else "record_field"), ("eqoa.events", "eqoa.events.Event")
        if "data" in p[3:] or "data" in p[2:]:
            at = p.index("data", 2)
            return "event_field", (".".join(p[:at]), ".".join(p[at + 1:]))
        return "event", None
    if p[1] in ("Color", "Vec2", "Vec3", "Error", "ChatHistory"):
        return ("record_field", ("eqoa", ".".join(p[:2])))
    module = ".".join(p[:2])
    rest = p[2:]
    if module in FUNCTION_MODULES:
        return "function", None
    if rest[0][:1].isupper():
        if len(rest) == 1:
            kids = any(n.startswith(name + ".") for n in names)
            return ("record" if kids else "type"), (module, name)
        return "record_field", (module, ".".join(p[:3]))
    if len(rest) == 1:
        return "function", None
    return "constant", None


class Tiers:
    def __init__(self, frozen_names, mods):
        self.names = sorted(frozen_names)
        self.nameset = set(frozen_names)
        self.mods = mods
        self.kind = {n: classify(n, self.nameset) for n in self.names}
        self.tier = {}

    def run(self):
        names, nameset, kind = self.names, self.nameset, self.kind
        direct = set()
        for m in self.mods:
            direct |= set(m.paths)
        proven = set()
        # 1 functions, constants, values: referenced by path
        for n in names:
            k = kind[n][0]
            if k in ("function", "constant") and n in direct:
                proven.add(n)
        # 2 events: subscribed to
        for n in names:
            if kind[n][0] == "event":
                ev = n[len("eqoa.events."):]
                if any(ev in m.events for m in self.mods):
                    proven.add(n)
        # 3 event data fields: the event is proven and a handler of THAT event reads the field from its
        #   data (api_tiers_flow: the handler's own parameter, or a function it passes the data to)
        for n in names:
            if kind[n][0] == "event_field":
                ev_name, field = kind[n][1]
                ev = ev_name[len("eqoa.events."):]
                if ev_name in proven and any(("ev", ev, field) in m.reads for m in self.mods):
                    proven.add(n)
        # 4 record fields: read from a value of that record that a mod really received
        for n in names:
            if kind[n][0] == "record_field":
                record, field = n.rsplit(".", 1)
                if any(("rec", record, field) in m.reads for m in self.mods):
                    proven.add(n)
        # 5 record types: a type with fields is proven when one of its fields is (receiving a value and only
        #   testing it for nil proves nothing); a type with no fields (RequestId, BatchId) is proven when a mod
        #   really received a value of it from a producer call
        for n in names:
            if kind[n][0] == "record":
                if any(x in proven for x in names if x.startswith(n + ".")):
                    proven.add(n)
            elif kind[n][0] == "type":
                if any(n in m.received for m in self.mods):
                    proven.add(n)
        # 6 ImDrawList methods: a `:Method(` call on a value bound from a draw-list getter
        for n in names:
            if kind[n][0] == "drawlist_method":
                if any(("dl", n.split(".")[-1]) in m.reads for m in self.mods):
                    proven.add(n)
        # 7 modules and containers: proven when anything below is proven
        for n in names:
            if kind[n][0] == "module":
                if any(x in proven for x in names if x.startswith(n + ".")):
                    proven.add(n)
        for n in names:
            k = kind[n][0]
            self.tier[n] = "compat" if k == "compat" else ("proven" if n in proven else "provisional")
        return self


def summarize(t):
    by = {}
    for n in t.names:
        k = t.kind[n][0]
        group = {"function": "function", "constant": "function", "module": "module", "event": "event",
                 "event_field": "field", "record_field": "field", "drawlist_method": "function",
                 "record": "type", "type": "type", "compat": "compat"}[k]
        if n.startswith("eqoa.actions.") and k == "function":
            group = "action"
        by.setdefault(group, {"proven": 0, "provisional": 0, "compat": 0})
        by[group][t.tier[n]] += 1
    total = {"proven": 0, "provisional": 0, "compat": 0}
    for n in t.names:
        total[t.tier[n]] += 1
    return total, by


def build(frozen_path=FROZEN, bundled=BUNDLED):
    frozen = json.loads(Path(frozen_path).read_text(encoding="utf-8"))
    t = Tiers(frozen["supported"], scan_bundled(frozen["supported"], bundled)).run()
    total, by = summarize(t)
    doc = {
        "api": "eqoa/1",
        "schema_version": 1,
        "rule": RULE,
        "counts": {"total": len(t.names), **total, "by_kind": {k: by[k] for k in sorted(by)}},
        "provisional": [n for n in t.names if t.tier[n] == "provisional"],
        "compat": [n for n in t.names if t.tier[n] == "compat"],
    }
    return doc, t


def render_json(doc):
    return json.dumps(doc, indent=1, sort_keys=False) + "\n"


def render_md(doc, t):
    prov = set(doc["provisional"])
    c = doc["counts"]
    lines = [
        "# Provisional `eqoa/1` names",
        "",
        "<!-- Generated by tools/api_tiers.py from docs/modding/eqoa-1-tiers.json. Do not edit by hand. -->",
        "",
        f"{c['provisional']} of the {c['proven'] + c['provisional']} `eqoa/1` names are provisional; "
        f"{c['proven']} are proven. (The FrontiersForge layer, {c['compat']} names, is neither: see "
        "[VERSIONING.md](VERSIONING.md).)",
        "",
        "A provisional name works and is covered by the client's automated tests, but no mod shipped with the "
        "client uses it yet, so it has not been exercised in real play. After the final freeze a provisional "
        "name may still be corrected, and every correction is listed in the release notes. When a mod "
        "uses a provisional name in real play it becomes proven and gets the full promise of "
        "[VERSIONING.md](VERSIONING.md); a name never goes back from proven to provisional.",
        "",
    ]
    groups = {}
    for n in doc["provisional"]:
        p = n.split(".")
        key = p[1] if p[0] == "eqoa" and len(p) > 1 else p[0]
        groups.setdefault(key, []).append(n)
    for key in sorted(groups, key=str.lower):
        names = groups[key]
        lines.append(f"## {key}")
        lines.append("")
        children = {}
        tops = []
        for n in names:
            parent = n.rsplit(".", 1)[0]
            kind = t.kind[n][0]
            if kind in ("record_field", "event_field") and parent in prov:
                children.setdefault(parent, []).append(n.rsplit(".", 1)[1])
            elif kind == "event_field":
                # the event is proven: show the field with its event
                children.setdefault("~" + parent.rsplit(".data", 1)[0] + ".data", []).append(
                    n.split(".data.", 1)[1])
            elif kind == "record_field":
                children.setdefault("~" + parent, []).append(n.rsplit(".", 1)[1])
            else:
                tops.append(n)
        for n in tops:
            extra = children.pop(n, None)
            if extra:
                lines.append(f"- `{n}` (fields: {', '.join('`' + e + '`' for e in extra)})")
            else:
                lines.append(f"- `{n}`")
        for parent in sorted(children):
            shown = parent.lstrip("~")
            what = "data fields" if shown.endswith(".data") else "fields"
            lines.append(f"- `{shown}` is proven; {what} no bundled mod reads yet: "
                         f"{', '.join('`' + e + '`' for e in children[parent])}")
        lines.append("")
    return "\n".join(lines).rstrip("\n") + "\n"


def evidence(t, name):
    """Lines saying where a bundled mod uses `name` (empty when none does): the mod, file:line and how the
    value was bound (a producer call, a handler parameter, a parameter its callers fill, a returned value)."""
    out = []
    kind = t.kind.get(name, (None, None))
    k = kind[0]
    for m in t.mods:
        if k in ("function", "constant"):
            hit = m.paths.get(name)
            if hit:
                out.append(f"{m.name}: {hit[0].name}:{hit[1]}: referenced by path")
        elif k == "event":
            ev = name[len("eqoa.events."):]
            if ev in m.events:
                out.append(f"{m.name}: {m.events[ev]}: subscribed")
        elif k == "event_field":
            ev = kind[1][0][len("eqoa.events."):]
            if ("ev", ev, kind[1][1]) in m.reads:
                out.append(f"{m.name}: {m.reads[('ev', ev, kind[1][1])]}")
        elif k == "record_field":
            record, field = name.rsplit(".", 1)
            if ("rec", record, field) in m.reads:
                out.append(f"{m.name}: {m.reads[('rec', record, field)]}")
        elif k == "drawlist_method":
            key = ("dl", name.split(".")[-1])
            if key in m.reads:
                out.append(f"{m.name}: {m.reads[key]}")
        elif k == "type":
            if name in m.received:
                out.append(f"{m.name}: received: {m.received[name]}")
        elif k == "record":
            prefix = name + "."
            for key, why in m.reads.items():
                if key[0] == "rec" and f"{key[1]}.{key[2]}".startswith(prefix) and key[1] == name:
                    out.append(f"{m.name}: field {key[2]}: {why}")
                    break
    if k == "module" and t.tier.get(name) == "proven":
        out.append("a name below it is proven")
    return out


def demotions(old, new):
    """Names the committed file counts as proven (not listed) that a fresh run lists as provisional."""
    old_prov = set(old.get("provisional", []))
    old_compat = set(old.get("compat", []))
    return sorted(n for n in new["provisional"] if n not in old_prov and n not in old_compat)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--check", action="store_true", help="compare with the committed files, write nothing")
    ap.add_argument("--allow-demote", action="store_true",
                    help="let a name move from proven to provisional (a deliberate change, e.g. a bundled mod "
                         "dropped a call, or the frozen record grew); without it both modes refuse")
    ap.add_argument("--frozen", default=str(FROZEN))
    ap.add_argument("--bundled", default=str(BUNDLED))
    ap.add_argument("--tiers", default=str(TIERS))
    ap.add_argument("--md", default=str(PROVISIONAL_MD))
    ap.add_argument("--why", metavar="NAME", help="print where a bundled mod uses NAME (or that none does)")
    ap.add_argument("--report", action="store_true", help="print the counts and the unresolved subscriptions")
    args = ap.parse_args(argv)
    doc, t = build(args.frozen, Path(args.bundled))
    text, md = render_json(doc), render_md(doc, t)
    if args.why:
        print(f"{args.why}: {t.tier.get(args.why, 'not in the frozen record')}")
        for line in evidence(t, args.why):
            print("  " + line)
        return 0
    if args.report:
        print(json.dumps(doc["counts"], indent=1))
        for m in t.mods:
            for path, line in m.unresolved:
                print(f"unresolved events.on: {path.relative_to(ROOT) if ROOT in path.parents else path}:{line}")
        return 0
    if args.check:
        tiers_path, md_path = Path(args.tiers), Path(args.md)
        problems = []
        committed = json.loads(tiers_path.read_text(encoding="utf-8")) if tiers_path.exists() else None
        if committed is None:
            problems.append(f"{tiers_path.name} is missing; run python tools/api_tiers.py")
        else:
            lost = demotions(committed, doc)
            gained = sorted(set(committed["provisional"]) - set(doc["provisional"]))
            if lost and not args.allow_demote:
                problems.append("these names would move from proven to provisional (a bundled mod stopped using "
                                "them, or they are new to the frozen record); deliberate? rerun with "
                                "python tools/api_tiers.py and review:\n  " + "\n  ".join(lost[:40])
                                + (f"\n  ... {len(lost) - 40} more" if len(lost) > 40 else ""))
            if tiers_path.read_text(encoding="utf-8") != text:
                if gained:
                    problems.append(f"{len(gained)} provisional names are now used by a bundled mod "
                                    "(they graduate to proven):\n  " + "\n  ".join(gained[:40]))
                problems.append(f"{tiers_path.name} differs from a fresh run; run python tools/api_tiers.py "
                                "and commit the result")
        if not md_path.exists() or md_path.read_text(encoding="utf-8") != md:
            problems.append(f"{md_path.name} differs from a fresh run; run python tools/api_tiers.py")
        if problems:
            print("api_tiers --check failed:", file=sys.stderr)
            for p in problems:
                print(" - " + p, file=sys.stderr)
            return 1
        print(f"api_tiers: OK ({doc['counts']['proven']} proven, {doc['counts']['provisional']} provisional, "
              f"{doc['counts']['compat']} compat)")
        return 0
    if Path(args.tiers).exists() and not args.allow_demote:
        lost = demotions(json.loads(Path(args.tiers).read_text(encoding="utf-8")), doc)
        if lost:
            print("refusing to write: these names would move from proven to provisional (a bundled mod stopped "
                  "using them, or they are new to the frozen record). If that is deliberate, rerun with "
                  "--allow-demote:\n  " + "\n  ".join(lost[:40]), file=sys.stderr)
            return 1
    Path(args.tiers).write_text(text, encoding="utf-8", newline="\n")
    Path(args.md).write_text(md, encoding="utf-8", newline="\n")
    print(f"wrote {args.tiers} and {args.md}: {doc['counts']['proven']} proven, "
          f"{doc['counts']['provisional']} provisional, {doc['counts']['compat']} compat")
    return 0


if __name__ == "__main__":
    sys.exit(main())
