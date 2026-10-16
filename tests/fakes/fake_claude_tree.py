#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import signal
import subprocess
import sys
import time


def main() -> None:
    record_path = os.environ.get("FAKE_CLAUDE_RECORD")
    if record_path:
        record = {
            "argv": sys.argv[1:],
            "cwd": os.getcwd(),
            "claude_config_dir": os.environ.get("CLAUDE_CONFIG_DIR"),
            "fake": "tree",
        }
        try:
            with open(record_path, "a", encoding="utf-8") as f:
                f.write(json.dumps(record) + "\n")
        except Exception:
            pass

    pidfile = os.environ.get("FAKE_TREE_PIDFILE")
    mode = os.environ.get("FAKE_TREE_MODE", "coop")
    sleep_secs = os.environ.get("FAKE_TREE_SLEEP", "317")

    # 1. Print system/init line
    init_event = {
        "type": "system",
        "subtype": "init",
        "session_id": "fake-tree-session",
        "model": "fake",
    }
    sys.stdout.write(json.dumps(init_event) + "\n")
    sys.stdout.flush()

    # 2. Start bash child with sleep in new session
    # Trailing '; true' prevents bash from exec'ing sleep, maintaining bash -> sleep tree
    bash_proc = subprocess.Popen(
        ["bash", "-c", f"sleep {sleep_secs}; true"],
        start_new_session=True,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    # 3. Print assistant line with Bash tool_use and usage
    assistant_event = {
        "type": "assistant",
        "session_id": "fake-tree-session",
        "message": {
            "id": "msg_tree_1",
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_tree_1",
                    "name": "Bash",
                    "input": {"command": f"sleep {sleep_secs}"},
                }
            ],
            "usage": {
                "input_tokens": 2,
                "output_tokens": 8,
                "cache_read_input_tokens": 100,
                "cache_creation_input_tokens": 50,
            },
        },
    }
    sys.stdout.write(json.dumps(assistant_event) + "\n")
    sys.stdout.flush()

    # 4. Write pidfile atomically
    if pidfile:
        temp_pidfile = f"{pidfile}.tmp.{os.getpid()}"
        with open(temp_pidfile, "w", encoding="utf-8") as f:
            f.write(f"{os.getpid()} {bash_proc.pid}\n")
        os.replace(temp_pidfile, pidfile)

    # 5. Configure signal handling
    if mode == "coop":
        def _handle_sigterm(signum, frame):
            try:
                os.killpg(bash_proc.pid, signal.SIGTERM)
            except (ProcessLookupError, PermissionError):
                pass
            try:
                bash_proc.wait(timeout=2.0)
            except Exception:
                pass

            result_event = {
                "type": "result",
                "subtype": "success",
                "is_error": False,
                "num_turns": 1,
                "session_id": "fake-tree-session",
                "total_cost_usd": 0.001,
                "usage": {
                    "input_tokens": 3,
                    "output_tokens": 9,
                    "cache_read_input_tokens": 100,
                    "cache_creation_input_tokens": 50,
                },
                "permission_denials": [],
            }
            sys.stdout.write(json.dumps(result_event) + "\n")
            sys.stdout.flush()
            os._exit(143)

        signal.signal(signal.SIGTERM, _handle_sigterm)
    elif mode == "ignore":
        signal.signal(signal.SIGTERM, signal.SIG_IGN)

    # 6. Idle loop
    while True:
        time.sleep(0.1)


if __name__ == "__main__":
    main()
