from __future__ import annotations

import os
from pathlib import Path
import re
import shlex
import sys
import unicodedata

from brain import (
    BrainError,
    EXIT_BLOCKED,
    EXIT_ERROR,
    EXIT_INTERRUPTED,
    EXIT_OK,
    EXIT_PROTOCOL,
)
from brain.agent import RoleSpec, abs_rules, build_command, execute_role, new_run_dir
from brain.config import load_config, repo_root
from brain import gitops
from brain.guard import write_hook_settings
from brain.planfile import has_blockers, parse_plan, plan_files, read_meta, section_text, validate_plan
from brain.projects import require_runnable, select_project

DEV_DISALLOWED_TOOLS: list[str] = [
    "Grep",
    "Glob",
    "WebFetch",
    "WebSearch",
    "NotebookEdit",
    "Task",
]


def dev_allowed_tools(files: list[str], root: Path | None = None) -> list[str]:
    all_files = list(files)
    if ".plan.md" not in all_files:
        all_files.append(".plan.md")
    tools: list[str] = []
    for f in all_files:
        clean_f = f[2:] if f.startswith("./") else f
        tools.extend([f"Read(./{clean_f})", f"Edit(./{clean_f})", f"Write(./{clean_f})"])
        if root is not None:
            tools.extend(abs_rules(root, [clean_f], ("Read", "Edit", "Write")))
    tools.extend(["Bash(git add *)", "Bash(git commit *)"])
    return tools


