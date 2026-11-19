#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Run the native loot scene against a disposable copy of the local EQOA shard."""

import argparse
from contextlib import closing
from datetime import datetime
from hashlib import sha256
import ipaddress
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import tempfile
import time
from uuid import uuid4

from run_gates import CargoLock, run_subprocess_tree


ROOT = Path(__file__).resolve().parents[1]
PASEO = Path.home() / "Paseo"
TARGET = PASEO / "targets" / "shared"
ACCOUNT = "lsgm"
CHARACTER = "Lsgm"
CREATURE = "fire bat"
MEAT_ID = 991167
JUNK_ID = 700425
MEAT_RECEIPT = r"chat: You receive (?:[1-9]\d* )?sliver of fire bat meat from the corpse\."
JUNK_RECEIPT = r"chat: You receive (?:[1-9]\d* )?torn fire bat wings from the corpse\."
BAG_MEAT_LOG = r"scene: Bag Viewer loot log \d{2}:\d{2} sliver of fire bat meat x[1-9]\d*(?:, from fire bat)?"
CORPSE_LIST = r"play: 0x0018 \([1-9]\d* bytes\) in mode 0"
# Auto-loot's own lines (eqoa.log.write "info" lands in the client log); the GM's #kill is not the player's hit
AUTO_MODE = rf"addon eqoa_bag_viewer: \[info\] auto-loot: mode Everything but junk for {CHARACTER}"
AUTO_NOT_YOURS = (
    r"addon eqoa_bag_viewer: \[info\] auto-loot: corpse 0x[0-9a-f]+: fire bat: skipped, not your kill \(you did not hit it\)"
)
AUTH_PORT = 9735
GAME_PORT = 10070
SERVER_TIMEOUT = 120
CLIENT_TIMEOUT = 120


def read_gm_password(accounts, user):
    pattern = rf"[ \t]*{re.escape(user)}[ \t]+(\S+)[ \t]+\(GM(?:,[^\n)]*)?\)[ \t]*"
    passwords = [m[1] for line in accounts.splitlines() if (m := re.fullmatch(pattern, line))]
    if len(passwords) != 1:
        raise ValueError("Expected one matching GM entry in accounts.local.txt")
    return passwords[0]


def scene_checks(log, scene="native", r65=False, auto_loot=False):
    """Match each event only after all prior events in this client's log."""
    patterns = {
        "entered world": rf'in the world as 0x[0-9a-f]+: .*\bname: "{CHARACTER}".*',
        "GM spawn confirmed": rf"chat: \[GM\] Spawned {CREATURE}\. Target it and #despawn to take it away\.",
        "spawn targeted": rf"script: target {CREATURE}: true",
        "GM kill confirmed": rf"chat: \[GM\] Killed {CREATURE}",
    }
    if scene == "bag-viewer":
        patterns["Bag Viewer key replayed"] = r"script: mod key pressed"
        if r65:
            patterns.update({
                "Bag Viewer requested nearest corpse": r"takeover: mod open_loot -> corpse 0x[0-9a-f]+, opcode 0x0015",
                "loot session hidden": r"takeover: loot window HIDDEN for the mod \(.+\)",
            })
        else:
            patterns.update({
                "targeted corpse interaction replayed": r"script: mod key pressed",
                "corpse opened": CORPSE_LIST,
            })
        x = 750 if r65 else 650
        patterns["Take all but junk clicked"] = rf"script: mouse click Left at {x} 615"
        patterns["non-junk loot receipt"] = MEAT_RECEIPT
        if r65:
            patterns["hidden loot session closed"] = r"takeover: loot hidden session closed \(.+\); 0x0016 (?:sent|not sent: the server had closed the corpse)"
        patterns["Bag Viewer recorded non-junk loot"] = BAG_MEAT_LOG
    else:
        patterns.update({
            "corpse opened": CORPSE_LIST,
            "native loot requested": r"play: loot request One\(\d+, [1-9]\d*\)",
            "item received": rf"(?:{MEAT_RECEIPT}|{JUNK_RECEIPT})",
            "corpse closed after take": r"play: 0x0016 \(\d+ bytes\) in mode 1",
            "corpse refreshed": CORPSE_LIST,
        })
    lines = iter(log.splitlines())
    checks = {
        name: any(re.fullmatch(pattern, line, re.IGNORECASE) for line in lines)
        for name, pattern in patterns.items()
    }
    if scene == "bag-viewer":
        checks["no junk loot receipt"] = re.search(rf"^{JUNK_RECEIPT}$", log, re.M | re.I) is None
    if auto_loot:
        checks["Auto-loot mode logged"] = re.search(rf"^{AUTO_MODE}$", log, re.M) is not None
        checks["GM kill logged as not the player's kill"] = re.search(rf"^{AUTO_NOT_YOURS}$", log, re.M) is not None
    return checks


