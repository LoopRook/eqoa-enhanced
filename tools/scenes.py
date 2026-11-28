# SPDX-License-Identifier: AGPL-3.0-or-later
"""The scene definitions for tools/scene_suite.py. Times are ms from the client's start (the world is up by ~5 s).

Coordinates are screenshot pixels at EQOA_OFFSCREEN=1280x720 (the HUD's 640x448 picture, scaled 1.607 and
centred). Mod windows are checked by what the client logs where the mod or the game says anything, and by
what is on the screen (`ss.dark_fraction`) where nothing is logged.
"""

from contextlib import closing
import sqlite3

import local_scene as ls
import scene_suite as ss

BAG_BOX = (415, 45, 870, 395)       # the Bags window's slot area
DIALOG_BOX = (380, 70, 900, 225)    # where the Dialog Window mod draws
BAG_KEY = "mod.eqoa_bag_viewer.open=b" + chr(10)   # the scratch profile binds B to the Bag Viewer
START_BOX =(440, 150, 840, 330)    # centre of the screen, where the Start Menu window sits
NPC = "Commander Nothard"           # a town NPC with a "Done" dialog, in view of the Freeport start spot
MERCHANT = "Heirloom Outfitter"     # a merchant 1.5 units from the start spot
SLOTS = [(440 + 57 * i, 135) for i in range(8)]   # the Bags grid's first row, in screenshot pixels

# (label, Escape presses to get back to play). This server's menu: a TEST sub-menu first, an Auction sub-menu between
# Ability and Pet. A sub-menu takes two Escapes (up one menu, then close); so does an entry that opens the game's own
# page (Ability, Quest; Options needs three: its page, then the menu twice); an entry that opens a mod window (Inventory) takes one.
START_ENTRIES = [("TEST", 2), ("Community", 2), ("Character", 2), ("Inventory", 1), ("Ability", 2), ("Auction", 2),
                 ("Pet", 2), ("Quest", 2), ("Support", 2), ("Options", 3)]


def addon_line(mod, text):
    return rf"addon {mod}: \[(?:info|warn|debug)\] {text}"


def dialog_scene():
    return ss.Scene(
        "1", "Hail an NPC: Enter presses Done, no chat line; Enter then opens chat and text arrives",
        mods=("eqoa_dialogs",),
        env={
            "EQOA_TARGET": f"{NPC}@8500", "EQOA_PLAY_KEYS": "q@10000",
            # Enter = Done. The NPC then asks "Will you help?": 2 = "Not now"; it answers, and Enter = Done again. Only then Enter opens chat.
            "EQOA_MOD_KEYS": "enter@14000;2@16500;enter@18500;enter@21000;text /s hello there@21500;enter@23500",
        },
        quit_after=29,
        shots="12500=1-dialog.png,15500=2-after-done.png,20000=3-closed.png,22800=4-typing.png,25500=5-sent.png",
        expect=[
            ("NPC targeted", rf"script: target {NPC}: true"),
            ("the first dialog is the mod's", r"takeover: text_dialog session started for the mod"),
            ("Dialog Window shows it", addon_line("eqoa_dialogs", r"Dialog window: a text_dialog opened")),
            ("Enter pressed Done: the server's next dialog (its answer) arrives", r"takeover: response_dialog session started for the mod"),
            ("text typed after the dialog reached the chat line", r"chat: Lsgm shouts: hello there"),
        ],
        control_patch={"eqoa_dialogs": [
            ('        keys["enter"] = function() end' + chr(10), ""),
            ('        keys["enter"], keys["1"] = run, run', '        keys["1"] = run'),
        ]},
        extra=lambda run_dir, log: [
            ("the dialog window was up", ss.window_up(run_dir / "1-dialog.png", DIALOG_BOX, 0.5)),
            ("the dialog is gone after the answer", not ss.window_up(run_dir / "3-closed.png", DIALOG_BOX, 0.3)),
        ],
    )


