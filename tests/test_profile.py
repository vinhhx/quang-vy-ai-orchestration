from __future__ import annotations

import json
from pathlib import Path
import unittest

from brain import BrainError
from brain.config import build_env, check_profile, load_config, repo_root
from tests.helpers import Sandbox, lead_scenario


class TestProfile(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        contract_dir = repo_root() / "tests" / "contract"
        with open(contract_dir / "profile_env.json", "r", encoding="utf-8") as f:
            cls.profile_fixture = json.load(f)
        cls.cfg = load_config()

    def test_p1_10_work_profile_env(self) -> None:
        case = next(c for c in self.profile_fixture["cases"] if c["id"] == "P1-10")
        env = build_env("work", self.cfg, case["base_env"])
        for k in case["expect"]["absent"]:
            self.assertNotIn(k, env)
        for k, v in case["expect"]["equals"].items():
            self.assertEqual(env.get(k), v)

        # CLI run: sandbox exports CLAUDE_CONFIG_DIR through extra_env
        sb = Sandbox()
        try:
            repo = sb.make_repo("proj-work")
            entry = sb.project_entry(repo, "proj-work", profile="work")
            sb.write_map([entry])
            scenario = lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])
            res = sb.run_brain(
                "lead",
                "proj-work",
                "sửa hàm app",
                scenario=scenario,
                extra_env={"CLAUDE_CONFIG_DIR": "/tmp/other_inherited_config"},
            )
            self.assertEqual(res.returncode, 0, f"Stderr: {res.stderr}")
            recs = sb.records()
            self.assertEqual(len(recs), 1)
            self.assertIsNone(recs[0]["claude_config_dir"])
        finally:
            sb.cleanup()

    def test_p1_10_personal_profile_env(self) -> None:
        case = next(c for c in self.profile_fixture["cases"] if c["id"] == "P1-10-personal")
        env = build_env("personal", self.cfg, case["base_env"])
        for k in case["expect"]["absent"]:
            self.assertNotIn(k, env)
        for k, v in case["expect"]["equals"].items():
            self.assertEqual(env.get(k), v)

        # CLI run: personal project reaches claude with configured personal dir
        sb = Sandbox()
        try:
            repo = sb.make_repo("proj-personal")
            entry = sb.project_entry(repo, "proj-personal", profile="personal")
            sb.write_map([entry])
            scenario = lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])
            res = sb.run_brain(
                "lead",
                "proj-personal",
                "sửa hàm app",
                scenario=scenario,
            )
            self.assertEqual(res.returncode, 0, f"Stderr: {res.stderr}")
            recs = sb.records()
            self.assertEqual(len(recs), 1)
            self.assertEqual(recs[0]["claude_config_dir"], str(sb.personal_dir))
        finally:
            sb.cleanup()

    def test_d07_empty_profile_refused_without_agent(self) -> None:
        sb = Sandbox()
        try:
            repo = sb.make_repo("proj-empty-profile")
            entry = sb.project_entry(repo, "proj-empty-profile", profile="")
            sb.write_map([entry])
            scenario = lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])
            res = sb.run_brain(
                "lead",
                "proj-empty-profile",
                "sửa hàm app",
                scenario=scenario,
            )
            self.assertEqual(res.returncode, 1)
            self.assertIn("profile", res.stderr.lower())
            self.assertEqual(sb.records(), [])
        finally:
            sb.cleanup()

    def test_contract_cases(self) -> None:
        cfg = {"profiles": self.profile_fixture["profiles"]}
        for case in self.profile_fixture["cases"]:
            with self.subTest(case_id=case["id"]):
                expect = case["expect"]
                if expect.get("error") == "profile":
                    with self.assertRaises(BrainError):
                        check_profile(case["profile"], cfg)
                else:
                    env = build_env(case["profile"], cfg, case["base_env"])
                    for k in expect.get("absent", []):
                        self.assertNotIn(k, env)
                    for k, v in expect.get("equals", {}).items():
                        self.assertEqual(env.get(k), v)


    def test_build_env_maintains_project_working_dir(self) -> None:
        for profile in ("work", "personal"):
            env = build_env(profile, self.cfg, {"CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR": "0"})
            self.assertEqual(env.get("CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR"), "1")


if __name__ == "__main__":
    unittest.main()
