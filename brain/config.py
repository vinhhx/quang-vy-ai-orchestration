from __future__ import annotations

import json
import os
from pathlib import Path

from brain import BrainError, EXIT_ERROR

NESTED_SESSION_VARS: tuple[str, ...] = (
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "CLAUDE_VC_ENV",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
)


def repo_root() -> Path:
    return Path(os.path.realpath(__file__)).resolve().parent.parent


def load_config() -> dict:
    cfg_path_env = os.environ.get("BRAIN_CONFIG")
    if cfg_path_env:
        path = Path(os.path.expanduser(cfg_path_env)).resolve()
    else:
        path = repo_root() / "brain" / "config.json"

    if not path.is_file():
        raise BrainError(f"Không tìm thấy file cấu hình: {path}", EXIT_ERROR)

    try:
        with open(path, "r", encoding="utf-8") as f:
            cfg = json.load(f)
    except Exception as e:
        raise BrainError(f"Lỗi đọc cấu hình {path}: {e}", EXIT_ERROR)

    if not isinstance(cfg, dict):
        raise BrainError("Cấu hình không hợp lệ (phải là JSON object)", EXIT_ERROR)

    models = cfg.get("models")
    if not isinstance(models, dict):
        raise BrainError("Cấu hình thiếu trường 'models'", EXIT_ERROR)
    for r in ("lead", "dev", "qa"):
        if r not in models or not models[r]:
            raise BrainError(f"Cấu hình thiếu model cho vai trò '{r}'", EXIT_ERROR)

    profiles = cfg.get("profiles")
    if not isinstance(profiles, dict):
        raise BrainError("Cấu hình thiếu trường 'profiles'", EXIT_ERROR)
    for p in ("work", "personal"):
        if p not in profiles:
            raise BrainError(f"Cấu hình thiếu profile '{p}'", EXIT_ERROR)

    return cfg


def runs_dir(cfg: dict) -> Path:
    if cfg.get("runs_dir"):
        return Path(os.path.expanduser(str(cfg["runs_dir"]))).resolve()
    xdg = os.environ.get("XDG_DATA_HOME")
    if xdg:
        return (Path(os.path.expanduser(xdg)) / "agent-brain" / "runs").resolve()
    return (Path.home() / ".local" / "share" / "agent-brain" / "runs").resolve()


def protocol_log_dir(cfg: dict) -> Path:
    if cfg.get("protocol_log_dir"):
        return Path(os.path.expanduser(str(cfg["protocol_log_dir"]))).resolve()
    return (repo_root() / "protocol-log").resolve()


def check_profile(profile: str | None, cfg: dict) -> None:
    if not profile or not str(profile).strip():
        raise BrainError(
            "Dự án chưa đặt profile (work|personal) — sửa projects_map.json rồi chạy lại",
            EXIT_ERROR,
        )
    profiles = cfg.get("profiles", {})
    if profile not in profiles:
        names = ", ".join(sorted(profiles.keys()))
        raise BrainError(
            f"Profile '{profile}' không hợp lệ. Các profile đã cấu hình: {names}",
            EXIT_ERROR,
        )


def build_env(
    profile: str, cfg: dict, base_env: dict[str, str] | None = None
) -> dict[str, str]:
    env = dict(os.environ if base_env is None else base_env)
    for var in NESTED_SESSION_VARS:
        env.pop(var, None)

    prof_cfg = cfg.get("profiles", {}).get(profile, {})
    for var in prof_cfg.get("unset_env", []):
        env.pop(var, None)

    claude_config_dir = prof_cfg.get("claude_config_dir")
    if claude_config_dir is None:
        env.pop("CLAUDE_CONFIG_DIR", None)
    else:
        dir_str = str(claude_config_dir)
        if dir_str.startswith("~"):
            home = env.get("HOME", os.path.expanduser("~"))
            dir_str = os.path.normpath(os.path.join(home, dir_str[1:].lstrip("/\\")))
        env["CLAUDE_CONFIG_DIR"] = dir_str

    return env


def describe_profile(profile: str, cfg: dict) -> str:
    prof_cfg = cfg.get("profiles", {}).get(profile, {})
    dir_val = prof_cfg.get("claude_config_dir")
    if dir_val is None:
        return "CLAUDE_CONFIG_DIR không đặt → mặc định ~/.claude"
    return f"CLAUDE_CONFIG_DIR={dir_val}"
