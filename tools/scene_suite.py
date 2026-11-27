#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Owner-test scene suite: scripted in-game scenes against the local server, PASS/FAIL/SKIP each.

Extends tools/local_scene.py (same server copy, accounts, scripted input and screenshots);
see docs/DEV-TOOLS.md section 4b. Run: python tools/scene_suite.py [--only N[,N]] [--exe PATH]
"""

import argparse
from contextlib import closing
from dataclasses import dataclass, field
from datetime import datetime
from hashlib import sha256
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import time
from uuid import uuid4

import local_scene as ls

HISTORY_DEFAULT = ls.PASEO / "scene-suite-history.json"
GUARD = "Guard Alinon"
PASS, FAIL, SKIP = "PASS", "FAIL", "SKIP"


def addon_key(addon):
    """The profile's storage key for a bundled mod (same digest local_scene uses for Bag Viewer)."""
    a = addon.encode()
    digest = sha256(b"eqoa-native-addons/storage/v2\0")
    for field_ in (a + b"/main.lua", a, a):
        digest.update(len(field_).to_bytes(8, "little") + field_)
    return digest.hexdigest()


@dataclass
class Scene:
    """One client launch. `expect` lines must appear in order; `absent` lines must never appear."""
    number: str
    name: str
    mods: tuple = ()
    env: dict = field(default_factory=dict)
    quit_after: int = 30
    expect: list = field(default_factory=list)      # (label, regex) in order
    absent: list = field(default_factory=list)      # (label, regex) anywhere
    mod_keys_file: str = ""
    shots: str = ""
    skip: object = None                              # callable(suite) -> reason or None
    extra: object = None                             # callable(run_dir, log) -> [(label, ok)]
    control_env: dict = field(default_factory=dict)    # negative control: env changes (e.g. leave out the Yes click)
    notes: tuple = ()                                 # parts of the scene this server cannot test, said as SKIP lines
    records_perf: bool = False                        # record this run's median fps/cpu/hud in the history file
    restart_at: float = 0                             # seconds after the client starts: restart the server then
    options: dict = field(default_factory=dict)       # mod id -> {option id: value} saved in the scratch profile
    seed: object = None                              # callable(sqlite3 connection): edits the fresh database copy
    fresh_db: bool = False                           # restart the server on a fresh copy of the seed database first
    patch: dict = field(default_factory=dict)         # mod id -> [(old, new)] edits to a COPY of its main.lua, every run (diagnostics)
    control_patch: dict = field(default_factory=dict)  # negative control: mod id -> [(old, new)] edits to its main.lua
    files: dict = field(default_factory=dict)          # file name -> text written in the scratch config folder (e.g. client.opt)
    control_files: dict = field(default_factory=dict)  # negative control: more such files (e.g. enhanced.opt with a switch on)
    leftovers: dict = field(default_factory=dict)      # mod id -> git revision: an older client's install of a mod (its files at
                                                       # that revision in the mods folder, recorded in bundled.json as installed)
    control_leftovers_edited: bool = False             # negative control: the leftovers carry a player's edit (so they are kept)


def evaluate(log, expect, absent):
    """Ordered `expect` matches, then `absent` checks: [(label, ok)]. Pure, so the tool itself is testable."""
    lines = iter(log.splitlines())
    out = [(label, any(re.fullmatch(p, line) for line in lines)) for label, p in expect]
    out += [(f"never: {label}", re.search(p, log, re.M) is None) for label, p in absent]
    return out


def verdict(checks, error=None):
    if error or not checks:
        return FAIL
    return PASS if all(ok for _, ok in checks) else FAIL


def png_ok(path):
    try:
        with open(path, "rb") as f:
            return f.read(8) == b"\x89PNG\r\n\x1a\n" and path.stat().st_size > 2000
    except OSError:
        return False


