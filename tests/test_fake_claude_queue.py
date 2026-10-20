#!/usr/bin/env python3
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class TestFakeClaudeQueue(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.dir_path = Path(self.temp_dir.name)
        self.fake_script = (
            Path(__file__).resolve().parent / "fakes" / "fake_claude.py"
        )
        self.scenario_file = self.dir_path / "scenario.json"
        self.state_file = self.dir_path / "scenario.json.state.json"

    def tearDown(self):
        self.temp_dir.cleanup()

    def _run_fake(
        self, prompt: str | None = None, extra_args: list[str] | None = None
    ) -> tuple[int, list[dict]]:
        cmd = [sys.executable, str(self.fake_script), "-p", "x"]
        if prompt is not None:
            cmd.extend(["--append-system-prompt", prompt])
        if extra_args:
            cmd.extend(extra_args)

        env = os.environ.copy()
        env["FAKE_CLAUDE_SCENARIO"] = str(self.scenario_file)

        res = subprocess.run(
            cmd,
            cwd=str(self.dir_path),
            env=env,
            capture_output=True,
            text=True,
        )
        events = []
        for line in res.stdout.splitlines():
            line = line.trim() if hasattr(line, "trim") else line.strip()
            if line:
                try:
                    events.append(json.loads(line))
                except Exception:
                    pass
        return res.returncode, events

    def test_queue_per_role_in_order(self):
        scenario = {
            "by_role": {
                "lead": [{"result_cost": 0.1}, {"result_cost": 0.2}],
                "qa": [{"result_cost": 0.5}],
            }
        }
        with open(self.scenario_file, "w", encoding="utf-8") as f:
            json.dump(scenario, f)

        # Call 1 Lead
        code, events = self._run_fake(prompt="Bạn là Lead.\nHãy lên plan.")
        self.assertEqual(code, 0)
        res_ev = next(e for e in events if e.get("type") == "result")
        self.assertEqual(res_ev.get("total_cost_usd"), 0.1)

        # Call 2 Lead
        code, events = self._run_fake(prompt="Bạn là Lead.\nHãy lên plan vòng 2.")
        self.assertEqual(code, 0)
        res_ev = next(e for e in events if e.get("type") == "result")
        self.assertEqual(res_ev.get("total_cost_usd"), 0.2)

        # Call 3 Lead (repeats last)
        code, events = self._run_fake(prompt="Bạn là Lead.\nHãy lên plan vòng 3.")
        self.assertEqual(code, 0)
        res_ev = next(e for e in events if e.get("type") == "result")
        self.assertEqual(res_ev.get("total_cost_usd"), 0.2)

        # Call 1 QA
        code, events = self._run_fake(prompt="Bạn là QA.\nHãy kiểm thử.")
        self.assertEqual(code, 0)
        res_ev = next(e for e in events if e.get("type") == "result")
        self.assertEqual(res_ev.get("total_cost_usd"), 0.5)

        # Verify state file
        self.assertTrue(self.state_file.is_file())
        with open(self.state_file, "r", encoding="utf-8") as f:
            state = json.load(f)
        self.assertEqual(state, {"lead": 3, "qa": 1})

    def test_reviewer_roles_detected(self):
        scenario = {
            "by_role": {
                "plan-review": [{"result_cost": 1.1}],
                "code-review": [{"result_cost": 2.2}],
                "dev": [{"result_cost": 3.3}],
            }
        }
        with open(self.scenario_file, "w", encoding="utf-8") as f:
            json.dump(scenario, f)

        # Plan Reviewer
        code, events = self._run_fake(prompt="Bạn là Plan Reviewer.\nXem xét plan.")
        self.assertEqual(code, 0)
        res_ev = next(e for e in events if e.get("type") == "result")
        self.assertEqual(res_ev.get("total_cost_usd"), 1.1)

        # Code Reviewer
        code, events = self._run_fake(prompt="Bạn là Code Reviewer.\nXem xét diff.")
        self.assertEqual(code, 0)
        res_ev = next(e for e in events if e.get("type") == "result")
        self.assertEqual(res_ev.get("total_cost_usd"), 2.2)

        # Dev (chế độ sửa)
        code, events = self._run_fake(prompt="Bạn là Dev (chế độ sửa).\nSửa lỗi.")
        self.assertEqual(code, 0)
        res_ev = next(e for e in events if e.get("type") == "result")
        self.assertEqual(res_ev.get("total_cost_usd"), 3.3)

    def test_result_tokens_shortcut(self):
        scenario = {
            "by_role": {
                "lead": [
                    {
                        "result_tokens": {
                            "input": 100,
                            "output": 20,
                            "cache_read": 3000,
                            "cache_write": 500,
                        }
                    }
                ]
            }
        }
        with open(self.scenario_file, "w", encoding="utf-8") as f:
            json.dump(scenario, f)

        code, events = self._run_fake(prompt="Bạn là Lead.\nLên plan.")
        self.assertEqual(code, 0)
        res_ev = next(e for e in events if e.get("type") == "result")
        expected_usage = {
            "input_tokens": 100,
            "output_tokens": 20,
            "cache_read_input_tokens": 3000,
            "cache_creation_input_tokens": 500,
        }
        self.assertEqual(res_ev.get("usage"), expected_usage)

    def test_entry_ops_and_exit_code(self):
        scenario = {
            "by_role": {
                "dev": [
                    {
                        "ops": [
                            {
                                "op": "write",
                                "path": "output.txt",
                                "content": "generated by dev",
                            }
                        ],
                        "exit_code": 3,
                    }
                ]
            }
        }
        with open(self.scenario_file, "w", encoding="utf-8") as f:
            json.dump(scenario, f)

        code, _ = self._run_fake(prompt="Bạn là Dev.\nThực hiện.")
        self.assertEqual(code, 3)
        created_file = self.dir_path / "output.txt"
        self.assertTrue(created_file.is_file())
        self.assertEqual(created_file.read_text(encoding="utf-8"), "generated by dev")

    def test_legacy_scenario_unchanged(self):
        scenario = {
            "sleep": 0.01,
            "ops": [
                {
                    "op": "write",
                    "path": "legacy.txt",
                    "content": "legacy content",
                }
            ],
        }
        with open(self.scenario_file, "w", encoding="utf-8") as f:
            json.dump(scenario, f)

        code, events = self._run_fake(prompt="Bạn là Lead.\nLập plan.")
        self.assertEqual(code, 0)
        res_ev = next(e for e in events if e.get("type") == "result")
        self.assertEqual(res_ev.get("total_cost_usd"), 0)
        expected_usage = {
            "input_tokens": 10,
            "output_tokens": 5,
            "cache_read_input_tokens": 0,
            "cache_creation_input_tokens": 0,
        }
        self.assertEqual(res_ev.get("usage"), expected_usage)
        self.assertFalse(self.state_file.exists())


if __name__ == "__main__":
    unittest.main()
