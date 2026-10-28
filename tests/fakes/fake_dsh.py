#!/usr/bin/env python3
"""Scenario-driven fake of the `dsh` CLI of the DeepSeek Harness (plan 06-23). Stdlib only.

It is run by the fake docker (tests/fakes/fake_docker.py) through `sh -c` exactly where the real
`dsh` would run in the container: `dsh --profile PROFILE PROMPT`. Output is plain text.

Environment:
  FAKE_DSH_RECORD    path; one JSON line per call:
                     {"argv": [...], "cwd": "...", "env_names": [...], "stdin_is_null": bool}
  FAKE_DSH_SCENARIO  path to a JSON object with these optional keys:
    lines        list of strings, printed one per line (default: two short lines)
    ops          [{"op": "write", "path": ..., "content": ...}]; paths relative to the cwd
    stderr       text written to stderr first
    sleep_s      sleep after the lines are printed
    spawn_child  start one long-lived child (python3 sleeping 300 s) and leave it running
    pidfile      path; written as "{own pid} {child pid}" once the child runs (needs spawn_child)
    exit_code    process exit code (default 0)
"""
from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import time


def stdin_is_null() -> bool:
    try:
        st = os.fstat(0)
        null = os.stat(os.devnull)
    except OSError:
        return False
    return stat.S_ISCHR(st.st_mode) and st.st_rdev == null.st_rdev


def load_scenario() -> dict:
    path = os.environ.get("FAKE_DSH_SCENARIO")
    if path and os.path.isfile(path):
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
            if isinstance(data, dict):
                return data
        except Exception as e:  # noqa: BLE001
            sys.stderr.write(f"Lỗi đọc FAKE_DSH_SCENARIO: {e}\n")
    return {}


def main() -> None:
    argv = sys.argv[1:]
    record_path = os.environ.get("FAKE_DSH_RECORD")
    if record_path:
        entry = {
            "argv": argv,
            "cwd": os.getcwd(),
            "env_names": sorted(os.environ.keys()),
            "stdin_is_null": stdin_is_null(),
        }
        with open(record_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(entry) + "\n")

    scenario = load_scenario()

    text = scenario.get("stderr", "")
    if text:
        sys.stderr.write(text)
        sys.stderr.flush()

    if scenario.get("spawn_child"):
        child = subprocess.Popen(
            ["python3", "-c", "import time; time.sleep(300)"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        pidfile = scenario.get("pidfile")
        if pidfile:
            tmp = f"{pidfile}.tmp"
            with open(tmp, "w", encoding="utf-8") as f:
                f.write(f"{os.getpid()} {child.pid}\n")
            os.replace(tmp, pidfile)

    for op in scenario.get("ops", []):
        if not isinstance(op, dict) or op.get("op") != "write":
            continue
        path = op.get("path", "")
        target = path if os.path.isabs(path) else os.path.join(os.getcwd(), path)
        parent = os.path.dirname(target)
        if parent:
            os.makedirs(parent, exist_ok=True)
        with open(target, "w", encoding="utf-8") as f:
            f.write(op.get("content", ""))

    lines = scenario.get("lines", ["dsh: đọc plan", "dsh: xong"])
    for line in lines:
        sys.stdout.write(str(line) + "\n")
        sys.stdout.flush()

    sleep_s = float(scenario.get("sleep_s", 0) or 0)
    if sleep_s > 0:
        time.sleep(sleep_s)

    sys.exit(int(scenario.get("exit_code", 0) or 0))


if __name__ == "__main__":
    main()
