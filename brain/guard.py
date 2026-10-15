from __future__ import annotations

from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shlex
import sys
import unicodedata

WRITE_TOOLS = ("Write", "Edit", "MultiEdit", "NotebookEdit")
READ_TOOLS = ("Read", "Grep", "Glob")
GUARDED_TOOLS = WRITE_TOOLS + READ_TOOLS


def _append_log(log_path: str | None, tool: str, path: str, decision: str, reason: str) -> None:
    if not log_path:
        return
    try:
        p = Path(log_path).resolve()
        p.parent.mkdir(parents=True, exist_ok=True)
        entry = {
            "ts": datetime.now(timezone.utc).isoformat(),
            "tool": tool,
            "path": path,
            "decision": decision,
            "reason": reason,
        }
        with open(p, "a", encoding="utf-8") as f:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")
    except Exception:
        pass


def write_hook_settings(
    run_dir: str | Path,
    root: str | Path,
    read_allow: list[str | Path] | None,
    write_allow: list[str | Path],
) -> Path:
    run_dir_path = Path(run_dir).resolve()
    run_dir_path.mkdir(parents=True, exist_ok=True)

    root_resolved = unicodedata.normalize("NFC", str(Path(root).resolve()))
    norm_write = [
        unicodedata.normalize("NFC", str(Path(p).resolve())) for p in write_allow
    ]
    norm_read = (
        [unicodedata.normalize("NFC", str(Path(p).resolve())) for p in read_allow]
        if read_allow is not None
        else None
    )

    log_path = (run_dir_path / "guard.jsonl").resolve()
    policy_path = run_dir_path / "policy.json"
    settings_path = run_dir_path / "settings.json"

    policy_data = {
        "root": root_resolved,
        "read_allow": norm_read,
        "write_allow": norm_write,
        "log": str(log_path),
    }
    policy_path.write_text(
        json.dumps(policy_data, indent=2, ensure_ascii=False), encoding="utf-8"
    )

    guard_py = Path(__file__).resolve()
    cmd = (
        f"{shlex.quote(sys.executable)} -I {shlex.quote(str(guard_py))} "
        f"--policy {shlex.quote(str(policy_path))}"
    )

    settings_data = {
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob",
                    "command": cmd,
                }
            ]
        }
    }
    settings_path.write_text(
        json.dumps(settings_data, indent=2, ensure_ascii=False), encoding="utf-8"
    )
    return settings_path


def main(argv: list[str] | None = None) -> int:
    if argv is None:
        argv = sys.argv[1:]

    policy_file: str | None = None
    i = 0
    while i < len(argv):
        if argv[i] == "--policy" and i + 1 < len(argv):
            policy_file = argv[i + 1]
            i += 2
        else:
            i += 1

    if not policy_file:
        sys.stderr.write("brain guard: thiếu đối số --policy\n")
        return 2

    try:
        with open(policy_file, "r", encoding="utf-8") as f:
            policy = json.load(f)
        if not isinstance(policy, dict):
            return 2
    except Exception as e:
        sys.stderr.write(f"brain guard: không thể đọc policy: {e}\n")
        return 2

    log_path = policy.get("log")

    try:
        raw_stdin = sys.stdin.read()
        if not raw_stdin.strip():
            return 2
        payload = json.loads(raw_stdin)
        if not isinstance(payload, dict):
            return 2
    except Exception as e:
        sys.stderr.write(f"brain guard: stdin không hợp lệ: {e}\n")
        return 2

    tool_name = payload.get("tool_name", "")
    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict):
        tool_input = {}
    cwd = payload.get("cwd") or policy.get("root", "")

    if tool_name not in GUARDED_TOOLS:
        _append_log(log_path, tool_name, "", "allow", f"tool {tool_name} not guarded")
        return 0

    raw_path: str | None = None
    if tool_name in WRITE_TOOLS:
        if tool_name == "NotebookEdit":
            raw_path = tool_input.get("notebook_path")
        else:
            raw_path = tool_input.get("file_path")
        if not raw_path:
            sys.stderr.write(
                f"brain guard: chặn {tool_name} <none> — ngoài danh sách được phép của vai trò (xem .plan.md)\n"
            )
            _append_log(
                log_path, tool_name, "", "block", "missing path for write-class tool"
            )
            return 2
    elif tool_name in READ_TOOLS:
        if tool_name in ("Grep", "Glob"):
            raw_path = tool_input.get("path")
            if not raw_path:
                raw_path = cwd
        else:
            raw_path = tool_input.get("file_path")
            if not raw_path:
                sys.stderr.write(
                    f"brain guard: chặn {tool_name} <none> — ngoài danh sách được phép của vai trò (xem .plan.md)\n"
                )
                _append_log(
                    log_path, tool_name, "", "block", "missing file_path for Read"
                )
                return 2

    raw_path_str = str(raw_path)
    norm_path = unicodedata.normalize("NFC", raw_path_str)
    if not os.path.isabs(norm_path):
        base_dir = cwd or policy.get("root", "")
        joined = os.path.join(base_dir, norm_path)
    else:
        joined = norm_path
    resolved = unicodedata.normalize("NFC", os.path.realpath(joined))

    write_allow_list = policy.get("write_allow") or []
    write_allow = {
        unicodedata.normalize("NFC", os.path.realpath(p)) for p in write_allow_list
    }

    read_allow_list = policy.get("read_allow")
    read_allow = (
        {unicodedata.normalize("NFC", os.path.realpath(p)) for p in read_allow_list}
        if read_allow_list is not None
        else None
    )

    decision = "block"
    reason = "unspecified"

    if tool_name in WRITE_TOOLS:
        if resolved in write_allow:
            decision = "allow"
            reason = "in write allowlist"
        else:
            decision = "block"
            reason = f"path {resolved} not in write allowlist"
    else:  # READ_TOOLS
        if read_allow is None:
            decision = "allow"
            reason = "read-free mode"
        else:
            if tool_name in ("Grep", "Glob"):
                decision = "block"
                reason = f"{tool_name} prohibited when read_allow is set"
            elif resolved in read_allow:
                decision = "allow"
                reason = "in read allowlist"
            else:
                decision = "block"
                reason = f"path {resolved} not in read allowlist"

    _append_log(log_path, tool_name, raw_path_str, decision, reason)

    if decision == "block":
        sys.stderr.write(
            f"brain guard: chặn {tool_name} {raw_path_str} — ngoài danh sách được phép của vai trò (xem .plan.md)\n"
        )
        sys.stderr.flush()
        return 2

    return 0


if __name__ == "__main__":
    sys.exit(main())