def start_menu_scene():
    steps, shots, t = [], ["7000=0-baseline.png"], 8000
    for i, (name, escapes) in enumerate(START_ENTRIES):
        slug = f"{i + 1:02d}-{name.lower()}"
        steps.append(f"esc@{t}")                                   # open the menu
        steps += [f"down@{t + 500 + 150 * k}" for k in range(i)]
        enter = t + 800 + 150 * i
        steps.append(f"enter@{enter}")
        shots.append(f"{enter + 1000}={slug}.png")
        steps += [f"esc@{enter + 1500 + 400 * k}" for k in range(escapes)]
        again = enter + 1500 + 400 * escapes + 600
        shots.append(f"{again - 300}={slug}-back.png")             # after Escape the menu / window must be gone
        steps.append(f"b@{again}")                                 # the player can go on: Bag Viewer opens
        shots.append(f"{again + 700}={slug}-then-bags.png")
        steps.append(f"b@{again + 1200}")
        t = again + 2200
    return ss.Scene(
        "2", "Start Menu: every entry opens something, Escape returns, another window still opens",
        mods=("eqoa_start_menu", "eqoa_bag_viewer", "eqoa_character"),
        mod_keys_file=BAG_KEY,
        env={"EQOA_MOD_KEYS": ";".join(steps)},
        quit_after=t // 1000 + 4,
        shots=",".join(shots),
        absent=[("the Start Menu failing to take its key", r"addon eqoa_start_menu: \[warn\]")],
        extra=start_menu_checks,
        # Escape at the top level no longer closes the menu: the player is stuck in it
        control_patch={"eqoa_start_menu": [("    else" + chr(10) + "        close()" + chr(10) + "    end" + chr(10) + "end" + chr(10) + chr(10) + "local function step",
                                            "    else" + chr(10) + "    end" + chr(10) + "end" + chr(10) + chr(10) + "local function step")]},
    )


def start_menu_checks(run_dir, log):
    base = run_dir / "0-baseline.png"
    out = []
    for i, (name, _) in enumerate(START_ENTRIES):
        slug = f"{i + 1:02d}-{name.lower()}"
        diff = ss.region_diff(base, run_dir / f"{slug}.png", START_BOX)
        out.append((f"{name}: something opened (centre of the screen changed, mean {diff if diff is None else round(diff, 1)})",
                    diff is not None and diff > 4.0))
        gone = ss.region_diff(base, run_dir / f"{slug}-back.png", START_BOX)
        out.append((f"{name}: Escape took the menu or window away (centre back to the world, change {gone if gone is None else round(gone, 1)})",
                    gone is not None and gone < 12.0))   # world noise is under ~6, a window left up is ~47
        out.append((f"{name}: after Escape the Bag Viewer still opens", ss.window_up(run_dir / f"{slug}-then-bags.png", BAG_BOX)))
    return out


def chat_scene():
    return ss.Scene(
        "3", "Chat: Enter, type, send; /g then type; Shift+T opens a tell",
        mods=("eqoa_chat",),
        env={
            "EQOA_MOD_KEYS": ";".join([
                "enter@8000", "text hello chat@8500", "enter@10000",
                "enter@12000", "text /g @12500", "text hello group@13200", "enter@15000",
                "t@17000", "text Lsgm hi tell@17500", "enter@19500",
            ]),
            "EQOA_MOUSE": "shift down@16800;shift up@17200",
        },
        quit_after=24,
        shots="9000=1-say-typing.png,12800=2-group-typing.png,18800=3-tell-typing.png,21500=4-after.png",
        expect=[
            ("Enter, type, send: the server says it", r"chat: Lsgm: hello chat"),
            ("/g then type: the line went to the group channel (the server's answer)", r"chat: You are not in a group\."),
            ("Shift+T then type: the tell came back to the character", r"chat: Lsgm tells you, 'hi tell'"),
            ("and it went out as a tell", r"chat: You told Lsgm, 'hi tell'"),
        ],
        absent=[
            ("the /g command typed as ordinary speech", r"^chat: Lsgm: /g"),
            ("the group line said aloud", r"^chat: Lsgm: hello group"),
            ("the tell said aloud as speech", r"^chat: Lsgm: Lsgm hi tell"),
        ],
        # the T key's "/tell " opens a Say line instead of a Tell line: what Shift+T must not do
        control_patch={"eqoa_chat": [('then state.channel, t = "tell", ""', 'then state.channel, t = "say", ""')]},
    )


CHAR_BOX = (390, 125, 893, 565)    # the Character window
MENU_BOX = (525, 115, 860, 385)    # where a Bag item's menu opens


def overlap_scene():
    return ss.Scene(
        "4", "Bag Viewer over Character: a click on the overlap reaches only the front window",
        mods=("eqoa_bag_viewer", "eqoa_character"),
        mod_keys_file=BAG_KEY,
        env={
            "EQOA_MOUSE": "shift down@8800;shift up@9300;click 620 159@13500",   # a Bag item that is also on Character's Factions tab
            # Shift+C opens Character, B opens Bags over it; B closes the Bags (the front window) and its item menu
            "EQOA_MOD_KEYS": "c@9000;b@11000;b@15500",
        },
        quit_after=21,
        shots="10500=1-character-alone.png,12500=2-bags-over-it.png,14500=3-clicked-overlap.png,18000=4-bags-closed.png",
        extra=overlap_checks,
        control_patch={"eqoa_bag_viewer": [("eqoa.input.claim_pointer(", "(function() end)(")]},
    )


