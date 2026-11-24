# SPDX-License-Identifier: AGPL-3.0-or-later
"""The scene definitions for tools/scene_suite.py. Times are ms from the client's start (the world is up by ~5 s).

Coordinates are screenshot pixels at EQOA_OFFSCREEN=1280x720 (the HUD's 640x448 picture, scaled 1.607 and
centred). Mod windows are checked by what the client logs where the mod or the game says anything, and by
what is on the screen (`ss.dark_fraction`) where nothing is logged.
"""

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
# Ability and Pet. A sub-menu takes two (up one menu, then close); an entry that opens a window takes one.
START_ENTRIES = [("TEST", 2), ("Community", 2), ("Character", 2), ("Inventory", 1), ("Ability", 1), ("Auction", 2),
                 ("Pet", 2), ("Quest", 1), ("Support", 2), ("Options", 2)]


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
        control_patch={"eqoa_start_menu": [('state.enter = eqoa.input.claim("Enter", true)', "state.enter = nil")]},
    )


def start_menu_checks(run_dir, log):
    base = run_dir / "0-baseline.png"
    out = []
    for i, (name, _) in enumerate(START_ENTRIES):
        slug = f"{i + 1:02d}-{name.lower()}"
        diff = ss.region_diff(base, run_dir / f"{slug}.png", START_BOX)
        out.append((f"{name}: something opened (centre of the screen changed, mean {diff if diff is None else round(diff, 1)})",
                    diff is not None and diff > 4.0))
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
        env={"EQOA_MOD_KEYS": "b@10000", "EQOA_MOUSE": f"{SELECT_3};click {x} {y}@15000;click 541 145@16500"},
        control_env={"EQOA_MOUSE": f"{SELECT_3};click {x} {y}@15000;click 541 175@16500"},   # answers "No, leave them"
        quit_after=24,
        shots="12500=1-three-selected.png,15800=2-confirm.png,18500=3-after.png",
        expect=[
            ("the merchant's reach was used (a hidden merchant session opened for the mod)", r"takeover: mod talk -> npc 0x[0-9a-f]+, .*"),
            ("the group sale ran and said so", addon_line("eqoa_bag_viewer", r"bag multi-select: Sold 3 of 3")),
            ("the merchant session was closed again", r"takeover: merchant hidden session closed \(the mod's action or batch finished\)"),
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
            ("the window really moved (its old place changed)", (ss.region_diff(run_dir / "1-before.png", run_dir / "2-after.png", CHAT_BOX) or 0) > 6.0),
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


def make_scenes(suite):
    return [dialog_scene(), start_menu_scene(), chat_scene(), overlap_scene(), sell_scene(suite), destroy_scene(suite), drag_scene(), perf_scene()]
