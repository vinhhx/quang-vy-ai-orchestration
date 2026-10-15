from __future__ import annotations

import argparse
import importlib
import sys

from brain import BrainError, EXIT_ERROR, EXIT_INTERRUPTED, EXIT_OK, EXIT_ROUTING

HANDLERS: dict[str, str] = {
    "scan": "brain.registry:cmd_scan",
    "add": "brain.registry:cmd_add",
    "lead": "brain.role_lead:cmd_lead",
    "dev": "brain.role_dev:cmd_dev",
    "qa": "brain.role_qa:cmd_qa",
}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="brain", description="QuangVy Orchestration CLI"
    )
    subparsers = parser.add_subparsers(dest="subcommand")

    p_scan = subparsers.add_parser("scan", help="Quét thư mục tìm dự án")
    p_scan.add_argument(
        "--root",
        action="append",
        default=[],
        metavar="DIR",
        help="Thư mục gốc cần quét",
    )

    p_add = subparsers.add_parser("add", help="Thêm dự án vào map")
    p_add.add_argument("path", help="Đường dẫn thư mục dự án")
    p_add.add_argument(
        "--type", choices=["code", "media"], default="code", help="Loại dự án"
    )

    p_lead = subparsers.add_parser(
        "lead", help="Khảo sát dự án và viết kế hoạch sửa"
    )
    p_lead.add_argument("project", help="Tên, id hoặc alias dự án")
    p_lead.add_argument("request", help="Yêu cầu cần làm")
    p_lead.add_argument("--name", default=None, help="Slug tên task")
    p_lead.add_argument(
        "--dry-run", action="store_true", help="Chạy thử không sinh tác vụ"
    )

    p_dev = subparsers.add_parser("dev", help="Thực thi kế hoạch sửa")
    p_dev.add_argument("project", help="Tên, id hoặc alias dự án")
    p_dev.add_argument(
        "--dry-run", action="store_true", help="Chạy thử không sinh tác vụ"
    )

    p_qa = subparsers.add_parser("qa", help="Kiểm định chất lượng sau khi sửa")
    p_qa.add_argument("project", help="Tên, id hoặc alias dự án")
    p_qa.add_argument(
        "--dry-run", action="store_true", help="Chạy thử không sinh tác vụ"
    )

    try:
        args = parser.parse_args(argv)
    except SystemExit as e:
        return e.code if isinstance(e.code, int) else (0 if e.code is None else 1)

    if not args.subcommand:
        parser.print_help(sys.stderr)
        return EXIT_ERROR

    handler_target = HANDLERS.get(args.subcommand)
    if not handler_target:
        sys.stderr.write(f"Lệnh '{args.subcommand}' chưa có trong bản cài này\n")
        return EXIT_ERROR

    mod_name, func_name = handler_target.split(":")
    try:
        mod = importlib.import_module(mod_name)
        func = getattr(mod, func_name)
    except (ImportError, AttributeError, ModuleNotFoundError):
        sys.stderr.write(f"Lệnh '{args.subcommand}' chưa có trong bản cài này\n")
        return EXIT_ERROR

    try:
        res = func(args)
        return res if isinstance(res, int) else EXIT_OK
    except BrainError as e:
        if e.code == EXIT_ROUTING:
            sys.stdout.write(f"{e.message}\n")
            sys.stdout.flush()
        else:
            sys.stderr.write(f"{e.message}\n")
            sys.stderr.flush()
        return e.code
    except KeyboardInterrupt:
        return EXIT_INTERRUPTED
