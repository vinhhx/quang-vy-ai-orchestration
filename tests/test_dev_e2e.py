from __future__ import annotations

import json
from pathlib import Path
import unittest

from tests.helpers import Sandbox, lead_scenario, plan_text


class TestDevE2E(unittest.TestCase):
    def setUp(self):
        self.sandbox = Sandbox()
        self.repo = self.sandbox.make_repo("web-ban-hang")
        self.entry = self.sandbox.project_entry(self.repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([self.entry])

    def tearDown(self):
        self.sandbox.cleanup()

    def _setup_lead(self, rows=None, title="Sửa app", checks="python3 -m unittest", blockers="") -> tuple[str, str]:
        if rows is None:
            rows = [("src/app.py", "app()", "trả về 2", "yêu cầu")]
        sc = lead_scenario(rows, title=title, checks=checks, blockers=blockers)
        res = self.sandbox.run_brain("lead", "web-ban-hang", "sửa app", scenario=sc)
        self.assertEqual(res.returncode, 0, f"Lead failed: {res.stderr}\n{res.stdout}")
        task = "sua-app"
        plan_sha = self.sandbox.git(self.repo, "rev-parse", "HEAD").strip()
        return task, plan_sha

    def test_p1_07_dev_edits_only_plan_files(self):
        task, plan_commit = self._setup_lead()

        dev_scenario = {
            "ops": [
                {"op": "read", "path": ".plan.md"},
                {"op": "read", "path": "src/app.py"},
                {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
                {"op": "git", "args": ["add", "src/app.py"]},
                {"op": "git", "args": ["commit", "-m", f"{task}: sửa app"]},
            ]
        }

        res = self.sandbox.run_brain("dev", "web-ban-hang", scenario=dev_scenario)
        self.assertEqual(res.returncode, 0, f"Dev failed: {res.stderr}\n{res.stdout}")

        # git diff --name-only <plan_commit>..HEAD == {src/app.py}
        diff_tree = [
            line.strip()
            for line in self.sandbox.git(
                self.repo, "diff", "--name-only", f"{plan_commit}..HEAD"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(diff_tree, ["src/app.py"])

        recs = self.sandbox.records()
        self.assertGreaterEqual(len(recs), 2)  # 1 lead, 1 dev
        dev_rec = recs[-1]
        argv = dev_rec["argv"]

        self.assertIn("Read(./src/app.py)", argv)
        self.assertIn("Edit(./src/app.py)", argv)
        self.assertIn("Write(./src/app.py)", argv)
        self.assertIn("Read(./.plan.md)", argv)
        self.assertIn("Edit(./.plan.md)", argv)
        self.assertIn(f"Read(/{self.repo}/src/app.py)", argv)
        self.assertIn(f"Edit(/{self.repo}/src/app.py)", argv)
        self.assertIn(f"Write(/{self.repo}/src/app.py)", argv)
        self.assertIn(f"Read(/{self.repo}/.plan.md)", argv)
        self.assertIn(f"Edit(/{self.repo}/.plan.md)", argv)
        self.assertIn(f"Write(/{self.repo}/.plan.md)", argv)
        self.assertIn("Bash(git add *)", argv)
        self.assertIn("Bash(git commit *)", argv)

        disallowed_idx = argv.index("--disallowedTools")
        disallowed_slice = argv[disallowed_idx + 1 :]
        self.assertIn("Grep", disallowed_slice)
        self.assertIn("Glob", disallowed_slice)

        self.assertIn("--settings", argv)
        self.assertIn("--model", argv)
        self.assertEqual(argv[argv.index("--model") + 1], "sonnet")

    def test_pending_change_committed_by_brain(self):
        task, plan_commit = self._setup_lead()

        dev_scenario = {
            "ops": [
                {"op": "read", "path": "src/app.py"},
                {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
            ]
        }

        res = self.sandbox.run_brain("dev", "web-ban-hang", scenario=dev_scenario)
        self.assertEqual(res.returncode, 0, f"Dev failed: {res.stderr}\n{res.stdout}")

        # Checked that brain committed the change
        head_subj = self.sandbox.git(self.repo, "log", "-1", "--format=%s").strip()
        self.assertTrue(head_subj.startswith(f"{task}: "))

        diff_tree = [
            line.strip()
            for line in self.sandbox.git(
                self.repo, "diff", "--name-only", f"{plan_commit}..HEAD"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(diff_tree, ["src/app.py"])

    def test_guard_blocks_outside_read_through_fake(self):
        task, plan_commit = self._setup_lead()
        (self.repo / "other.py").write_text("# other\n", encoding="utf-8")
        self.sandbox.git(self.repo, "add", "other.py")
        self.sandbox.git(self.repo, "commit", "-m", f"{task}: add other")
        plan_commit2 = self.sandbox.git(self.repo, "rev-parse", "HEAD").strip()

        dev_scenario = {
            "ops": [
                {"op": "read", "path": "src/app.py"},
                {"op": "read", "path": "other.py"},
                {"op": "write", "path": "src/app.py", "content": "def app():\n    return 3\n"},
            ]
        }

        res = self.sandbox.run_brain("dev", "web-ban-hang", scenario=dev_scenario)
        self.assertEqual(res.returncode, 0, f"Dev failed: {res.stderr}\n{res.stdout}")

        dev_rec = self.sandbox.records()[-1]
        self.assertTrue(any("other.py" in b["path"] for b in dev_rec.get("blocked", [])))

        # Check guard.jsonl in run dir
        runs = list(self.sandbox.runs.glob(f"web-ban-hang/{task}/*-dev/guard.jsonl"))
        self.assertTrue(len(runs) >= 1)
        guard_log = runs[-1]
        lines = [json.loads(line) for line in guard_log.read_text(encoding="utf-8").splitlines() if line.strip()]
        self.assertTrue(any(l["decision"] == "block" and "other.py" in l["path"] for l in lines))

    def test_backstop_catches_outside_write_exits_3(self):
        task, plan_commit = self._setup_lead()

        dev_scenario = {
            "ignore_hooks": True,
            "ops": [
                {"op": "write", "path": "outside.py", "content": "# forbidden\n"},
            ],
        }

        res = self.sandbox.run_brain("dev", "web-ban-hang", scenario=dev_scenario)
        self.assertEqual(res.returncode, 3)
        self.assertIn("outside.py", res.stdout)
        self.assertIn("Dev đã sửa file ngoài plan", res.stdout)
        self.assertIn("brain không tự hoàn tác", res.stdout)

        # HEAD remains plan_commit
        curr_head = self.sandbox.git(self.repo, "rev-parse", "HEAD").strip()
        self.assertEqual(curr_head, plan_commit)
        # outside.py still exists
        self.assertTrue((self.repo / "outside.py").is_file())

    def test_p1_08_dev_blocked_writes_vuong_mac(self):
        task, plan_commit = self._setup_lead()

        dev_scenario = {
            "ops": [
                {
                    "op": "write",
                    "path": ".plan.md",
                    "content": "\nThiếu tài liệu API\n",
                    "append": True,
                }
            ]
        }

        res = self.sandbox.run_brain("dev", "web-ban-hang", scenario=dev_scenario)
        self.assertEqual(res.returncode, 4)

        head_subj = self.sandbox.git(self.repo, "log", "-1", "--format=%s").strip()
        self.assertEqual(head_subj, f"{task}: vướng mắc")

        diff_tree = [
            line.strip()
            for line in self.sandbox.git(
                self.repo, "diff", "--name-only", f"{plan_commit}..HEAD"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(diff_tree, [".plan.md"])

    def test_refusals_exit_1_and_no_record(self):
        records_before = len(self.sandbox.records())

        # 1. Not on task branch (on main)
        res = self.sandbox.run_brain("dev", "web-ban-hang")
        self.assertEqual(res.returncode, 1)
        self.assertEqual(len(self.sandbox.records()), records_before)

        # Setup lead
        task, plan_commit = self._setup_lead()
        records_lead = len(self.sandbox.records())

        # 2. Dirty tree
        (self.repo / "dirty.txt").write_text("dirty\n", encoding="utf-8")
        res_dirty = self.sandbox.run_brain("dev", "web-ban-hang")
        self.assertEqual(res_dirty.returncode, 1)
        self.assertIn("chưa commit", res_dirty.stderr)
        self.assertEqual(len(self.sandbox.records()), records_lead)
        (self.repo / "dirty.txt").unlink()

        # 3. Plan with zero rows
        empty_plan = plan_text("Trống", [])
        sc_empty = {
            "ops": [
                {"op": "write", "path": ".plan.md", "content": empty_plan, "keep_meta": True}
            ]
        }
        # Run lead to rewrite plan with empty rows
        self.sandbox.run_brain("lead", "web-ban-hang", "sửa app", scenario=sc_empty)
        # dev must refuse
        res_empty = self.sandbox.run_brain("dev", "web-ban-hang")
        self.assertEqual(res_empty.returncode, 1)

    def test_refusal_unresolved_vuong_mac_before_run(self):
        task, plan_commit = self._setup_lead(blockers="Chưa có hướng dẫn")
        res = self.sandbox.run_brain("dev", "web-ban-hang")
        self.assertEqual(res.returncode, 1)
        self.assertIn("Vướng mắc", res.stderr)

    def test_refusal_traversal_row(self):
        task, plan_commit = self._setup_lead()
        p = self.repo / ".plan.md"
        old_text = p.read_text(encoding="utf-8")
        from brain.planfile import read_meta, set_meta
        meta = read_meta(old_text)
        new_text = plan_text("Sửa", [("../outside.py", "1", "sửa", "hack")])
        if meta:
            new_text = set_meta(new_text, meta)
        p.write_text(new_text, encoding="utf-8")
        self.sandbox.git(self.repo, "add", ".plan.md")
        self.sandbox.git(self.repo, "commit", "-m", f"{task}: plan")

        res = self.sandbox.run_brain("dev", "web-ban-hang")
        self.assertEqual(res.returncode, 1)

    def test_timeout_exits_1_no_commit(self):
        task, plan_commit = self._setup_lead()
        self.sandbox.write_config(timeout_seconds={"lead": 30, "dev": 1, "qa": 30})

        dev_scenario = {
            "sleep": 5,
            "ops": [
                {"op": "write", "path": "src/app.py", "content": "def app(): return 9\n"}
            ],
        }

        res = self.sandbox.run_brain("dev", "web-ban-hang", scenario=dev_scenario)
        self.assertEqual(res.returncode, 1)

        curr_head = self.sandbox.git(self.repo, "rev-parse", "HEAD").strip()
        self.assertEqual(curr_head, plan_commit)

    def test_cach_kiem_tra_never_executed_and_printed(self):
        check_cmd = "touch KHONG_DUOC_TAO"
        task, plan_commit = self._setup_lead(checks=check_cmd)

        dev_scenario = {
            "ops": [
                {"op": "write", "path": "src/app.py", "content": "def app(): return 2\n"}
            ]
        }

        res = self.sandbox.run_brain("dev", "web-ban-hang", scenario=dev_scenario)
        self.assertEqual(res.returncode, 0)
        self.assertFalse((self.repo / "KHONG_DUOC_TAO").exists())
        self.assertIn(check_cmd, res.stdout)
        self.assertIn("brain không tự chạy", res.stdout)

    def test_non_conforming_commit_subject_warns(self):
        task, plan_commit = self._setup_lead()

        dev_scenario = {
            "ops": [
                {"op": "write", "path": "src/app.py", "content": "def app(): return 2\n"},
                {"op": "git", "args": ["add", "src/app.py"]},
                {"op": "git", "args": ["commit", "-m", "random"]},
            ]
        }

        res = self.sandbox.run_brain("dev", "web-ban-hang", scenario=dev_scenario)
        self.assertEqual(res.returncode, 0)
        self.assertIn("CẢNH BÁO", res.stderr)
        self.assertIn("random", res.stderr)

    def test_qa_report_not_in_dev_prompt_or_argv(self):
        task, plan_commit = self._setup_lead()

        dev_scenario = {
            "ops": [
                {"op": "write", "path": "src/app.py", "content": "def app(): return 2\n"},
            ]
        }

        res = self.sandbox.run_brain("dev", "web-ban-hang", scenario=dev_scenario)
        self.assertEqual(res.returncode, 0)

        dev_rec = self.sandbox.records()[-1]
        argv_str = " ".join(dev_rec["argv"])
        self.assertNotIn("qa-report", argv_str)

        prompt_file = Path(__file__).resolve().parent.parent / "brain" / "prompts" / "dev.md"
        prompt_text = prompt_file.read_text(encoding="utf-8")
        self.assertNotIn("qa-report", prompt_text)


    def test_dev_allowed_tools_with_root(self):
        from brain.role_dev import dev_allowed_tools
        tools = dev_allowed_tools(["src/app.py"], root=Path("/my/proj"))
        self.assertIn("Read(./src/app.py)", tools)
        self.assertIn("Read(//my/proj/src/app.py)", tools)
        self.assertIn("Edit(//my/proj/src/app.py)", tools)
        self.assertIn("Write(//my/proj/src/app.py)", tools)
        self.assertIn("Read(//my/proj/.plan.md)", tools)
        self.assertIn("Edit(//my/proj/.plan.md)", tools)
        self.assertIn("Write(//my/proj/.plan.md)", tools)


if __name__ == "__main__":
    unittest.main()
