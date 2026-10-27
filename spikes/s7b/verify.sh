#!/usr/bin/env bash
# S7b evidence summary: bash spikes/s7b/verify.sh DIR
# Prints one `probe.key=value` line per fact for every DIR/s7b-*.jsonl, plus the process-tree counters
# copied from DIR/s7b-procs.txt as `procs.*`. Observations only; it never judges and always exits 0.
set -u
DIR="${1:-}"
if [ -z "$DIR" ] || [ ! -d "$DIR" ]; then
  echo "LỖI s7b verify: cần thư mục chứa s7b-*.jsonl" >&2
  exit 0
fi

python3 -I - "$DIR" <<'PY'
import glob
import json
import os
import re
import sys

d = sys.argv[1]


def read(path):
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError:
        return None


def clean(s, n=120):
    return " ".join(str(s).split())[:n]


def side_value(text, key):
    if text is None:
        return "none"
    for line in text.splitlines():
        m = re.match(r"^" + re.escape(key) + r"=(.*)$", line.strip())
        if m:
            return m.group(1)
    return "none"


for path in sorted(glob.glob(os.path.join(d, "s7b-*.jsonl"))):
    base = os.path.basename(path)[len("s7b-"):-len(".jsonl")]
    name = base.replace("-", "_")
    perm = "none"
    step_types = set()
    tool_keys = "none"
    result = None
    text = read(path) or ""
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
        except ValueError:
            continue
        if not isinstance(obj, dict):
            continue
        ev = obj.get("event")
        if ev == "init" and isinstance(obj.get("init"), dict):
            perm = str(obj["init"].get("permission_mode", "none"))
        elif ev == "step_update" and isinstance(obj.get("step_update"), dict):
            su = obj["step_update"]
            st = su.get("step_type")
            if st is not None:
                step_types.add(str(st))
            if st == "tool" and tool_keys == "none":
                tool_keys = ",".join(sorted(su.keys()))
        elif ev == "result" and isinstance(obj.get("result"), dict):
            result = obj["result"]

    status = "none"
    err_head = "none"
    usage_keys = "none"
    num_turns = "none"
    if result is not None:
        status = clean(result.get("status", "none"))
        if result.get("error"):
            err_head = clean(result["error"])
        usage = result.get("usage")
        if isinstance(usage, dict) and usage:
            usage_keys = ",".join(sorted(usage.keys()))
        if "num_turns" in result:
            num_turns = clean(result["num_turns"])

    stem = os.path.join(d, "s7b-" + base)
    exit_code = side_value(read(stem + ".exit"), "exit_code")
    file_flag = side_value(read(stem + ".file"), "file")
    err_text = read(stem + ".stderr.txt")
    stderr_head = "none"
    if err_text:
        first = err_text.splitlines()[0] if err_text.splitlines() else ""
        if first.strip():
            stderr_head = clean(first)

    print(f"{name}.permission_mode={perm}")
    print(f"{name}.step_types={','.join(sorted(step_types)) if step_types else 'none'}")
    print(f"{name}.tool_step_keys={tool_keys}")
    print(f"{name}.status={status}")
    print(f"{name}.result_error_head={err_head}")
    print(f"{name}.usage_keys={usage_keys}")
    print(f"{name}.num_turns={num_turns}")
    print(f"{name}.exit_code={exit_code}")
    print(f"{name}.file={file_flag}")
    print(f"{name}.stderr_head={stderr_head}")

procs = read(os.path.join(d, "s7b-procs.txt"))
if procs is not None:
    for key in ("leftover_after_exit", "survivors_after_kill", "survivors_after_sweep"):
        print(f"procs.{key}={side_value(procs, key)}")
PY
exit 0