def overlap_checks(run_dir, log):
    alone, over, clicked, closed = (run_dir / n for n in
                                    ("1-character-alone.png", "2-bags-over-it.png", "3-clicked-overlap.png", "4-bags-closed.png"))
    menu = ss.region_diff(over, clicked, MENU_BOX)
    back = ss.region_diff(alone, closed, CHAR_BOX)
    return [
        ("both windows were up (Bags over Character)", ss.window_up(over, BAG_BOX)),
        (f"the click reached the front window (Bags item menu opened, change {menu if menu is None else round(menu, 1)})",
         menu is not None and menu > 5.0),
        (f"the window behind did not react (Character still on Summary, change {back if back is None else round(back, 1)})",
         back is not None and back < 4.0),
    ]


SELECT_3 = "ctrl down@12000;click 440 135@12300;click 497 135@12600;click 554 135@12900;ctrl up@13200;rclick 440 135@14000"
GROUP_MENU = {"sell": (515, 213), "destroy": (520, 258)}


def sell_scene(suite):
    x, y = GROUP_MENU["sell"]
    return ss.Scene(
        "5a", "Bag Viewer: Ctrl+click three items, group Sell at a merchant",
        mods=("eqoa_bag_viewer",), mod_keys_file=BAG_KEY, fresh_db=True,
        env={"EQOA_MOD_KEYS": "b@10000;right@16500;enter@17200", "EQOA_MOUSE": f"{SELECT_3};click {x} {y}@15000"},   # the client's own question: Right to Yes, Enter
        control_env={"EQOA_MOD_KEYS": "b@10000;enter@17200"},   # Enter on the default No: nothing may be sold
        quit_after=24,
        shots="12500=1-three-selected.png,16000=2-confirm.png,19500=3-after.png",
        expect=[
            ("the merchant's reach was used (a hidden merchant session opened for the mod)", r"takeover: mod talk -> npc 0x[0-9a-f]+, .*"),
            ("the group sale ran and said so", addon_line("eqoa_bag_viewer", r"bag multi-select: Sold 3 of 3")),
            ("the merchant session was closed again", r"takeover: merchant hidden session closed \((the mod's action or batch finished|released by the mod)\)"),
        ],
        extra=lambda run_dir, log: [("the character kept 1 of its 4 items (3 sold)", suite.inventory()[0] == 1)],
        notes=("worn items are untouched: not tested, the seed character's four heirloom items cannot be worn",),
    )


def destroy_scene(suite):
    x, y = GROUP_MENU["destroy"]
    return ss.Scene(
        "5b", "Bag Viewer: Ctrl+click three items, group Destroy with the Yes",
        mods=("eqoa_bag_viewer",), mod_keys_file=BAG_KEY, fresh_db=True,
        env={"EQOA_MOD_KEYS": "b@10000;right@16500;enter@17200", "EQOA_MOUSE": f"{SELECT_3};click {x} {y}@15000"},
        control_env={"EQOA_MOD_KEYS": "b@10000;enter@17200"},   # Enter on the default No: nothing may be destroyed
        quit_after=25,
        shots="12500=1-three-selected.png,16000=2-the-question.png,19500=3-after.png",
        expect=[
            ("the game's own question was answered Yes and the three were destroyed", addon_line("eqoa_bag_viewer", r"bag multi-select: Destroyed 3 of 3")),
        ],
        extra=lambda run_dir, log: [("the character kept 1 of its 4 items (3 destroyed)", suite.inventory()[0] == 1)],
        notes=("worn items are untouched: not tested, the seed character's four heirloom items cannot be worn",),
    )


CHAT_BOX = (166, 470, 662, 668)    # where the Chat mod's window starts, bottom left
CHAT_GRIP = (410, 481)             # the thin strip at its top that moves it


def drag_script(grip):
    steps = [f"click {grip[0]} {grip[1]}@9500"]                    # the cursor goes to the grip
    t = 10000
    for dx, dy in ((-80, -120), (200, 60), (-150, 90), (90, -200), (-100, 150), (60, -40)):
        steps += [f"press l@{t}", f"move {dx} {dy}@{t + 150}", f"move {dx // 2} {dy // 2}@{t + 300}", f"release l@{t + 450}"]
        t += 1500
    return ";".join(steps), t


def chat_moved(run_dir):
    """The Chat mod keeps its window's place in the profile (x defaults to 4, y to the bottom): moved means it differs."""
    import json
    try:
        profile = json.loads((run_dir / "config" / "addons" / "profiles" / "Default.json").read_text(encoding="utf-8"))
        saved = dict(profile["addons"][ss.addon_key("eqoa_chat")]["table"])
    except (OSError, KeyError, TypeError, ValueError):
        return False
    return saved.get("x", 4) != 4 or "y" in saved


