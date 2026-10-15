from __future__ import annotations

import unittest

from tests.helpers import Sandbox


class TestLeadE2E(unittest.TestCase):
    def setUp(self):
        self.sandbox = Sandbox()
        self.addCleanup(self.sandbox.cleanup)

    def test_tracer_lead_routes_runs_and_commits_plan(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(
            repo,
            id="web-ban-hang",
            name_vi="Web bán hàng",
            profile="work",
        )
        self.sandbox.write_map([entry])

        scenario = {
            "ops": [
                {
                    "op": "write",
                    "path": ".plan.md",
                    "content": "# Plan: them nut dang xuat\n\n## Mục tiêu\nThêm nút đăng xuất\n",
                }
            ]
        }

        res = self.sandbox.run_brain(
            "lead",
            "web bán hàng",
            "thêm nút đăng xuất",
            scenario=scenario,
        )
        self.assertEqual(
            res.returncode,
            0,
            f"Command failed (exit {res.returncode}):\nSTDOUT: {res.stdout}\nSTDERR: {res.stderr}",
        )

        current_branch = self.sandbox.git(repo, "branch", "--show-current").strip()
        self.assertEqual(current_branch, "task/them-nut-dang-xuat")

        head_subject = self.sandbox.git(repo, "log", "-1", "--format=%s").strip()
        self.assertEqual(head_subject, "them-nut-dang-xuat: plan")

        diff_tree = [
            line.strip()
            for line in self.sandbox.git(
                repo, "diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(diff_tree, [".plan.md"])

        records = self.sandbox.records()
        self.assertEqual(len(records), 1)
        rec = records[0]

        argv = rec["argv"]
        self.assertIn("--output-format", argv)
        self.assertEqual(argv[argv.index("--output-format") + 1], "stream-json")
        self.assertIn("--verbose", argv)
        self.assertIn("--model", argv)
        self.assertEqual(argv[argv.index("--model") + 1], "opus")
        self.assertIn("--allowedTools", argv)

        self.assertIsNone(rec["claude_config_dir"])
        self.assertEqual(rec["present_vars"], [])

        jsonl_files = list(self.sandbox.runs.glob("**/stream.jsonl"))
        self.assertEqual(len(jsonl_files), 1)

    def test_help_lists_subcommands(self):
        res = self.sandbox.run_brain("--help")
        self.assertEqual(res.returncode, 0)
        for sub in ("scan", "add", "lead", "dev", "qa"):
            self.assertIn(sub, res.stdout)

    def test_empty_map_exits_2(self):
        self.sandbox.write_map([])
        res = self.sandbox.run_brain("lead", "x", "y", "--dry-run")
        self.assertEqual(res.returncode, 2)
        self.assertIn("Không tìm thấy dự án \"x\"", res.stdout)
        self.assertIn("(trống) — chạy: brain scan --root <thư mục>", res.stdout)


if __name__ == "__main__":
    unittest.main()