def local_host(config):
    """Read the local install's simple bind_address scalar without copying its config."""
    match = re.search(r"(?m)^\s*bind_address:\s*([^#\r\n]+)", config)
    if not match:
        raise ValueError("Set bind_address in the local server config or pass --host")
    return match[1].strip().strip("\"'")


def server_config(host):
    address = ipaddress.IPv4Address(host)
    lan = (ipaddress.IPv4Network(net) for net in ("10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"))
    if not address.is_loopback and not any(address in net for net in lan):
        raise ValueError("The scene requires a private or loopback IPv4 address assigned to this PC")
    # JSON is valid YAML. Use only the local installation's address and its preseeded DB.
    return {
        "server": {
            "bind_address": str(address), "server_ip": str(address), "db_path": "eqoa.db",
            "game_port": GAME_PORT, "auth_port": AUTH_PORT, "patch_port": 0, "account_port": 0,
        },
        "world": {"session_timeout_game": "30s"},
        "patch": {"worlds_udp_port": 0},
        "auth": {"allow_auto_register": False, "require_tcp_login": True},
    }


def copy_database(source, destination):
    # Back up in read-only mode so SQLite includes any committed WAL records.
    with closing(sqlite3.connect(source.as_uri() + "?mode=ro", uri=True)) as source_db:
        with closing(sqlite3.connect(destination)) as dest_db:
            source_db.backup(dest_db)
            # The daily welcome dialog occupies the loot key's window mode.
            # Mark it dismissed for the test character in the disposable database only.
            dest_db.execute(
                "INSERT INTO charwelcome (serverid, never) "
                "SELECT serverid, 1 FROM characters WHERE charName = ? "
                "ON CONFLICT(serverid) DO UPDATE SET never = 1",
                (CHARACTER,),
            )
            dest_db.commit()


def write_json(path, contents):
    path.write_text(json.dumps(contents), encoding="utf-8")


def main_checkout_iso():
    common_dir = subprocess.run(
        ["git", "-c", "safe.directory=*", "rev-parse", "--path-format=absolute", "--git-common-dir"],
        cwd=ROOT, capture_output=True, encoding="utf-8", check=False,
    )
    if common_dir.returncode == 0 and common_dir.stdout.strip():
        return Path(common_dir.stdout.strip()).parent / "run" / "eqoa.iso"
    return ROOT / "run" / "eqoa.iso"


def base_environment():
    return {k: v for k, v in os.environ.items() if not k.upper().startswith("EQOA_")}


def prepare_profile(config, host, scene, auto_loot=False):
    # Select the bundled defaults so the first-run addon panel stays closed.
    profiles = config / "addons" / "profiles"
    profiles.mkdir(parents=True)
    enabled = {}
    if scene == "bag-viewer":
        addon = b"eqoa_bag_viewer"
        digest = sha256(b"eqoa-native-addons/storage/v2\0")
        for field in (addon + b"/main.lua", addon, addon):
            digest.update(len(field).to_bytes(8, "little") + field)
        key = digest.hexdigest()
        enabled[key] = True
        (config / "mod-keys.opt").write_text("mod.eqoa_bag_viewer.open=b\n", encoding="utf-8")
        # A relative eqoa.files path maps to the addon's private files/data mount.
        files = config / "addons" / f"{key}-files" / "files" / "data"
        files.mkdir(parents=True)
        auto = f"auto:{CHARACTER}=non_junk\n" if auto_loot else ""
        (files / "junk.cfg").write_text(f"grey=0\n{auto}{JUNK_ID}\n", encoding="utf-8")
    write_json(profiles / "Default.json", {"schemaVersion": 1, "enabled": enabled, "addons": {}, "layout": ""})
    write_json(profiles / "characters.json", {
        "schemaVersion": 1, "characters": {host: {CHARACTER.lower(): "Default"}}, "classes": {},
    })


