from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unicodedata
import unittest

REPO_ROOT = Path(__file__).resolve().parent.parent
GUARD_CONTRACT_PATH = REPO_ROOT / "tests" / "contract" / "guard.json"

EXPECTED_IDS = [
    "G-read-allowed",
    "G-read-denied",
    "G-relative-path",
    "G-parent-traversal",
    "G-symlink-outside",
    "G-edit-allowed",
    "G-write-new-outside",
    "G-multiedit-allowed",
    "G-multiedit-blocked",
    "G-notebookedit-allowed",
    "G-notebookedit-blocked",
    "G-grep-blocked",
    "G-glob-blocked",
    "G-readfree-read-any",
    "G-readfree-grep",
    "G-readfree-write-outside",
    "G-bash-not-guarded",
    "G-malformed-stdin",
    "G-missing-policy",
    "G-write-missing-path",
    "G-nfd-path",
]


def _substitute(obj, root_str: str, outside_str: str):
    if isinstance(obj, str):
        return obj.replace("{root}", root_str).replace("{outside}", outside_str)
    if isinstance(obj, dict):
        return {k: _substitute(v, root_str, outside_str) for k, v in obj.items()}
    if isinstance(obj, list):
        return [_substitute(item, root_str, outside_str) for item in obj]
    return obj


class TestGuardContract(unittest.TestCase):
    def _load_contract(self) -> dict:
        with open(GUARD_CONTRACT_PATH, "r", encoding="utf-8") as f:
            return json.load(f)

    def test_guard_fixture_ids(self):
        data = self._load_contract()
        ids = [case["id"] for case in data.get("cases", [])]
        self.assertEqual(ids, EXPECTED_IDS)

    def test_guard_contract_cases(self):
        data = self._load_contract()
        layout = data.get("layout", {})
        default_policy = data.get("default_policy", {})

        for case in data.get("cases", []):
            cid = case["id"]
            with self.subTest(id=cid):
                with tempfile.TemporaryDirectory() as temp_dir_str:
                    temp_dir = Path(temp_dir_str).resolve()
                    root = temp_dir / "root"
                    outside = temp_dir / "outside"
                    root.mkdir(parents=True, exist_ok=True)
                    outside.mkdir(parents=True, exist_ok=True)

                    for rel_name, text in layout.get("files", {}).items():
                        target_file = root / rel_name
                        target_file.parent.mkdir(parents=True, exist_ok=True)
                        target_file.write_text(text, encoding="utf-8")

                    for rel_name, text in layout.get("outside_files", {}).items():
                        target_file = outside / rel_name
                        target_file.parent.mkdir(parents=True, exist_ok=True)
                        target_file.write_text(text, encoding="utf-8")

                    for link_name, target_tpl in layout.get("symlinks", {}).items():
                        target_str = target_tpl.replace("{root}", str(root)).replace(
                            "{outside}", str(outside)
                        )
                        link_file = root / link_name
                        link_file.parent.mkdir(parents=True, exist_ok=True)
                        os.symlink(target_str, link_file)

                    case_policy = (
                        case["policy"] if "policy" in case else default_policy
                    )

                    log_path = temp_dir / "guard.jsonl"

                    if "policy_path" in case:
                        policy_file = Path(
                            _substitute(case["policy_path"], str(root), str(outside))
                        )
                    else:
                        policy_file = temp_dir / "policy.json"
                        read_allow_raw = case_policy.get("read_allow")
                        if read_allow_raw is not None:
                            abs_read_allow = [
                                str(root / entry) for entry in read_allow_raw
                            ]
                        else:
                            abs_read_allow = None

                        write_allow_raw = case_policy.get("write_allow") or []
                        abs_write_allow = [
                            str(root / entry) for entry in write_allow_raw
                        ]

                        policy_data = {
                            "root": str(root),
                            "read_allow": abs_read_allow,
                            "write_allow": abs_write_allow,
                            "log": str(log_path),
                        }
                        policy_file.write_text(
                            json.dumps(policy_data, indent=2, ensure_ascii=False),
                            encoding="utf-8",
                        )

                    if "stdin_raw" in case:
                        stdin_data = _substitute(
                            case["stdin_raw"], str(root), str(outside)
                        )
                    else:
                        payload = _substitute(case["payload"], str(root), str(outside))
                        if case.get("normalize_payload_path") == "NFD":
                            tool_input = payload.get("tool_input", {})
                            if "file_path" in tool_input:
                                tool_input["file_path"] = unicodedata.normalize(
                                    "NFD", tool_input["file_path"]
                                )
                        stdin_data = json.dumps(payload)

                    cmd = [
                        sys.executable,
                        "-I",
                        "brain/guard.py",
                        "--policy",
                        str(policy_file),
                    ]
                    res = subprocess.run(
                        cmd,
                        input=stdin_data,
                        text=True,
                        capture_output=True,
                        cwd=REPO_ROOT,
                    )

                    expect = case["expect"]
                    self.assertEqual(res.returncode, expect["exit"])

                    for fragment in expect.get("stderr_contains", []):
                        self.assertIn(fragment, res.stderr)

                    if "decision" in expect and expect["decision"] is not None:
                        self.assertTrue(
                            log_path.is_file(),
                            f"Case {cid}: expected log file {log_path} to exist",
                        )
                        lines = [
                            line.strip()
                            for line in log_path.read_text(encoding="utf-8").splitlines()
                            if line.strip()
                        ]
                        self.assertTrue(
                            len(lines) > 0,
                            f"Case {cid}: expected log records in {log_path}",
                        )
                        last_record = json.loads(lines[-1])
                        self.assertEqual(last_record["decision"], expect["decision"])
                        if "tool" in expect and expect["tool"] is not None:
                            self.assertEqual(last_record["tool"], expect["tool"])


if __name__ == "__main__":
    unittest.main()
