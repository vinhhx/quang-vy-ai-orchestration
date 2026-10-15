from __future__ import annotations

import unittest
from pathlib import Path

from brain import BrainError
from brain.agent import RoleSpec, build_command
from brain.planfile import PLAN_SECTIONS, parse_plan, read_meta
from tests.helpers import Sandbox, lead_scenario, plan_text


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

        scenario = lead_scenario([("src/app.py", "1-5", "thêm nút đăng xuất", "yêu cầu")])

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

    def test_p1_06_lead_only_writes_plan(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(
            repo,
            id="web-ban-hang",
            name_vi="Web bán hàng",
            profile="work",
        )
        self.sandbox.write_map([entry])

        scenario = lead_scenario([("src/app.py", "10-20", "sửa hàm app", "yêu cầu")])
        res = self.sandbox.run_brain(
            "lead",
            "web bán hàng",
            "sửa hàm app",
            scenario=scenario,
        )
        self.assertEqual(res.returncode, 0, f"STDOUT: {res.stdout}\nSTDERR: {res.stderr}")

        # Porcelain status must be empty
        status = self.sandbox.git(repo, "status", "--porcelain")
        self.assertEqual(status.strip(), "")

        # HEAD commit touches only .plan.md
        diff_tree = [
            line.strip()
            for line in self.sandbox.git(
                repo, "diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(diff_tree, [".plan.md"])

        # HEAD commit subject is {task}: plan
        head_subject = self.sandbox.git(repo, "log", "-1", "--format=%s").strip()
        self.assertEqual(head_subject, "sua-ham-app: plan")

        # Plan content checks
        plan_content = (repo / ".plan.md").read_text(encoding="utf-8")
        self.assertTrue(plan_content.startswith("# Plan: "))
        meta = read_meta(plan_content)
        self.assertIsNotNone(meta)
        self.assertEqual(meta["task"], "sua-ham-app")
        self.assertEqual(meta["branch"], "task/sua-ham-app")
        self.assertEqual(meta["base_branch"], "main")
        self.assertTrue(meta["base_commit"])
        self.assertEqual(meta["request"], "sửa hàm app")

        plan = parse_plan(plan_content)
        for sec in PLAN_SECTIONS:
            self.assertIn(sec, plan.sections)

    def test_lead_edits_out_of_plan_path_exits_3(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(
            repo,
            id="web-ban-hang",
            name_vi="Web bán hàng",
            profile="work",
        )
        self.sandbox.write_map([entry])
        base_commit = self.sandbox.git(repo, "rev-parse", "HEAD").strip()

        # Scenario writes .plan.md but also edits src/app.py
        p_text = plan_text("Sửa app", [("src/app.py", "1", "sửa", "lý do")])
        scenario = {
            "ops": [
                {"op": "read", "path": ".plan.md"},
                {"op": "write", "path": ".plan.md", "content": p_text, "keep_meta": True},
                {"op": "write", "path": "src/app.py", "content": "# fake claude hacked app.py\n"},
            ]
        }

        res = self.sandbox.run_brain(
            "lead",
            "web bán hàng",
            "sửa app",
            scenario=scenario,
        )
        self.assertEqual(res.returncode, 3, f"Expected exit 3, got {res.returncode}. STDOUT: {res.stdout}\nSTDERR: {res.stderr}")
        combined_output = res.stdout + res.stderr
        self.assertIn("src/app.py", combined_output)

        # HEAD still equals base commit (no commit made)
        head_commit = self.sandbox.git(repo, "rev-parse", "HEAD").strip()
        self.assertEqual(head_commit, base_commit)

        # src/app.py still keeps the edit (brain reverts nothing)
        content = (repo / "src/app.py").read_text(encoding="utf-8")
        self.assertEqual(content, "# fake claude hacked app.py\n")

    def test_dirty_tree_refuses_exit_1(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(
            repo,
            id="web-ban-hang",
            name_vi="Web bán hàng",
            profile="work",
        )
        self.sandbox.write_map([entry])

        # Make tree dirty
        (repo / "src" / "app.py").write_text("# uncommitted dirty work\n", encoding="utf-8")

        res = self.sandbox.run_brain("lead", "web bán hàng", "thêm nút")
        self.assertEqual(res.returncode, 1)
        self.assertIn("Working tree có thay đổi", res.stderr)
        self.assertEqual(len(self.sandbox.records()), 0)

        branches = self.sandbox.git(repo, "branch", "--list", "task/*").strip()
        self.assertEqual(branches, "")

    def test_empty_profile_refuses_exit_1(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="")
        self.sandbox.write_map([entry])

        res = self.sandbox.run_brain("lead", "web-ban-hang", "task")
        self.assertEqual(res.returncode, 1)
        self.assertEqual(len(self.sandbox.records()), 0)

    def test_unknown_profile_refuses_exit_1(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="nonexistent")
        self.sandbox.write_map([entry])

        res = self.sandbox.run_brain("lead", "web-ban-hang", "task")
        self.assertEqual(res.returncode, 1)
        self.assertEqual(len(self.sandbox.records()), 0)

    def test_missing_project_path_refuses_exit_1(self):
        entry = self.sandbox.project_entry(self.sandbox.root / "missing-dir", id="missing-proj", profile="work")
        self.sandbox.write_map([entry])

        res = self.sandbox.run_brain("lead", "missing-proj", "task")
        self.assertEqual(res.returncode, 1)
        self.assertEqual(len(self.sandbox.records()), 0)

    def test_not_a_git_repo_refuses_exit_1(self):
        non_git = self.sandbox.root / "non-git"
        non_git.mkdir(parents=True, exist_ok=True)
        entry = self.sandbox.project_entry(non_git, id="non-git", profile="work")
        self.sandbox.write_map([entry])

        res = self.sandbox.run_brain("lead", "non-git", "task")
        self.assertEqual(res.returncode, 1)
        self.assertEqual(len(self.sandbox.records()), 0)

    def test_ambiguous_project_query_exits_2(self):
        repo1 = self.sandbox.make_repo("proj1")
        repo2 = self.sandbox.make_repo("proj2")
        e1 = self.sandbox.project_entry(repo1, id="proj1", aliases=("shop",))
        e2 = self.sandbox.project_entry(repo2, id="proj2", aliases=("shop",))
        self.sandbox.write_map([e1, e2])

        res = self.sandbox.run_brain("lead", "shop", "task")
        self.assertEqual(res.returncode, 2)
        self.assertIn("proj1", res.stdout)
        self.assertIn("proj2", res.stdout)
        self.assertEqual(len(self.sandbox.records()), 0)

    def test_plan_with_zero_changes_fails_validation_exit_1(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([entry])
        base_commit = self.sandbox.git(repo, "rev-parse", "HEAD").strip()

        # Zero changes
        scenario = lead_scenario([])
        res = self.sandbox.run_brain("lead", "web-ban-hang", "task", scenario=scenario)
        self.assertEqual(res.returncode, 1)
        self.assertIn("no_changes", res.stderr)
        self.assertIn("Sửa tay .plan.md rồi: git -C", res.stderr)
        self.assertIn('commit -m "task: plan" -- .plan.md', res.stderr)

        # Plan stays uncommitted, HEAD unchanged
        head_commit = self.sandbox.git(repo, "rev-parse", "HEAD").strip()
        self.assertEqual(head_commit, base_commit)
        self.assertTrue((repo / ".plan.md").is_file())

    def test_lead_timeout_exits_1_no_commit(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([entry])
        base_commit = self.sandbox.git(repo, "rev-parse", "HEAD").strip()

        self.sandbox.write_config(timeout_seconds={"lead": 1, "dev": 30, "qa": 30})
        scenario = {"sleep": 5}
        res = self.sandbox.run_brain("lead", "web-ban-hang", "task", scenario=scenario)
        self.assertEqual(res.returncode, 1)
        self.assertIn("quá thời gian (timeout)", res.stderr)

        head_commit = self.sandbox.git(repo, "rev-parse", "HEAD").strip()
        self.assertEqual(head_commit, base_commit)

    def test_fake_overwrites_meta_restores_meta(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([entry])

        # Write plan content WITHOUT meta
        content_no_meta = plan_text("Nhiem vu", [("src/app.py", "1", "sửa", "lý do")])
        scenario = {
            "ops": [
                {"op": "read", "path": ".plan.md"},
                {"op": "write", "path": ".plan.md", "content": content_no_meta, "keep_meta": False},
            ]
        }
        res = self.sandbox.run_brain("lead", "web-ban-hang", "nhiệm vụ", scenario=scenario)
        self.assertEqual(res.returncode, 0, f"STDOUT: {res.stdout}\nSTDERR: {res.stderr}")

        plan_content = (repo / ".plan.md").read_text(encoding="utf-8")
        meta = read_meta(plan_content)
        self.assertIsNotNone(meta)
        self.assertEqual(meta["task"], "nhiem-vu")
        self.assertEqual(meta["branch"], "task/nhiem-vu")
        self.assertEqual(meta["base_branch"], "main")
        self.assertTrue(meta["base_commit"])
        self.assertEqual(meta["request"], "nhiệm vụ")

    def test_replan_on_same_task_branch_preserves_base_commit(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([entry])

        # First run
        sc1 = lead_scenario([("src/app.py", "1", "sửa lần 1", "lý do 1")])
        res1 = self.sandbox.run_brain("lead", "web-ban-hang", "nhiệm vụ", scenario=sc1)
        self.assertEqual(res1.returncode, 0)

        meta1 = read_meta((repo / ".plan.md").read_text(encoding="utf-8"))
        self.assertIsNotNone(meta1)

        # Second run on current task branch
        sc2 = lead_scenario([("src/app.py", "2", "sửa lần 2", "lý do 2")])
        res2 = self.sandbox.run_brain("lead", "web-ban-hang", "nhiệm vụ", scenario=sc2)
        self.assertEqual(res2.returncode, 0, f"STDOUT: {res2.stdout}\nSTDERR: {res2.stderr}")

        meta2 = read_meta((repo / ".plan.md").read_text(encoding="utf-8"))
        self.assertIsNotNone(meta2)
        self.assertEqual(meta2["base_commit"], meta1["base_commit"])
        self.assertEqual(meta2["base_branch"], meta1["base_branch"])
        self.assertEqual(meta2["created_at"], meta1["created_at"])

    def test_branch_exists_not_checked_out_refuses_exit_1(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([entry])

        # Create branch task/my-slug while on main
        self.sandbox.git(repo, "branch", "task/my-slug")

        res = self.sandbox.run_brain("lead", "web-ban-hang", "yêu cầu", "--name", "my-slug")
        self.assertEqual(res.returncode, 1)
        self.assertIn("đã tồn tại nhưng chưa được checkout", res.stderr)
        self.assertEqual(len(self.sandbox.records()), 0)

    def test_dry_run_creates_no_branch_and_no_record(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([entry])

        res = self.sandbox.run_brain("lead", "web-ban-hang", "thêm nút", "--dry-run")
        self.assertEqual(res.returncode, 0)
        self.assertEqual(len(self.sandbox.records()), 0)
        branches = self.sandbox.git(repo, "branch", "--list", "task/*").strip()
        self.assertEqual(branches, "")

    def test_build_command_rejects_permission_bypass(self):
        spec = RoleSpec(
            role="lead",
            prompt_file=self.sandbox.root / "prompt.md",
            user_input="test",
            allowed_tools=[],
            disallowed_tools=[],
        )
        (self.sandbox.root / "prompt.md").write_text("prompt", encoding="utf-8")

        for bypass_flag in [
            "--dangerously-skip-permissions",
            "--allow-dangerously-skip-permissions",
            "--permission-mode dont-ask",
            "--bypassPermissions",
        ]:
            cfg = {
                "claude_bin": "claude",
                "models": {"lead": "opus"},
                "common_args": [bypass_flag],
            }
            with self.assertRaises(BrainError):
                build_command(spec, cfg)

    def test_replan_after_timeout_regenerates_and_keeps_base_commit(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([entry])
        base = self.sandbox.git(repo, "rev-parse", "HEAD").strip()

        self.sandbox.write_config(timeout_seconds={"lead": 1, "dev": 30, "qa": 30})
        res1 = self.sandbox.run_brain(
            "lead",
            "web-ban-hang",
            "thêm nút",
            scenario={"sleep": 5},
        )
        self.assertEqual(res1.returncode, 1)
        current_branch = self.sandbox.git(repo, "branch", "--show-current").strip()
        self.assertEqual(current_branch, "task/them-nut")
        status1 = self.sandbox.git(repo, "status", "--porcelain")
        self.assertIn("?? .plan.md", status1)

        self.sandbox.write_config()
        scenario = lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])
        res2 = self.sandbox.run_brain(
            "lead",
            "web-ban-hang",
            "thêm nút",
            scenario=scenario,
        )
        self.assertEqual(res2.returncode, 0, f"STDOUT: {res2.stdout}\nSTDERR: {res2.stderr}")

        head_subject = self.sandbox.git(repo, "log", "-1", "--format=%s").strip()
        self.assertEqual(head_subject, "them-nut: plan")

        status2 = self.sandbox.git(repo, "status", "--porcelain")
        self.assertEqual(status2.strip(), "")

        plan_content = (repo / ".plan.md").read_text(encoding="utf-8")
        meta = read_meta(plan_content)
        self.assertIsNotNone(meta)
        self.assertEqual(meta["base_commit"], base)

    def test_replan_after_validation_failure_succeeds(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([entry])

        sc1 = lead_scenario([])
        res1 = self.sandbox.run_brain("lead", "web-ban-hang", "thêm nút", scenario=sc1)
        self.assertEqual(res1.returncode, 1)
        status1 = self.sandbox.git(repo, "status", "--porcelain")
        self.assertIn(".plan.md", status1)
        head_subject1 = self.sandbox.git(repo, "log", "-1", "--format=%s").strip()
        self.assertEqual(head_subject1, "init")

        sc2 = lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])
        res2 = self.sandbox.run_brain("lead", "web-ban-hang", "thêm nút", scenario=sc2)
        self.assertEqual(res2.returncode, 0, f"STDOUT: {res2.stdout}\nSTDERR: {res2.stderr}")

        head_subject2 = self.sandbox.git(repo, "log", "-1", "--format=%s").strip()
        self.assertEqual(head_subject2, "them-nut: plan")

    def test_replan_refuses_when_other_files_dirty(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([entry])

        self.sandbox.write_config(timeout_seconds={"lead": 1, "dev": 30, "qa": 30})
        res1 = self.sandbox.run_brain(
            "lead",
            "web-ban-hang",
            "thêm nút",
            scenario={"sleep": 5},
        )
        self.assertEqual(res1.returncode, 1)

        records_before = len(self.sandbox.records())
        (repo / "src" / "app.py").write_text("# manual edit\n", encoding="utf-8")

        self.sandbox.write_config()
        sc2 = lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])
        res2 = self.sandbox.run_brain(
            "lead",
            "web-ban-hang",
            "thêm nút",
            scenario=sc2,
        )
        self.assertEqual(res2.returncode, 1)
        combined_output = res2.stdout + res2.stderr
        self.assertIn("src/app.py", combined_output)
        self.assertEqual(len(self.sandbox.records()), records_before)
        self.assertEqual(
            (repo / "src" / "app.py").read_text(encoding="utf-8"),
            "# manual edit\n",
        )

    def test_leftover_plan_on_other_branch_refuses(self):
        repo = self.sandbox.make_repo("web-ban-hang")
        entry = self.sandbox.project_entry(repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([entry])

        (repo / ".plan.md").write_text("# leftover plan\n", encoding="utf-8")

        res = self.sandbox.run_brain("lead", "web-ban-hang", "việc khác")
        self.assertEqual(res.returncode, 1)
        combined_output = res.stdout + res.stderr
        self.assertIn(".plan.md", combined_output)

        branches = self.sandbox.git(repo, "branch", "--list", "task/*").strip()
        self.assertEqual(branches, "")
        self.assertEqual(len(self.sandbox.records()), 0)


if __name__ == "__main__":
    unittest.main()