def dark_fraction(path, box, below=48):
    """Share of pixels in `box` (x0, y0, x1, y1) darker than `below` (0-255 luminance): a mod window's panel is
    near-black, the Freeport start spot's world is not. Needs Pillow; without it the check cannot be made, so it
    returns None and the scene check fails with that reason (never a silent pass)."""
    try:
        from PIL import Image
    except ImportError:
        return None
    try:
        with Image.open(path) as im:
            crop = im.convert("L").crop(box)
            data = list(crop.getdata())
    except OSError:
        return None
    return sum(1 for v in data if v < below) / max(1, len(data))


def green_pixels(path, box):
    """Pixels in `box` that are clearly green (the Dialog Window's hand-in mark is a green frame and button)."""
    try:
        from PIL import Image
        with Image.open(path) as im:
            data = list(im.convert("RGB").crop(box).getdata())
    except (ImportError, OSError):
        return None
    return sum(1 for r, g, b in data if g > 110 and g > r + 50 and g > b + 40)


def window_up(path, box, threshold=0.7):
    f = dark_fraction(path, box)
    return f is not None and f >= threshold


def region_diff(a, b, box):
    """Mean absolute luminance difference of `box` between two screenshots (None without Pillow or a file)."""
    try:
        from PIL import Image, ImageChops, ImageStat
        with Image.open(a) as ia, Image.open(b) as ib:
            d = ImageChops.difference(ia.convert("L").crop(box), ib.convert("L").crop(box))
            return ImageStat.Stat(d).mean[0]
    except (ImportError, OSError):
        return None


def png_differs(a, b):
    try:
        return a.read_bytes() != b.read_bytes()
    except OSError:
        return False


def perf_stats(log):
    """Median fps and cpu ms/frame of the `perf:` lines (five-second windows), skipping the first (load) window."""
    rows = [(float(m[1]), float(m[2])) for m in re.finditer(r"^perf: ([\d.]+) fps, cpu ([\d.]+) ms/frame", log, re.M)]
    rows = rows[1:] if len(rows) > 2 else rows
    if not rows:
        return None
    return {"fps": statistics.median(r[0] for r in rows), "cpu_ms": statistics.median(r[1] for r in rows),
            "windows": len(rows)}


def hud_stats(log):
    """Median ms/frame the HUD phase took; 0.0 when no window listed it (the client prints a phase from 0.005 ms)."""
    if not re.search(r"^perf: ", log, re.M):
        return None
    m = re.findall(r"^perf: .*?hud (\d+\.\d+)", log, re.M)
    return statistics.median(float(x) for x in m) if m else 0.0


def perf_delta(history, current, build):
    """A printable comparison with the previous recorded run (None when there is none)."""
    previous = history[-1] if history else None
    if previous is None:
        return "first recorded run: no previous build to compare"
    d_fps = current["fps"] - previous["fps"]
    d_cpu = current["cpu_ms"] - previous["cpu_ms"]
    return (f"vs previous ({previous.get('build', '?')}): fps {previous['fps']:.1f} -> {current['fps']:.1f} ({d_fps:+.1f}), "
            f"cpu {previous['cpu_ms']:.2f} -> {current['cpu_ms']:.2f} ms ({d_cpu:+.2f})")


def load_history(path):
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        return data if isinstance(data, list) else []
    except (OSError, ValueError):
        return []


