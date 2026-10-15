from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import unicodedata

from brain.config import protocol_log_dir
from brain.planfile import has_blockers, parse_plan, parse_qa_verdict, plan_files, read_meta
from brain import streamlog

SCRIPT_LINES: tuple[str, ...] = (
    "- Task:",
    "- Dự án:",
    "- Số turn của Dev trước khi bắt đầu sửa:",
    "- Dev có đọc file ngoài plan không:",
)

USER_LINES: tuple[str, ...] = (
    "- QA phát hiện được gì:",
    "- Phải làm lại không, vì sao:",
)

TURN_FOOTER = (
    "> Định nghĩa turn: một assistant turn là một message.id riêng biệt trên dòng "
    "type=assistant. Turn trước khi sửa là số turn trợ lý trước công cụ Edit/Write "
    "đầu tiên sửa file ngoài giao thức."
)


def log_path(cfg: dict, project: dict, task: str) -> Path:
    return protocol_log_dir(cfg) / f"{project['id']}--{task}.md"


def _format_raw_log_path(path: Path | str) -> str:
    p_str = str(path)
    home = str(Path.home())
    if p_str.startswith(home):
        return "~" + p_str[len(home) :]
    return p_str


def _count_guard_blocks(run_dir: Path) -> int:
    guard_path = run_dir / "guard.jsonl"
    if not guard_path.is_file():
        return 0
    blocks = 0
    try:
        with open(guard_path, "r", encoding="utf-8", errors="replace") as f:
            for line in f:
                line_s = line.strip()
                if not line_s:
                    continue
                try:
                    data = json.loads(line_s)
                    if isinstance(data, dict) and data.get("decision") == "block":
                        blocks += 1
                except Exception:
                    pass
    except Exception:
        pass
    return blocks


def _parse_existing_log(text: str) -> dict:
    parts = text.split("## Số liệu tự động (brain ghi, không sửa tay)", 1)
    header_section = parts[0]
    auto_section = parts[1] if len(parts) > 1 else ""

    lines_map = {}
    current_key = None
    accumulated_lines = []

    all_keys = list(SCRIPT_LINES) + list(USER_LINES)

    for line in header_section.splitlines():
        matched_key = None
        for k in all_keys:
            if line.startswith(k):
                matched_key = k
                break
        if matched_key:
            if current_key:
                lines_map[current_key] = "\n".join(accumulated_lines)
            current_key = matched_key
            val = line[len(matched_key) :].strip()
            accumulated_lines = [val] if val else []
        elif current_key:
            accumulated_lines.append(line)

    if current_key:
        lines_map[current_key] = "\n".join(accumulated_lines)

    existing_rows = []
    if auto_section:
        for line in auto_section.splitlines():
            s = line.strip()
            if s.startswith("|") and not s.startswith("| Thời điểm") and not re.match(r"^\|(?:\s*:?-+:?\s*\|)+$", s):
                existing_rows.append(s)

    return {
        "lines_map": lines_map,
        "existing_rows": existing_rows,
    }


