#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Run the native loot scene against a disposable copy of the local EQOA shard."""

import argparse
from contextlib import closing
from datetime import datetime
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


def scene_checks(log):
    """Match each event only after all prior events in this client's log."""
    patterns = {
        "entered world": rf'in the world as 0x[0-9a-f]+: .*\bname: "{CHARACTER}".*',
        "GM spawn confirmed": rf"chat: \[GM\] Spawned {CREATURE}\. Target it and #despawn to take it away\.",
        "spawn targeted": rf"script: target {CREATURE}: true",
        "GM kill confirmed": rf"chat: \[GM\] Killed {CREATURE}",
        "corpse opened": r"play: 0x0018 \([1-9]\d* bytes\) in mode 0",
        "native loot requested": r"play: loot request One\(\d+, [1-9]\d*\)",
        "item received": r"chat: You receive (?:[1-9]\d* )?(?:sliver of fire bat meat|torn fire bat wings) from the corpse\.",
        "corpse closed after take": r"play: 0x0016 \(\d+ bytes\) in mode 1",
        "corpse refreshed": r"play: 0x0018 \([1-9]\d* bytes\) in mode 0",
    }
    lines = iter(log.splitlines())
    return {
        name: any(re.fullmatch(pattern, line, re.IGNORECASE) for line in lines)
        for name, pattern in patterns.items()
    }


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


def prepare_profile(config, host):
    # Select the bundled defaults so the first-run addon panel stays closed.
    profiles = config / "addons" / "profiles"
    profiles.mkdir(parents=True)
    write_json(profiles / "Default.json", {"schemaVersion": 1, "enabled": {}, "addons": {}, "layout": ""})
    write_json(profiles / "characters.json", {
        "schemaVersion": 1, "characters": {host: {CHARACTER.lower(): "Default"}}, "classes": {},
    })


def client_binary(run_dir):
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


def client_environment(run_dir, runtime_dir, iso, password):
    return {
        **base_environment(),
        "EQOA_CONFIG": str(run_dir / "config"),
        "EQOA_CACHE": str(run_dir / "cache"),
        "EQOA_FORGE_MODS": str(runtime_dir / "mods"),
        "EQOA_ISO": str(iso), "EQOA_PASSWORD": password, "EQOA_CHAR": CHARACTER,
        "EQOA_AUTH_PORT": str(AUTH_PORT), "EQOA_GAME_PORT": str(GAME_PORT),
        "EQOA_AUDIO": "0", "EQOA_NO_UPDATE": "1", "EQOA_OFFSCREEN": "1280x720",
        "EQOA_SAY": f"#spawn {CREATURE}@6000;#kill@10000",
        "EQOA_TARGET": f"{CREATURE}@8500",
        "EQOA_PLAY_KEYS": "g@12500;down@14500;enter@16000",
        "EQOA_SHOTS": "13750=corpse.png,17750=scene.png",
        "EQOA_QUIT": "20",
    }


def run_scene(run_dir, install, host, iso, checks):
    password = read_gm_password((install / "accounts.local.txt").read_text(encoding="utf-8-sig"), ACCOUNT)
    source = install / "eqoa-server"
    config = server_config(host)
    exe = client_binary(run_dir)
    prepare_profile(run_dir / "config", host)
    creationflags = subprocess.CREATE_NO_WINDOW
    # Stage the release binary because the client can create a mods folder beside its executable.
    with tempfile.TemporaryDirectory(dir=run_dir, prefix="runtime-") as scratch:
        runtime = Path(scratch)
        client = runtime / exe.name
        shutil.copyfile(exe, client)
        copy_database(source / "eqoa.db", runtime / "eqoa.db")
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
                    env=client_environment(run_dir, runtime, iso, password),
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server-dir", type=Path, default=PASEO / "local-server")
    parser.add_argument("--iso", type=Path, default=os.environ.get("EQOA_ISO"),
                        help="Frontiers disc image, defaults to EQOA_ISO or the main checkout's run/eqoa.iso")
    parser.add_argument("--host", help="Local IPv4 address, defaults to the server's bind_address")
    args = parser.parse_args()
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
        run_scene(run_dir, install, host, iso.resolve(), checks)
    except (OSError, ValueError, RuntimeError, sqlite3.Error, subprocess.TimeoutExpired) as exc:
        error = str(exc)
    except KeyboardInterrupt:
        error = "Scene interrupted"
    client_log = run_dir / "client.log"
    log = client_log.read_text(encoding="utf-8", errors="replace") if client_log.exists() else ""
    checks.update(scene_checks(log))
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