def drag_scene():
    script, t = drag_script(CHAT_GRIP)
    return ss.Scene(
        "6", "Drag the chat window several times: no stall line in the client log",
        mods=("eqoa_chat",),
        env={"EQOA_MOUSE": script},
        control_env={"EQOA_MOUSE": drag_script((300, 600))[0]},     # presses inside the window body, not on its grip
        quit_after=t // 1000 + 4,
        shots=f"9000=1-before.png,{t + 500}=2-after.png",
        absent=[("a stall line (a frame of a second or more)", r"^stall: ")],
        extra=lambda run_dir, log: [
            ("the window really moved (its saved place is no longer the default)", chat_moved(run_dir)),
            ("and its old place on the screen changed", (ss.region_diff(run_dir / "1-before.png", run_dir / "2-after.png", CHAT_BOX) or 0) > 6.0),
            ("the six drags reached the game as pointer input", len(__import__("re").findall(r"^script: mouse click Left at", log, __import__("re").M)) >= 1),
        ],
    )


def perf_scene():
    return ss.Scene(
        "7", "Fixed-spot 60 s frame-rate run",
        mods=("eqoa_chat", "eqoa_bag_viewer", "eqoa_start_menu", "eqoa_dialogs"),
        env={"EQOA_PERF": "1"},
        records_perf=True,
        quit_after=72,
        shots="30000=1-perf.png",
        expect=[],
    )


# --- scene 8: quest hand-ins with the Dialog Window mod -------------------------------------------------------------
# A fixture quest is written into the DISPOSABLE database copy only: "Bracer Test", given by Commander Nothard, one
# give step that takes a militia bracer (item 8570, which `#giveitem` hands the GM). The server offers the step's
# answer as an option only when the bracer is held; the step's journal line is the quest's title ("Bracer Test") because
# this server's journal shows the step's line, and the mod matches an option that names a journal entry.
BRACER = 8570
HANDIN_BOX = (810, 128, 900, 152)       # the Hand in button beside the first option
HANDIN_BUTTON = (855, 140)
FINISHED = 'Quests: Lsgm finished "Bracer Test"'


def handin_seed(*answers):
    lines = chr(10).join(answers)

    def seed(db):
        db.execute("INSERT INTO quests (questid,name,enabled,giver,source) VALUES (990001,'Bracer Test',1,'Commander Nothard','scene-suite')")
        db.execute("INSERT INTO queststeps (questid,step,kind,target,items,count,journal,answer,says,waiting,reminder) "
                   "VALUES (990001,1,'give','Commander Nothard',?,1,'Bracer Test',?,'Well done. Keep it.','Have you the bracer?','Bring me the bracer.')",
                   (str(BRACER), lines))
        db.execute("INSERT INTO charquests (serverid,questid,step,count,done,slot,updated,picks) VALUES (2,990001,1,0,0,0,'','')")
    return seed


def handin_scene(suite, number, name, *, answers=("Hand over the Bracer Test",), give=True, click=False, auto=False,
                 expect_mark=True, expect_finished=True, expect_auto=False, control_env=None):
    say = f"#giveitem {BRACER} Lsgm@6000" if give else "#testmode@6000"
    mouse = {"EQOA_MOUSE": f"click {HANDIN_BUTTON[0]} {HANDIN_BUTTON[1]}@14500"} if click else {}

    def checks(run_dir, log):
        server = suite.server_since()
        mark = ss.green_pixels(run_dir / "1-dialog.png", HANDIN_BOX)
        out = [] if expect_auto else [(f"the Hand in mark {'is' if expect_mark else 'is not'} on the option (green pixels {mark})",
                                       mark is not None and ((mark > 60) if expect_mark else (mark < 10)))]
        out.append((f"the server {'finished' if expect_finished else 'did not finish'} the quest (server log)", (FINISHED in server) == expect_finished))
        out.append((f"the bracer is {'gone from' if expect_finished else 'still in'} the character's bag (database)",
                    suite.item_count(BRACER) == (0 if expect_finished else (1 if give else 0))))
        auto_said = "handing in Bracer Test automatically" in log
        out.append((f"the mod {'answered by itself' if expect_auto else 'left the answer to the player'}", auto_said == expect_auto))
        return out

    expect = [("Nothard hailed", r"script: target Commander Nothard: true")]
    expect.append(("the server's answer to the hail is " + ("a question (response dialog)" if give else "only his words (no question)"),
                   r"takeover: response_dialog session started for the mod" if give else r"takeover: text_dialog session started for the mod"))
    return ss.Scene(
        number, name, mods=("eqoa_dialogs",), fresh_db=True, seed=handin_seed(*answers),
        options={"eqoa_dialogs": {"auto_answer": True}} if auto else {},
        env={"EQOA_SAY": say, "EQOA_TARGET": "Commander Nothard@9000", "EQOA_PLAY_KEYS": "q@10500", **mouse},
        control_env=control_env or {},
        quit_after=22, shots="12500=1-dialog.png,16500=2-after.png", expect=expect, extra=checks,
        absent=[] if give else [("a question when the bracer is not held", r"takeover: response_dialog session started")],
    )


