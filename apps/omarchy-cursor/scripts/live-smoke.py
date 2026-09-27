#!/usr/bin/env python3
"""Explicit desktop integration test. Moves the pointer and injects clicks/shortcuts.
Close preferences first. Restores the original preferences, bindings, and pointer.
Requires the app installed and running, and `cargo build --release --examples`.
"""
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
TARGET = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
APP = Path.home() / ".local/bin/omarchy-cursor"
CONFIG = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "omarchy-cursor/config.json"


def run(*args):
    return subprocess.check_output([str(a) for a in args], text=True, timeout=10).strip()


def command(name):
    reply = run(APP, name)
    if reply.startswith("error:"):
        raise AssertionError(reply)
    return json.loads(reply)


def move(x, y):
    run("hyprctl", "dispatch", f"hl.dsp.cursor.move({{x={x},y={y}}})")
    time.sleep(0.12)


def layers():
    result = json.loads(run("hyprctl", "-j", "layers"))
    return {name: [l for group in monitor["levels"].values() for l in group
                   if l["namespace"] == "omarchy-cursor"]
            for name, monitor in result.items()}


def save(config):
    temporary = CONFIG.with_suffix(".smoke.tmp")
    temporary.write_text(json.dumps(config, indent=2) + "\n")
    temporary.replace(CONFIG)
    command("reload")


def key(letter, hold=150):
    run(TARGET / "release/examples/keyboard_probe", letter, str(hold))
    time.sleep(0.15)


def passed(name):
    print("PASS", name, flush=True)


assert run("omarchy-shell", "lock", "isLocked") == "false", "Unlock the desktop before running this test"
assert not any(c["class"] == "omarchy-cursor" for c in json.loads(run("hyprctl", "-j", "clients"))), "Close preferences before running this test"
original = CONFIG.read_text()
position = json.loads(run("hyprctl", "-j", "cursorpos"))
config = json.loads(original)
try:
    config.update(enabled=True, auto_hide=False, attention=False, magnify_hold=False)
    save(config)
    key("c")
    assert not command("status")["enabled"]
    key("c")
    assert command("status")["enabled"]
    passed("physical-map Super+Alt+C toggle")

    monitors = json.loads(run("hyprctl", "-j", "monitors"))
    for monitor in monitors:
        w, h = monitor["width"], monitor["height"]
        if monitor["transform"] % 2:
            w, h = h, w
        w, h = round(w / monitor["scale"]), round(h / monitor["scale"])
        x, y = monitor["x"] + w // 2, monitor["y"] + h // 2
        move(x, y)
        visible = layers()[monitor["name"]][0]
        assert abs(visible["x"] + visible["w"] / 2 - x) <= 1
        assert abs(visible["y"] + visible["h"] / 2 - y) <= 1
        before = command("status")["captured_frames"]
        key("m")
        time.sleep(0.3)
        status = command("status")
        assert status["magnifying"] and status["captured_frames"] > before
        assert status["capture_error"] is None
        for dx, dy in [(0, 0), (w-1, 0), (0, h-1), (w-1, h-1)]:
            move(monitor["x"] + dx, monitor["y"] + dy)
            time.sleep(0.12)
            assert command("status")["capture_error"] is None
        key("m")
        passed(f"{monitor['name']}: highlight alignment, magnifier, four corners")

    # Test mouse binding feedback over a neutral area; no application click needed.
    # The live click-through preferences test is documented separately.
    move(monitors[0]["x"] + 800, monitors[0]["y"] + 500)
    config.update(auto_hide=True, hide_after=0.3)
    save(config)
    time.sleep(0.85)
    assert all(l["w"] == 1 for group in layers().values() for l in group)
    move(monitors[0]["x"] + 830, monitors[0]["y"] + 500)
    assert any(l["w"] > 1 for group in layers().values() for l in group)
    passed("idle hide and reappear on motion")

    config.update(auto_hide=False, magnify_hold=True)
    save(config)
    run(APP, "install")
    process = subprocess.Popen([str(TARGET / "release/examples/keyboard_probe"), "m", "600"])
    time.sleep(0.2)
    assert command("status")["magnifying"]
    process.wait(timeout=3)
    time.sleep(0.15)
    assert not command("status")["magnifying"]
    passed("hold-to-magnify press and release")

    bad = dict(config, zoom=0)
    CONFIG.write_text(json.dumps(bad))
    assert run(APP, "reload").startswith("error:")
    assert command("status")["enabled"]
    passed("invalid preferences rejected without stopping daemon")

    duplicate = subprocess.run([str(APP), "daemon"], text=True, capture_output=True, timeout=3)
    assert duplicate.returncode != 0 and "already running" in duplicate.stderr
    passed("duplicate daemon blocked")
finally:
    CONFIG.write_text(original)
    command("reload")
    if command("status")["magnifying"]:
        command("magnify")
    run(APP, "install")
    move(position["x"], position["y"])
    assert not run("hyprctl", "configerrors").strip()
    passed("preferences, bindings, and pointer restored; Hyprland configuration clean")
