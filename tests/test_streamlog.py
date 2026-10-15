from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from brain.streamlog import EDIT_TOOLS, PROTOCOL_FILES, iter_events, summarize


class TestStreamlog(unittest.TestCase):
    def test_dev_two_reads_then_edit(self):
        fixture = Path("tests/fixtures/stream/dev_two_reads_then_edit.jsonl")
        s = summarize(fixture, plan_files=["src/app.py"], root="/work/proj")
        self.assertEqual(s["assistant_turns"], 4)
        self.assertEqual(s["turns_before_first_edit"], 2)
        self.assertEqual(s["first_edit_file"], "src/app.py")
        self.assertEqual(s["reads"], [".plan.md", "src/app.py"])
        self.assertEqual(s["reads_outside_plan"], [])
        self.assertEqual(s["edits"], ["src/app.py"])
        self.assertEqual(s["tool_counts"], {"Read": 2, "Edit": 1})
        self.assertEqual(
            s["usage"],
            {
                "input_tokens": 1200,
                "output_tokens": 300,
                "cache_read_input_tokens": 5000,
                "cache_creation_input_tokens": 800,
            },
        )
        self.assertEqual(s["permission_denials"], 1)
        self.assertEqual(s["session_id"], "s-1")
        self.assertTrue(s["format_ok"])

    def test_dev_reads_outside_plan_when_empty_plan_files(self):
        fixture = Path("tests/fixtures/stream/dev_two_reads_then_edit.jsonl")
        s = summarize(fixture, plan_files=[], root="/work/proj")
        self.assertEqual(s["reads_outside_plan"], ["src/app.py"])

    def test_parallel_reads(self):
        fixture = Path("tests/fixtures/stream/parallel_reads.jsonl")
        s = summarize(fixture, plan_files=["a", "b"], root=None)
        self.assertEqual(s["assistant_turns"], 2)
        self.assertEqual(s["turns_before_first_edit"], 1)
        self.assertEqual(s["edits"], ["a"])

    def test_dev_blocked(self):
        fixture = Path("tests/fixtures/stream/dev_blocked.jsonl")
        s = summarize(fixture, plan_files=[], root="/work/proj")
        self.assertIsNone(s["turns_before_first_edit"])
        self.assertEqual(s["edits"], [".plan.md"])

    def test_malformed_lines(self):
        fixture = Path("tests/fixtures/stream/malformed_lines.jsonl")
        s = summarize(fixture)
        self.assertEqual(s["bad_lines"], 3)
        self.assertTrue(s["format_ok"])

    def test_stream_no_result_line(self):
        with tempfile.NamedTemporaryFile("w+", encoding="utf-8", suffix=".jsonl") as f:
            f.write('{"type": "system", "subtype": "init", "session_id": "s-no-res"}\n')
            f.write('{"type": "assistant", "message": {"id": "m1", "content": []}}\n')
            f.flush()
            s = summarize(f.name)
        self.assertFalse(s["format_ok"])
        self.assertEqual(
            s["usage"],
            {
                "input_tokens": 0,
                "output_tokens": 0,
                "cache_read_input_tokens": 0,
                "cache_creation_input_tokens": 0,
            },
        )

    @unittest.skipUnless(
        Path("spikes/s1/raw/s1.jsonl").exists(),
        "A-02: Phase 1 S1 fixture not present yet",
    )
    def test_real_s1_fixture(self):
        s = summarize("spikes/s1/raw/s1.jsonl")
        self.assertTrue(s["format_ok"])
        self.assertTrue(bool(s["session_id"]))
        self.assertEqual(s["assistant_turns"], 2)


if __name__ == "__main__":
    unittest.main()
