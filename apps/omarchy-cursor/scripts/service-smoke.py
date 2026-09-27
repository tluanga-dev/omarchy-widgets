#!/usr/bin/env python3
"""Test the installed helper and widget adapter, restoring saved preferences.
Temporarily toggles highlighting and restarts the helper; does not inject input.
"""
import json
import os
from pathlib import Path
import subprocess
import time

REPO = Path(__file__).resolve().parents[3]
APP = Path.home() / ".local/bin/omarchy-cursor"
HELPER = REPO / "plugins/dev.cursor/cursor-control"
CONFIG = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "omarchy-cursor/config.json"


def run(*args):
    return subprocess.check_output([str(a) for a in args], text=True, timeout=10).strip()


def state():
    return json.loads(run(APP, "status"))


def save(text):
    temporary = CONFIG.with_suffix(".smoke.tmp")
    temporary.write_text(text)
    temporary.replace(CONFIG)


original = CONFIG.read_text()
initial = state()
try:
    assert json.loads(run(HELPER, "status"))["running"]
    run(HELPER, "toggle")
    assert state()["enabled"] != initial["enabled"]
    run(HELPER, "toggle")
    assert state()["enabled"] == initial["enabled"]
    print("PASS widget status and highlight toggle", flush=True)

    invalid = json.loads(original)
    invalid["zoom"] = 0
    save(json.dumps(invalid))
    assert run(APP, "reload").startswith("error:")
    assert state()["running"]
    save(original)
    run(APP, "reload")
    print("PASS invalid preferences rejected without stopping helper", flush=True)

    duplicate = subprocess.run([str(APP), "daemon"], text=True, capture_output=True, timeout=5)
    assert duplicate.returncode != 0 and "already running" in duplicate.stderr
    print("PASS duplicate daemon blocked", flush=True)

    # Immediate restart exercises the socket-close/systemd-exit race.
    for _ in range(3):
        run(HELPER, "quit")
        for _ in range(30):
            if not json.loads(run(HELPER, "status"))["running"]:
                break
            time.sleep(0.05)
        assert not json.loads(run(HELPER, "status"))["running"]
        run(HELPER, "start")
        assert state()["running"]
    print("PASS widget stop and immediate restart, three cycles", flush=True)
finally:
    save(original)
    run(APP, "start")
    run(APP, "reload")
    if initial["magnifying"] != state()["magnifying"]:
        run(APP, "magnify")
    assert not run("hyprctl", "configerrors")
    print("PASS preferences restored; Hyprland configuration clean", flush=True)
