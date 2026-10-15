from __future__ import annotations

import os
from pathlib import Path
import subprocess

from brain import BrainError, EXIT_ERROR


class GitError(BrainError):
    def __init__(self, message: str, code: int = EXIT_ERROR):
        super().__init__(message, code)


def git(root: str | Path, *args: str) -> str:
    env = dict(os.environ)
    env["GIT_TERMINAL_PROMPT"] = "0"
    res = subprocess.run(
        ["git", *args],
        cwd=str(root),
        capture_output=True,
        text=True,
        env=env,
    )
    if res.returncode != 0:
        msg = res.stderr.strip() or f"Lệnh git thất bại (exit {res.returncode}): git {' '.join(str(a) for a in args)}"
        raise GitError(msg)
    return res.stdout


def is_repo_root(path: str | Path) -> bool:
    p = Path(path).resolve()
    if not p.is_dir():
        return False
    try:
        top = git(p, "rev-parse", "--show-toplevel").strip()
        return Path(top).resolve() == p
    except Exception:
        return False


def changed_paths(root: str | Path) -> set[str]:
    raw = git(root, "status", "--porcelain=v1", "-z", "--untracked-files=all")
    if not raw:
        return set()
    parts = raw.split("\0")
    if parts and parts[-1] == "":
        parts.pop()
    paths: set[str] = set()
    i = 0
    while i < len(parts):
        entry = parts[i]
        if not entry:
            i += 1
            continue
        status = entry[:2]
        path = entry[3:] if len(entry) >= 3 else ""
        if path:
            paths.add(path)
        if "R" in status or "C" in status:
            i += 1
            if i < len(parts) and parts[i]:
                paths.add(parts[i])
        i += 1
    return paths


def current_branch(root: str | Path) -> str | None:
    try:
        out = git(root, "branch", "--show-current").strip()
        return out if out else None
    except Exception:
        return None


def head_sha(root: str | Path) -> str:
    return git(root, "rev-parse", "HEAD").strip()


def branch_exists(root: str | Path, name: str) -> bool:
    try:
        git(root, "rev-parse", "--verify", f"refs/heads/{name}")
        return True
    except GitError:
        return False


def create_branch(root: str | Path, name: str) -> None:
    git(root, "checkout", "-b", name, "HEAD")


def commit_paths(root: str | Path, paths: list[str], message: str) -> str:
    git(root, "add", "-f", "--", *paths)
    git(root, "commit", "-m", message, "--", *paths)
    return head_sha(root)


def diff_names(root: str | Path, a: str, b: str) -> set[str]:
    out = git(root, "diff", "--name-only", f"{a}..{b}")
    return {line.strip() for line in out.splitlines() if line.strip()}


def find_commit_by_subject(root: str | Path, subject: str) -> str | None:
    out = git(root, "log", "--format=%H %s", "HEAD")
    for line in out.splitlines():
        line = line.strip()
        if not line:
            continue
        parts = line.split(" ", 1)
        sha = parts[0]
        subj = parts[1] if len(parts) > 1 else ""
        if subj == subject:
            return sha
    return None


def commit_subjects(root: str | Path, a: str, b: str) -> list[tuple[str, str]]:
    out = git(root, "log", "--reverse", "--format=%H %s", f"{a}..{b}")
    res: list[tuple[str, str]] = []
    for line in out.splitlines():
        line = line.strip()
        if not line:
            continue
        parts = line.split(" ", 1)
        sha = parts[0]
        subj = parts[1] if len(parts) > 1 else ""
        res.append((sha, subj))
    return res
