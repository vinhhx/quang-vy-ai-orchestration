from __future__ import annotations

import contextlib
import io
from pathlib import Path
import tempfile
import unittest
import unittest.mock

import brain.cli
from brain import BrainError
from brain.projects import load_map
from tests.helpers import Sandbox


class TestRobustness(unittest.TestCase):
    def setUp(self) -> None:
        self.sandbox = Sandbox()
        self.addCleanup(self.sandbox.cleanup)
        self.map_file = self.sandbox.xdg_config / "agent-brain" / "projects_map.json"
        self.map_file.parent.mkdir(parents=True, exist_ok=True)

    def test_corrupt_map_refused_and_left_unchanged(self) -> None:
        corrupt_bytes = b'{"projects": [ {"id": "a",'
        self.map_file.write_bytes(corrupt_bytes)
        res = self.sandbox.run_brain("lead", "x", "y", "--dry-run")
        self.assertEqual(res.returncode, 1)
        self.assertIn("projects_map.json", res.stderr)
        self.assertIn("không ghi đè", res.stderr)
        self.assertNotIn("(trống)", res.stdout)
        self.assertEqual(self.map_file.read_bytes(), corrupt_bytes)

    def test_non_object_map_refused(self) -> None:
        self.map_file.write_text("[]", encoding="utf-8")
        res = self.sandbox.run_brain("lead", "x", "y", "--dry-run")
        self.assertEqual(res.returncode, 1)
        self.assertIn("JSON object", res.stderr)

    def test_projects_not_list_refused(self) -> None:
        self.map_file.write_text('{"scan_roots": [], "projects": {}}', encoding="utf-8")
        res = self.sandbox.run_brain("lead", "x", "y", "--dry-run")
        self.assertEqual(res.returncode, 1)

    def test_missing_map_is_empty(self) -> None:
        if self.map_file.exists():
            self.map_file.unlink()
        res = self.sandbox.run_brain("lead", "x", "y", "--dry-run")
        self.assertEqual(res.returncode, 2)
        self.assertIn("(trống) — chạy: brain scan --root <thư mục>", res.stdout)

    def test_load_map_direct(self) -> None:
        with tempfile.NamedTemporaryFile("w", delete=False, encoding="utf-8") as f:
            f.write("{invalid json")
            temp_path = Path(f.name)
        try:
            with self.assertRaises(BrainError) as ctx:
                load_map(temp_path)
            self.assertEqual(ctx.exception.code, 1)
        finally:
            temp_path.unlink(missing_ok=True)

        non_existent = self.sandbox.root / "no_such_file_123.json"
        self.assertEqual(load_map(non_existent), {"scan_roots": [], "projects": []})

    def test_missing_handler_module_message(self) -> None:
        with unittest.mock.patch.dict(brain.cli.HANDLERS, {"scan": "brain.no_such_module_xyz:cmd"}):
            buf = io.StringIO()
            with contextlib.redirect_stderr(buf):
                ret = brain.cli.main(["scan"])
            self.assertEqual(ret, 1)
            self.assertIn("chưa có trong bản cài này", buf.getvalue())

    def test_missing_handler_function_message(self) -> None:
        with unittest.mock.patch.dict(brain.cli.HANDLERS, {"scan": "brain.config:no_such_function_xyz"}):
            buf = io.StringIO()
            with contextlib.redirect_stderr(buf):
                ret = brain.cli.main(["scan"])
            self.assertEqual(ret, 1)
            self.assertIn("chưa có trong bản cài này", buf.getvalue())

    def test_broken_handler_import_not_hidden(self) -> None:
        with unittest.mock.patch.dict(brain.cli.HANDLERS, {"scan": "tests.fakes.broken_handler:cmd_broken"}):
            with self.assertRaises(ModuleNotFoundError) as ctx:
                brain.cli.main(["scan"])
            self.assertEqual(ctx.exception.name, "brain_nonexistent_dependency_for_tests")


if __name__ == "__main__":
    unittest.main()
