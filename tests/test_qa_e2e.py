from __future__ import annotations

import json
from pathlib import Path
import unittest

from tests.helpers import Sandbox, lead_scenario


class TestQAE2E(unittest.TestCase):
    def setUp(self):
        self.sandbox = Sandbox()
        self.repo = self.sandbox.make_repo("web-ban-hang")
        self.entry = self.sandbox.project_entry(self.repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([self.entry])

    def tearDown(self):
        self.sandbox.cleanup()

    def _setup_lead_and_dev(self) -> tuple[str, str, str]:
        # Lead
        sc_lead = lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])
        res_lead = self.sandbox.run_brain("lead", "web-ban-hang", "sửa app", scenario=sc_lead)
        self.assertEqual(res_lead.returncode, 0)
        task = "sua-app"
        plan_sha = self.sandbox.git(self.repo, "rev-parse", "HEAD").strip()

        # Dev
        sc_dev = {
            "ops": [
                {"op": "read", "path": ".plan.md"},
                {"op": "read", "path": "src/app.py"},
                {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
                {"op": "git", "args": ["add", "src/app.py"]},
                {"op": "git", "args": ["commit", "-m", f"{task}: sửa app"]},
            ]
        }
        res_dev = self.sandbox.run_brain("dev", "web-ban-hang", scenario=sc_dev)
        self.assertEqual(res_dev.returncode, 0)
        dev_sha = self.sandbox.git(self.repo, "rev-parse", "HEAD").strip()

        return task, plan_sha, dev_sha

    def test_p1_09_qa_writes_report_without_code_change(self):
        task, plan_sha, dev_sha = self._setup_lead_and_dev()

        qa_report_content = (
            "# QA: sửa app\n"
            "## Thay đổi có khớp plan không (từng mục)\n"
            "- src/app.py: khớp.\n"
            "## Sửa ngoài plan (nếu có)\n"
            "Không.\n"
            "## Rủi ro\n"
            "Không phát hiện rủi ro.\n"
            "## Kết luận\n"
            "Đạt\n"
        )
        qa_scenario = {
            "ops": [
                {
                    "op": "write",
                    "path": ".qa-report.md",
                    "content": qa_report_content,
                    "keep_meta": True,
                }
            ]
        }

        res = self.sandbox.run_brain("qa", "web-ban-hang", scenario=qa_scenario)
        self.assertEqual(res.returncode, 0, f"QA failed: {res.stderr}\n{res.stdout}")

        # HEAD commit has subject {task}: qa
        head_subj = self.sandbox.git(self.repo, "log", "-1", "--format=%s").strip()
        self.assertEqual(head_subj, f"{task}: qa")

        # HEAD touches only .qa-report.md
        diff_tree = [
            line.strip()
            for line in self.sandbox.git(
                self.repo, "diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(diff_tree, [".qa-report.md"])

        # Check records
        recs = self.sandbox.records()
        self.assertGreaterEqual(len(recs), 3)  # lead, dev, qa
        qa_rec = recs[-1]
        argv = qa_rec["argv"]

        self.assertIn("Write(./.qa-report.md)", argv)
        self.assertIn("--model", argv)
        self.assertEqual(argv[argv.index("--model") + 1], "sonnet")

        # user_input contains diff range and exclusion
        user_input = argv[argv.index("-p") + 1]
        self.assertIn(f"git diff {plan_sha}..HEAD", user_input)
        self.assertIn("':(exclude).plan.md'", user_input)
        self.assertIn("':(exclude).qa-report.md'", user_input)

        # system prompt starts with Bạn là QA.
        prompt_text = argv[argv.index("--append-system-prompt") + 1]
        self.assertTrue(prompt_text.startswith("Bạn là QA."))

    def test_qa_modifies_code_exits_3(self):
        task, plan_sha, dev_sha = self._setup_lead_and_dev()

        qa_scenario = {
            "ops": [
                {
                    "op": "write",
                    "path": ".qa-report.md",
                    "content": "# QA\n## Kết luận\nĐạt\n",
                    "keep_meta": True,
                },
                {
                    "op": "write",
                    "path": "src/app.py",
                    "content": "def app(): return 999\n",
                },
            ]
        }

        res = self.sandbox.run_brain("qa", "web-ban-hang", scenario=qa_scenario)
        self.assertEqual(res.returncode, 3)
        self.assertIn("src/app.py", res.stdout)
        self.assertIn("QA đã sửa file ngoài .qa-report.md", res.stdout)

        curr_head = self.sandbox.git(self.repo, "rev-parse", "HEAD").strip()
        self.assertEqual(curr_head, dev_sha)

    def test_qa_missing_verdict_exits_1(self):
        task, plan_sha, dev_sha = self._setup_lead_and_dev()

        qa_scenario = {
            "ops": [
                {
                    "op": "write",
                    "path": ".qa-report.md",
                    "content": "# QA\n## Kết luận\nChưa biết\n",
                    "keep_meta": True,
                }
            ]
        }

        res = self.sandbox.run_brain("qa", "web-ban-hang", scenario=qa_scenario)
        self.assertEqual(res.returncode, 1)
        self.assertIn("Kết luận", res.stderr)

        curr_head = self.sandbox.git(self.repo, "rev-parse", "HEAD").strip()
        self.assertEqual(curr_head, dev_sha)
        self.assertTrue((self.repo / ".qa-report.md").is_file())

    def test_qa_verdict_can_sua_exits_0_prints_next_steps(self):
        task, plan_sha, dev_sha = self._setup_lead_and_dev()

        qa_report_content = (
            "# QA: sửa app\n"
            "## Thay đổi có khớp plan không (từng mục)\n"
            "Chưa khớp.\n"
            "## Sửa ngoài plan (nếu có)\n"
            "Không.\n"
            "## Rủi ro\n"
            "Có lỗi logic.\n"
            "## Kết luận\n"
            "Cần sửa\n"
        )
        qa_scenario = {
            "ops": [
                {
                    "op": "write",
                    "path": ".qa-report.md",
                    "content": qa_report_content,
                    "keep_meta": True,
                }
            ]
        }

        res = self.sandbox.run_brain("qa", "web-ban-hang", scenario=qa_scenario)
        self.assertEqual(res.returncode, 0)
        self.assertIn("Cần sửa", res.stdout)
        self.assertIn("brain lead", res.stdout)
        self.assertIn("brain dev", res.stdout)

        head_subj = self.sandbox.git(self.repo, "log", "-1", "--format=%s").strip()
        self.assertEqual(head_subj, f"{task}: qa")

    def test_qa_no_code_change_refuses_exit_1(self):
        sc_lead = lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])
        res_lead = self.sandbox.run_brain("lead", "web-ban-hang", "sửa app", scenario=sc_lead)
        self.assertEqual(res_lead.returncode, 0)

        records_before = len(self.sandbox.records())
        res_qa = self.sandbox.run_brain("qa", "web-ban-hang")
        self.assertEqual(res_qa.returncode, 1)
        self.assertIn("chưa có thay đổi code sau plan", res_qa.stderr)
        self.assertEqual(len(self.sandbox.records()), records_before)

    def test_qa_refusals_not_on_task_branch_or_missing_plan_commit(self):
        # 1. On main branch
        res1 = self.sandbox.run_brain("qa", "web-ban-hang")
        self.assertEqual(res1.returncode, 1)

        # 2. On task branch without plan commit
        self.sandbox.git(self.repo, "checkout", "-b", "task/fake-task")
        (self.repo / ".plan.md").write_text("# Plan\n<!-- brain-meta: {\"task\": \"fake-task\"} -->\n", encoding="utf-8")
        self.sandbox.git(self.repo, "add", ".plan.md")
        self.sandbox.git(self.repo, "commit", "-m", "init fake task")
        res2 = self.sandbox.run_brain("qa", "web-ban-hang")
        self.assertEqual(res2.returncode, 1)
        self.assertIn("fake-task: plan", res2.stderr)

    def test_qa_dry_run(self):
        task, plan_sha, dev_sha = self._setup_lead_and_dev()
        res = self.sandbox.run_brain("qa", "web-ban-hang", "--dry-run")
        self.assertEqual(res.returncode, 0)
        self.assertIn("git diff", res.stdout)
        self.assertIn(plan_sha, res.stdout)


if __name__ == "__main__":
    unittest.main()
