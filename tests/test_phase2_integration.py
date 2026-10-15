from __future__ import annotations

import importlib
import json
from pathlib import Path
import re
import unittest

from brain import cli, tasklog
from brain.config import repo_root
from tests.helpers import Sandbox, lead_scenario


class TestPhase2Integration(unittest.TestCase):
    def test_traceability_every_p1_id_has_a_test(self):
        suite = unittest.defaultTestLoader.discover("tests", top_level_dir=".")

        def _flatten(s):
            for item in s:
                if isinstance(item, unittest.TestSuite):
                    yield from _flatten(item)
                else:
                    yield item.id()

        test_ids = list(_flatten(suite))
        for i in range(1, 11):
            pattern = f"test_p1_{i:02d}_"
            matching = [tid for tid in test_ids if pattern in tid]
            self.assertTrue(
                len(matching) > 0,
                f"No test found matching pattern '{pattern}' for P1-{i:02d}",
            )

    def test_all_cli_handlers_resolve(self):
        for cmd, target in cli.HANDLERS.items():
            mod_name, func_name = target.split(":")
            mod = importlib.import_module(mod_name)
            func = getattr(mod, func_name, None)
            self.assertIsNotNone(
                func,
                f"Handler target '{target}' for command '{cmd}' does not exist",
            )
            self.assertTrue(
                callable(func),
                f"Handler target '{target}' for command '{cmd}' is not callable",
            )

    def test_readme_documents_commands_and_exit_codes(self):
        readme_path = repo_root() / "brain" / "README.md"
        self.assertTrue(readme_path.is_file(), f"Missing {readme_path}")
        content = readme_path.read_text(encoding="utf-8")

        for cmd in ["brain scan", "brain add", "brain lead", "brain dev", "brain qa"]:
            self.assertIn(cmd, content, f"README does not document '{cmd}'")

        for code in ["0", "1", "2", "3", "4", "130"]:
            pattern = rf"\|\s*`?{code}`?\s*\|"
            self.assertRegex(
                content,
                pattern,
                f"README does not have exit code table row for '{code}'",
            )

    def test_contract_fixtures_have_versions_and_ids(self):
        contract_dir = repo_root() / "tests" / "contract"
        json_files = sorted(contract_dir.glob("*.json"))
        self.assertTrue(len(json_files) > 0, "No contract fixture files found")

        for fpath in json_files:
            with open(fpath, "r", encoding="utf-8") as f:
                data = json.load(f)
            self.assertIsInstance(data, dict, f"{fpath} is not a JSON object")
            self.assertEqual(data.get("version"), 1, f"{fpath} version != 1")

            cases = data.get("cases")
            self.assertIsInstance(cases, list, f"{fpath} missing 'cases' list")
            self.assertTrue(len(cases) > 0, f"{fpath} has empty cases list")

            ids = []
            for case in cases:
                self.assertIn("id", case, f"Case in {fpath} missing 'id'")
                ids.append(case["id"])

            self.assertEqual(
                len(ids),
                len(set(ids)),
                f"Duplicate case ids found in {fpath}: {ids}",
            )

    def test_pipeline_fills_task_log(self):
        sandbox = Sandbox()
        try:
            repo = sandbox.make_repo("web-ban-hang")
            entry = sandbox.project_entry(repo, id="web-ban-hang", profile="work")
            sandbox.write_map([entry])
            task = "sua-app"

            # 1. Lead
            sc_lead = lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])
            res_lead = sandbox.run_brain("lead", "web-ban-hang", "sửa app", scenario=sc_lead)
            self.assertEqual(res_lead.returncode, 0, f"Lead failed: {res_lead.stderr}")

            # 2. Dev
            sc_dev = {
                "ops": [
                    {"op": "read", "path": ".plan.md"},
                    {"op": "read", "path": "src/app.py"},
                    {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
                    {"op": "git", "args": ["add", "src/app.py"]},
                    {"op": "git", "args": ["commit", "-m", f"{task}: sửa app"]},
                ]
            }
            res_dev = sandbox.run_brain("dev", "web-ban-hang", scenario=sc_dev)
            self.assertEqual(res_dev.returncode, 0, f"Dev failed: {res_dev.stderr}")

            # 3. QA
            qa_report_content = (
                "# QA: sửa app\n"
                "## Thay đổi có khớp plan không (từng mục)\n"
                "- src/app.py: khớp.\n"
                "## Sửa ngoài plan (nếu có)\n"
                "Không.\n"
                "## Rủi ro\n"
                "Không có.\n"
                "## Kết luận\n"
                "Đạt\n"
            )
            sc_qa = {
                "ops": [
                    {
                        "op": "write",
                        "path": ".qa-report.md",
                        "content": qa_report_content,
                        "keep_meta": True,
                    }
                ]
            }
            res_qa = sandbox.run_brain("qa", "web-ban-hang", scenario=sc_qa)
            self.assertEqual(res_qa.returncode, 0, f"QA failed: {res_qa.stderr}")

            log_file = sandbox.protocol_log / f"{entry['id']}--{task}.md"
            self.assertTrue(log_file.is_file(), f"Task log file {log_file} does not exist")
            content = log_file.read_text(encoding="utf-8")

            self.assertIn("- Số turn của Dev trước khi bắt đầu sửa: 2", content)
            self.assertIn("- Dev có đọc file ngoài plan không: Không", content)

            qa_rows = [l for l in content.splitlines() if "| qa |" in l]
            self.assertTrue(len(qa_rows) > 0, "No QA row found in task log")
            self.assertIn("Đạt", qa_rows[0], f"QA row does not contain 'Đạt': {qa_rows[0]}")

            lines_map = {}
            for line in content.splitlines():
                for k in tasklog.USER_LINES:
                    if line.startswith(k):
                        lines_map[k] = line[len(k):].strip()
            for k in tasklog.USER_LINES:
                self.assertEqual(lines_map.get(k, None), "", f"User line '{k}' was not empty")

            self.assertEqual(tasklog.check_logs(sandbox.protocol_log, min_complete=1), 1)

            # Fill both user lines
            filled_content = content.replace(
                "- QA phát hiện được gì:",
                "- QA phát hiện được gì: Không có lỗi",
            ).replace(
                "- Phải làm lại không, vì sao:",
                "- Phải làm lại không, vì sao: Không",
            )
            log_file.write_text(filled_content, encoding="utf-8")

            self.assertEqual(tasklog.check_logs(sandbox.protocol_log, min_complete=1), 0)
        finally:
            sandbox.cleanup()


if __name__ == "__main__":
    unittest.main()
