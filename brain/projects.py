from __future__ import annotations

import json
import os
from pathlib import Path

from brain import BrainError, EXIT_ERROR, EXIT_ROUTING
from brain.config import check_profile
from brain import gitops
from brain.routing import resolve, RoutingError, normalize
import tempfile
import urllib.parse


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


def save_map(data: dict, path: Path | str | None = None) -> None:
    p = Path(path).resolve() if path is not None else map_path()
    p.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_path = tempfile.mkstemp(
        prefix="projects_map_", suffix=".tmp", dir=str(p.parent)
    )
    try:
        content = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            f.write(content)
            f.flush()
            os.fsync(f.fileno())
        os.replace(tmp_path, p)
    except Exception:
        if os.path.exists(tmp_path):
            try:
                os.remove(tmp_path)
            except OSError:
                pass
        raise


def git_remote(path: Path | str) -> str | None:
    try:
        raw = gitops.git(path, "remote", "get-url", "origin").strip()
    except Exception:
        return None
    if not raw:
        return None
    try:
        parsed = urllib.parse.urlsplit(raw)
        if parsed.scheme in ("http", "https") and (parsed.username or parsed.password):
            netloc = parsed.hostname
            if parsed.port:
                netloc = f"{netloc}:{parsed.port}"
            return urllib.parse.urlunsplit(
                (parsed.scheme, netloc, parsed.path, parsed.query, parsed.fragment)
            )
    except Exception:
        pass
    return raw


def unique_id(data: dict, base: str, path: Path | str) -> str:
    abs_path = os.path.realpath(str(path))
    used_ids: dict[str, str] = {}
    for p in data.get("projects", []):
        pid = p.get("id")
        ppath = os.path.realpath(str(p.get("path", "")))
        if pid:
            used_ids[pid] = ppath

    if base not in used_ids or used_ids[base] == abs_path:
        return base
    i = 2
    while True:
        candidate = f"{base}-{i}"
        if candidate not in used_ids or used_ids[candidate] == abs_path:
            return candidate
        i += 1


def new_entry(data: dict, path: Path | str, type: str = "code") -> dict:
    abs_path = str(Path(path).resolve())
    base = Path(path).name
    pid = unique_id(data, base, abs_path)
    return {
        "id": pid,
        "path": abs_path,
        "name_vi": base,
        "aliases": [],
        "type": type,
        "repo": git_remote(abs_path) if type == "code" else None,
        "profile": "",
        "missing": False,
    }


def scan(data: dict) -> dict:
    scan_roots = data.get("scan_roots", [])
    bad_roots: list[str] = []
    valid_roots: list[str] = []
    for r in scan_roots:
        p = Path(r)
        if p.is_dir():
            valid_roots.append(str(p.resolve()))
        else:
            bad_roots.append(str(r))

    projects = data.setdefault("projects", [])
    projects_by_realpath: dict[str, dict] = {}
    for p in projects:
        if p.get("path"):
            projects_by_realpath[os.path.realpath(str(p["path"]))] = p

    added_count = 0
    restored_count = 0
    unchanged_count = 0
    suffixed_ids: list[tuple[str, str, str]] = []
    scanned_realpaths: set[str] = set()

    for r in valid_roots:
        try:
            entries = sorted(os.scandir(r), key=lambda e: e.name)
        except OSError:
            bad_roots.append(r)
            continue
        for entry in entries:
            if not entry.is_dir(follow_symlinks=True):
                continue
            child_path = Path(entry.path).resolve()
            git_path = child_path / ".git"
            if not git_path.exists():
                continue
            real_child = os.path.realpath(str(child_path))
            scanned_realpaths.add(real_child)
            if real_child in projects_by_realpath:
                proj = projects_by_realpath[real_child]
                proj["repo"] = git_remote(str(child_path))
                if proj.get("missing", False):
                    proj["missing"] = False
                    restored_count += 1
                else:
                    unchanged_count += 1
            else:
                base_name = child_path.name
                new_proj = new_entry(data, child_path, type="code")
                if new_proj["id"] != base_name:
                    suffixed_ids.append((base_name, new_proj["id"], str(child_path)))
                projects.append(new_proj)
                projects_by_realpath[real_child] = new_proj
                added_count += 1

    for p in projects:
        p_path = p.get("path")
        if not p_path:
            continue
        real_p = os.path.realpath(str(p_path))
        if real_p in scanned_realpaths:
            continue
        if not os.path.isdir(real_p):
            p["missing"] = True
        else:
            if p.get("missing", False):
                p["missing"] = False
                restored_count += 1
            else:
                unchanged_count += 1

    total_missing = sum(1 for p in projects if p.get("missing", False))

    return {
        "added": added_count,
        "missing": total_missing,
        "restored": restored_count,
        "unchanged": unchanged_count,
        "bad_roots": bad_roots,
        "suffixed": suffixed_ids,
    }


def add_project(
    data: dict, path: Path | str, type: str | None = None
) -> tuple[dict, bool]:
    abs_path = str(Path(path).resolve())
    real_path = os.path.realpath(abs_path)
    projects = data.setdefault("projects", [])

    for p in projects:
        if os.path.realpath(str(p.get("path", ""))) == real_path:
            if p.get("missing"):
                p["missing"] = False
            return p, False

    if type is None:
        type = "code" if (Path(abs_path) / ".git").exists() else "media"

    entry = new_entry(data, abs_path, type=type)
    projects.append(entry)
    return entry, True


def find_duplicates(projects: list[dict]) -> list[tuple[str, list[str]]]:
    key_to_ids: dict[str, list[str]] = {}
    for p in projects:
        pid = p.get("id")
        if not pid:
            continue
        keys = set()
        k_id = normalize(pid)
        if k_id:
            keys.add(k_id)
        if p.get("name_vi"):
            k_name = normalize(p["name_vi"])
            if k_name:
                keys.add(k_name)
        for alias in p.get("aliases", []):
            k_alias = normalize(alias)
            if k_alias:
                keys.add(k_alias)
        for k in keys:
            if k not in key_to_ids:
                key_to_ids[k] = []
            if pid not in key_to_ids[k]:
                key_to_ids[k].append(pid)

    duplicates: list[tuple[str, list[str]]] = [
        (k, ids) for k, ids in key_to_ids.items() if len(ids) > 1
    ]
    duplicates.sort(key=lambda item: item[0])
    return duplicates