class Suite:
    def __init__(self, args):
        self.args = args
        self.install = args.server_dir.resolve()
        stamp = datetime.now().strftime("%Y%m%d-%H%M%S-")
        self.dir = self.install / "runs" / f"suite-{stamp}{uuid4().hex[:6]}"
        self.dir.mkdir(parents=True)
        self.host = args.host or ls.local_host((self.install / "eqoa-server/config.yaml").read_text(encoding="utf-8-sig"))
        iso = args.iso or ls.main_checkout_iso()
        if not Path(iso).is_file():
            raise ValueError("Set EQOA_ISO or pass --iso pointing to a local Frontiers disc image")
        self.iso = Path(iso).resolve()
        self.password = ls.read_gm_password((self.install / "accounts.local.txt").read_text(encoding="utf-8-sig"), ls.ACCOUNT)
        self.exe = args.exe.resolve() if args.exe else build_release(self.dir)
        self.server = None
        self.runtime = None
        self.merchant_available = None   # decided by the sell scene's probe

    # --- server -----------------------------------------------------------------
    def start_server(self):
        self.scratch = tempfile.TemporaryDirectory(dir=self.dir, prefix="runtime-")
        self.runtime = Path(self.scratch.name)
        shutil.copyfile(self.exe, self.runtime / "eqoa.exe")
        self.database = self.runtime / "eqoa.db"
        ls.copy_database(self.install / "eqoa-server" / "eqoa.db", self.database)
        ls.write_json(self.runtime / "config.yaml", ls.server_config(self.host))
        self.server_log = self.dir / "server.log"
        with self.server_log.open("wb") as log:
            self.server = subprocess.Popen(
                [self.install / "eqoa-server" / "gameserver.exe", "-config", "config.yaml"], cwd=self.runtime,
                env=ls.base_environment(), stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
        ls.wait_for_server(self.server, self.server_log)
        self.starts = 1

    def reset_database(self, seed=None):
        """A fresh copy of the seed database (a scene that sells or destroys items starts from the character's own)."""
        if self.server is not None and self.server.poll() is None:
            self.server.terminate()
            self.server.wait(timeout=15)
        for suffix in ("", "-wal", "-shm"):
            Path(str(self.database) + suffix).unlink(missing_ok=True)
        ls.copy_database(self.install / "eqoa-server" / "eqoa.db", self.database)
        if seed:       # the scene's own starting state, written to the disposable copy before the server reads it
            with closing(sqlite3.connect(self.database)) as db:
                seed(db)
                db.commit()
        with self.server_log.open("ab") as log:
            self.server = subprocess.Popen(
                [self.install / "eqoa-server" / "gameserver.exe", "-config", "config.yaml"], cwd=self.runtime,
                env=ls.base_environment(), stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
        # wait for a ready line written after this restart
        deadline = time.monotonic() + ls.SERVER_TIMEOUT
        while time.monotonic() < deadline:
            if self.server.poll() is not None:
                raise RuntimeError("Local server exited on restart. See server.log")
            if self.server_log.read_text(encoding="utf-8", errors="replace").count("Game server ready on port") >= self.starts + 1:
                self.starts += 1
                return
            time.sleep(0.5)
        raise TimeoutError("Local server did not become ready after a database reset")

    def bounce_server(self, down_seconds=4):
        """Stop the local server, keep it down for a while, start it again on the same database (a restart)."""
        if self.server is not None and self.server.poll() is None:
            self.server.terminate()
            self.server.wait(timeout=15)
        time.sleep(down_seconds)
        with self.server_log.open("ab") as log:
            self.server = subprocess.Popen(
                [self.install / "eqoa-server" / "gameserver.exe", "-config", "config.yaml"], cwd=self.runtime,
                env=ls.base_environment(), stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
        self.starts += 1

    def server_since(self):
        """The local server's log lines written since this scene's client was started."""
        with self.server_log.open("rb") as f:
            f.seek(self.log_mark)
            return f.read().decode("utf-8", errors="replace")

    def item_count(self, pattern_id):
        """How many of one item the character carries in the live database copy."""
        with closing(sqlite3.connect(self.database.as_uri() + "?mode=ro", uri=True)) as db:
            return db.execute("SELECT COALESCE(SUM(1), 0) FROM charinventory JOIN characters USING (serverid) "
                              "WHERE charName = ? AND patternid = ?", (ls.CHARACTER, pattern_id)).fetchone()[0]

    def char_quest(self, questid):
        """(step, done, picks) of the character's row for a quest in the live database copy, or None."""
        with closing(sqlite3.connect(self.database.as_uri() + "?mode=ro", uri=True)) as db:
            return db.execute("SELECT step, done, picks FROM charquests JOIN characters USING (serverid) "
                              "WHERE charName = ? AND questid = ?", (ls.CHARACTER, questid)).fetchone()

    def has_spell(self, spellid):
        with closing(sqlite3.connect(self.database.as_uri() + "?mode=ro", uri=True)) as db:
            return db.execute("SELECT COUNT(*) FROM spells JOIN characters USING (serverid) WHERE charName = ? AND spellid = ?",
                              (ls.CHARACTER, spellid)).fetchone()[0] > 0

    def inventory(self):
        """(rows, stack sum, tunar) of the character in the live database copy."""
        with closing(sqlite3.connect(self.database.as_uri() + "?mode=ro", uri=True)) as db:
            row = db.execute("SELECT COUNT(*), COALESCE(SUM(stackleft), 0) FROM charinventory JOIN characters USING (serverid) "
                             "WHERE charName = ?", (ls.CHARACTER,)).fetchone()
            tunar = db.execute("SELECT tunar FROM characters WHERE charName = ?", (ls.CHARACTER,)).fetchone()[0]
        return row[0], row[1], tunar

    def build_label(self):
        """Names the build under test: this tree's git commit (and a mark when it has uncommitted changes) plus the
        client executable's own hash, so two runs of the same commit with different binaries are told apart."""
        def git(*a):
            r = subprocess.run(["git", "-c", "safe.directory=*", *a], cwd=ls.ROOT, capture_output=True, encoding="utf-8", check=False)
            return r.stdout.strip() if r.returncode == 0 else ""
        sha = git("rev-parse", "--short", "HEAD") or "nogit"
        dirty = "+local" if git("status", "--porcelain", "--untracked-files=no") else ""
        digest = sha256(self.exe.read_bytes()).hexdigest()[:8]
        return self.args.build_label or f"{sha}{dirty}/{digest}"

    def record_perf(self, log):
        """Median fps / cpu / hud of the run into the history file (outside the repo); returns (stats, text)."""
        stats = perf_stats(log)
        if stats is None:
            return None, "no perf lines in the client log"
        stats["hud_ms"] = hud_stats(log)
        build = self.build_label()
        history = load_history(self.args.history)
        text = f"build {build}: median {stats['fps']:.1f} fps, cpu {stats['cpu_ms']:.2f} ms/frame, hud {stats['hud_ms']} ms over {stats['windows']} windows; " +             perf_delta(history, stats, build)
        if not self.args.no_history:
            history.append({"time": datetime.now().isoformat(timespec="seconds"), "build": build, **stats})
            self.args.history.parent.mkdir(parents=True, exist_ok=True)
            self.args.history.write_text(json.dumps(history, indent=1), encoding="utf-8")
            text += f"  (history: {self.args.history}, {len(history)} runs)"
        return stats, text

    def stop_server(self):
        if self.server is not None:
            if self.server.poll() is None:
                self.server.terminate()
            try:
                self.server.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.server.kill()
            self.server = None
        if getattr(self, "scratch", None):
            try:
                self.scratch.cleanup()
            except OSError:
                pass

    # --- one client launch --------------------------------------------------------
    def profile(self, config, scene, control=False):
        profiles = config / "addons" / "profiles"
        profiles.mkdir(parents=True)
        enabled = {addon_key(m): True for m in scene.mods}
        if "eqoa_bag_viewer" in scene.mods:
            files = config / "addons" / f"{addon_key('eqoa_bag_viewer')}-files" / "files" / "data"
            files.mkdir(parents=True)
            (files / "junk.cfg").write_text(f"grey=0\n{ls.JUNK_ID}\n", encoding="utf-8")
        if scene.mod_keys_file:
            (config / "mod-keys.opt").write_text(scene.mod_keys_file, encoding="utf-8")
        for name, text in {**scene.files, **(scene.control_files if control else {})}.items():   # the player's saved options
            (config / name).write_text(text, encoding="utf-8")
        for m, opts in scene.options.items():      # a mod's switches are kept in addons/<its key>.json
            ls.write_json(config / "addons" / f"{addon_key(m)}.json", {"table": [[k, v] for k, v in opts.items()]})
        ls.write_json(profiles / "Default.json", {"schemaVersion": 1, "enabled": enabled, "addons": {}, "layout": ""})
        ls.write_json(profiles / "characters.json", {
            "schemaVersion": 1, "characters": {self.host: {ls.CHARACTER.lower(): "Default"}}, "classes": {}})

    def run_client(self, scene, folder, patch=None, control=False):
        run_dir = self.dir / folder
        run_dir.mkdir()
        self.profile_dir = run_dir / "config"
        self.profile(run_dir / "config", scene, control)
        mods = run_dir / "mods"      # per run, so a negative control's edited mod never reaches another scene
        if scene.leftovers:
            write_leftovers(mods, run_dir / "config", scene.leftovers, edited=control and scene.control_leftovers_edited)
        env = {
            **ls.base_environment(),
            "EQOA_CONFIG": str(run_dir / "config"), "EQOA_CACHE": str(self.dir / "cache"),
            "EQOA_FORGE_MODS": str(mods), "EQOA_ISO": str(self.iso), "EQOA_PASSWORD": self.password,
            "EQOA_CHAR": ls.CHARACTER, "EQOA_AUTH_PORT": str(ls.AUTH_PORT), "EQOA_GAME_PORT": str(ls.GAME_PORT),
            "EQOA_AUDIO": "0", "EQOA_NO_UPDATE": "1", "EQOA_OFFSCREEN": "1280x720",
            "EQOA_QUIT": str(scene.quit_after), **scene.env, **(scene.control_env if control else {}),
        }
        if scene.shots:
            env["EQOA_SHOTS"] = scene.shots
        for mod_id, edits in (patch or {}).items():
            self.write_patched_mod(mods, mod_id, edits)
        error = None
        try:
            timer = None
            if scene.restart_at and not control:      # the control leaves the server up
                import threading
                timer = threading.Timer(scene.restart_at, self.bounce_server)
                timer.start()
            with (run_dir / "client.log").open("wb") as log:
                result = subprocess.run([self.runtime / "eqoa.exe", "play", self.host, ls.ACCOUNT], cwd=run_dir, env=env,
                                        stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW,
                                        timeout=ls.CLIENT_TIMEOUT + scene.quit_after, check=False)
            if result.returncode != 0:
                error = f"client exited with code {result.returncode}"
        except subprocess.TimeoutExpired:
            error = "client timed out"
        finally:
            if timer is not None:
                timer.join()
        time.sleep(3)  # the server holds the session briefly after a client ends
        log_text = (run_dir / "client.log").read_text(encoding="utf-8", errors="replace")
        return run_dir, log_text, error

    @staticmethod
    def write_patched_mod(mods, mod_id, edits):
        """Negative control: seed the mods folder with an edited main.lua. The client's installer keeps a file the
        player changed and adds the mod's other files, so the edit is what runs."""
        source = (ls.ROOT / "bundled-mods" / mod_id / "main.lua").read_text(encoding="utf-8")
        for old, new in edits:
            if old not in source:
                raise RuntimeError(f"control patch for {mod_id} does not apply: {old!r} not found")
            source = source.replace(old, new)
        (mods / mod_id).mkdir(parents=True, exist_ok=True)
        (mods / mod_id / "main.lua").write_text(source, encoding="utf-8", newline="\n")

    # --- running ------------------------------------------------------------------
    def run_scene(self, scene, control=False):
        reason = scene.skip(self) if scene.skip else None
        if reason:
            return {"scene": scene.number, "name": scene.name, "verdict": SKIP, "reason": reason, "shots": [], "checks": []}
        folder = f"{scene.number}-{re.sub('[^a-z0-9]+', '-', scene.name.lower()).strip('-')[:40]}" + ("-control" if control else "")
        if scene.fresh_db:
            self.reset_database(scene.seed)
        self.log_mark = self.server_log.stat().st_size      # what the server says from here on is this scene's
        for attempt in range(4):
            shutil.rmtree(self.dir / folder, ignore_errors=True)
            patch = {m: list(e) for m, e in scene.patch.items()}
            for m, e in (scene.control_patch.items() if control else ()):
                patch.setdefault(m, []).extend(e)
            run_dir, log, error = self.run_client(scene, folder, patch, control)
            if "You are already logged in" not in log:
                break
            time.sleep(20)      # the server still holds the last scene's session (30 s): ask again
        checks = [("entered world", re.search(rf'in the world as 0x[0-9a-f]+: .*\bname: "{ls.CHARACTER}"', log) is not None)]
        checks += evaluate(log, scene.expect, scene.absent)
        shots = sorted(run_dir.glob("*.png"))
        checks += [(f"screenshot {p.name}", png_ok(p)) for p in shots]
        if scene.extra:
            try:
                checks += scene.extra(run_dir, log)
            except Exception as exc:   # a scene's own checks failing to run is a FAIL with the reason, not a crash
                checks.append((f"scene checks could not run: {type(exc).__name__}: {exc}", False))
        note = None
        if getattr(scene, "records_perf", False):
            stats, note = self.record_perf(log)
            checks.append(("enough perf windows to judge (at least 8 of 5 s)", bool(stats) and stats["windows"] >= 8))
            checks.append(("the client drew frames (median fps above 1)", bool(stats) and stats["fps"] > 1))
        v = verdict(checks, error)
        return {"scene": scene.number, "name": scene.name, "verdict": v, "reason": error or "",
                "shots": [str(p) for p in shots], "checks": checks, "run_dir": str(run_dir), "log": log,
                "notes": scene.notes, "note": note}


def mods_folder_key(mods):
    """The client's `bundled::mods_folder_key` on Windows: the SHA-256 of the canonical folder, lower case, backslashes."""
    s = os.path.realpath(mods)
    for prefix in ("\\\\?\\UNC\\", "\\\\?\\"):
        if s.startswith(prefix):
            s = ("\\\\" if "UNC" in prefix else "") + s[len(prefix):]
    return sha256(s.replace("/", "\\").lower().encode()).hexdigest()


def write_leftovers(mods, config, leftovers, edited=False):
    """An older client's install of each mod: its files at a git revision written into `mods`, and `bundled.json` saying
    the bundler installed them with those hashes (as an older client wrote it). `edited` adds a player's line to each
    main.lua (a control: a folder the player changed is kept)."""
    mods.mkdir(parents=True, exist_ok=True)
    key = mods_folder_key(mods)
    registry = {"folders": {key: []}, "shipped": {key: {}}}
    for mod_id, rev in leftovers.items():
        names = subprocess.run(["git", "-c", "safe.directory=*", "ls-tree", "-r", "--name-only", rev, f"bundled-mods/{mod_id}"],
                               cwd=ls.ROOT, capture_output=True, check=True).stdout.decode().split()
        for name in names:
            data = subprocess.run(["git", "-c", "safe.directory=*", "show", f"{rev}:{name}"], cwd=ls.ROOT,
                                  capture_output=True, check=True).stdout
            rel = name[len("bundled-mods/"):]
            registry["shipped"][key][rel] = [sha256(data).hexdigest()]
            if edited and rel.endswith("/main.lua"):
                data += b"\n-- a line the player added\n"
            (mods / rel).parent.mkdir(parents=True, exist_ok=True)
            (mods / rel).write_bytes(data)
        registry["folders"][key].append(mod_id)
    (config / "addons").mkdir(parents=True, exist_ok=True)
    ls.write_json(config / "addons" / "bundled.json", registry)


def build_release(run_dir):
    """Build this tree's release client under the shared cargo lock (a no-op when already up to date)."""
    env = {**ls.base_environment(), "CARGO_TARGET_DIR": str(ls.TARGET)}
    with ls.CargoLock():
        code, log = ls.run_subprocess_tree(["cargo", "build", "--release", "--locked", "-p", "eqoa"], ls.ROOT, env,
                                           timeout_minutes=60, step_name="release", progress_interval=60)
    (run_dir / "build.log").write_text(log, encoding="utf-8")
    if code != 0:
        raise RuntimeError("Release build failed. See build.log")
    return ls.TARGET / "release" / "eqoa.exe"


def say_script(*steps):
    return ";".join(steps)


def build_scenes(suite):
    """The seven suite scenes. Times are milliseconds from the client's start; the world is entered by ~5 s."""
    from scenes import make_scenes
    return make_scenes(suite)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", nargs="?", default="suite", choices=("suite",))
    parser.add_argument("--server-dir", type=Path, default=ls.PASEO / "local-server")
    parser.add_argument("--iso", type=Path, default=os.environ.get("EQOA_ISO"))
    parser.add_argument("--host")
    parser.add_argument("--exe", type=Path, help="release client to use instead of the shared build")
    parser.add_argument("--only", help="comma list of scene numbers")
    parser.add_argument("--control", action="store_true",
                        help="negative controls: run each chosen scene with its control_patch (a mod edited to lack the "
                             "behaviour); a control is OK when the scene FAILs")
    parser.add_argument("--history", type=Path, default=HISTORY_DEFAULT)
    parser.add_argument("--no-history", action="store_true", help="do not record the perf run")
    parser.add_argument("--build-label", help="label for this build in the perf history (default: git short sha)")
    args = parser.parse_args(argv)
    if os.name != "nt":
        parser.error("This tool uses the Windows local test server and release client")
    suite = Suite(args)
    print(f"Suite folder {suite.dir}", flush=True)
    wanted = set(args.only.split(",")) if args.only else None
    results = []
    try:
        suite.start_server()
        for scene in build_scenes(suite):
            if wanted and scene.number not in wanted and scene.number.rstrip("abcd") not in wanted:
                continue
            if args.control and not (scene.control_patch or scene.control_env or scene.control_files
                                     or scene.control_leftovers_edited or scene.restart_at):
                continue
            print(f"Scene {scene.number}: {scene.name}" + (" (negative control)" if args.control else ""), flush=True)
            r = suite.run_scene(scene, control=args.control)
            if args.control:
                r["note"] = "control OK: the assertions caught the missing behaviour" if r["verdict"] == FAIL else                     "CONTROL BROKEN: the scene still passed with the behaviour removed"
                r["verdict"] = PASS if r["verdict"] == FAIL else FAIL
            results.append(r)
            print(f"  {r['verdict']}", flush=True)
    finally:
        suite.stop_server()
    lines = []
    for r in results:
        label = ("CONTROL OK" if r["verdict"] == PASS else "CONTROL BROKEN") if args.control else r["verdict"]
        lines.append(f"{label}  scene {r['scene']}: {r['name']}" + (f"  ({r['reason']})" if r['reason'] else ""))
        for label, ok in r["checks"]:
            lines.append(f"      {'ok  ' if ok else 'FAIL'} {label}")
        for p in r["shots"]:
            lines.append(f"      shot {p}")
        if r.get("note"):
            lines.append(f"      {r['note']}")
        for note in r.get("notes", ()):
            lines.append(f"      SKIP part: {note}")
    summary = "\n".join(lines) + "\n"
    (suite.dir / "summary.txt").write_text(summary, encoding="utf-8")
    print(summary, end="")
    return 0 if all(r["verdict"] != FAIL for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())
