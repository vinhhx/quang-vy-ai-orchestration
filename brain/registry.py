from __future__ import annotations

import argparse
import os
from pathlib import Path
import sys

from brain import BrainError, EXIT_ERROR
from brain.projects import add_project, find_duplicates, load_map, save_map, scan


def cmd_scan(args: argparse.Namespace) -> int:
    data = load_map()

    existing_roots = list(data.get("scan_roots", []))
    roots_set = {os.path.realpath(r) for r in existing_roots}
    for r in getattr(args, "root", []):
        r_abs = str(Path(r).resolve())
        r_real = os.path.realpath(r_abs)
        if r_real not in roots_set:
            existing_roots.append(r_abs)
            roots_set.add(r_real)
    data["scan_roots"] = existing_roots

    if not data["scan_roots"]:
        raise BrainError(
            "Chưa có scan_roots — chạy: brain scan --root <thư mục>", EXIT_ERROR
        )

    report = scan(data)
    save_map(data)

    num_roots = len(data["scan_roots"]) - len(report.get("bad_roots", []))
    sys.stdout.write(
        f"Đã quét {num_roots} thư mục gốc: +{report['added']} mới, "
        f"{report['missing']} thiếu (missing), {report['restored']} khôi phục, "
        f"{report['unchanged']} không đổi\n"
    )

    for base, new_id, path in report.get("suffixed", []):
        sys.stdout.write(f"Trùng id: đặt id là '{new_id}' thay cho '{base}'\n")

    for key, ids in find_duplicates(data.get("projects", [])):
        ids_str = ", ".join(ids)
        sys.stderr.write(f'CẢNH BÁO: tên/alias trùng "{key}": {ids_str}\n')

    return 0


def cmd_add(args: argparse.Namespace) -> int:
    target_path = Path(args.path).resolve()
    if not target_path.is_dir():
        raise BrainError(f"Thư mục không tồn tại: {target_path}", EXIT_ERROR)

    data = load_map()

    has_type_flag = any(a == "--type" or a.startswith("--type=") for a in sys.argv)
    type_arg = args.type if has_type_flag else None

    entry, created = add_project(data, target_path, type=type_arg)
    save_map(data)

    if created:
        sys.stdout.write(f"Đã thêm {entry['id']}\n")
    else:
        sys.stdout.write(f"Đã có {entry['id']} — giữ nguyên các trường sửa tay\n")

    for key, ids in find_duplicates(data.get("projects", [])):
        ids_str = ", ".join(ids)
        sys.stderr.write(f'CẢNH BÁO: tên/alias trùng "{key}": {ids_str}\n')

    return 0