def cmd_dev(args) -> int:
    cfg = load_config()
    project = select_project(args.project)
    require_runnable(project, cfg)

    proj_path = Path(os.path.expanduser(project["path"])).resolve()

    plan_path = proj_path / ".plan.md"
    if not plan_path.is_file():
        raise BrainError("Không tìm thấy .plan.md ở gốc dự án", EXIT_ERROR)

    text = plan_path.read_text(encoding="utf-8")
    meta = read_meta(text)
    if not meta or not meta.get("task"):
        raise BrainError("File .plan.md không có metadata hợp lệ", EXIT_ERROR)

    task = meta["task"]
    expected_branch = meta.get("branch") or f"task/{task}"

    curr_branch = gitops.current_branch(proj_path)
    if not curr_branch or not curr_branch.startswith("task/") or curr_branch != expected_branch:
        raise BrainError(
            f"Không ở nhánh task '{expected_branch}' (nhánh hiện tại: '{curr_branch}') — không đoán",
            EXIT_ERROR,
        )

    dirty = gitops.changed_paths(proj_path)
    if dirty:
        raise BrainError(
            f"Working tree có thay đổi chưa commit — hãy commit thay đổi tay với message '{task}: plan':\n"
            + "\n".join(f"  {p}" for p in sorted(dirty)),
            EXIT_ERROR,
        )

    plan = parse_plan(text)
    errors = validate_plan(plan, proj_path)
    files = plan_files(plan)
    if errors or not files:
        err_msg = (
            "Kế hoạch trong .plan.md không hợp lệ:\n"
            + "\n".join(f"  - {e}" for e in errors)
            if errors
            else "Kế hoạch không có file nào trong bảng Thay đổi"
        )
        raise BrainError(err_msg, EXIT_ERROR)

    if has_blockers(plan):
        raise BrainError(
            "Vướng mắc chưa xử lý trong .plan.md — hãy xử lý trước khi chạy dev",
            EXIT_ERROR,
        )

    allowed = dev_allowed_tools(files, root=proj_path)

    if getattr(args, "dry_run", False):
        print(f"Allowlist: {', '.join(files)}")
        spec_dry = RoleSpec(
            role="dev",
            prompt_file=repo_root() / "brain" / "prompts" / "dev.md",
            user_input=(
                f"Task: {task}\n"
                "Đọc .plan.md rồi sửa đúng các mục trong bảng Thay đổi.\n"
                f"File được đọc và sửa: {', '.join(files)}\n"
                f'Commit với message: "{task}: <mô tả ngắn>".'
            ),
            allowed_tools=allowed,
            disallowed_tools=DEV_DISALLOWED_TOOLS,
            extra_args=["--settings", "<run_dir/settings.json>"],
        )
        cmd = build_command(spec_dry, cfg)
        print(f"Command: {shlex.join(cmd)}")
        return 0

    dev_base = gitops.head_sha(proj_path)
    run_dir = new_run_dir(cfg, project, task, "dev")
    read_allow = [(proj_path / f).resolve() for f in files] + [(proj_path / ".plan.md").resolve()]
    write_allow = list(read_allow)
    settings = write_hook_settings(
        run_dir=run_dir,
        root=proj_path,
        read_allow=read_allow,
        write_allow=write_allow,
    )

    spec = RoleSpec(
        role="dev",
        prompt_file=repo_root() / "brain" / "prompts" / "dev.md",
        user_input=(
            f"Task: {task}\n"
            "Đọc .plan.md rồi sửa đúng các mục trong bảng Thay đổi.\n"
            f"File được đọc và sửa: {', '.join(files)}\n"
            f'Commit với message: "{task}: <mô tả ngắn>".'
        ),
        allowed_tools=allowed,
        disallowed_tools=DEV_DISALLOWED_TOOLS,
        extra_args=["--settings", str(settings)],
    )

    res = execute_role(spec, project, task, cfg, run_dir=run_dir)

    changed_now = gitops.changed_paths(proj_path)
    if res.timed_out:
        if changed_now:
            sys.stderr.write("Các file đã thay đổi:\n" + "\n".join(f"  {p}" for p in sorted(changed_now)) + "\n")
        raise BrainError(f"Dev chạy quá thời gian (timeout) trên task '{task}'", EXIT_ERROR)
    if res.interrupted:
        if changed_now:
            sys.stderr.write("Các file đã thay đổi:\n" + "\n".join(f"  {p}" for p in sorted(changed_now)) + "\n")
        raise BrainError("Dev bị ngắt bởi người dùng", EXIT_INTERRUPTED)
    if res.exit_code != 0:
        if changed_now:
            sys.stderr.write("Các file đã thay đổi:\n" + "\n".join(f"  {p}" for p in sorted(changed_now)) + "\n")
        raise BrainError(f"Dev thất bại với mã lỗi {res.exit_code} trên task '{task}'", EXIT_ERROR)

    diff_tree = gitops.diff_names(proj_path, dev_base, "HEAD")
    uncommitted = gitops.changed_paths(proj_path)
    all_changed = diff_tree | uncommitted
    all_changed_nfc = {unicodedata.normalize("NFC", p) for p in all_changed}
    allowed_nfc = {unicodedata.normalize("NFC", f) for f in files} | {".plan.md"}
    violations = all_changed_nfc - allowed_nfc
    if violations:
        print("Dev đã sửa file ngoài plan — vi phạm giao thức (ROLE-06):")
        for p in sorted(violations):
            print(f"  {p}")
        print("brain không tự hoàn tác")
        return EXIT_PROTOCOL

    plan_curr_text = plan_path.read_text(encoding="utf-8")
    plan_curr = parse_plan(plan_curr_text)
    has_block = has_blockers(plan_curr)

    pending = gitops.changed_paths(proj_path)
    if pending:
        if has_block:
            msg = f"{task}: vướng mắc"
        else:
            muctieu = plan_curr.sections.get("Mục tiêu", "")
            short = ""
            for line in muctieu.splitlines():
                line_s = line.strip()
                line_s = re.sub(r"^[-*+\d.]+\s*", "", line_s).strip()
                if line_s:
                    short = line_s[:72]
                    break
            if not short:
                short = "dev"
            msg = f"{task}: {short}"
        gitops.commit_paths(proj_path, sorted(pending), msg)

    commits = gitops.commit_subjects(proj_path, dev_base, "HEAD")
    for sha, subj in commits:
        if not subj.startswith(f"{task}: "):
            sys.stderr.write(f"CẢNH BÁO: Commit {sha[:7]} có subject không đúng chuẩn '{task}: ...': '{subj}'\n")
            sys.stderr.flush()

    final_changed = gitops.diff_names(proj_path, dev_base, "HEAD")
    if final_changed:
        print("Các file đã sửa:")
        for p in sorted(final_changed):
            print(f"  {p}")

    kt_text = section_text(plan_curr_text, "Cách kiểm tra")
    print(f"Cách kiểm tra (brain không tự chạy — hãy chạy tay trong {proj_path}):\n{kt_text}")

    if has_block:
        vm_text = section_text(plan_curr_text, "Vướng mắc")
        print(f"Vướng mắc:\n{vm_text}")
        print("Bước tiếp theo: sửa .plan.md hoặc chạy lại brain lead")
        return EXIT_BLOCKED

    return EXIT_OK
