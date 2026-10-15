from __future__ import annotations

import json
import os
import tempfile
import unittest
import unicodedata
from pathlib import Path

from brain.planfile import (
    PLAN_SECTIONS,
    QA_SECTIONS,
    QA_VERDICTS,
    Change,
    Plan,
    render_plan_skeleton,
    render_qa_skeleton,
    read_meta,
    set_meta,
    parse_plan,
    validate_plan,
    plan_files,
    has_blockers,
    section_text,
    parse_qa_verdict,
)


class TestPlanfileContract(unittest.TestCase):
    def setUp(self) -> None:
        self.repo_root = Path(__file__).resolve().parent.parent

    def test_plan_contract_cases(self) -> None:
        fixture_path = self.repo_root / "tests" / "contract" / "plan.json"
        data = json.loads(fixture_path.read_text(encoding="utf-8"))
        self.assertEqual(data["version"], 1)

        for case in data["cases"]:
            case_id = case["id"]
            with self.subTest(id=case_id):
                plan = parse_plan(case["markdown"])
                expect = case["expect"]

                self.assertEqual(plan_files(plan), expect["files"])
                self.assertEqual(has_blockers(plan), expect["has_blockers"])

                root = None
                temp_dir = None
                try:
                    if "root_layout" in case:
                        temp_dir = tempfile.TemporaryDirectory()
                        root_path = Path(temp_dir.name) / "repo"
                        root_path.mkdir()
                        outside_dir = Path(temp_dir.name) / "outside"
                        outside_dir.mkdir()
                        (outside_dir / "target.py").write_text("print('outside')", encoding="utf-8")

                        layout = case["root_layout"]
                        for f in layout.get("files", []):
                            fp = root_path / f
                            fp.parent.mkdir(parents=True, exist_ok=True)
                            fp.write_text("ok", encoding="utf-8")

                        for link_name, link_target in layout.get("symlinks", {}).items():
                            lp = root_path / link_name
                            lp.parent.mkdir(parents=True, exist_ok=True)
                            if link_target.startswith("../outside"):
                                lp.symlink_to(outside_dir / "target.py")
                            else:
                                lp.symlink_to(link_target)
                        root = root_path

                    errors = validate_plan(plan, root=root)
                    is_valid = len(errors) == 0
                    self.assertEqual(is_valid, expect["valid"])
                    self.assertEqual(errors, expect["error_codes"])
                finally:
                    if temp_dir is not None:
                        temp_dir.cleanup()

    def test_qa_report_contract_cases(self) -> None:
        fixture_path = self.repo_root / "tests" / "contract" / "qa_report.json"
        data = json.loads(fixture_path.read_text(encoding="utf-8"))
        self.assertEqual(data["version"], 1)

        for case in data["cases"]:
            case_id = case["id"]
            with self.subTest(id=case_id):
                verdict = parse_qa_verdict(case["markdown"])
                self.assertEqual(verdict, case["expect"]["verdict"])

    def test_render_plan_skeleton(self) -> None:
        meta = {
            "task": "task-test-slug",
            "branch": "task/test-slug",
            "base_branch": "main",
            "base_commit": "abc1234",
            "request": "Thêm tính năng đăng nhập",
            "created_at": "2026-10-15T07:00:00Z",
        }
        rendered = render_plan_skeleton("Thêm tính năng đăng nhập", meta)
        lines = rendered.splitlines()

        # Line 1: # Plan: <title>
        self.assertTrue(lines[0].startswith("# Plan: Thêm tính năng đăng nhập"))
        # Line 2: <!-- brain-meta: ... -->
        self.assertTrue(lines[1].startswith("<!-- brain-meta:"))
        self.assertTrue(lines[1].endswith("-->"))

        # Verify all 5 PLAN_SECTIONS appear
        for sec in PLAN_SECTIONS:
            self.assertTrue(
                any(line.startswith("## ") and unicodedata.normalize("NFC", line[3:].strip()).startswith(sec) for line in lines),
                f"Missing section {sec} in rendered skeleton",
            )

        # Parse skeleton
        plan = parse_plan(rendered)
        self.assertEqual(plan.title, "Thêm tính năng đăng nhập")
        self.assertEqual(plan.meta, meta)
        self.assertEqual(len(plan.changes), 0)
        self.assertEqual(plan_files(plan), [])
        self.assertFalse(has_blockers(plan))

        # Validate skeleton gives no_changes
        errors = validate_plan(plan)
        self.assertEqual(errors, ["no_changes"])

    def test_render_qa_skeleton(self) -> None:
        meta = {
            "task": "task-test-slug",
            "plan_commit": "def5678",
        }
        rendered = render_qa_skeleton("Kiểm thử tính năng", meta)
        lines = rendered.splitlines()

        self.assertTrue(lines[0].startswith("# QA: Kiểm thử tính năng"))
        self.assertTrue(lines[1].startswith("<!-- brain-meta:"))
        self.assertTrue(lines[1].endswith("-->"))

        for sec in QA_SECTIONS:
            self.assertTrue(
                any(line.startswith("## ") and unicodedata.normalize("NFC", line[3:].strip()).startswith(sec) for line in lines),
                f"Missing QA section {sec} in rendered QA skeleton",
            )

    def test_meta_roundtrip_with_closing_comment_in_request(self) -> None:
        meta = {
            "task": "tricky-task",
            "branch": "task/tricky",
            "base_branch": "main",
            "base_commit": "1112223",
            "request": "Cần fix lỗi --> và kiểm tra lại --> kết quả",
            "created_at": "2026-10-15T08:00:00Z",
        }
        rendered = render_plan_skeleton("Tricky task", meta)
        # Verify literal --> is not present inside the json string before the real comment close
        meta_line = rendered.splitlines()[1]
        self.assertTrue(meta_line.endswith(" -->"))
        payload = meta_line[len("<!-- brain-meta: ") : -len(" -->")]
        self.assertNotIn("-->", payload)
        self.assertIn("\\u003e", payload)

        # read_meta recovers the exact original dict
        read = read_meta(rendered)
        self.assertEqual(read, meta)

        # set_meta replacement roundtrip
        new_meta = {
            "task": "updated-task",
            "request": "yêu cầu mới --> có mũi tên -->",
        }
        updated = set_meta(rendered, new_meta)
        self.assertEqual(read_meta(updated), new_meta)

    def test_nfc_nfd_normalization(self) -> None:
        nfd_title = unicodedata.normalize("NFD", "Tối ưu hóa")
        meta = {"task": "test-nfd"}
        rendered = render_plan_skeleton(nfd_title, meta)
        plan = parse_plan(rendered)
        self.assertEqual(plan.title, unicodedata.normalize("NFC", nfd_title))


if __name__ == "__main__":
    unittest.main()