def client_binary(run_dir, override):
    if override is not None:
        if not override.is_file():
            raise ValueError("--exe must name an existing release executable")
        return override.resolve()
    exe = TARGET / "release" / "eqoa.exe"
    if not exe.is_file():
        with CargoLock():
            if not exe.is_file():
                env = {**base_environment(), "CARGO_TARGET_DIR": str(TARGET)}
                code, log = run_subprocess_tree(
                    ["cargo", "build", "--release", "--locked", "-p", "eqoa"], ROOT, env,
                    timeout_minutes=60, step_name="release", progress_interval=60,
                )
                (run_dir / "build.log").write_text(log, encoding="utf-8")
                if code != 0:
                    raise RuntimeError("Release build failed. See build.log")
    return exe


def wait_for_server(server, log):
    deadline = time.monotonic() + SERVER_TIMEOUT
    with log.open(encoding="utf-8", errors="replace") as reader:
        while time.monotonic() < deadline:
            if server.poll() is not None:
                raise RuntimeError(f"Local server exited with code {server.returncode}. See server.log")
            if any("Game server ready on port" in line for line in reader):
                return
            time.sleep(0.25)
    raise TimeoutError("Local server did not become ready. See server.log")


def client_environment(run_dir, runtime_dir, iso, password, scene, r65):
    env = {
        **base_environment(),
        "EQOA_CONFIG": str(run_dir / "config"),
        "EQOA_CACHE": str(run_dir / "cache"),
        "EQOA_FORGE_MODS": str(runtime_dir / "mods"),
        "EQOA_ISO": str(iso), "EQOA_PASSWORD": password, "EQOA_CHAR": CHARACTER,
        "EQOA_AUTH_PORT": str(AUTH_PORT), "EQOA_GAME_PORT": str(GAME_PORT),
        "EQOA_AUDIO": "0", "EQOA_NO_UPDATE": "1", "EQOA_OFFSCREEN": "1280x720",
        "EQOA_SAY": f"#spawn {CREATURE}@6000;#kill@10000",
        "EQOA_TARGET": f"{CREATURE}@8500",
    }
    if scene == "bag-viewer":
        clicks = "click 830 94@12500;click 480 615@14300;click 750 615@18500" if r65 else (
            "click 830 80@15500;click 650 615@18500"
        )
        env.update(
            EQOA_MOD_KEYS="b@11500;b@32000" if r65 else "b@11500;g@13000",
            EQOA_MOUSE=clicks,
            EQOA_SCENE_BAG_LOG="1",
            EQOA_SHOTS="17000=corpse.png,30000=scene.png",
            EQOA_QUIT="35" if r65 else "32",
        )
    else:
        env.update(EQOA_PLAY_KEYS="g@12500;down@14500;enter@16000",
                   EQOA_SHOTS="13750=corpse.png,17750=scene.png", EQOA_QUIT="20")
    return env


def bag_item_counts(database):
    query = """SELECT patternid, SUM(stackleft) FROM charinventory
               JOIN characters USING (serverid) WHERE charName = ? AND patternid IN (?, ?)
               GROUP BY patternid"""
    with closing(sqlite3.connect(database.as_uri() + "?mode=ro", uri=True)) as db:
        counts = dict(db.execute(query, (CHARACTER, MEAT_ID, JUNK_ID)))
    return counts.get(MEAT_ID, 0), counts.get(JUNK_ID, 0)


