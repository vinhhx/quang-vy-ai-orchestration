#!/usr/bin/env python3
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import time

NESTED_SESSION_VARS = (
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "CLAUDE_VC_ENV",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
)


def main() -> None:
    record_path = os.environ.get("FAKE_CLAUDE_RECORD")
    if record_path:
        present_vars = [v for v in NESTED_SESSION_VARS if v in os.environ]
        record = {
            "argv": sys.argv[1:],
            "cwd": os.getcwd(),
            "claude_config_dir": os.environ.get("CLAUDE_CONFIG_DIR"),
            "present_vars": present_vars,
        }
        with open(record_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(record) + "\n")

    scenario: dict = {}
    scenario_path = os.environ.get("FAKE_CLAUDE_SCENARIO")
    if scenario_path and os.path.isfile(scenario_path):
        try:
            with open(scenario_path, "r", encoding="utf-8") as f:
                scenario = json.load(f)
        except Exception as e:
            sys.stderr.write(f"Lỗi đọc FAKE_CLAUDE_SCENARIO: {e}\n")

    if "stderr" in scenario:
        sys.stderr.write(scenario["stderr"])
        sys.stderr.flush()

    if "sleep" in scenario:
        time.sleep(float(scenario["sleep"]))

    raw_lines = scenario.get("raw_lines")
    events = scenario.get("events")
    ops = scenario.get("ops", [])

    tool_events: list[dict] = []
    for i, op in enumerate(ops):
        op_type = op.get("op")
        if op_type == "write":
            target_path = Path(op["path"])
            if not target_path.is_absolute():
                target_path = Path(os.getcwd()) / target_path
            existed = target_path.exists()
            tool_name = "Edit" if existed else "Write"
            target_path.parent.mkdir(parents=True, exist_ok=True)
            mode = "a" if op.get("append") else "w"
            with open(target_path, mode, encoding="utf-8") as f:
                f.write(op.get("content", ""))
            tool_input = {"file_path": str(target_path.resolve())}
        elif op_type == "read":
            target_path = Path(op["path"])
            if not target_path.is_absolute():
                target_path = Path(os.getcwd()) / target_path
            tool_name = "Read"
            tool_input = {"file_path": str(target_path.resolve())}
        elif op_type == "git":
            tool_name = "Bash"
            args = op.get("args", [])
            subprocess.run(["git", *args], cwd=os.getcwd(), capture_output=True)
            tool_input = {"command": " ".join(["git"] + [str(a) for a in args])}
        else:
            continue

        asst_event = {
            "type": "assistant",
            "session_id": "fake-session",
            "message": {
                "id": f"msg_{i}",
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": f"toolu_{i}",
                        "name": tool_name,
                        "input": tool_input,
                    }
                ],
            },
        }
        user_event = {
            "type": "user",
            "message": {
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": f"toolu_{i}",
                        "content": "ok",
                    }
                ]
            },
        }
        tool_events.append(asst_event)
        tool_events.append(user_event)

    if raw_lines is not None:
        for line in raw_lines:
            sys.stdout.write(line if line.endswith("\n") else line + "\n")
            sys.stdout.flush()
    elif events is not None:
        for ev in events:
            line = json.dumps(ev) if isinstance(ev, dict) else str(ev)
            sys.stdout.write(line if line.endswith("\n") else line + "\n")
            sys.stdout.flush()
    else:
        init_event = {
            "type": "system",
            "subtype": "init",
            "session_id": "fake-session",
            "model": "fake",
        }
        sys.stdout.write(json.dumps(init_event) + "\n")
        sys.stdout.flush()

        for ev in tool_events:
            sys.stdout.write(json.dumps(ev) + "\n")
            sys.stdout.flush()

        result_event = {
            "type": "result",
            "subtype": "success",
            "is_error": False,
            "num_turns": len(ops) + 1,
            "session_id": "fake-session",
            "total_cost_usd": 0,
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "cache_read_input_tokens": 0,
                "cache_creation_input_tokens": 0,
            },
            "permission_denials": [],
        }
        sys.stdout.write(json.dumps(result_event) + "\n")
        sys.stdout.flush()

    exit_code = scenario.get("exit_code", 0)
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