def handin_scenes(suite):
    wrong = {"EQOA_SAY": "#giveitem 8398 Lsgm@6000"}       # field rations: not what the step takes
    return [
        handin_scene(suite, "8a", "Quest hand-in: the Hand in button is clicked and the server takes the bracer",
                     click=True, control_env=wrong),
        handin_scene(suite, "8b", "Quest hand-in: without the bracer there is no question and no button",
                     give=False, expect_mark=False, expect_finished=False,
                     control_env={"EQOA_SAY": f"#giveitem {BRACER} Lsgm@6000"}),   # the bracer held: a question and a button
        handin_scene(suite, "8c", "Quest hand-in, automatic: the one real answer (Goodbye beside it does not count) is sent",
                     answers=("Hand over the Bracer Test", "Goodbye"), auto=True, expect_auto=True,
                     control_env={"EQOA_SAY": "#testmode@6000"}),
        handin_scene(suite, "8d", "Quest hand-in, automatic on but two real answers: only marked, never sent",
                     answers=("Hand over the Bracer Test", "Tell me about the weather"), auto=True, expect_finished=False,
                     control_env={"EQOA_SAY": "#testmode@6000"}),
    ]


def restart_scene(suite):
    """The local server is stopped under a client that is in the world and started again 4 s later."""
    return ss.Scene(
        "9", "Server restart under a client in the world: the client reports the loss and ends cleanly",
        mods=(), restart_at=14, quit_after=60,
        # after the restart the player says something: the unanswered line is what shows the client the server is gone
        env={"EQOA_MOD_KEYS": "enter@10000;text /s before@10500;enter@12000;enter@28000;text /s anyone@28500;enter@30500;enter@40000;text /s again@40500;enter@42500"}, shots="12000=1-before-restart.png",
        expect=[("the client says the connection was lost", r"play: disconnected \(reason 10\): Connection problem or server currently unavailable")],
        extra=lambda run_dir, log: [
            ("the server came back up (a second 'Game server ready' since this scene began)",
             suite.server_since().count("Game server ready on port") >= 1),
        ],
        notes=("re-entering the world after the restart: SKIP, the scripted client has no reconnect path (it ends at the disconnect and "
               "its next `play` is a new login), so 'world entry again' cannot be driven in one run",),
    )


# --- scene 10: REAL quests, unchanged rows (only the character's own quest row, items and position are set) -------------
# A diagnostic edit of a COPY of the Dialog Window's main.lua (not the bundled mod) logs what the mod sees at each dialog.
DUMP_OLD = "    state.auto_ref = s.ref" + chr(10) + "    if s.kind"
DUMP_NEW = """    state.auto_ref = s.ref
    do
        local d = s.data or {}
        local seen = {}
        for _, k in ipairs({ "text", "title", "options", "talked_to", "npc", "speaker", "id", "ref", "kind", "can_pass", "detail", "page", "more" }) do
            local v = d[k]
            if v ~= nil then seen[#seen + 1] = k .. "=" .. (type(v) == "table" and "<table>" or tostring(v)) end
        end
        note("info", "DUMP kind=" .. tostring(s.kind) .. " ref=" .. tostring(s.ref) .. " data fields present{" .. table.concat(seen, "; ") .. "}")
        local tt = d.talked_to
        if type(tt) == "table" then
            local f = {}
            for _, k in ipairs({ "name", "id", "ref", "title", "level", "kind", "race", "class" }) do
                if tt[k] ~= nil then f[#f + 1] = k .. "=" .. tostring(tt[k]) end
            end
            note("info", "DUMP talked_to fields{" .. table.concat(f, "; ") .. "}")
        end
        note("info", "DUMP talked_to=" .. tostring(d.talked_to and d.talked_to.name))
        note("info", "DUMP text=" .. tostring(d.text))
        for i, o in ipairs(d.options or {}) do note("info", "DUMP option " .. i .. "=" .. tostring(o)) end
        for _, q in ipairs(eqoa.quests.list() or {}) do
            local f = {}
            for _, k in ipairs({ "title", "index", "ref", "id", "name", "text", "step", "done" }) do
                if q[k] ~= nil then f[#f + 1] = k .. "=" .. tostring(q[k]) end
            end
            note("info", "DUMP journal entry {" .. table.concat(f, "; ") .. "}")
        end
    end
    if s.kind""".replace("\n", chr(10))
