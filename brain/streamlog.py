from __future__ import annotations

import json
from pathlib import Path
import posixpath
import unicodedata
from typing import Iterator

EDIT_TOOLS: tuple[str, ...] = ("Edit", "Write", "MultiEdit", "NotebookEdit")
PROTOCOL_FILES: tuple[str, ...] = (".plan.md", ".qa-report.md")


class EventStream:
    def __init__(self, path: str | Path):
        self.path = Path(path)
        self.total_lines = 0
        self.bad_lines = 0

    def __iter__(self) -> Iterator[dict]:
        if not self.path.is_file():
            return
        with open(self.path, "r", encoding="utf-8", errors="replace") as f:
            for line in f:
                self.total_lines += 1
                line_s = line.strip()
                if not line_s:
                    continue
                try:
                    data = json.loads(line_s)
                except Exception:
                    self.bad_lines += 1
                    continue
                if not isinstance(data, dict):
                    self.bad_lines += 1
                    continue
                yield data


def iter_events(path: str | Path) -> EventStream:
    return EventStream(path)


def _normalize_tool_path(p: str, root: str | Path | None = None) -> str:
    p = unicodedata.normalize("NFC", str(p)).strip()
    if not p:
        return ""
    if root is not None:
        root_norm = posixpath.normpath(unicodedata.normalize("NFC", str(root)).strip())
        p_norm = posixpath.normpath(p)
        if p_norm == root_norm:
            return "."
        if p_norm.startswith(root_norm + "/"):
            return p_norm[len(root_norm) + 1 :]
        if not posixpath.isabs(p_norm):
            if p_norm.startswith("./"):
                p_norm = p_norm[2:]
            return p_norm
        try:
            p_path = Path(p).resolve()
            r_path = Path(root).resolve()
            rel = p_path.relative_to(r_path)
            return rel.as_posix()
        except (ValueError, RuntimeError):
            pass
    else:
        p_norm = posixpath.normpath(p)
        if not posixpath.isabs(p_norm):
            if p_norm.startswith("./"):
                return p_norm[2:]
            return p_norm
    return posixpath.normpath(p)


def summarize(
    path: str | Path,
    plan_files: list[str] | None = None,
    root: str | Path | None = None,
) -> dict:
    stream = iter_events(path)

    session_id: str | None = None
    model: str = ""
    assistant_turns = 0
    seen_msg_ids: set[str] = set()

    turns_before_first_edit: int | None = None
    first_edit_file: str | None = None

    reads: list[str] = []
    edits: list[str] = []
    tool_counts: dict[str, int] = {}

    result_event: dict | None = None

    for ev in stream:
        ev_type = ev.get("type")

        if ev_type == "system" and ev.get("subtype") == "init":
            if not session_id and ev.get("session_id"):
                session_id = ev.get("session_id")
            if not model and ev.get("model"):
                model = str(ev.get("model"))

        elif ev_type == "assistant":
            msg = ev.get("message")
            msg_id = None
            content_list: list = []

            if isinstance(msg, dict):
                msg_id = msg.get("id")
                content = msg.get("content")
                if isinstance(content, list):
                    content_list = content
                if not model and msg.get("model"):
                    model = str(msg.get("model"))
            elif "content" in ev and isinstance(ev["content"], list):
                content_list = ev["content"]

            if not msg_id:
                msg_id = ev.get("id")

            if msg_id:
                if msg_id not in seen_msg_ids:
                    seen_msg_ids.add(msg_id)
                    assistant_turns += 1
            else:
                assistant_turns += 1

            for item in content_list:
                if not isinstance(item, dict):
                    continue
                if item.get("type") == "tool_use":
                    tool_name = item.get("name")
                    if tool_name:
                        tool_counts[tool_name] = tool_counts.get(tool_name, 0) + 1

                    tool_input = item.get("input")
                    if isinstance(tool_input, dict):
                        raw_path = (
                            tool_input.get("file_path")
                            or tool_input.get("notebook_path")
                            or tool_input.get("path")
                        )
                        if raw_path:
                            norm_p = _normalize_tool_path(raw_path, root)
                            if tool_name == "Read":
                                if norm_p not in reads:
                                    reads.append(norm_p)
                            elif tool_name in EDIT_TOOLS:
                                if norm_p not in edits:
                                    edits.append(norm_p)
                                is_proto = (
                                    norm_p in PROTOCOL_FILES
                                    or posixpath.basename(norm_p) in PROTOCOL_FILES
                                )
                                if not is_proto and turns_before_first_edit is None:
                                    turns_before_first_edit = assistant_turns - 1
                                    first_edit_file = norm_p

        elif ev_type == "result":
            result_event = ev
            if not session_id and ev.get("session_id"):
                session_id = ev.get("session_id")

    # Format result and usage
    format_ok = result_event is not None
    if result_event is not None:
        usage_obj = result_event.get("usage")
        if not isinstance(usage_obj, dict):
            usage_obj = {}
        usage = {
            "input_tokens": usage_obj.get("input_tokens", 0),
            "output_tokens": usage_obj.get("output_tokens", 0),
            "cache_read_input_tokens": usage_obj.get("cache_read_input_tokens", 0),
            "cache_creation_input_tokens": usage_obj.get("cache_creation_input_tokens", 0),
        }
        denials = result_event.get("permission_denials")
        permission_denials = len(denials) if isinstance(denials, list) else 0
        result_dict: dict | None = {
            "subtype": result_event.get("subtype"),
            "is_error": bool(result_event.get("is_error", False)),
            "num_turns": result_event.get("num_turns", 0),
            "total_cost_usd": result_event.get("total_cost_usd", 0.0),
            "duration_ms": result_event.get("duration_ms", 0),
        }
    else:
        usage = {
            "input_tokens": 0,
            "output_tokens": 0,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0,
        }
        permission_denials = 0
        result_dict = None

    # Reads outside plan
    reads_outside_plan: list[str] | None = None
    if plan_files is not None:
        norm_plan_files = {
            _normalize_tool_path(f, root) for f in plan_files
        }
        reads_outside_plan = []
        for r in reads:
            if r not in PROTOCOL_FILES and posixpath.basename(r) not in PROTOCOL_FILES:
                if r not in norm_plan_files:
                    reads_outside_plan.append(r)

    return {
        "session_id": session_id or "",
        "model": model,
        "assistant_turns": assistant_turns,
        "turns_before_first_edit": turns_before_first_edit,
        "first_edit_file": first_edit_file,
        "reads": reads,
        "reads_outside_plan": reads_outside_plan,
        "edits": edits,
        "tool_counts": tool_counts,
        "permission_denials": permission_denials,
        "result": result_dict,
        "usage": usage,
        "format_ok": format_ok,
        "bad_lines": stream.bad_lines,
        "total_lines": stream.total_lines,
    }
