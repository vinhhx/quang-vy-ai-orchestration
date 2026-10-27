#!/usr/bin/env python3
"""Scenario-driven fake of the `agy` CLI (headless, --output-format stream-json).

Shape follows the S7 evidence (spikes/s7/raw/s7-stream.jsonl): every line is an object with an
`event` key: `init`, `step_update`, `result`. Stdlib only.

Environment:
  FAKE_AGY_RECORD    path; one JSON line is appended per call:
                     {"argv": [...], "cwd": "...", "env_names": [sorted names], "permission": "..."}
  FAKE_AGY_SCENARIO  path to a JSON object with these optional keys:
    lines           list of objects, printed as JSON lines (replaces the default stream)
    raw_lines       list of strings, printed as-is (after `lines`)
    delay_ms        sleep between printed lines
    final_newline   default true; false leaves the very last line without a newline
    exit_code       process exit code (default 0)
    stderr          text written to stderr first
    ops             [{"op": "write", "path": ..., "content": ...}]; executed only when the
                    permission is not request-review. Otherwise nothing is written and stderr gets
                    "Tool call needs approval; denied in headless mode: write <path>"
    response        result.response (default "OK\\n")
    status          result.status (default "SUCCESS")
    error           result.error (default: the stderr text) when status is not SUCCESS
    sleep_s         sleep before the result line
    spawn_child     start one long-lived child (python3 sleeping 300 s) first and leave it running

Permission, from argv: --dangerously-skip-permissions -> always-proceed, --mode accept-edits ->
accept-edits, --sandbox -> sandbox, otherwise request-review.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time

USAGE = {
    "input_tokens": 120,
    "output_tokens": 30,
    "thinking_tokens": 5,
    "cache_read_tokens": 40,
    "total_tokens": 150,
}

# Flags that take a value (so their operand is not mistaken for something else).
VALUE_FLAGS = {
    "-p", "--print", "--prompt", "--output-format", "--input-format", "--print-timeout",
    "--mode", "--model", "--effort", "--json-schema", "--conversation", "--add-dir", "--log-file",
}


def parse_argv(argv: list[str]) -> dict:
    out: dict = {"prompt": None, "mode": None, "sandbox": False, "skip": False, "model": None}
    i = 0
    while i < len(argv):
        a = argv[i]
        val = argv[i + 1] if i + 1 < len(argv) else None
        if a in ("-p", "--print", "--prompt"):
            out["prompt"] = val
            i += 2
        elif a == "--mode":
            out["mode"] = val
            i += 2
        elif a == "--model":
            out["model"] = val
            i += 2
        elif a == "--sandbox":
            out["sandbox"] = True
            i += 1
        elif a == "--dangerously-skip-permissions":
            out["skip"] = True
            i += 1
        elif a in VALUE_FLAGS:
            i += 2
        else:
            i += 1
    return out


def permission_of(opts: dict) -> str:
    if opts["skip"]:
        return "always-proceed"
    if opts["mode"] == "accept-edits":
        return "accept-edits"
    if opts["sandbox"]:
        return "sandbox"
    return "request-review"


def load_scenario() -> dict:
    path = os.environ.get("FAKE_AGY_SCENARIO")
    if path and os.path.isfile(path):
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
            if isinstance(data, dict):
                return data
        except Exception as e:  # noqa: BLE001
            sys.stderr.write(f"Lỗi đọc FAKE_AGY_SCENARIO: {e}\n")
    return {}


def emit(text: str, newline: bool = True) -> None:
    sys.stdout.write(text + ("\n" if newline else ""))
    sys.stdout.flush()


def main() -> None:
    argv = sys.argv[1:]

    if "--max-turns" in argv:
        sys.stderr.write("flags provided but not defined: -max-turns\n")
        sys.exit(2)
    if "--version" in argv:
        emit("agy 1.4.7 (fake)")
        sys.exit(0)
    if "--help" in argv or "-h" in argv:
        sys.stderr.write(
            "Usage: agy [flags]\n"
            "  -p, --print, --prompt TEXT   run one prompt headless\n"
            "  --output-format text|json|stream-json\n"
            "  --print-timeout DURATION\n"
            "  --mode accept-edits|auto|plan\n"
            "  --sandbox\n"
            "  --dangerously-skip-permissions\n"
            "  --model M\n"
        )
        sys.exit(0)

    opts = parse_argv(argv)
    perm = permission_of(opts)

    record_path = os.environ.get("FAKE_AGY_RECORD")
    if record_path:
        record = {
            "argv": argv,
            "cwd": os.getcwd(),
            "env_names": sorted(os.environ.keys()),
            "permission": perm,
        }
        with open(record_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(record) + "\n")

    scenario = load_scenario()

    stderr_text = scenario.get("stderr", "")
    if stderr_text:
        sys.stderr.write(stderr_text)
        sys.stderr.flush()

    if scenario.get("spawn_child"):
        subprocess.Popen(
            ["python3", "-c", "import time; time.sleep(300)"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

    # file operations: executed only when a permission flag is present, soft-denied otherwise
    ops = [o for o in scenario.get("ops", []) if isinstance(o, dict) and o.get("op") == "write"]
    for o in ops:
        path = o.get("path", "")
        if perm == "request-review":
            sys.stderr.write(f"Tool call needs approval; denied in headless mode: write {path}\n")
            sys.stderr.flush()
        else:
            target = path if os.path.isabs(path) else os.path.join(os.getcwd(), path)
            parent = os.path.dirname(target)
            if parent:
                os.makedirs(parent, exist_ok=True)
            with open(target, "w", encoding="utf-8") as f:
                f.write(o.get("content", ""))

    cid = "fake-conv-0000"
    status = scenario.get("status", "SUCCESS")
    response = scenario.get("response", "OK\n")
    sleep_s = float(scenario.get("sleep_s", 0) or 0)

    # entries: (text, is_result)
    entries: list[tuple[str, bool]] = []
    if "lines" in scenario or "raw_lines" in scenario:
        for obj in scenario.get("lines", []):
            is_result = isinstance(obj, dict) and obj.get("event") == "result"
            entries.append((json.dumps(obj, ensure_ascii=False), is_result))
        for raw in scenario.get("raw_lines", []):
            entries.append((str(raw), False))
    else:
        entries.append((json.dumps({
            "event": "init",
            "conversation_id": cid,
            "init": {
                "cwd": os.getcwd(),
                "tools": ["run_command", "view_file", "write_to_file"],
                "permission_mode": perm,
            },
        }), False))
        idx = 0
        entries.append((json.dumps({"event": "step_update", "step_update": {
            "conversation_id": cid, "step_index": idx, "state": "DONE", "step_type": "user_input"}}), False))
        for o in ops:
            idx += 1
            entries.append((json.dumps({"event": "step_update", "step_update": {
                "conversation_id": cid, "step_index": idx, "state": "DONE", "step_type": "tool",
                "tool": {"name": "write_file", "path": o.get("path", "")}}}), False))
        idx += 1
        entries.append((json.dumps({"event": "step_update", "step_update": {
            "conversation_id": cid, "step_index": idx, "state": "DONE", "step_type": "agent_response",
            "text_delta": response, "duration_seconds": 0.5, "usage": USAGE}}, ensure_ascii=False), False))
        result: dict = {
            "conversation_id": cid,
            "status": status,
            "response": response,
            "duration_seconds": 0.6,
            "num_turns": 1,
            "usage": USAGE,
        }
        if status != "SUCCESS":
            result["error"] = scenario.get("error", stderr_text.strip() or "error")
        entries.append((json.dumps({"event": "result", "result": result}, ensure_ascii=False), True))

    delay = float(scenario.get("delay_ms", 0) or 0) / 1000.0
    final_newline = bool(scenario.get("final_newline", True))
    last = len(entries) - 1
    for i, (text, is_result) in enumerate(entries):
        if is_result and sleep_s > 0:
            time.sleep(sleep_s)
        emit(text, newline=(i != last or final_newline))
        if delay > 0 and i != last:
            time.sleep(delay)

    sys.exit(int(scenario.get("exit_code", 0) or 0))


if __name__ == "__main__":
    main()
