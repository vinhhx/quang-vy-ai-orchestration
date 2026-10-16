from __future__ import annotations

import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from brain.agent import RunResult
from brain.planfile import render_plan_skeleton
import brain.tasklog as tasklog
from tests.helpers import Sandbox, lead_scenario


class TestTasklog(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name).resolve()
        self.protocol_dir = self.root / "protocol_log"
        self.protocol_dir.mkdir(parents=True, exist_ok=True)
        self.runs_dir = self.root / "runs"
        self.runs_dir.mkdir(parents=True, exist_ok=True)
        self.cfg = {
            "runs_dir": str(self.runs_dir),
            "protocol_log_dir": str(self.protocol_dir),
            "common_args": [],
            "models": {
                "lead": "opus",
                "dev": "sonnet",
                "qa": "sonnet",
            },
            "profiles": {
                "work": {
                    "claude_config_dir": None,
                    "unset_env": [],
                },
            },
        }
        self.project_dir = self.root / "my-project"
        self.project_dir.mkdir(parents=True, exist_ok=True)
        self.project = {
            "id": "proj-1",
            "name_vi": "Dự án Một",
            "path": str(self.project_dir),
            "profile": "work",
        }

    def tearDown(self):
        self.temp_dir.cleanup()

    def _create_plan_file(self, task: str, request: str, rows: list[str] | None = None, blockers: str = ""):
        meta = {"task": task, "request": request, "base_commit": "abc", "branch": f"task/{task}"}
        skel = render_plan_skeleton(task, meta)
        if rows:
            table = "\n".join(f"| {i+1} | {f} | func | change | why |" for i, f in enumerate(rows))
            skel = skel.replace("| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|",
                                f"| # | File | Dòng / hàm | Sửa gì | Lý do |\n|---|---|---|---|---|\n{table}")
        if blockers:
            skel = skel.replace("## Vướng mắc (Dev ghi vào đây nếu có)\n",
                                f"## Vướng mắc (Dev ghi vào đây nếu có)\n{blockers}\n")
        plan_path = self.project_dir / ".plan.md"
        plan_path.write_text(skel, encoding="utf-8")
        return plan_path

    def test_record_run_creates_new_task_log(self):
        self._create_plan_file("task-1", "Yêu cầu tính năng A", ["src/app.py"])
        raw_log = Path("tests/fixtures/stream/dev_two_reads_then_edit.jsonl").resolve()
        run_dir = self.root / "run1"
        run_dir.mkdir()
        result = RunResult(
            role="dev",
            exit_code=0,
            raw_log=raw_log,
            stderr_log=run_dir / "stderr.txt",
            run_dir=run_dir,
            started_at="2026-10-15T08:00:00Z",
            finished_at="2026-10-15T08:05:00Z",
            timed_out=False,
            interrupted=False,
        )

        log_path = tasklog.record_run(self.project, "task-1", "dev", result, self.cfg)
        self.assertTrue(log_path.is_file())
        self.assertEqual(log_path.name, "proj-1--task-1.md")

        content = log_path.read_text(encoding="utf-8")
        lines = content.splitlines()
        self.assertEqual(lines[0], "# Nhật ký task: task-1")
        self.assertIn("- Task: task-1 — Yêu cầu tính năng A", content)
        self.assertIn("- Dự án: proj-1 (Dự án Một)", content)
        self.assertIn("- Số turn của Dev trước khi bắt đầu sửa: 2", content)
        self.assertIn("- Dev có đọc file ngoài plan không: Không", content)
        self.assertIn("- QA phát hiện được gì:", content)
        self.assertIn("- Phải làm lại không, vì sao:", content)
        self.assertIn("## Số liệu tự động (brain ghi, không sửa tay)", content)
        self.assertIn("Số lần chạy: lead 0 · dev 1 · qa 0", content)

    def test_dev_rewrites_turn_and_read_lines_and_increments_dev_count(self):
        self._create_plan_file("task-1", "Yêu cầu tính năng A", ["src/app.py"])
        run_dir = self.root / "run1"
        run_dir.mkdir()
        result1 = RunResult(
            role="dev",
            exit_code=0,
            raw_log=Path("tests/fixtures/stream/dev_two_reads_then_edit.jsonl").resolve(),
            stderr_log=run_dir / "stderr.txt",
            run_dir=run_dir,
            started_at="2026-10-15T08:00:00Z",
            finished_at="2026-10-15T08:05:00Z",
            timed_out=False,
            interrupted=False,
        )
        tasklog.record_run(self.project, "task-1", "dev", result1, self.cfg)

        # Second dev run that reads other.py
        run_dir2 = self.root / "run2"
        run_dir2.mkdir()
        stream_other = self.root / "stream_other.jsonl"
        stream_other.write_text(
            '{"type": "system", "subtype": "init", "session_id": "s-other"}\n'
            '{"type": "assistant", "message": {"id": "m1", "content": [{"type": "tool_use", "name": "Read", "input": {"file_path": "other.py"}}]}}\n'
            '{"type": "assistant", "message": {"id": "m2", "content": [{"type": "tool_use", "name": "Edit", "input": {"file_path": "src/app.py"}}]}}\n'
            '{"type": "result", "subtype": "success", "session_id": "s-other", "num_turns": 2, "usage": {"input_tokens": 10, "output_tokens": 10}}\n',
            encoding="utf-8",
        )
        result2 = RunResult(
            role="dev",
            exit_code=0,
            raw_log=stream_other,
            stderr_log=run_dir2 / "stderr.txt",
            run_dir=run_dir2,
            started_at="2026-10-15T08:10:00Z",
            finished_at="2026-10-15T08:15:00Z",
            timed_out=False,
            interrupted=False,
        )
        log_path = tasklog.record_run(self.project, "task-1", "dev", result2, self.cfg)
        content = log_path.read_text(encoding="utf-8")
        self.assertIn("- Dev có đọc file ngoài plan không: Có: other.py", content)
        self.assertIn("Số lần chạy: lead 0 · dev 2 · qa 0", content)


    def test_user_lines_byte_identical_after_runs(self):
        self._create_plan_file("task-2", "Req", ["src/app.py"])
        run_dir = self.root / "run1"
        run_dir.mkdir()
        res = RunResult(
            role="dev",
            exit_code=0,
            raw_log=Path("tests/fixtures/stream/dev_two_reads_then_edit.jsonl").resolve(),
            stderr_log=run_dir / "stderr.txt",
            run_dir=run_dir,
            started_at="2026-10-15T08:00:00Z",
            finished_at="2026-10-15T08:05:00Z",
            timed_out=False,
            interrupted=False,
        )
        log_path = tasklog.record_run(self.project, "task-2", "dev", res, self.cfg)

        # Hand edit user lines
        user_text = (
            "- QA phát hiện được gì: Phát hiện lỗi chia cho 0 tại dòng 15.\n"
            "- Phải làm lại không, vì sao: Có, cần xử lý ZeroDivisionError."
        )
        content = log_path.read_text(encoding="utf-8")
        content = content.replace(
            "- QA phát hiện được gì:\n- Phải làm lại không, vì sao:",
            user_text,
        )
        log_path.write_text(content, encoding="utf-8")

        # Run 3 more times
        for i in range(3):
            rdir = self.root / f"run_qa_{i}"
            rdir.mkdir()
            rres = RunResult(
                role="qa",
                exit_code=0,
                raw_log=Path("tests/fixtures/stream/dev_two_reads_then_edit.jsonl").resolve(),
                stderr_log=rdir / "stderr.txt",
                run_dir=rdir,
                started_at="2026-10-15T08:20:00Z",
                finished_at="2026-10-15T08:25:00Z",
                timed_out=False,
                interrupted=False,
            )
            tasklog.record_run(self.project, "task-2", "qa", rres, self.cfg)

        new_content = log_path.read_text(encoding="utf-8")
        self.assertIn(user_text, new_content)

    def test_qa_verdict_can_sua_and_dev_blocker(self):
        # QA verdict Cần sửa
        self._create_plan_file("task-3", "Req", ["src/app.py"])
        qa_report = self.project_dir / ".qa-report.md"
        qa_report.write_text("# QA\n## Kết luận\nCần sửa\n", encoding="utf-8")

        run_dir = self.root / "run_qa"
        run_dir.mkdir()
        res_qa = RunResult(
            role="qa",
            exit_code=0,
            raw_log=Path("tests/fixtures/stream/dev_two_reads_then_edit.jsonl").resolve(),
            stderr_log=run_dir / "stderr.txt",
            run_dir=run_dir,
            started_at="2026-10-15T08:00:00Z",
            finished_at="2026-10-15T08:05:00Z",
            timed_out=False,
            interrupted=False,
        )
        log_path = tasklog.record_run(self.project, "task-3", "qa", res_qa, self.cfg)
        content = log_path.read_text(encoding="utf-8")
        self.assertIn("Cần sửa", content)

        # Dev blockers
        self._create_plan_file("task-3", "Req", ["src/app.py"], blockers="Thiếu API key")
        run_dir_dev = self.root / "run_dev_b"
        run_dir_dev.mkdir()
        res_dev = RunResult(
            role="dev",
            exit_code=4,
            raw_log=Path("tests/fixtures/stream/dev_blocked.jsonl").resolve(),
            stderr_log=run_dir_dev / "stderr.txt",
            run_dir=run_dir_dev,
            started_at="2026-10-15T08:10:00Z",
            finished_at="2026-10-15T08:15:00Z",
            timed_out=False,
            interrupted=False,
        )
        tasklog.record_run(self.project, "task-3", "dev", res_dev, self.cfg)
        content2 = log_path.read_text(encoding="utf-8")
        self.assertIn("- Số turn của Dev trước khi bắt đầu sửa: chưa sửa code (Vướng mắc)", content2)
        self.assertIn("Vướng mắc", content2)

    def test_blocked_calls_count_includes_guard_blocks(self):
        self._create_plan_file("task-guard", "Req", ["src/app.py"])
        run_dir = self.root / "run_guard"
        run_dir.mkdir()
        guard_log = run_dir / "guard.jsonl"
        with open(guard_log, "w", encoding="utf-8") as f:
            f.write(json.dumps({"ts": 1, "tool": "Read", "path": "x", "decision": "allow", "reason": "ok"}) + "\n")
            f.write(json.dumps({"ts": 2, "tool": "Read", "path": "y", "decision": "block", "reason": "forbidden"}) + "\n")
            f.write(json.dumps({"ts": 3, "tool": "Edit", "path": "z", "decision": "block", "reason": "forbidden"}) + "\n")

        # dev_two_reads_then_edit has 1 permission_denial. Together with 2 guard blocks -> 3 blocked calls!
        res = RunResult(
            role="dev",
            exit_code=0,
            raw_log=Path("tests/fixtures/stream/dev_two_reads_then_edit.jsonl").resolve(),
            stderr_log=run_dir / "stderr.txt",
            run_dir=run_dir,
            started_at="2026-10-15T08:00:00Z",
            finished_at="2026-10-15T08:05:00Z",
            timed_out=False,
            interrupted=False,
        )
        log_path = tasklog.record_run(self.project, "task-guard", "dev", res, self.cfg)
        content = log_path.read_text(encoding="utf-8")
        # Row has blocked count = 3
        self.assertIn("| 3 |", content)

    def test_no_information_leak_and_home_expansion(self):
        self._create_plan_file("task-leak", "Yêu cầu bình thường", ["src/app.py"])
        run_dir = self.root / "run_leak"
        run_dir.mkdir()
        # raw log contains SENTINEL-KHONG-DUOC-LOG
        raw_log = Path.home() / ".cache" / "stream.jsonl"
        raw_log.parent.mkdir(parents=True, exist_ok=True)
        raw_log.write_text(Path("tests/fixtures/stream/dev_two_reads_then_edit.jsonl").read_text(encoding="utf-8"), encoding="utf-8")

        res = RunResult(
            role="dev",
            exit_code=0,
            raw_log=raw_log,
            stderr_log=run_dir / "stderr.txt",
            run_dir=run_dir,
            started_at="2026-10-15T08:00:00Z",
            finished_at="2026-10-15T08:05:00Z",
            timed_out=False,
            interrupted=False,
        )
        log_path = tasklog.record_run(self.project, "task-leak", "dev", res, self.cfg)
        content = log_path.read_text(encoding="utf-8")

        self.assertNotIn("SENTINEL-KHONG-DUOC-LOG", content)
        self.assertNotIn(str(Path.home()), content)
        self.assertIn("~/.cache/stream.jsonl", content)


    def test_check_logs_min_complete(self):
        log_dir = self.root / "check_test_dir"
        log_dir.mkdir()

        # Create 4 complete logs
        for i in range(1, 5):
            p = log_dir / f"p1--t{i}.md"
            p.write_text(
                f"# Nhật ký task: t{i}\n"
                "- Task: t\n"
                "- Dự án: p\n"
                "- Số turn của Dev trước khi bắt đầu sửa: 2\n"
                "- Dev có đọc file ngoài plan không: Không\n"
                "- QA phát hiện được gì: Không có lỗi\n"
                "- Phải làm lại không, vì sao: Không\n",
                encoding="utf-8",
            )

        # 1 incomplete log (missing QA lines)
        p5 = log_dir / "p1--t5.md"
        p5.write_text(
            "# Nhật ký task: t5\n"
            "- Task: t5\n"
            "- Dự án: p\n"
            "- Số turn của Dev trước khi bắt đầu sửa: 2\n"
            "- Dev có đọc file ngoài plan không: Không\n"
            "- QA phát hiện được gì:\n"
            "- Phải làm lại không, vì sao:\n",
            encoding="utf-8",
        )

        # Also add non-task files that must be ignored
        (log_dir / "nghiem-thu-p1.md").write_text("# nghiệm thu", encoding="utf-8")
        (log_dir / "README.md").write_text("# readme", encoding="utf-8")

        # 4 complete + 1 incomplete -> check_logs(..., min_complete=5) exits 1
        code = tasklog.check_logs(log_dir, min_complete=5)
        self.assertEqual(code, 1)

        # Fill p5 completely -> 5 complete -> check_logs exits 0
        p5.write_text(
            "# Nhật ký task: t5\n"
            "- Task: t5\n"
            "- Dự án: p\n"
            "- Số turn của Dev trước khi bắt đầu sửa: 2\n"
            "- Dev có đọc file ngoài plan không: Không\n"
            "- QA phát hiện được gì: Đạt\n"
            "- Phải làm lại không, vì sao: Không\n",
            encoding="utf-8",
        )
        code2 = tasklog.check_logs(log_dir, min_complete=5)
        self.assertEqual(code2, 0)

    def test_record_run_failure_prints_warning(self):
        from brain.agent import RoleSpec, execute_role
        self._create_plan_file("task-fail", "Req", ["src/app.py"])
        # Mock tasklog.record_run to raise OSError
        spec = RoleSpec(
            role="lead",
            prompt_file=Path("brain/prompts/lead.md"),
            user_input="test",
            allowed_tools=[],
            disallowed_tools=[],
        )
        with patch("brain.tasklog.record_run", side_effect=OSError("Disk full")):
            with patch("brain.agent.subprocess.Popen") as mock_popen:
                mock_proc = mock_popen.return_value
                mock_proc.wait.return_value = 0
                mock_proc.stdout = []
                stderr_capture = io.StringIO()
                with patch("sys.stderr", stderr_capture):
                    res = execute_role(spec, self.project, "task-fail", self.cfg)
                self.assertEqual(res.exit_code, 0)
                self.assertIn("cảnh báo: không ghi được nhật ký", stderr_capture.getvalue())

    def test_e2e_lead_creates_task_log(self):
        sb = Sandbox()
        try:
            repo = sb.make_repo("proj_lead", files={"src/app.py": "def f(): return 1\n"})
            sb.write_map([sb.project_entry(repo, "proj_lead", name_vi="Dự án Lead")])
            scenario = lead_scenario([("src/app.py", "f", "sửa", "lý do")])
            res = sb.run_brain("lead", "proj_lead", "Yêu cầu e2e", "--name", "task-e2e", scenario=scenario)
            self.assertEqual(res.returncode, 0)

            # Check that log exists in sb.protocol_log
            log_file = sb.protocol_log / "proj_lead--task-e2e.md"
            self.assertTrue(log_file.is_file(), f"Expected {log_file} to exist")
            content = log_file.read_text(encoding="utf-8")
            self.assertIn("- Task: task-e2e — Yêu cầu e2e", content)
            self.assertIn("- Dự án: proj_lead (Dự án Lead)", content)
            self.assertIn("Số lần chạy: lead 1 · dev 0 · qa 0", content)
        finally:
            sb.cleanup()


if __name__ == "__main__":
    unittest.main()