def record_run(
    project: dict,
    task: str,
    role: str,
    result: Any,
    cfg: dict,
) -> Path:
    proj_path = Path(os.path.expanduser(project["path"])).resolve()

    plan_file = proj_path / ".plan.md"
    plan_meta = None
    p_files = None
    has_block = False

    if plan_file.is_file():
        try:
            plan_text = plan_file.read_text(encoding="utf-8")
            plan_meta = read_meta(plan_text)
            plan_obj = parse_plan(plan_text)
            p_files = plan_files(plan_obj)
            has_block = has_blockers(plan_obj)
        except Exception:
            pass

    qa_verdict = None
    qa_file = proj_path / ".qa-report.md"
    if qa_file.is_file():
        try:
            qa_text = qa_file.read_text(encoding="utf-8")
            qa_verdict = parse_qa_verdict(qa_text)
        except Exception:
            pass

    s = streamlog.summarize(
        result.raw_log,
        plan_files=p_files if role == "dev" else None,
        root=proj_path,
    )

    guard_blocks = _count_guard_blocks(result.run_dir)
    blocked_calls = s.get("permission_denials", 0) + guard_blocks

    target_path = log_path(cfg, project, task)
    target_path.parent.mkdir(parents=True, exist_ok=True)

    if target_path.is_file():
        old_text = target_path.read_text(encoding="utf-8")
        parsed = _parse_existing_log(old_text)
        lines_map = parsed["lines_map"]
        existing_rows = parsed["existing_rows"]
    else:
        tmpl_path = Path(__file__).resolve().parent / "templates" / "tasklog.md"
        if tmpl_path.is_file():
            tmpl_text = tmpl_path.read_text(encoding="utf-8")
        else:
            tmpl_text = "\n".join(SCRIPT_LINES + USER_LINES) + "\n"
        parsed = _parse_existing_log(tmpl_text)
        lines_map = parsed["lines_map"]
        existing_rows = []

    # Update Task and Dự án if empty
    task_label = "- Task:"
    if not lines_map.get(task_label):
        req = plan_meta.get("request") if isinstance(plan_meta, dict) else None
        lines_map[task_label] = f"{task} — {req}" if req else task

    proj_label = "- Dự án:"
    if not lines_map.get(proj_label):
        p_id = project.get("id", "")
        name_vi = project.get("name_vi", "")
        if name_vi and name_vi != p_id:
            lines_map[proj_label] = f"{p_id} ({name_vi})"
        else:
            lines_map[proj_label] = p_id

    # Rewrite Dev lines only for role == "dev"
    turn_label = "- Số turn của Dev trước khi bắt đầu sửa:"
    read_label = "- Dev có đọc file ngoài plan không:"

    if role == "dev":
        if has_block or result.exit_code == 4:
            lines_map[turn_label] = "chưa sửa code (Vướng mắc)"
        elif s.get("turns_before_first_edit") is not None:
            lines_map[turn_label] = str(s["turns_before_first_edit"])
        else:
            lines_map[turn_label] = "-"

        out_reads = s.get("reads_outside_plan")
        if out_reads:
            lines_map[read_label] = f"Có: {', '.join(out_reads)}"
        else:
            lines_map[read_label] = "Không"

    # Format new row
    try:
        fin_str = result.finished_at.replace("Z", "+00:00")
        dt = datetime.fromisoformat(fin_str).astimezone()
        ts_str = dt.strftime("%Y-%m-%d %H:%M")
    except Exception:
        ts_str = result.finished_at[:16]

    if role == "dev":
        turns_before_str = (
            str(s["turns_before_first_edit"])
            if s.get("turns_before_first_edit") is not None
            else "-"
        )
        if has_block:
            turns_before_str = "chưa sửa"
        reads_col_str = (
            f"Có: {', '.join(s['reads_outside_plan'])}"
            if s.get("reads_outside_plan")
            else "Không"
        )
    else:
        turns_before_str = "-"
        reads_col_str = "-"

    usage = s.get("usage", {})
    tokens_str = f"{usage.get('input_tokens', 0)}/{usage.get('output_tokens', 0)}"
    cache_str = f"{usage.get('cache_read_input_tokens', 0)}/{usage.get('cache_creation_input_tokens', 0)}"

    if result.interrupted:
        status_str = "Ctrl+C"
    elif result.timed_out:
        status_str = "timeout"
    elif role == "dev" and (has_block or result.exit_code == 4):
        status_str = "Vướng mắc"
    elif role == "qa" and qa_verdict:
        status_str = qa_verdict
    elif result.exit_code != 0:
        status_str = f"exit {result.exit_code}"
    else:
        status_str = "Đạt" if role == "qa" else "-"

    raw_path_str = _format_raw_log_path(result.raw_log)

    new_row = (
        f"| {ts_str} | {role} | {result.exit_code} | {s['assistant_turns']} | "
        f"{turns_before_str} | {reads_col_str} | {tokens_str} | {cache_str} | "
        f"{blocked_calls} | {status_str} | {raw_path_str} |"
    )

    all_rows = existing_rows + [new_row]

    lead_n = sum(1 for r in all_rows if "| lead |" in r)
    dev_m = sum(1 for r in all_rows if "| dev |" in r)
    qa_k = sum(1 for r in all_rows if "| qa |" in r)
    count_line = f"Số lần chạy: lead {lead_n} · dev {dev_m} · qa {qa_k}"

    # Assemble output text
    out_lines = [f"# Nhật ký task: {task}", ""]
    for k in SCRIPT_LINES + USER_LINES:
        val = lines_map.get(k, "")
        if "\n" in val:
            out_lines.append(f"{k} {val}")
        else:
            out_lines.append(f"{k} {val}".rstrip())

    out_lines.extend([
        "",
        "## Số liệu tự động (brain ghi, không sửa tay)",
        "",
        count_line,
        "",
        "| Thời điểm | Vai trò | Exit | Turns | Turn trước sửa | Đọc ngoài plan | Token in/out | Cache read/create | Bị chặn | Kết quả | Raw log |",
        "|---|---|---|---|---|---|---|---|---|---|---|",
    ])
    out_lines.extend(all_rows)
    out_lines.extend(["", TURN_FOOTER, ""])

    final_content = "\n".join(out_lines)

    # Atomic write
    fd, tmp_path = tempfile.mkstemp(
        prefix="tasklog_", suffix=".tmp", dir=str(target_path.parent)
    )
    with open(fd, "w", encoding="utf-8") as f:
        f.write(final_content)
    os.replace(tmp_path, target_path)

    turn_val = (
        str(s["turns_before_first_edit"])
        if s.get("turns_before_first_edit") is not None
        else "-"
    )
    read_val = (
        ("Có" if s["reads_outside_plan"] else "Không")
        if s.get("reads_outside_plan") is not None
        else "-"
    )
    sys.stderr.write(
        f"[brain] nhật ký: {target_path} · turn trước sửa {turn_val} · "
        f"đọc ngoài plan {read_val} · token {tokens_str}\n"
    )
    sys.stderr.flush()

    return target_path


