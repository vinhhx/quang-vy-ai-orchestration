import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class TestLiveHelpers(unittest.TestCase):
    def setUp(self):
        self.repo_root = Path(__file__).resolve().parent.parent
        self.script_path = self.repo_root / "scripts" / "live" / "claude-allow-sleep.sh"
        self.fake_claude = self.repo_root / "tests" / "fakes" / "fake_claude.py"

    def test_wrapper_inserts_sleep_rule(self):
        with tempfile.NamedTemporaryFile(mode="r", delete=False) as f:
            record_path = f.name
        self.addCleanup(lambda: os.unlink(record_path) if os.path.exists(record_path) else None)

        env = os.environ.copy()
        env["AGENT_BRAIN_REAL_CLAUDE"] = str(self.fake_claude)
        env["FAKE_CLAUDE_RECORD"] = record_path

        cmd = [
            str(self.script_path),
            "-p",
            "x",
            "--allowedTools",
            "Read",
            "Grep",
            "--disallowedTools",
            "Task",
        ]
        res = subprocess.run(cmd, env=env, capture_output=True, text=True)
        self.assertEqual(res.returncode, 0, f"stdout: {res.stdout}, stderr: {res.stderr}")

        with open(record_path, "r", encoding="utf-8") as f:
            lines = [json.loads(line) for line in f if line.strip()]
        self.assertTrue(lines, "No fake claude record written")
        recorded_argv = lines[-1]["argv"]
        expected_argv = [
            "-p",
            "x",
            "--allowedTools",
            "Bash(sleep *)",
            "Read",
            "Grep",
            "--disallowedTools",
            "Task",
        ]
        self.assertEqual(recorded_argv, expected_argv)

    def test_wrapper_without_allowed_tools_passes_through(self):
        with tempfile.NamedTemporaryFile(mode="r", delete=False) as f:
            record_path = f.name
        self.addCleanup(lambda: os.unlink(record_path) if os.path.exists(record_path) else None)

        env = os.environ.copy()
        env["AGENT_BRAIN_REAL_CLAUDE"] = str(self.fake_claude)
        env["FAKE_CLAUDE_RECORD"] = record_path

        cmd = [
            str(self.script_path),
            "-p",
            "x",
            "--disallowedTools",
            "Task",
        ]
        res = subprocess.run(cmd, env=env, capture_output=True, text=True)
        self.assertEqual(res.returncode, 0, f"stdout: {res.stdout}, stderr: {res.stderr}")

        with open(record_path, "r", encoding="utf-8") as f:
            lines = [json.loads(line) for line in f if line.strip()]
        self.assertTrue(lines, "No fake claude record written")
        recorded_argv = lines[-1]["argv"]
        expected_argv = ["-p", "x", "--disallowedTools", "Task"]
        self.assertEqual(recorded_argv, expected_argv)


if __name__ == "__main__":
    unittest.main()