def run_scene(run_dir, install, host, iso, checks, args):
    password = read_gm_password((install / "accounts.local.txt").read_text(encoding="utf-8-sig"), ACCOUNT)
    source = install / "eqoa-server"
    config = server_config(host)
    exe = client_binary(run_dir, args.exe)
    prepare_profile(run_dir / "config", host, args.scene, args.auto_loot)
    creationflags = subprocess.CREATE_NO_WINDOW
    # Stage the release binary because the client can create a mods folder beside its executable.
    with tempfile.TemporaryDirectory(dir=run_dir, prefix="runtime-") as scratch:
        runtime = Path(scratch)
        client = runtime / "eqoa.exe"
        shutil.copyfile(exe, client)
        database = runtime / "eqoa.db"
        copy_database(source / "eqoa.db", database)
        if args.scene == "bag-viewer":
            before = bag_item_counts(database)
        write_json(runtime / "config.yaml", config)
        server_log = run_dir / "server.log"
        with server_log.open("wb") as log:
            server = subprocess.Popen(
                [source / "gameserver.exe", "-config", "config.yaml"], cwd=runtime,
                env=base_environment(),
                stdout=log, stderr=subprocess.STDOUT, creationflags=creationflags,
            )
        try:
            print("Waiting for local server", flush=True)
            wait_for_server(server, server_log)
            checks["server ready"] = True
            print("Running scripted client", flush=True)
            with (run_dir / "client.log").open("wb") as log:
                result = subprocess.run(
                    [client, "play", host, ACCOUNT], cwd=run_dir,
                    env=client_environment(run_dir, runtime, iso, password, args.scene, args.r65),
                    stdout=log, stderr=subprocess.STDOUT, creationflags=creationflags,
                    timeout=CLIENT_TIMEOUT, check=False,
                )
            checks["client exited successfully"] = result.returncode == 0
            if result.returncode != 0:
                raise RuntimeError(f"Client exited with code {result.returncode}. See client.log")
        finally:
            if server.poll() is None:
                server.terminate()
            server.wait(timeout=15)
            checks["server stopped"] = True
        if args.scene == "bag-viewer":
            after = bag_item_counts(database)
            checks["non-junk item in inventory"] = after[0] > before[0]
            checks["junk absent from inventory gains"] = after[1] == before[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server-dir", type=Path, default=PASEO / "local-server")
    parser.add_argument("--iso", type=Path, default=os.environ.get("EQOA_ISO"),
                        help="Frontiers disc image, defaults to EQOA_ISO or the main checkout's run/eqoa.iso")
    parser.add_argument("--host", help="Local IPv4 address, defaults to the server's bind_address")
    parser.add_argument("--scene", choices=("native", "bag-viewer"), default="native")
    parser.add_argument("--r65", action="store_true", help="Use the round 6.5 Bag Viewer and require takeover logs")
    parser.add_argument("--auto-loot", action="store_true",
                        help="With --r65: Auto-loot on (Everything but junk) and its log lines required")
    parser.add_argument("--exe", type=Path, help="Use the specified release client instead of the shared build")
    args = parser.parse_args()
    if args.r65 and args.scene != "bag-viewer":
        parser.error("--r65 requires --scene bag-viewer")
    if args.auto_loot and not args.r65:
        parser.error("--auto-loot requires --r65")
    if os.name != "nt":
        parser.error("This tool uses the Windows local test server and release client")
    install = args.server_dir.resolve()
    runs = install / "runs"
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S-")
    run_dir = runs / f"{stamp}{uuid4().hex[:8]}"
    try:
        runs.mkdir(exist_ok=True)
        run_dir.mkdir()
    except OSError as exc:
        parser.exit(2, f"Could not create a run folder: {exc}\n")
    print(f"Run folder {run_dir}", flush=True)
    checks = {"server ready": False, "client exited successfully": False, "server stopped": False}
    error = None
    try:
        iso = args.iso or main_checkout_iso()
        if not iso.is_file():
            raise ValueError("Set EQOA_ISO or pass --iso pointing to a local Frontiers disc image")
        host = args.host or local_host((install / "eqoa-server/config.yaml").read_text(encoding="utf-8-sig"))
        run_scene(run_dir, install, host, iso.resolve(), checks, args)
    except (OSError, ValueError, RuntimeError, sqlite3.Error, subprocess.TimeoutExpired) as exc:
        error = str(exc)
    except KeyboardInterrupt:
        error = "Scene interrupted"
    client_log = run_dir / "client.log"
    log = client_log.read_text(encoding="utf-8", errors="replace") if client_log.exists() else ""
    checks.update(scene_checks(log, args.scene, args.r65, args.auto_loot))
    screenshot = run_dir / "scene.png"
    checks["in-world screenshot"] = False
    if screenshot.is_file():
        with screenshot.open("rb") as shot:
            checks["in-world screenshot"] = (
                shot.read(8) == b"\x89PNG\r\n\x1a\n"
                and re.search(rf"(?m)^shot scene\.png at \d+ ms \(play as {CHARACTER}\)$", log) is not None
            )
    passed = error is None and all(checks.values())
    lines = ["PASS" if passed else "FAIL", *(f"{'PASS' if ok else 'FAIL'}  {name}" for name, ok in checks.items())]
    if error:
        lines.append(error)
    summary = "\n".join(lines) + "\n"
    (run_dir / "summary.txt").write_text(summary, encoding="utf-8")
    print(summary, end="")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