DUMP_PATCH = {"eqoa_dialogs": [(DUMP_OLD, DUMP_NEW)]}


def real_seed(questid):
    def seed(db):
        db.execute("INSERT INTO charquests (serverid,questid,step,count,done,slot,updated,picks) VALUES (2,?,1,0,0,0,'','')", (questid,))
    return seed


def real_scene(suite, number, name, *, questid, quest, giver, item, count, answer_key, expect_text, give=True):
    say = ["#testmode@5000"]
    if give:
        say.append(f"#giveitem {item} Lsgm {count}@6000")
    say.append(f"#goto {giver}@8000")
    keys = (f"{answer_key}@21000;enter@24000" if answer_key else "enter@21000")

    def checks(run_dir, log):
        server = suite.server_since()
        dump = [ln for ln in log.splitlines() if "Dialog window: DUMP" in ln]
        (run_dir / "mod-dump.txt").write_text(chr(10).join(dump), encoding="utf-8")
        mark = ss.green_pixels(run_dir / "1-dialog.png", HANDIN_BOX)
        return [
            (f'the server finished "{quest}" (server log)', f'Quests: Lsgm finished "{quest}"' in server),
            (f"the {item} items were taken (database rows left: {suite.item_count(item)})", suite.item_count(item) == 0),
            (f"RECORDED, not required: green Hand in mark on the first option row: {'yes' if (mark or 0) > 60 else 'no'} (green pixels {mark})", True),
        ]

    return ss.Scene(
        number, name, mods=("eqoa_dialogs",), fresh_db=True, seed=real_seed(questid), patch=DUMP_PATCH,
        env={"EQOA_SAY": ";".join(say), "EQOA_TARGET": f"{giver}@16500", "EQOA_PLAY_KEYS": "q@18000", "EQOA_MOD_KEYS": keys},
        control_env={"EQOA_SAY": f"#testmode@5000;#goto {giver}@8000"},           # nothing held: the step must not be done
        quit_after=29, shots="20000=1-dialog.png,22800=2-after-answer.png,26000=3-end.png",
        expect=[("the giver was hailed", rf"script: target {giver}: true"),
                ] + ([("the answer option was taken by the mod (response dialog)", r"takeover: response_dialog session started for the mod")] if answer_key else [])
        + [("the Dialog Window showed his words", addon_line("eqoa_dialogs", r"Dialog window: a (text|response)_dialog opened"))],
        extra=checks,
    )


def realquest_scenes(suite):
    return [
        real_scene(suite, "10a", 'REAL quest 100083 "Dragonfly Wings": a give step WITH answer words (Solenia Freyar, Freeport)',
                   questid=100083, quest="Dragonfly Wings", giver="Solenia Freyar", item=4873, count=2, answer_key="1",
                   expect_text="Yes, here they are."),
        real_scene(suite, "10b", 'REAL quest 102386 "A Zombie Swatch": a give step with NO answer words (Fatima Ubel, Secrets)',
                   questid=102386, quest="A Zombie Swatch", giver="Fatima Ubel", item=996077, count=3, answer_key=None,
                   expect_text="Fatima stitches"),
    ]


def choice_option(suite, questid, step, index):
    """(answer words, spell id or None) of one option of a real choice step, read from the server's own rows."""
    import json
    with closing(sqlite3.connect((suite.install / "eqoa-server" / "eqoa.db").as_uri() + "?mode=ro", uri=True)) as db:
        choices = json.loads(db.execute("SELECT choices FROM queststeps WHERE questid = ? AND step = ?", (questid, step)).fetchone()[0])
    option = choices["options"][index]
    return option["answer"], (int(option["spells"]) if option.get("spells") else None)


