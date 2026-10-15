from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
import os
from pathlib import Path
import subprocess
import sys
import threading

from brain import BrainError, EXIT_ERROR, EXIT_INTERRUPTED
from brain.config import build_env, describe_profile, runs_dir


@dataclass
class RoleSpec:
    role: str
    prompt_file: Path
    user_input: str
    allowed_tools: list[str]
    disallowed_tools: list[str]
    extra_args: list[str] = field(default_factory=list)


@dataclass
class RunResult:
    role: str
    exit_code: int
    raw_log: Path
    stderr_log: Path
    run_dir: Path
    started_at: str
    finished_at: str
    timed_out: bool
    interrupted: bool


def new_run_dir(cfg: dict, project: dict, task: str, role: str) -> Path:
    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
    base = runs_dir(cfg)
    target = base / project["id"] / task / f"{ts}-{role}"

    cur = target
    to_chmod = []
    while not cur.exists() and cur != cur.parent:
        to_chmod.append(cur)
        cur = cur.parent

    target.mkdir(parents=True, exist_ok=True)
    for p in to_chmod:
        try:
            os.chmod(p, 0o700)
        except OSError:
            pass
    os.chmod(target, 0o700)
    return target


def build_command(spec: RoleSpec, cfg: dict) -> list[str]:
    all_extra = (cfg.get("common_args", []) or []) + (spec.extra_args or [])
    for arg in all_extra:
        if (
            arg.startswith("--dangerously-skip-permissions")
            or arg.startswith("--allow-dangerously-skip-permissions")
            or arg.startswith("--permission-mode")
            or "bypassPermissions" in arg
        ):
            raise BrainError(f"Cờ quyền không được phép: {arg}", EXIT_ERROR)

    prompt_path = Path(spec.prompt_file).resolve()
    if not prompt_path.is_file():
        raise BrainError(f"Không tìm thấy prompt file: {prompt_path}", EXIT_ERROR)
    with open(prompt_path, "r", encoding="utf-8") as f:
        prompt_text = f.read()

    claude_bin = cfg.get("claude_bin", "claude")
    models = cfg.get("models", {})
    if spec.role not in models:
        raise BrainError(f"Không tìm thấy model cho vai trò: {spec.role}", EXIT_ERROR)

    cmd = [
        claude_bin,
        "-p",
        spec.user_input,
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        models[spec.role],
        "--append-system-prompt",
        prompt_text,
    ]
    cmd.extend(cfg.get("common_args", []) or [])

    max_turns = cfg.get("max_turns", {}).get(spec.role)
    if max_turns is not None:
        cmd.extend(["--max-turns", str(max_turns)])

    cmd.extend(spec.extra_args or [])

    if spec.allowed_tools:
        cmd.append("--allowedTools")
        cmd.extend(spec.allowed_tools)

    if spec.disallowed_tools:
        cmd.append("--disallowedTools")
        cmd.extend(spec.disallowed_tools)

    return cmd


def execute_role(
    spec: RoleSpec,
    project: dict,
    task: str,
    cfg: dict,
    run_dir: Path | None = None,
) -> RunResult:
    if run_dir is None:
        run_dir = new_run_dir(cfg, project, task, spec.role)
    else:
        run_dir = Path(run_dir).resolve()
        run_dir.mkdir(parents=True, exist_ok=True)
        os.chmod(run_dir, 0o700)

    raw_log = run_dir / "stream.jsonl"
    stderr_log = run_dir / "stderr.txt"

    argv = build_command(spec, cfg)
    env = build_env(project.get("profile", ""), cfg)

    p_profile = project.get("profile", "")
    p_desc = describe_profile(p_profile, cfg)
    model = cfg.get("models", {}).get(spec.role, "")
    sys.stderr.write(f"[brain] {spec.role} · {project.get('id', '')} · profile {p_profile} ({p_desc}) · model {model}\n")
    sys.stderr.flush()

    started_at = datetime.now(timezone.utc).isoformat()
    timeout = cfg.get("timeout_seconds", {}).get(spec.role, 1800)
    timed_out = False
    interrupted = False
    exit_code = 1

    stderr_f = open(stderr_log, "w", encoding="utf-8", errors="replace")
    try:
        try:
            proc = subprocess.Popen(
                argv,
                cwd=project["path"],
                env=env,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=stderr_f,
                text=True,
                encoding="utf-8",
                errors="replace",
            )
        except FileNotFoundError:
            raise BrainError(f"Không tìm thấy lệnh claude: {argv[0]}", EXIT_ERROR)

        def _reader():
            with open(raw_log, "w", encoding="utf-8", errors="replace") as f:
                if proc.stdout:
                    for line in proc.stdout:
                        f.write(line)
                        f.flush()

        reader_thread = threading.Thread(target=_reader)
        reader_thread.daemon = True
        reader_thread.start()

        try:
            exit_code = proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            proc.terminate()
            try:
                exit_code = proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                exit_code = proc.wait()
        except KeyboardInterrupt:
            interrupted = True
            proc.terminate()
            try:
                exit_code = proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                exit_code = proc.wait()
            reader_thread.join(timeout=2)
            stderr_f.close()
            finished_at = datetime.now(timezone.utc).isoformat()
            sys.stderr.write(f"[brain] {spec.role} xong · exit {exit_code} · log {raw_log}\n")
            sys.stderr.flush()
            raise BrainError("Đã dừng (Ctrl+C)", EXIT_INTERRUPTED)

        reader_thread.join(timeout=5)
    finally:
        if not stderr_f.closed:
            stderr_f.close()

    finished_at = datetime.now(timezone.utc).isoformat()
    sys.stderr.write(f"[brain] {spec.role} xong · exit {exit_code} · log {raw_log}\n")
    sys.stderr.flush()

    return RunResult(
        role=spec.role,
        exit_code=exit_code,
        raw_log=raw_log,
        stderr_log=stderr_log,
        run_dir=run_dir,
        started_at=started_at,
        finished_at=finished_at,
        timed_out=timed_out,
        interrupted=interrupted,
    )
