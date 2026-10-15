from __future__ import annotations

import json
import os
from pathlib import Path

from brain import BrainError, EXIT_ERROR, EXIT_ROUTING
from brain.config import check_profile
from brain import gitops
from brain.routing import resolve, RoutingError


def map_path() -> Path:
    xdg = os.environ.get("XDG_CONFIG_HOME")
    if xdg:
        return (Path(os.path.expanduser(xdg)) / "agent-brain" / "projects_map.json").resolve()
    return (Path.home() / ".config" / "agent-brain" / "projects_map.json").resolve()


def load_map(path: Path | str | None = None) -> dict:
    p = Path(path).resolve() if path is not None else map_path()
    if not p.exists():
        return {"scan_roots": [], "projects": []}
    try:
        with open(p, "r", encoding="utf-8") as f:
            data = json.load(f)
    except (OSError, json.JSONDecodeError, ValueError) as e:
        raise BrainError(
            f"Không đọc được {p}: {e} — sửa file rồi chạy lại (brain không ghi đè file này)",
            EXIT_ERROR,
        )
    if not isinstance(data, dict):
        raise BrainError(
            f"Không đọc được {p}: phải là JSON object — sửa file rồi chạy lại (brain không ghi đè file này)",
            EXIT_ERROR,
        )
    if not isinstance(data.get("projects", []), list):
        raise BrainError(
            f"Không đọc được {p}: 'projects' phải là danh sách — sửa file rồi chạy lại (brain không ghi đè file này)",
            EXIT_ERROR,
        )
    if not isinstance(data.get("scan_roots", []), list):
        raise BrainError(
            f"Không đọc được {p}: 'scan_roots' phải là danh sách — sửa file rồi chạy lại (brain không ghi đè file này)",
            EXIT_ERROR,
        )
    data.setdefault("scan_roots", [])
    data.setdefault("projects", [])
    return data


def select_project(query: str, data: dict | None = None) -> dict:
    if data is None:
        data = load_map()
    projects = data.get("projects", [])

    try:
        return resolve(query, projects)
    except RoutingError as err:
        if err.kind == "ambiguous":
            header = f'Tên "{err.query}" khớp {len(err.candidates)} dự án — dừng, không chạy agent. Gọi lại bằng id chính xác:'
        else:
            header = f'Không tìm thấy dự án "{err.query}" — dừng, không chạy agent. Danh sách dự án:'

        lines = [header]
        if not err.candidates:
            lines.append("  (trống) — chạy: brain scan --root <thư mục>")
        else:
            for c in err.candidates:
                cid = c.get("id", "")
                cname = c.get("name_vi", "")
                cpath = c.get("path", "")
                lines.append(f"  {cid}\t{cname}\t{cpath}")
        text = "\n".join(lines)
        raise BrainError(text, EXIT_ROUTING)


def require_runnable(project: dict, cfg: dict) -> None:
    if project.get("missing"):
        raise BrainError(f"Dự án '{project.get('id')}' bị đánh dấu missing — không thể chạy agent", EXIT_ERROR)

    path_val = project.get("path")
    if not path_val:
        raise BrainError(f"Dự án '{project.get('id')}' không có đường dẫn hợp lệ", EXIT_ERROR)

    proj_path = Path(os.path.expanduser(str(path_val))).resolve()
    if not proj_path.is_dir():
        raise BrainError(f"Đường dẫn dự án '{project.get('id')}' không tồn tại: {proj_path}", EXIT_ERROR)

    if not gitops.is_repo_root(proj_path):
        raise BrainError(f"Thư mục dự án '{project.get('id')}' không phải repo git: {proj_path}", EXIT_ERROR)

    check_profile(project.get("profile"), cfg)
