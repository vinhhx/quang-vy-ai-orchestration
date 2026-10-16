#!/usr/bin/env python3
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest


def _find_child_pid(ppid: int) -> int | None:
    try:
        entries = os.listdir("/proc")
    except Exception:
        return None
    for entry in entries:
        if not entry.isdigit():
            continue
        pid = int(entry)
        stat_path = f"/proc/{pid}/stat"
        try:
            with open(stat_path, "r", encoding="utf-8") as f:
                content = f.read()
            rp = content.rfind(")")
            if rp != -1:
                fields = content[rp + 2 :].split()
                if len(fields) > 1 and int(fields[1]) == ppid:
                    return pid
        except (FileNotFoundError, ProcessLookupError, PermissionError, ValueError):
            continue
    return None


def _is_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except (ProcessLookupError, PermissionError):
        return False


class TestFakeClaudeTree(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.pidfile = Path(self.temp_dir.name) / "tree.pid"
        self.fake_script = (
            Path(__file__).resolve().parent / "fakes" / "fake_claude_tree.py"
        )
        self.procs_to_clean: list[int] = []
        self.groups_to_clean: list[int] = []

    def tearDown(self):
        for pgid in self.groups_to_clean:
            try:
                os.killpg(pgid, signal.SIGKILL)
            except Exception:
                pass
        for pid in self.procs_to_clean:
            try:
                os.kill(pid, signal.SIGKILL)
            except Exception:
                pass
        self.temp_dir.cleanup()

    def test_fake_tree_coop(self):
        env = dict(os.environ)
        env["FAKE_TREE_PIDFILE"] = str(self.pidfile)
        env["FAKE_TREE_MODE"] = "coop"
        env["FAKE_TREE_SLEEP"] = "317"

        proc = subprocess.Popen(
            [sys.executable, str(self.fake_script)],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
        )
        self.procs_to_clean.append(proc.pid)

        # Poll up to 5s for the pidfile
        start_wait = time.time()
        fake_pid, bash_pid = None, None
        while time.time() - start_wait < 5.0:
            if self.pidfile.is_file():
                try:
                    content = self.pidfile.read_text(encoding="utf-8").strip()
                    parts = content.split()
                    if len(parts) >= 2:
                        fake_pid = int(parts[0])
                        bash_pid = int(parts[1])
                        break
                except Exception:
                    pass
            time.sleep(0.05)

        self.assertIsNotNone(fake_pid, "pidfile was not written in time")
        self.assertEqual(fake_pid, proc.pid)
        self.assertIsNotNone(bash_pid)
        self.groups_to_clean.append(bash_pid)

        # Check sessions
        fake_sid = os.getsid(fake_pid)
        bash_sid = os.getsid(bash_pid)
        self.assertEqual(bash_sid, bash_pid)
        self.assertNotEqual(fake_sid, bash_sid)

        # Check sleep child
        sleep_pid = None
        start_sleep_wait = time.time()
        while time.time() - start_sleep_wait < 3.0:
            sleep_pid = _find_child_pid(bash_pid)
            if sleep_pid is not None:
                break
            time.sleep(0.05)

        self.assertIsNotNone(sleep_pid, "sleep child not found in /proc")
        self.procs_to_clean.append(sleep_pid)

        # First stdout line is init
        init_line = proc.stdout.readline()
        init_json = json.loads(init_line)
        self.assertEqual(init_json.get("type"), "system")
        self.assertEqual(init_json.get("subtype"), "init")

        # Send SIGTERM to fake
        os.kill(proc.pid, signal.SIGTERM)

        # Wait for fake to exit with 143
        exit_code = proc.wait(timeout=5.0)
        self.assertEqual(exit_code, 143)

        # Stdout includes a "type": "result" line
        rest_stdout = proc.stdout.read()
        has_result = False
        for line in rest_stdout.splitlines():
            try:
                data = json.loads(line)
                if data.get("type") == "result":
                    has_result = True
                    break
            except Exception:
                pass
        self.assertTrue(has_result, "expected a result line in stdout after SIGTERM")

        # Bash and sleep are gone within 3 s
        gone_wait = time.time()
        while time.time() - gone_wait < 3.0:
            if not _is_alive(bash_pid) and not _is_alive(sleep_pid):
                break
            time.sleep(0.05)

        self.assertFalse(_is_alive(bash_pid), "bash pid still alive after 3s")
        self.assertFalse(_is_alive(sleep_pid), "sleep pid still alive after 3s")

    def test_fake_tree_ignore(self):
        env = dict(os.environ)
        env["FAKE_TREE_PIDFILE"] = str(self.pidfile)
        env["FAKE_TREE_MODE"] = "ignore"
        env["FAKE_TREE_SLEEP"] = "317"

        proc = subprocess.Popen(
            [sys.executable, str(self.fake_script)],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
        )
        self.procs_to_clean.append(proc.pid)

        # Poll up to 5s for the pidfile
        start_wait = time.time()
        fake_pid, bash_pid = None, None
        while time.time() - start_wait < 5.0:
            if self.pidfile.is_file():
                try:
                    content = self.pidfile.read_text(encoding="utf-8").strip()
                    parts = content.split()
                    if len(parts) >= 2:
                        fake_pid = int(parts[0])
                        bash_pid = int(parts[1])
                        break
                except Exception:
                    pass
            time.sleep(0.05)

        self.assertIsNotNone(fake_pid, "pidfile was not written in time")
        self.assertEqual(fake_pid, proc.pid)
        self.assertIsNotNone(bash_pid)
        self.groups_to_clean.append(bash_pid)

        # Check sleep child
        sleep_pid = None
        start_sleep_wait = time.time()
        while time.time() - start_sleep_wait < 3.0:
            sleep_pid = _find_child_pid(bash_pid)
            if sleep_pid is not None:
                break
            time.sleep(0.05)

        self.assertIsNotNone(sleep_pid, "sleep child not found in /proc")
        self.procs_to_clean.append(sleep_pid)

        # Send SIGTERM to fake
        os.kill(proc.pid, signal.SIGTERM)

        # SIGTERM leaves the fake alive after 1 s
        time.sleep(1.0)
        self.assertIsNone(proc.poll(), "fake should ignore SIGTERM and stay alive")

        # Cleanup: SIGKILL fake and bash group
        try:
            os.kill(proc.pid, signal.SIGKILL)
        except Exception:
            pass
        try:
            os.killpg(bash_pid, signal.SIGKILL)
        except Exception:
            pass

        # Wait until all three are gone
        gone_wait = time.time()
        while time.time() - gone_wait < 3.0:
            if not _is_alive(fake_pid) and not _is_alive(bash_pid) and not _is_alive(sleep_pid):
                break
            time.sleep(0.05)

        self.assertFalse(_is_alive(fake_pid))
        self.assertFalse(_is_alive(bash_pid))
        self.assertFalse(_is_alive(sleep_pid))


if __name__ == "__main__":
    unittest.main()
