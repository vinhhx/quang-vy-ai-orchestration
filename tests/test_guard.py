from __future__ import annotations

import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import unicodedata
import unittest

from brain.config import repo_root


class TestGuardHook(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name).resolve()
        self.guard_py = (repo_root() / "brain" / "guard.py").resolve()

        self.src_dir = self.root / "src"
        self.src_dir.mkdir(parents=True, exist_ok=True)
        self.app_py = self.src_dir / "app.py"
        self.app_py.write_text("def app(): pass\n", encoding="utf-8")

        self.plan_md = self.root / ".plan.md"
        self.plan_md.write_text("# Plan\n", encoding="utf-8")

        self.other_py = self.root / "other.py"
        self.other_py.write_text("# other\n", encoding="utf-8")

        self.log_file = self.root / "guard.jsonl"
        self.policy_file = self.root / "policy.json"

        self.policy = {
            "root": str(self.root),
            "read_allow": [str(self.app_py), str(self.plan_md)],
            "write_allow": [str(self.app_py), str(self.plan_md)],
            "log": str(self.log_file),
        }
        self.write_policy(self.policy)

    def tearDown(self):
        self.temp_dir.cleanup()

    def write_policy(self, data: dict) -> None:
        self.policy_file.write_text(json.dumps(data, indent=2), encoding="utf-8")

    def run_guard(self, payload: dict | str, policy_path: Path | None = None) -> subprocess.CompletedProcess:
        p = str(policy_path or self.policy_file)
        cmd = [sys.executable, "-I", str(self.guard_py), "--policy", p]
        stdin_data = payload if isinstance(payload, str) else json.dumps(payload)
        return subprocess.run(
            cmd,
            input=stdin_data,
            text=True,
            capture_output=True,
        )

    def read_log_records(self) -> list[dict]:
        if not self.log_file.is_file():
            return []
        records = []
        for line in self.log_file.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line:
                records.append(json.loads(line))
        return records

    def test_read_allowed(self):
        payload = {
            "tool_name": "Read",
            "tool_input": {"file_path": str(self.app_py)},
            "cwd": str(self.root),
        }
        res = self.run_guard(payload)
        self.assertEqual(res.returncode, 0)
        records = self.read_log_records()
        self.assertEqual(len(records), 1)
        self.assertEqual(records[0]["decision"], "allow")
        self.assertEqual(records[0]["tool"], "Read")

    def test_read_denied_stderr_names_path(self):
        payload = {
            "tool_name": "Read",
            "tool_input": {"file_path": str(self.other_py)},
            "cwd": str(self.root),
        }
        res = self.run_guard(payload)
        self.assertEqual(res.returncode, 2)
        self.assertIn("other.py", res.stderr)
        self.assertIn("brain guard: chặn Read", res.stderr)
        records = self.read_log_records()
        self.assertEqual(len(records), 1)
        self.assertEqual(records[0]["decision"], "block")

    def test_relative_path_resolved_against_cwd(self):
        payload = {
            "tool_name": "Read",
            "tool_input": {"file_path": "src/app.py"},
            "cwd": str(self.root),
        }
        res = self.run_guard(payload)
        self.assertEqual(res.returncode, 0)

    def test_parent_traversal_blocked(self):
        payload = {
            "tool_name": "Read",
            "tool_input": {"file_path": "src/../other.py"},
            "cwd": str(self.root),
        }
        res = self.run_guard(payload)
        self.assertEqual(res.returncode, 2)
        self.assertIn("other.py", res.stderr)

    def test_symlink_pointing_outside_blocked(self):
        outside_dir = tempfile.TemporaryDirectory()
        try:
            outside_file = Path(outside_dir.name) / "secret.py"
            outside_file.write_text("secret = 1\n", encoding="utf-8")
            link_file = self.root / "link.py"
            os.symlink(outside_file, link_file)

            payload = {
                "tool_name": "Read",
                "tool_input": {"file_path": str(link_file)},
                "cwd": str(self.root),
            }
            res = self.run_guard(payload)
            self.assertEqual(res.returncode, 2)
        finally:
            outside_dir.cleanup()

    def test_edit_allowed_write_outside_blocked(self):
        payload_edit = {
            "tool_name": "Edit",
            "tool_input": {"file_path": str(self.app_py)},
            "cwd": str(self.root),
        }
        res_edit = self.run_guard(payload_edit)
        self.assertEqual(res_edit.returncode, 0)

        payload_write = {
            "tool_name": "Write",
            "tool_input": {"file_path": str(self.root / "new.py")},
            "cwd": str(self.root),
        }
        res_write = self.run_guard(payload_write)
        self.assertEqual(res_write.returncode, 2)
        self.assertIn("new.py", res_write.stderr)

    def test_multiedit_uses_file_path(self):
        payload_ok = {
            "tool_name": "MultiEdit",
            "tool_input": {"file_path": str(self.app_py)},
            "cwd": str(self.root),
        }
        self.assertEqual(self.run_guard(payload_ok).returncode, 0)

        payload_bad = {
            "tool_name": "MultiEdit",
            "tool_input": {"file_path": str(self.other_py)},
            "cwd": str(self.root),
        }
        self.assertEqual(self.run_guard(payload_bad).returncode, 2)

    def test_notebookedit_uses_notebook_path(self):
        payload_ok = {
            "tool_name": "NotebookEdit",
            "tool_input": {"notebook_path": str(self.app_py)},
            "cwd": str(self.root),
        }
        self.assertEqual(self.run_guard(payload_ok).returncode, 0)

        payload_bad = {
            "tool_name": "NotebookEdit",
            "tool_input": {"notebook_path": str(self.other_py)},
            "cwd": str(self.root),
        }
        self.assertEqual(self.run_guard(payload_bad).returncode, 2)

    def test_grep_and_glob_blocked_when_read_allow_set(self):
        payload_grep = {
            "tool_name": "Grep",
            "tool_input": {"pattern": "def", "path": str(self.root)},
            "cwd": str(self.root),
        }
        res_grep = self.run_guard(payload_grep)
        self.assertEqual(res_grep.returncode, 2)

        payload_glob = {
            "tool_name": "Glob",
            "tool_input": {"pattern": "*.py", "path": str(self.root)},
            "cwd": str(self.root),
        }
        res_glob = self.run_guard(payload_glob)
        self.assertEqual(res_glob.returncode, 2)

    def test_read_free_mode(self):
        self.policy["read_allow"] = None
        self.write_policy(self.policy)

        payload_read = {
            "tool_name": "Read",
            "tool_input": {"file_path": str(self.other_py)},
            "cwd": str(self.root),
        }
        self.assertEqual(self.run_guard(payload_read).returncode, 0)

        payload_grep = {
            "tool_name": "Grep",
            "tool_input": {"pattern": "def", "path": str(self.root)},
            "cwd": str(self.root),
        }
        self.assertEqual(self.run_guard(payload_grep).returncode, 0)

        payload_write = {
            "tool_name": "Write",
            "tool_input": {"file_path": str(self.other_py)},
            "cwd": str(self.root),
        }
        self.assertEqual(self.run_guard(payload_write).returncode, 2)

    def test_non_guarded_tool_allowed(self):
        payload = {
            "tool_name": "Bash",
            "tool_input": {"command": "git status"},
            "cwd": str(self.root),
        }
        self.assertEqual(self.run_guard(payload).returncode, 0)

    def test_fail_closed_on_malformed_input_or_missing_policy(self):
        res1 = self.run_guard("not a json")
        self.assertEqual(res1.returncode, 2)

        res2 = self.run_guard("{}", policy_path=self.root / "nonexistent.json")
        self.assertEqual(res2.returncode, 2)

        payload_missing_path = {
            "tool_name": "Write",
            "tool_input": {},
            "cwd": str(self.root),
        }
        self.assertEqual(self.run_guard(payload_missing_path).returncode, 2)

    def test_nfc_nfd_normalization(self):
        # Vietnamese name with diacritics: tiếng_việt.py
        nfc_name = unicodedata.normalize("NFC", "tiếng_việt.py")
        nfd_name = unicodedata.normalize("NFD", "tiếng_việt.py")
        self.assertNotEqual(nfc_name, nfd_name)

        target = self.root / nfc_name
        target.write_text("# vn\n", encoding="utf-8")

        self.policy["read_allow"].append(str(target))
        self.write_policy(self.policy)

        # Call with NFD path
        payload = {
            "tool_name": "Read",
            "tool_input": {"file_path": str(self.root / nfd_name)},
            "cwd": str(self.root),
        }
        self.assertEqual(self.run_guard(payload).returncode, 0)

    def test_write_hook_settings(self):
        from brain.guard import write_hook_settings

        run_dir = self.root / "run_test"
        run_dir.mkdir(parents=True, exist_ok=True)

        settings_path = write_hook_settings(
            run_dir=run_dir,
            root=self.root,
            read_allow=[self.app_py, self.plan_md],
            write_allow=[self.app_py, self.plan_md],
        )

        self.assertTrue(settings_path.is_file())
        settings_data = json.loads(settings_path.read_text(encoding="utf-8"))
        hooks = settings_data.get("hooks", {}).get("PreToolUse", [])
        self.assertEqual(len(hooks), 1)
        entry = hooks[0]
        self.assertEqual(entry["matcher"], "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob")
        self.assertNotIn("command", entry)
        nested_hooks = entry.get("hooks")
        self.assertIsInstance(nested_hooks, list)
        self.assertEqual(len(nested_hooks), 1)
        cmd = nested_hooks[0].get("command", "")
        self.assertEqual(entry["hooks"], [{"type": "command", "command": cmd}])
        self.assertIn("-I", cmd)
        self.assertIn(str(self.guard_py), cmd)
        self.assertIn("--policy", cmd)

        policy_path = run_dir / "policy.json"
        self.assertTrue(policy_path.is_file())
        self.assertIn(str(policy_path), cmd)
        pdata = json.loads(policy_path.read_text(encoding="utf-8"))
        self.assertEqual(pdata["root"], str(self.root))
        self.assertIn(str(self.app_py), pdata["read_allow"])
        self.assertEqual(pdata["log"], str(run_dir / "guard.jsonl"))

    def test_fake_ignores_flat_hook_shape(self):
        fake_claude = (repo_root() / "tests" / "fakes" / "fake_claude.py").resolve()
        run_dir = self.root / "flat_run"
        run_dir.mkdir(parents=True, exist_ok=True)

        policy_path = run_dir / "policy.json"
        policy_data = {
            "root": str(self.root),
            "read_allow": [str(self.app_py)],
            "write_allow": [str(self.app_py)],
            "log": str(run_dir / "guard.jsonl"),
        }
        policy_path.write_text(json.dumps(policy_data, indent=2), encoding="utf-8")

        cmd = (
            f"{shlex.quote(sys.executable)} -I {shlex.quote(str(self.guard_py))} "
            f"--policy {shlex.quote(str(policy_path))}"
        )
        flat_settings = {
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Read|Edit|Write|MultiEdit|NotebookEdit|Grep|Glob",
                        "command": cmd,
                    }
                ]
            }
        }
        settings_path = run_dir / "settings.json"
        settings_path.write_text(json.dumps(flat_settings, indent=2), encoding="utf-8")

        scenario_path = self.root / "flat_scenario.json"
        scenario = {
            "ops": [
                {"op": "read", "path": str(self.other_py)},
            ]
        }
        scenario_path.write_text(json.dumps(scenario), encoding="utf-8")

        record_path = self.root / "flat_record.jsonl"
        env = dict(os.environ)
        env["FAKE_CLAUDE_SCENARIO"] = str(scenario_path)
        env["FAKE_CLAUDE_RECORD"] = str(record_path)

        res = subprocess.run(
            [sys.executable, str(fake_claude), "--settings", str(settings_path)],
            cwd=str(self.root),
            env=env,
            capture_output=True,
            text=True,
        )
        self.assertEqual(res.returncode, 0)

        self.assertTrue(record_path.is_file())
        recs = [json.loads(line) for line in record_path.read_text(encoding="utf-8").splitlines() if line.strip()]
        self.assertEqual(len(recs), 1)
        self.assertEqual(recs[0]["blocked"], [])
        self.assertFalse((run_dir / "guard.jsonl").exists())

    def test_fake_claude_hook_simulation(self):
        from brain.guard import write_hook_settings

        fake_claude = (repo_root() / "tests" / "fakes" / "fake_claude.py").resolve()
        run_dir = self.root / "fake_run"
        run_dir.mkdir(parents=True, exist_ok=True)

        settings_path = write_hook_settings(
            run_dir=run_dir,
            root=self.root,
            read_allow=[self.app_py, self.plan_md],
            write_allow=[self.app_py, self.plan_md],
        )

        scenario_path = self.root / "fake_scenario.json"
        scenario = {
            "ops": [
                {"op": "read", "path": str(self.app_py)},
                {"op": "read", "path": str(self.other_py)},
            ]
        }
        scenario_path.write_text(json.dumps(scenario), encoding="utf-8")

        record_path = self.root / "fake_record.jsonl"
        env = dict(os.environ)
        env["FAKE_CLAUDE_SCENARIO"] = str(scenario_path)
        env["FAKE_CLAUDE_RECORD"] = str(record_path)

        res = subprocess.run(
            [sys.executable, str(fake_claude), "--settings", str(settings_path)],
            cwd=str(self.root),
            env=env,
            capture_output=True,
            text=True,
        )
        self.assertEqual(res.returncode, 0)

        # Check records
        self.assertTrue(record_path.is_file())
        recs = [json.loads(line) for line in record_path.read_text(encoding="utf-8").splitlines() if line.strip()]
        self.assertEqual(len(recs), 1)
        rec = recs[0]
        self.assertEqual(rec["settings"], str(settings_path))
        self.assertEqual(len(rec["blocked"]), 1)
        self.assertEqual(rec["blocked"][0]["tool"], "Read")
        self.assertIn("other.py", rec["blocked"][0]["path"])

        # Check guard.jsonl
        guard_log = run_dir / "guard.jsonl"
        self.assertTrue(guard_log.is_file())
        log_lines = [json.loads(line) for line in guard_log.read_text(encoding="utf-8").splitlines() if line.strip()]
        self.assertEqual(len(log_lines), 2)
        self.assertEqual(log_lines[0]["decision"], "allow")
        self.assertEqual(log_lines[1]["decision"], "block")


if __name__ == "__main__":
    unittest.main()