def choice_scene(suite, number, name, *, questid, giver, level, tclass, index, step=1, complete=True, records_pick=True, riddle_first=False):
    """A REAL choice step (rows unchanged). Auto mode is ON: the mod must leave the choice to the player, whose key
    (option number index+1) sends the pick; the server records it. The control presses nothing: if the mod answered for
    the player the control would record a pick and fail to fail."""
    answer, spell = choice_option(suite, questid, step, index)
    expect_pick = f"{step}:{index}"

    def seed(db):
        db.execute("INSERT INTO charquests (serverid,questid,step,count,done,slot,updated,picks) VALUES (2,?,?,0,0,0,'','')", (questid, step))
        db.execute("UPDATE characters SET level=?, tclass=? WHERE charName='Lsgm'", (level, tclass))

    def checks(run_dir, log):
        server = suite.server_since()
        (run_dir / "mod-dump.txt").write_text(chr(10).join(ln for ln in log.splitlines() if "Dialog window: DUMP" in ln), encoding="utf-8")
        row = suite.char_quest(questid)
        out = [(f'the server recorded the pick "{answer}" (server log)', f'Quests: Lsgm picked "{answer}"' in server)]
        if records_pick:
            out.append((f"charquests.picks is {expect_pick!r} (database row: {row})", row is not None and row[2] == expect_pick))
        if complete:
            out.append((f"the step was completed (the quest advanced past step {step} or is done): {row}",
                        row is not None and (row[0] != step or row[1] == 1)))
        if spell:
            out.append((f"the option's spell {spell} was given to the character (database)", suite.has_spell(spell)))
        out.append(("the mod never answered by itself", "automatically" not in log))
        return out

    return ss.Scene(
        number, name, mods=("eqoa_dialogs",), options={"eqoa_dialogs": {"auto_answer": True}}, fresh_db=True, seed=seed,
        patch=DUMP_PATCH,
        env={"EQOA_SAY": f"#testmode@5000;#goto {giver}@8000", "EQOA_TARGET": f"{giver}@16500", "EQOA_PLAY_KEYS": "q@18000",
             "EQOA_MOD_KEYS": (f"enter@20000;{index + 1}@22500;enter@25000" if riddle_first else f"{index + 1}@21000;enter@24000")},
        control_env={"EQOA_MOD_KEYS": ""},
        quit_after=29,
        shots=("19500=0-the-riddle.png,21500=1-the-choice.png,24000=2-after-pick.png,27000=3-end.png" if riddle_first
               else "20000=1-the-choice.png,22500=2-after-pick.png,26000=3-end.png"),
        expect=[("the giver was hailed", rf"script: target {giver}: true"),
                ("the mod took the choice (response dialog)", r"takeover: response_dialog session started for the mod")],
        extra=checks,
    )


def choice_scenes(suite):
    return [
        choice_scene(suite, "10c", "REAL class path choice 100951 \"The Paladin's Reward, Level 30\": 6 options, the player takes the SECOND, auto on",
                     questid=100951, giver="Sir Arik Lightfire", level=30, tclass=2, index=1),
        choice_scene(suite, "10d", "REAL choice 101136 \"The Shaman's Epic Reward\": 6 options, the player takes the SIXTH, auto on",
                     questid=101136, giver="Ediopolus", level=60, tclass=8, index=5),
        choice_scene(suite, "10e", "REAL riddle choice 101298 \"The Manticores' Riddles\" step 1: the player answers, the mod never does, auto on",
                     questid=101298, giver="Sektamoot", level=53, tclass=0, index=1, complete=False, records_pick=False, riddle_first=True),
        choice_scene(suite, "10f", "REAL level 20 class choice 100020 \"Tillian Bracelets\" (class 9, last step, 3 options, the third gives a spell), auto on",
                     questid=100020, giver="Tessarina Starshimmer", level=20, tclass=9, index=2, step=8),
    ]


# The game's own Stat Window ("Stats", an X, HP/HoT/AC and the resists) sits at the left, under the compass; the Stats Box
# mod's panel (2 columns x 8 rows, no title) at HUD (200, 54). Calibrated on the 2026-11-27 pictures (PC interface, HUD scale
# 85, 1280x720): the game's window makes GAME_STATS_BOX ~0.95 dark (the world there 0.23-0.58); the Stats Box makes
# STATS_BOX_BOX ~0.84 dark (0.00-0.03 without it). Its right column only: the Frames mod's windows can cover the left one.
GAME_STATS_BOX = (20, 300, 290, 462)
STATS_BOX_BOX = (410, 100, 512, 288)
# the owner's mods (his Melee profile, 2026-11-27) and his PC interface; his keys.opt moves Controls to B, so F1 is free
OWNER_MODS = ("eqoa_quest_journal", "eqoa_world_map", "eqoa_character", "eqoa_mana_bars", "eqoa_start_menu",
              "eqoa_target_of_target", "eqoa_spellbook", "eqoa_round_minimap", "eqoa_nameplates", "eqoa_crafting",
              "eqoa_bag_viewer", "eqoa_dialogs", "eqoa_creature_journal", "eqoa_chat", "eqoa_mail", "eqoa_frames",
              "eqoa_map_search", "eqoa_social", "eqoa_trade")
