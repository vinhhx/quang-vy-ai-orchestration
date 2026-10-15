from __future__ import annotations

import json
from pathlib import Path
import unittest

from brain.config import repo_root
from brain.routing import normalize, slugify, resolve, RoutingError
from tests.helpers import Sandbox


class TestRouting(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        contract_dir = repo_root() / "tests" / "contract"
        with open(contract_dir / "normalize.json", "r", encoding="utf-8") as f:
            cls.normalize_fixture = json.load(f)
        with open(contract_dir / "routing.json", "r", encoding="utf-8") as f:
            cls.routing_fixture = json.load(f)

    def test_p1_01_vietnamese_name_matches(self) -> None:
        projects = self.routing_fixture["projects"]
        res = resolve("web bán hàng", projects)
        self.assertEqual(res["id"], "web-ban-hang")

    def test_p1_02_alias_matches(self) -> None:
        projects = self.routing_fixture["projects"]
        res1 = resolve("shop", projects)
        self.assertEqual(res1["id"], "web-ban-hang")
        res2 = resolve("Bán Hàng", projects)
        self.assertEqual(res2["id"], "web-ban-hang")

    def test_p1_03_ambiguous_stops_without_agent(self) -> None:
        projects = self.routing_fixture["projects"]
        with self.assertRaises(RoutingError) as cm:
            resolve("web", projects)
        self.assertEqual(cm.exception.kind, "ambiguous")
        self.assertEqual([c["id"] for c in cm.exception.candidates], ["web-ban-hang", "web-admin"])

        # CLI level check in Sandbox
        sb = Sandbox()
        try:
            sb.write_map(projects)
            res = sb.run_brain("lead", "web", "thêm tính năng")
            self.assertEqual(res.returncode, 2)
            self.assertIn("web-ban-hang", res.stdout)
            self.assertIn("web-admin", res.stdout)
            self.assertEqual(sb.records(), [])
        finally:
            sb.cleanup()

    def test_p1_04_no_match_lists_all_without_agent(self) -> None:
        projects = self.routing_fixture["projects"]
        with self.assertRaises(RoutingError) as cm:
            resolve("khong ton tai", projects)
        self.assertEqual(cm.exception.kind, "none")
        self.assertEqual(
            [c["id"] for c in cm.exception.candidates],
            ["web-ban-hang", "web-admin", "admin", "da-nang-tour"],
        )

        # CLI level check in Sandbox
        sb = Sandbox()
        try:
            sb.write_map(projects)
            res = sb.run_brain("lead", "khong ton tai", "thêm tính năng")
            self.assertEqual(res.returncode, 2)
            for p in projects:
                self.assertIn(p["id"], res.stdout)
            self.assertEqual(sb.records(), [])
        finally:
            sb.cleanup()

    def test_contract_cases(self) -> None:
        for case in self.normalize_fixture["cases"]:
            with self.subTest(normalize_case=case["id"]):
                self.assertEqual(normalize(case["input"]), case["normalized"])
                self.assertEqual(slugify(case["input"]), case["slug"])

        projects = self.routing_fixture["projects"]
        for case in self.routing_fixture["cases"]:
            with self.subTest(routing_case=case["id"]):
                expect = case["expect"]
                if expect["kind"] == "match":
                    matched = resolve(case["query"], projects)
                    self.assertEqual(matched["id"], expect["project"])
                else:
                    with self.assertRaises(RoutingError) as cm:
                        resolve(case["query"], projects)
                    self.assertEqual(cm.exception.kind, expect["kind"])
                    self.assertEqual(
                        [c["id"] for c in cm.exception.candidates],
                        expect["candidates"],
                    )


if __name__ == "__main__":
    unittest.main()