def check_logs(directory: str | Path, min_complete: int = 5) -> int:
    d = Path(directory).resolve()
    if not d.is_dir():
        sys.stderr.write(f"Thư mục không tồn tại: {d}\n")
        return 1

    task_files = sorted(f for f in d.iterdir() if f.is_file() and "--" in f.name and f.name.endswith(".md"))
    total = len(task_files)
    complete = 0

    all_keys = list(SCRIPT_LINES) + list(USER_LINES)

    for p in task_files:
        try:
            text = p.read_text(encoding="utf-8")
        except Exception as e:
            sys.stderr.write(f"Không đọc được file {p.name}: {e}\n")
            continue

        parsed = _parse_existing_log(text)
        missing_labels = []
        for k in all_keys:
            val = parsed["lines_map"].get(k, "").strip()
            if not val:
                missing_labels.append(k)

        if missing_labels:
            labels_str = ", ".join(missing_labels)
            sys.stderr.write(f"{p.name}: thiếu {labels_str}\n")
        else:
            complete += 1

    sys.stdout.write(f"{complete}/{total} nhật ký đủ 6 dòng\n")
    sys.stdout.flush()

    return 0 if complete >= min_complete else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python3 -m brain.tasklog", description="Kiểm tra nhật ký task"
    )
    subparsers = parser.add_subparsers(dest="subcommand")
    p_check = subparsers.add_parser("check", help="Kiểm tra các nhật ký task trong thư mục")
    p_check.add_argument("dir", help="Thư mục chứa nhật ký (protocol-log)")
    p_check.add_argument("--min", type=int, default=5, help="Số nhật ký tối thiểu đủ 6 dòng")

    args = parser.parse_args(argv)
    if args.subcommand != "check":
        parser.print_help(sys.stderr)
        return 1

    return check_logs(args.dir, min_complete=args.min)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
