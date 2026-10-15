from __future__ import annotations

from datetime import datetime, timezone
import os
from pathlib import Path
import shlex
import sys

from brain import BrainError, EXIT_ERROR, EXIT_PROTOCOL
from brain.agent import RoleSpec, build_command, execute_role
from brain.config import describe_profile, load_config, repo_root
from brain import gitops
from brain.planfile import parse_plan, read_meta, render_plan_skeleton, set_meta, validate_plan
from brain.projects import require_runnable, select_project
from brain.routing import slugify

LEAD_ALLOWED_TOOLS: list[str] = [
    "Read",
    "Grep",
    "Glob",
    "Write(./.plan.md)",
    "Edit(./.plan.md)",
    "Bash(git log *)",
    "Bash(git show *)",
    "Bash(git diff *)",
    "Bash(git status *)",
]

LEAD_DISALLOWED_TOOLS: list[str] = [
    "WebFetch",
    "WebSearch",
    "NotebookEdit",
    "Task",
]


def cmd_lead(args) -> int:
    cfg = load_config()
    project = select_project(args.project)
    require_runnable(project, cfg)

    proj_path = Path(os.path.expanduser(project["path"])).resolve()
    dirty = gitops.changed_paths(proj_path)
    if dirty:
        raise BrainError(
            "Working tree có thay đổi chưa commit — dừng, không tự stash:\n"
            + "\n".join(sorted(dirty)),
            EXIT_ERROR,
        )

    task = slugify(args.name or args.request)
    if not task:
        raise BrainError(
            "Không thể tạo tên task từ yêu cầu — hãy truyền cờ --name <slug>",
            EXIT_ERROR,
        )
    branch = f"task/{task}"

    curr_branch = gitops.current_branch(proj_path)
    is_replan = curr_branch == branch

    if is_replan:
        plan_path = proj_path / ".plan.md"
        existing_meta = None
        if plan_path.is_file():
            existing_meta = read_meta(plan_path.read_text(encoding="utf-8"))
        if existing_meta:
            base_branch = existing_meta.get("base_branch")
            base_commit = existing_meta.get("base_commit")
            created_at = existing_meta.get("created_at") or datetime.now(timezone.utc).isoformat()
        else:
            base_branch = None
            base_commit = gitops.head_sha(proj_path)
            created_at = datetime.now(timezone.utc).isoformat()
    elif gitops.branch_exists(proj_path, branch):
        raise BrainError(
            f"Nhánh '{branch}' đã tồn tại nhưng chưa được checkout — hãy checkout nhánh đó hoặc dùng cờ --name khác",
            EXIT_ERROR,
        )
    else:
        base_branch = curr_branch
        base_commit = gitops.head_sha(proj_path)
        created_at = datetime.now(timezone.utc).isoformat()
        if curr_branch and curr_branch.startswith("task/"):
            sys.stderr.write(
                f"Cảnh báo: Nhánh mới '{branch}' được tạo từ nhánh task '{curr_branch}' (stacking)\n"
            )
            sys.stderr.flush()

    meta = {
        "task": task,
        "branch": branch,
        "base_branch": base_branch,
        "base_commit": base_commit,
        "request": args.request,
        "created_at": created_at,
    }

    spec = RoleSpec(
        role="lead",
        prompt_file=repo_root() / "brain" / "prompts" / "lead.md",
        user_input=(
            f"Yêu cầu: {args.request}\n"
            f"Task: {task}\n"
            "File .plan.md ở gốc dự án đã có sẵn khung (skeleton). "
            "Giữ nguyên hai dòng đầu (# Plan: và <!-- brain-meta: ... -->). "
            "Mỗi dòng trong bảng Thay đổi chỉ ghi đúng một file (đường dẫn tương đối so với gốc repo). "
            "Ghi kế hoạch vào .plan.md theo đúng mẫu."
        ),
        allowed_tools=LEAD_ALLOWED_TOOLS,
        disallowed_tools=LEAD_DISALLOWED_TOOLS,
    )

    if getattr(args, "dry_run", False):
        cmd = build_command(spec, cfg)
        display_cmd = list(cmd)
        for i, val in enumerate(display_cmd):
            if val == "--append-system-prompt" and i + 1 < len(display_cmd):
                display_cmd[i + 1] = "<brain/prompts/lead.md>"
                break

        p_prof = project.get("profile", "")
        p_desc = describe_profile(p_prof, cfg)
        model = cfg.get("models", {}).get("lead", "")

        print(f"Project: {project.get('id')}")
        print(f"Profile: {p_prof} ({p_desc})")
        print(f"Model: {model}")
        print(f"Branch: {branch}")
        print(f"Command: {shlex.join(display_cmd)}")
        return 0

    if not is_replan:
        gitops.create_branch(proj_path, branch)

    plan_path = proj_path / ".plan.md"
    plan_path.write_text(render_plan_skeleton(args.request, meta), encoding="utf-8")

    res = execute_role(spec, project, task, cfg)
    if res.timed_out:
        raise BrainError(f"Lead chạy quá thời gian (timeout) trên task '{task}'", EXIT_ERROR)
    if res.exit_code != 0:
        raise BrainError(f"Lead thất bại với mã lỗi {res.exit_code} trên task '{task}'", EXIT_ERROR)

    extra = gitops.changed_paths(proj_path) - {".plan.md"}
    if extra:
        print("Lead đã sửa file ngoài .plan.md — vi phạm giao thức (ROLE-01):")
        for p in sorted(extra):
            print(f"  {p}")
        print("Gợi ý: brain không tự revert thay đổi; bạn hãy kiểm tra và xử lý bằng tay.")
        return EXIT_PROTOCOL

    if not plan_path.is_file():
        raise BrainError(f"Lead hoàn thành nhưng không tìm thấy file .plan.md", EXIT_ERROR)

    text = plan_path.read_text(encoding="utf-8")
    curr_meta = read_meta(text)
    if curr_meta is None or curr_meta != meta:
        text = set_meta(text, meta)
        plan_path.write_text(text, encoding="utf-8")

    plan = parse_plan(text)
    errors = validate_plan(plan, proj_path)
    if errors:
        err_msg = (
            f"Kế hoạch trong .plan.md không hợp lệ:\n"
            + "\n".join(f"  - {e}" for e in errors)
            + f"\nSửa tay .plan.md rồi: git -C {proj_path} commit -m \"{task}: plan\" -- .plan.md"
        )
        raise BrainError(err_msg, EXIT_ERROR)

    gitops.commit_paths(proj_path, [".plan.md"], f"{task}: plan")
    print(str(plan_path))
    return 0
