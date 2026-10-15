from __future__ import annotations

import os
from pathlib import Path

from brain import (
    BrainError,
    EXIT_ERROR,
    EXIT_INTERRUPTED,
    EXIT_OK,
    EXIT_PROTOCOL,
)
from brain.agent import RoleSpec, execute_role
from brain.config import load_config, repo_root
from brain import gitops
from brain.planfile import parse_qa_verdict, read_meta, render_qa_skeleton
from brain.projects import require_runnable, select_project

QA_ALLOWED_TOOLS: list[str] = [
    "Read",
    "Grep",
    "Glob",
    "Write(./.qa-report.md)",
    "Edit(./.qa-report.md)",
    "Bash(git diff *)",
    "Bash(git log *)",
    "Bash(git show *)",
    "Bash(git status *)",
]

QA_DISALLOWED_TOOLS: list[str] = [
    "WebFetch",
    "WebSearch",
    "NotebookEdit",
    "Task",
]


def cmd_qa(args) -> int:
    cfg = load_config()
    project = select_project(args.project)
    require_runnable(project, cfg)

    proj_path = Path(os.path.expanduser(project["path"])).resolve()

    plan_path = proj_path / ".plan.md"
    if not plan_path.is_file():
        raise BrainError("Không tìm thấy .plan.md ở gốc dự án", EXIT_ERROR)

    text_plan = plan_path.read_text(encoding="utf-8")
    meta = read_meta(text_plan)
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

    dirty = gitops.changed_paths(proj_path) - {".qa-report.md"}
    if dirty:
        raise BrainError(
            "Working tree có thay đổi chưa commit:\n"
            + "\n".join(f"  {p}" for p in sorted(dirty)),
            EXIT_ERROR,
        )

    plan_commit = gitops.find_commit_by_subject(proj_path, f"{task}: plan")
    if plan_commit is None:
        raise BrainError(
            f"Không tìm thấy commit '{task}: plan' trên nhánh hiện tại",
            EXIT_ERROR,
        )

    code = gitops.diff_names(proj_path, plan_commit, "HEAD") - {".plan.md", ".qa-report.md"}
    if not code:
        raise BrainError("chưa có thay đổi code sau plan", EXIT_ERROR)

    diff_cmd = f"git diff {plan_commit}..HEAD -- . ':(exclude).plan.md' ':(exclude).qa-report.md'"

    if getattr(args, "dry_run", False):
        print(f"Diff command: {diff_cmd}")
        return 0

    qa_path = proj_path / ".qa-report.md"
    qa_meta = {"task": task, "plan_commit": plan_commit}
    req = meta.get("request", task)
    qa_path.write_text(render_qa_skeleton(req, qa_meta), encoding="utf-8")

    user_input = (
        f"Task: {task}\n"
        f"Đầu vào: .plan.md và kết quả lệnh: {diff_cmd}\n"
        "Ghi .qa-report.md theo khung có sẵn; mục Kết luận ghi đúng một dòng: Đạt hoặc Cần sửa."
    )

    spec = RoleSpec(
        role="qa",
        prompt_file=repo_root() / "brain" / "prompts" / "qa.md",
        user_input=user_input,
        allowed_tools=QA_ALLOWED_TOOLS,
        disallowed_tools=QA_DISALLOWED_TOOLS,
    )

    res = execute_role(spec, project, task, cfg)
    if res.timed_out:
        raise BrainError(f"QA chạy quá thời gian (timeout) trên task '{task}'", EXIT_ERROR)
    if res.interrupted:
        raise BrainError("QA bị ngắt bởi người dùng", EXIT_INTERRUPTED)
    if res.exit_code != 0:
        raise BrainError(f"QA thất bại với mã lỗi {res.exit_code} trên task '{task}'", EXIT_ERROR)

    extra = gitops.changed_paths(proj_path) - {".qa-report.md"}
    if extra:
        print("QA đã sửa file ngoài .qa-report.md — vi phạm giao thức (ROLE-04):")
        for p in sorted(extra):
            print(f"  {p}")
        return EXIT_PROTOCOL

    if not qa_path.is_file():
        raise BrainError("QA không tạo file .qa-report.md", EXIT_ERROR)

    qa_text = qa_path.read_text(encoding="utf-8")
    verdict = parse_qa_verdict(qa_text)
    if verdict is None:
        raise BrainError("QA chưa ghi Kết luận (Đạt / Cần sửa)", EXIT_ERROR)

    gitops.commit_paths(proj_path, [".qa-report.md"], f"{task}: qa")
    print(f"Kết luận QA: {verdict}")

    if verdict == "Cần sửa":
        print("Bước tiếp theo:")
        print(f"- Sửa .plan.md (commit '{task}: plan') hoặc chạy lại brain lead ... --name {task}")
        print("- Rồi chạy brain dev")
        print("Dev không đọc báo cáo QA.")

    return EXIT_OK
