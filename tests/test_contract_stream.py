from __future__ import annotations

import json
from pathlib import Path
import unittest

from brain.streamlog import summarize

REPO_ROOT = Path(__file__).resolve().parent.parent
STREAM_CONTRACT_PATH = REPO_ROOT / "tests" / "contract" / "stream.json"


class TestStreamContract(unittest.TestCase):
    def _load_contract(self) -> dict:
        with open(STREAM_CONTRACT_PATH, "r", encoding="utf-8") as f:
            return json.load(f)

    def test_stream_contract_cases(self):
        data = self._load_contract()
        self.assertEqual(data.get("version"), 1)
        cases = data.get("cases", [])
        self.assertTrue(len(cases) > 0)

        for case in cases:
            cid = case["id"]
            with self.subTest(id=cid):
                file_path = REPO_ROOT / case["file"]
                plan_files = case.get("plan_files")
                root = case.get("root")
                summary = summarize(file_path, plan_files=plan_files, root=root)
                self.assertEqual(summary, case["expect"])

    def test_stream_fixture_covers_all_samples(self):
        data = self._load_contract()
        cases = data.get("cases", [])
        case_files = {case["file"] for case in cases}

        expected_files = {
            "spikes/s1/raw/s1.jsonl",
            "spikes/s4/raw/s4-r1.jsonl",
            "spikes/s4/raw/s4-r2.jsonl",
            "spikes/s4/raw/s4-r3.jsonl",
        }
        for p in (REPO_ROOT / "tests" / "fixtures" / "stream").glob("*.jsonl"):
            expected_files.add(p.relative_to(REPO_ROOT).as_posix())

        self.assertEqual(case_files, expected_files)

    def test_stream_expect_rust_shape(self):
        data = self._load_contract()
        token_keys = (
            "input_tokens",
            "output_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
        )

        for case in data.get("cases", []):
            cid = case["id"]
            with self.subTest(id=cid):
                self.assertIn("expect_rust", case)
                er = case["expect_rust"]
                self.assertIn("usage_source", er)
                self.assertIn(er["usage_source"], {"result", "partial", "none"})
                self.assertIn("usage", er)
                usage = er["usage"]
                for k in token_keys:
                    self.assertIn(k, usage)
                    self.assertIsInstance(usage[k], int)

                if case["expect"].get("result") is not None:
                    self.assertEqual(er["usage_source"], "result")
                    self.assertEqual(er["usage"], case["expect"]["usage"])


if __name__ == "__main__":
    unittest.main()