OWNER_PC = "ui_mode=pc" + chr(10) + "hud_scale=85" + chr(10) + "health_numbers=on" + chr(10) + "chat_always=on" + chr(10)
OWNER_KEYS = "key.map_out=" + chr(10) + "key.controls=b" + chr(10)


def stat_window_checks(game_up):
    """game_up: shot name -> True (the game's own Stat Window is on screen) / False. In every shot, loading ones too, the
    Stats Box mod's panel is not on screen."""
    def checks(run_dir, log):
        out = []
        for shot in sorted(run_dir.glob("*.png")):
            box = ss.dark_fraction(shot, STATS_BOX_BOX)
            out.append((f"{shot.name}: no Stats Box panel (dark {box if box is None else round(box, 2)})",
                        box is not None and box <= 0.3))
        for name, up in game_up.items():
            game = ss.dark_fraction(run_dir / name, GAME_STATS_BOX)
            shown = game if game is None else round(game, 2)
            out.append((f"{name}: the game's own Stat Window is {'on screen' if up else 'not on screen'} (dark {shown})",
                        game is not None and (game >= 0.85 if up else game < 0.85)))
        return out
    return checks


def stats_box_retired_scenes():
    """The Stats Box left the bundle (2026-11-27): F4 and /stats are the game's own Stat Window's again. 11a: the owner's old
    install as the dev build 4936f2c0 left it: the Stats Box's folder in his mods folder (recorded in bundled.json as the
    bundler's), the mod still ON in his profile, his old key line for it (F1), his twenty mods and PC interface, the Stat Window
    off at load. The client takes the unchanged folder out at start: nothing of the mod loads, F4 shows the game's window and
    F4 hides it, F1 does nothing. Control: the same folder with a line the player added is his own mod and is kept: it loads,
    holds the game's window, and the checks fail. 11b: a fresh install with the Stat Window on at load: the game's window is up,
    F4 hides it, F4 shows it again."""
    retired_line = (r"Bundled mods: eqoa_stats_box is no longer part of the client; its folder, unchanged since the client "
                    r"put it there, was taken out")
    absent = [("anything from the Stats Box mod", r"addon eqoa_stats_box:"),
              ("the Stats Box's own toggle", r"stats box (shown|hidden)"),
              ("a Controls row or warning about its key", r"Toggle Stats Box")]
    old = ss.Scene(
        "11a", "Stats Box retired: the owner's old install (leftover folder, mod on in his profile, F1 line); F4 twice, F1",
        mods=OWNER_MODS + ("eqoa_stats_box",), leftovers={"eqoa_stats_box": "4936f2c0"}, control_leftovers_edited=True,
        mod_keys_file="mod.eqoa_stats_box.toggle=f1" + chr(10),
        files={"client.opt": "ability_bars=4" + chr(10), "interface-pc.opt": OWNER_PC, "keys.opt": OWNER_KEYS},
        env={"EQOA_MOD_KEYS": "f4@10000;f4@13000;f1@16000"},
        quit_after=20,
        shots="1500=0a-load.png,2200=0b-load.png,2900=0c-load.png,3600=0d-load.png,4500=0e-load.png,9000=1-at-load.png,"
              "11500=2-f4-once.png,14500=3-f4-twice.png,17500=4-old-f1.png",
        expect=[("the leftover folder is taken out at start", retired_line)],
        absent=absent,
        extra=stat_window_checks({"1-at-load.png": False, "2-f4-once.png": True, "3-f4-twice.png": False,
                                  "4-old-f1.png": False}),
    )
    fresh = ss.Scene(
        "11b", "Stats Box retired: a fresh install, the Stat Window on at load; F4 twice",
        mods=OWNER_MODS,
        files={"client.opt": "stat_window=on" + chr(10) + "ability_bars=4" + chr(10), "interface-pc.opt": OWNER_PC,
               "keys.opt": OWNER_KEYS},
        env={"EQOA_MOD_KEYS": "f4@10000;f4@13000"},
        quit_after=17,
        shots="1500=0a-load.png,2900=0b-load.png,9000=1-at-load.png,11500=2-f4-once.png,14500=3-f4-twice.png",
        absent=absent,
        extra=stat_window_checks({"1-at-load.png": True, "2-f4-once.png": False, "3-f4-twice.png": True}),
    )
    return [old, fresh]


def make_scenes(suite):
    return [*stats_box_retired_scenes(), dialog_scene(), start_menu_scene(), chat_scene(), overlap_scene(), sell_scene(suite), destroy_scene(suite),
            drag_scene(), perf_scene(), *handin_scenes(suite), restart_scene(suite), *realquest_scenes(suite), *choice_scenes(suite)]
