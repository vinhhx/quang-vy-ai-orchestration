from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import unittest

from brain.config import repo_root
from brain.projects import load_map
from tests.helpers import Sandbox


class TestProjects(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        contract_path = repo_root() / "tests" / "contract" / "scan.json"
        with open(contract_path, "r", encoding="utf-8") as f:
            cls.scan_fixture = json.load(f)

    def _materialize_layout(self, sb: Sandbox, layout: dict) -> dict[str, Path]:
        root_paths: dict[str, Path] = {}
        roots_spec = layout.get("roots", {})
        for r_name, folders in roots_spec.items():
            r_dir = sb.root / "roots" / r_name
            r_dir.mkdir(parents=True, exist_ok=True)
            root_paths[r_name] = r_dir.resolve()
            for f_name, f_spec in folders.items():
                f_dir = r_dir / f_name
                f_dir.mkdir(parents=True, exist_ok=True)
                if f_spec.get("git"):
                    sb.make_repo(f_name, root=r_dir, remote=f_spec.get("remote"))
                if "children" in f_spec:
                    for c_name, c_spec in f_spec["children"].items():
                        c_dir = f_dir / c_name
                        c_dir.mkdir(parents=True, exist_ok=True)
                        if c_spec.get("git"):
                            sb.make_repo(c_name, root=f_dir, remote=c_spec.get("remote"))
        return root_paths

    def _execute_case(self, case: dict) -> None:
        sb = Sandbox()
        try:
            root_paths = self._materialize_layout(sb, case.get("layout", {}))
            format_dict = {k: str(v) for k, v in root_paths.items()}

            all_stderr: list[str] = []
            actual_exit_codes: list[int] = []

            for step in case.get("steps", []):
                op = step.get("op")
                if op == "scan":
                    cmd = ["scan"]
                    for r in step.get("roots", []):
                        cmd.extend(["--root", str(root_paths[r])])
                    res = sb.run_brain(*cmd)
                    actual_exit_codes.append(res.returncode)
                    all_stderr.append(res.stderr)
                elif op == "add":
                    raw_path = step["path"].format(**format_dict)
                    cmd = ["add", raw_path]
                    if "type" in step:
                        cmd.extend(["--type", step["type"]])
                    res = sb.run_brain(*cmd)
                    actual_exit_codes.append(res.returncode)
                    all_stderr.append(res.stderr)
                elif op == "edit":
                    map_file = sb.xdg_config / "agent-brain" / "projects_map.json"
                    with open(map_file, "r", encoding="utf-8") as f:
                        data = json.load(f)
                    proj_id = step["project"]
                    found = False
                    for p in data.get("projects", []):
                        if p.get("id") == proj_id:
                            p.update(step["set"])
                            found = True
                            break
                    self.assertTrue(found, f"Project '{proj_id}' not found for edit")
                    with open(map_file, "w", encoding="utf-8") as f:
                        json.dump(data, f, ensure_ascii=False, indent=2)
                    actual_exit_codes.append(0)
                elif op == "mkrepo":
                    r_dir = root_paths[step["root"]]
                    sb.make_repo(step["name"], root=r_dir, remote=step.get("remote"))
                elif op == "rmdir":
                    target = Path(step["path"].format(**format_dict))
                    if target.is_dir():
                        shutil.rmtree(target)
                elif op == "mkdir":
                    target = Path(step["path"].format(**format_dict))
                    target.mkdir(parents=True, exist_ok=True)
                    if step.get("git"):
                        sb.make_repo(target.name, root=target.parent)

            expect = case.get("expect", {})
            if "exit_codes" in expect:
                self.assertEqual(actual_exit_codes, expect["exit_codes"])

            map_file = sb.xdg_config / "agent-brain" / "projects_map.json"
            if map_file.exists():
                with open(map_file, "r", encoding="utf-8") as f:
                    final_data = json.load(f)
            else:
                final_data = {"scan_roots": [], "projects": []}

            projects_by_id = {p["id"]: p for p in final_data.get("projects", [])}

            for exp_id, exp_fields in expect.get("projects", {}).items():
                self.assertIn(exp_id, projects_by_id, f"Project '{exp_id}' should be in map")
                actual_proj = projects_by_id[exp_id]
                for k, v in exp_fields.items():
                    if isinstance(v, str):
                        v_fmt = v.format(**format_dict)
                    else:
                        v_fmt = v
                    self.assertEqual(
                        actual_proj.get(k),
                        v_fmt,
                        f"Field '{k}' on project '{exp_id}' mismatch: {actual_proj.get(k)} != {v_fmt}",
                    )

            for abs_id in expect.get("absent", []):
                self.assertNotIn(abs_id, projects_by_id, f"Project '{abs_id}' should be absent")

            combined_stderr = "\n".join(all_stderr)
            for warn in expect.get("warnings", []):
                key = warn["key"]
                self.assertIn(f'CẢNH BÁO: tên/alias trùng "{key}":', combined_stderr)
                for wid in warn["ids"]:
                    self.assertIn(wid, combined_stderr)
        finally:
            sb.cleanup()

    def test_p1_05_scan_keeps_hand_edits(self) -> None:
        case = next(c for c in self.scan_fixture["cases"] if c["id"] == "P1-05")
        self._execute_case(case)

    def test_route_06_duplicate_alias_warned(self) -> None:
        case = next(c for c in self.scan_fixture["cases"] if c["id"] == "ROUTE-06-alias")
        self._execute_case(case)

    def test_scan_contract_cases(self) -> None:
        for case in self.scan_fixture["cases"]:
            with self.subTest(case_id=case["id"]):
                self._execute_case(case)

    def test_scan_requires_root(self) -> None:
        sb = Sandbox()
        try:
            res = sb.run_brain("scan")
            self.assertEqual(res.returncode, 1)
            self.assertIn("Chưa có scan_roots — chạy: brain scan --root <thư mục>", res.stderr)
        finally:
            sb.cleanup()

    def test_add_missing_path_fails(self) -> None:
        sb = Sandbox()
        try:
            res = sb.run_brain("add", "/path/that/does/not/exist/at/all")
            self.assertEqual(res.returncode, 1)
            self.assertIn("không tồn tại", res.stderr)
        finally:
            sb.cleanup()

    def test_saved_map_is_utf8_with_newline(self) -> None:
        from brain.projects import save_map

        sb = Sandbox()
        try:
            map_file = sb.xdg_config / "agent-brain" / "projects_map.json"
            sample_data = {
                "scan_roots": ["/tmp/test"],
                "projects": [
                    {
                        "id": "web-ban-hang",
                        "path": "/tmp/test/web-ban-hang",
                        "name_vi": "Web bán hàng",
                        "aliases": ["bán hàng", "shop"],
                        "type": "code",
                        "repo": None,
                        "profile": "work",
                        "missing": False,
                    }
                ],
            }
            save_map(sample_data, path=map_file)

            with open(map_file, "rb") as f:
                content = f.read()

            self.assertTrue(content.endswith(b"\n"))
            self.assertIn("Web bán hàng".encode("utf-8"), content)
            self.assertNotIn(b"\\u", content)
        finally:
            sb.cleanup()


if __name__ == "__main__":
    unittest.main()
