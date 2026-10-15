from __future__ import annotations

import unittest

from brain.config import repo_root
from tests.helpers import Sandbox, lead_scenario


class TestPipelineE2E(unittest.TestCase):
    def setUp(self):
        self.sandbox = Sandbox()
        self.repo = self.sandbox.make_repo("web-ban-hang")
        self.entry = self.sandbox.project_entry(self.repo, id="web-ban-hang", profile="work")
        self.sandbox.write_map([self.entry])

    def tearDown(self):
        self.sandbox.cleanup()

    def test_role_05_pipeline_lead_dev_qa(self):
        task = "sua-app"

        # 1. Lead
        sc_lead = lead_scenario([("src/app.py", "app()", "trả về 2", "demo")])
        res_lead = self.sandbox.run_brain("lead", "web-ban-hang", "sửa app", scenario=sc_lead)
        self.assertEqual(res_lead.returncode, 0, f"Lead failed: {res_lead.stderr}\n{res_lead.stdout}")

        # 2. Dev
        sc_dev = {
            "ops": [
                {"op": "read", "path": ".plan.md"},
                {"op": "read", "path": "src/app.py"},
                {"op": "write", "path": "src/app.py", "content": "def app():\n    return 2\n"},
                {"op": "git", "args": ["add", "src/app.py"]},
                {"op": "git", "args": ["commit", "-m", f"{task}: sửa app"]},
            ]
        }
        res_dev = self.sandbox.run_brain("dev", "web-ban-hang", scenario=sc_dev)
        self.assertEqual(res_dev.returncode, 0, f"Dev failed: {res_dev.stderr}\n{res_dev.stdout}")

        # 3. QA
        qa_report_content = (
            "# QA: sửa app\n"
            "## Thay đổi có khớp plan không (từng mục)\n"
            "- src/app.py: khớp.\n"
            "## Sửa ngoài plan (nếu có)\n"
            "Không.\n"
            "## Rủi ro\n"
            "Không có.\n"
            "## Kết luận\n"
            "Đạt\n"
        )
        sc_qa = {
            "ops": [
                {
                    "op": "write",
                    "path": ".qa-report.md",
                    "content": qa_report_content,
                    "keep_meta": True,
                }
            ]
        }
        res_qa = self.sandbox.run_brain("qa", "web-ban-hang", scenario=sc_qa)
        self.assertEqual(res_qa.returncode, 0, f"QA failed: {res_qa.stderr}\n{res_qa.stdout}")

        # Verify commits in order on the task branch
        subjects = [
            line.strip()
            for line in self.sandbox.git(
                self.repo, "log", "--reverse", "--format=%s", "main..HEAD"
            ).splitlines()
            if line.strip()
        ]
        self.assertEqual(
            subjects,
            [f"{task}: plan", f"{task}: sửa app", f"{task}: qa"],
        )

        # Verify 3 records with models opus, sonnet, sonnet
        recs = self.sandbox.records()
        self.assertEqual(len(recs), 3)

        lead_argv = recs[0]["argv"]
        dev_argv = recs[1]["argv"]
        qa_argv = recs[2]["argv"]

        self.assertEqual(lead_argv[lead_argv.index("--model") + 1], "opus")
        self.assertEqual(dev_argv[dev_argv.index("--model") + 1], "sonnet")
        self.assertEqual(qa_argv[qa_argv.index("--model") + 1], "sonnet")

        # Verify system prompts start with expected prefixes
        lead_prompt = lead_argv[lead_argv.index("--append-system-prompt") + 1]
        dev_prompt = dev_argv[dev_argv.index("--append-system-prompt") + 1]
        qa_prompt = qa_argv[qa_argv.index("--append-system-prompt") + 1]

        self.assertTrue(lead_prompt.startswith("Bạn là Lead."))
        self.assertTrue(dev_prompt.startswith("Bạn là Dev."))
        self.assertTrue(qa_prompt.startswith("Bạn là QA."))

        # Verify 3 separate prompt files exist
        prompts_dir = repo_root() / "brain" / "prompts"
        self.assertTrue((prompts_dir / "lead.md").is_file())
        self.assertTrue((prompts_dir / "dev.md").is_file())
        self.assertTrue((prompts_dir / "qa.md").is_file())


if __name__ == "__main__":
    unittest.main()
