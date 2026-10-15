from __future__ import annotations

import os
from pathlib import Path
import shlex

from brain import BrainError, EXIT_ERROR
from brain.agent import RoleSpec, build_command, execute_role
from brain.config import describe_profile, load_config, repo_root
from brain import gitops
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

    spec = RoleSpec(
        role="lead",
        prompt_file=repo_root() / "brain" / "prompts" / "lead.md",
        user_input=f"Yêu cầu: {args.request}\nTask: {task}\nGhi kế hoạch vào .plan.md ở gốc dự án theo đúng mẫu.",
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

    gitops.create_branch(proj_path, branch)

    res = execute_role(spec, project, task, cfg)
    if res.timed_out:
        raise BrainError(f"Lead chạy quá thời gian (timeout) trên task '{task}'", EXIT_ERROR)
    if res.exit_code != 0:
        raise BrainError(f"Lead thất bại với mã lỗi {res.exit_code} trên task '{task}'", EXIT_ERROR)

    plan_path = proj_path / ".plan.md"
    if not plan_path.is_file():
        raise BrainError(f"Lead hoàn thành nhưng không tạo file .plan.md", EXIT_ERROR)

    gitops.commit_paths(proj_path, [".plan.md"], f"{task}: plan")
    print(str(plan_path))
    return 0
