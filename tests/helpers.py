from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

from brain.config import NESTED_SESSION_VARS, repo_root


class Sandbox:
    def __init__(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name).resolve()

        self.xdg_config = self.root / "xdg_config"
        self.xdg_data = self.root / "xdg_data"
        self.runs = self.root / "runs"
        self.protocol_log = self.root / "protocol_log"
        self.personal_dir = self.root / "personal_cfg"
        self.repos = self.root / "repos"

        for d in (
            self.xdg_config,
            self.xdg_data,
            self.runs,
            self.protocol_log,
            self.personal_dir,
            self.repos,
        ):
            d.mkdir(parents=True, exist_ok=True)

        self.fake_claude = (repo_root() / "tests" / "fakes" / "fake_claude.py").resolve()
        os.chmod(self.fake_claude, 0o755)

        self.record_file = self.root / "fake_claude_record.jsonl"
        self.git_config_file = self.root / "temp_gitconfig"
        with open(self.git_config_file, "w", encoding="utf-8") as f:
            f.write(
                "[user]\n  name = Sandbox User\n  email = sandbox@example.com\n"
                "[commit]\n  gpgsign = false\n"
                "[init]\n  defaultBranch = main\n"
            )

        self.env = dict(os.environ)
        self.env.pop("CLAUDE_CONFIG_DIR", None)
        for v in NESTED_SESSION_VARS:
            self.env.pop(v, None)

        self.env["XDG_CONFIG_HOME"] = str(self.xdg_config)
        self.env["XDG_DATA_HOME"] = str(self.xdg_data)
        self.env["GIT_CONFIG_GLOBAL"] = str(self.git_config_file)
        self.env["GIT_CONFIG_NOSYSTEM"] = "1"
        self.env["FAKE_CLAUDE_RECORD"] = str(self.record_file)

        self.config_file = self.root / "brain_config.json"
        self.write_config()

    def write_config(self, **overrides) -> Path:
        cfg = {
            "claude_bin": str(self.fake_claude),
            "models": {
                "lead": "opus",
                "dev": "sonnet",
                "qa": "sonnet",
            },
            "profiles": {
                "work": {
                    "claude_config_dir": None,
                    "unset_env": [
                        "ANTHROPIC_API_KEY",
                        "ANTHROPIC_AUTH_TOKEN",
                        "ANTHROPIC_BASE_URL",
                    ],
                },
                "personal": {
                    "claude_config_dir": str(self.personal_dir),
                    "unset_env": [
                        "ANTHROPIC_API_KEY",
                        "ANTHROPIC_AUTH_TOKEN",
                        "ANTHROPIC_BASE_URL",
                    ],
                },
            },
            "common_args": ["--strict-mcp-config"],
            "max_turns": {
                "lead": None,
                "dev": None,
                "qa": None,
            },
            "timeout_seconds": {
                "lead": 30,
                "dev": 30,
                "qa": 30,
            },
            "runs_dir": str(self.runs),
            "protocol_log_dir": str(self.protocol_log),
        }
        cfg.update(overrides)
        with open(self.config_file, "w", encoding="utf-8") as f:
            json.dump(cfg, f, indent=2)

        self.env["BRAIN_CONFIG"] = str(self.config_file)
        return self.config_file

    def make_repo(
        self,
        name: str,
        files: dict[str, str] | None = None,
        root: Path | None = None,
        remote: str | None = None,
    ) -> Path:
        target = (root or self.repos) / name
        target.mkdir(parents=True, exist_ok=True)

        self.git(target, "init", "-b", "main")

        if files is None:
            files = {
                "src/app.py": "def app():\n    return 1\n",
                "README.md": "# demo\n",
            }

        for rel_path, content in files.items():
            fpath = target / rel_path
            fpath.parent.mkdir(parents=True, exist_ok=True)
            with open(fpath, "w", encoding="utf-8") as f:
                f.write(content)
            self.git(target, "add", "-f", "--", str(rel_path))

        self.git(target, "commit", "-m", "init")

        if remote:
            self.git(target, "remote", "add", "origin", remote)

        return target

    def write_map(self, projects: list[dict], scan_roots: tuple = ()) -> Path:
        map_dir = self.xdg_config / "agent-brain"
        map_dir.mkdir(parents=True, exist_ok=True)
        map_file = map_dir / "projects_map.json"
        data = {
            "scan_roots": list(scan_roots),
            "projects": list(projects),
        }
        with open(map_file, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2)
        return map_file

    def project_entry(
        self,
        path: str | Path,
        id: str,
        profile: str = "work",
        name_vi: str | None = None,
        aliases: tuple = (),
    ) -> dict:
        return {
            "id": id,
            "path": str(path),
            "name_vi": name_vi if name_vi is not None else id,
            "aliases": list(aliases),
            "type": "code",
            "repo": None,
            "profile": profile,
            "missing": False,
        }

    def run_brain(
        self,
        *args: str,
        scenario: dict | None = None,
        extra_env: dict | None = None,
        timeout: int = 60,
    ) -> subprocess.CompletedProcess:
        env = dict(self.env)
        if extra_env:
            env.update(extra_env)

        scenario_file = None
        if scenario is not None:
            fd, scenario_file = tempfile.mkstemp(
                prefix="scenario_", suffix=".json", dir=str(self.root)
            )
            with open(fd, "w", encoding="utf-8") as f:
                json.dump(scenario, f)
            env["FAKE_CLAUDE_SCENARIO"] = scenario_file

        cmd = [sys.executable, str(repo_root() / "brain" / "brain"), *args]
        try:
            res = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                env=env,
                timeout=timeout,
            )
            return res
        finally:
            if scenario_file and os.path.exists(scenario_file):
                try:
                    os.remove(scenario_file)
                except OSError:
                    pass

    def records(self) -> list[dict]:
        if not self.record_file.is_file():
            return []
        records = []
        with open(self.record_file, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    records.append(json.loads(line))
        return records

    def git(self, repo: str | Path, *args: str) -> str:
        res = subprocess.run(
            ["git", *args],
            cwd=str(repo),
            capture_output=True,
            text=True,
            env=self.env,
        )
        if res.returncode != 0:
            raise RuntimeError(f"Git failed ({res.returncode}): {res.stderr}")
        return res.stdout

    def cleanup(self) -> None:
        self.temp_dir.cleanup()


def plan_text(
    title: str,
    rows: list[tuple[str, str, str, str] | list[str]],
    checks: str = "python3 -m unittest",
    blockers: str = "",
) -> str:
    lines = [
        f"# Plan: {title}",
        "## Mục tiêu",
        f"Thực hiện {title}",
        "## Thay đổi",
        "| # | File | Dòng / hàm | Sửa gì | Lý do |",
        "|---|---|---|---|---|",
    ]
    for i, r in enumerate(rows, start=1):
        f, where, what, why = r
        lines.append(f"| {i} | {f} | {where} | {what} | {why} |")
    lines.extend([
        "## Không được đụng tới",
        "",
        "## Cách kiểm tra",
        checks,
        "## Vướng mắc (Dev ghi vào đây nếu có)",
        blockers,
        "",
    ])
    return "\n".join(lines)


def lead_scenario(rows: list, **kw) -> dict:
    title = kw.get("title", "Kế hoạch")
    checks = kw.get("checks", "python3 -m unittest")
    blockers = kw.get("blockers", "")

    ops: list[dict] = [{"op": "read", "path": ".plan.md"}]
    for r in rows:
        f = r[0]
        ops.append({"op": "read", "path": f})

    p_text = plan_text(title, rows, checks=checks, blockers=blockers)
    ops.append({
        "op": "write",
        "path": ".plan.md",
        "content": p_text,
        "keep_meta": True,
    })

    if "extra_ops" in kw:
        ops.extend(kw["extra_ops"])

    scenario: dict = {"ops": ops}
    for k, v in kw.items():
        if k not in ("title", "checks", "blockers", "extra_ops"):
            scenario[k] = v
    return scenario

