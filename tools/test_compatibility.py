"""Fail closed when compatibility results or known failure boundaries drift."""

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest.mock import patch

import compatibility
from compatibility import check_api, check_w3c, digest


class ApiGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.baseline = {
            "contexts": ["chunksize=0 deferral=0"],
            "tests": ["test_ok", "test_allocation"],
            "failures": {
                "test_allocation": {
                    "contexts": ["chunksize=0 deferral=0"],
                    "assertions": ["test_allocation at alloc_tests.c:10"],
                }
            },
        }
        self.result: dict[str, Any] = {
            "returncode": 1,
            "selection_complete": True,
            "library_origin_verified": True,
            "timed_out": False,
            "passed": 1,
            "failed": 1,
            "results": [
                {
                    "context": "chunksize=0 deferral=0",
                    "test": "test_ok",
                    "outcome": "pass",
                    "code": 0,
                },
                {
                    "context": "chunksize=0 deferral=0",
                    "test": "test_allocation",
                    "outcome": "fail",
                    "code": 100,
                },
            ],
        }
        self.log = (
            "XEME_BEGIN\tchunksize=0 deferral=0\ttest_ok\n"
            "XEME_BEGIN\tchunksize=0 deferral=0\ttest_allocation\n"
            "ASSERTION: test_allocation at /temporary/adapted/alloc_tests.c:10\n"
        )

    def test_known_failure_retains_original_result(self) -> None:
        report = check_api(self.result, self.log, self.baseline, False)
        self.assertTrue(report["passed"])
        self.assertEqual(report["known_failures"], 1)
        self.assertEqual(self.result["returncode"], 1)
        self.assertFalse(
            check_api(self.result, self.log, self.baseline, True)["passed"]
        )

    def test_same_method_new_assertion_is_rejected(self) -> None:
        log = self.log.replace("alloc_tests.c:10", "alloc_tests.c:20")
        self.assertFalse(check_api(self.result, log, self.baseline, False)["passed"])

    def test_timeout_and_incomplete_duplicate_or_relabelled_inventory(self) -> None:
        for change in (
            "missing",
            "duplicate",
            "renamed",
            "timeout",
            "origin",
            "counts",
        ):
            result = copy.deepcopy(self.result)
            if change == "missing":
                result["results"].pop()
            elif change == "duplicate":
                result["results"].append(result["results"][0])
            elif change == "renamed":
                result["results"][0]["test"] = "test_unexpected"
            elif change == "timeout":
                result["timed_out"] = True
            elif change == "origin":
                result["library_origin_verified"] = False
            else:
                result["passed"] = 2
            with self.subTest(change=change), self.assertRaises(ValueError):
                check_api(result, self.log, self.baseline, False)

    def test_improvement_is_reported_separately(self) -> None:
        self.result.update(returncode=0, failed=0, passed=2)
        self.result["results"][1].update(outcome="pass", code=0)
        report = check_api(
            self.result, self.log.split("ASSERTION:")[0], self.baseline, False
        )
        self.assertTrue(report["passed"])
        self.assertEqual(
            [row["test"] for row in report["improvements"]], ["test_allocation"]
        )

    def test_reference_limits_allow_large_upstream_inputs_without_relaxing_candidate(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            baseline = root / "baseline.json"
            baseline.write_text(json.dumps({"api": self.baseline}))
            libraries = {
                label: root / f"{label}.so" for label in ("xeme", "reference")
            }
            for library in libraries.values():
                library.write_bytes(b"fixture library")
            config = root / "expat_config.h"
            config.write_text("fixture config")

            def suite(
                command: list[str], output: Path, accepted: tuple[int, ...]
            ) -> None:
                reference = command[command.index("--library") + 1] == str(
                    libraries["reference"]
                )
                self.assertEqual(accepted, (0,) if reference else (0, 1))
                directory = Path(command[command.index("--output") + 1])
                directory.mkdir()
                result = copy.deepcopy(self.result)
                log = self.log
                if reference:
                    result.update(returncode=0, failed=0, passed=2)
                    result["results"][1].update(outcome="pass", code=0)
                    log = log.split("ASSERTION:")[0]
                (directory / "results.json").write_text(json.dumps(result))
                (directory / "tests.log").write_text(log)
                output.write_text("fixture run")

            with (
                patch.object(compatibility, "BASELINE", baseline),
                patch.object(compatibility, "run", side_effect=suite),
                patch.object(
                    sys,
                    "argv",
                    [
                        "compatibility.py",
                        "api",
                        "--source",
                        str(root),
                        "--config",
                        str(config),
                        "--library",
                        str(libraries["xeme"]),
                        "--reference",
                        str(libraries["reference"]),
                        "--output",
                        str(root / "report"),
                    ],
                ),
                patch("builtins.print"),
            ):
                self.assertEqual(compatibility.main(), 0)
            report = json.loads((root / "report/gate.json").read_text())
            candidate, reference = report["commands"]
            for option in ("--memory-mib", "--rss-mib", "--test-timeout"):
                self.assertNotIn(option, candidate)
            self.assertEqual(reference[reference.index("--memory-mib") + 1], "4096")
            self.assertEqual(reference[reference.index("--test-timeout") + 1], "30")
            self.assertNotIn("--rss-mib", reference)


class W3cGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.row: dict[str, Any] = {
            "id": "edition-name",
            "chunk": 1,
            "type": "valid",
            "path": "name.xml",
            "namespaces": True,
            "result": {
                "status": 0,
                "error": 4,
                "index": 1,
                "loaded": [{"path": "name.xml", "sha256": "same"}],
                "children": [],
                "resolver_errors": [],
            },
        }
        catalog = [{"ID": "edition-name", "TYPE": "valid", "path": "name.xml"}]
        self.save("catalog", catalog)
        self.baseline = {
            "catalog_digest": digest(catalog),
            "catalog_descriptors": 1,
            "selected_descriptors": 1,
            "chunks": [1],
            "source_digest": digest({"name.xml": "same"}),
            "inventory_digest": digest([("edition-name", 1)]),
            "catalog_failures": ["edition-name"],
        }
        self.summary: dict[str, Any] = {
            "worker_failures": {},
            "inconclusive": {"reference": [], "xeme": []},
            "catalog_descriptors": 1,
            "selected_descriptors": 1,
            "chunks": [1],
            "catalog_source_sha256": {"name.xml": "same"},
            "mismatches": {
                "reference": [copy.deepcopy(self.row)],
                "xeme": [self.row],
            },
        }
        self.save("summary", self.summary)
        for engine in ("reference", "xeme"):
            self.save(engine, {"rows": [self.row]})

    def save(self, name: str, data: Any) -> None:
        (self.directory / f"{name}.json").write_text(json.dumps(data))

    def test_known_catalog_failure_is_still_reported(self) -> None:
        report = check_w3c(self.directory, self.baseline)
        self.assertTrue(report["passed"])
        self.assertEqual(report["conformance_failures"], {"reference": 1, "xeme": 1})

    def test_acceptance_and_loaded_bytes_differ_but_positions_may_differ(self) -> None:
        self.row["result"].update(index=9, error=2)
        self.save("xeme", {"rows": [self.row]})
        self.save("summary", self.summary)
        self.assertTrue(check_w3c(self.directory, self.baseline)["passed"])
        for field, value in (
            ("status", 1),
            ("children", [{"path": "child", "status": 0}]),
        ):
            row = copy.deepcopy(self.row)
            row["result"][field] = value
            self.save("xeme", {"rows": [row]})
            summary = copy.deepcopy(self.summary)
            summary["mismatches"]["xeme"] = [] if field == "status" else [row]
            self.save("summary", summary)
            with self.subTest(field=field):
                self.assertFalse(check_w3c(self.directory, self.baseline)["passed"])

    def test_worker_cannot_change_requested_input_or_namespace_mode(self) -> None:
        for field, value in (
            ("namespaces", False),
            ("path", "another.xml"),
            ("type", "error"),
        ):
            row = copy.deepcopy(self.row)
            row[field] = value
            self.save("xeme", {"rows": [row]})
            with self.subTest(field=field), self.assertRaises(ValueError):
                check_w3c(self.directory, self.baseline)
        for loaded in ([], [{"path": "name.xml", "sha256": "wrong"}]):
            row = copy.deepcopy(self.row)
            row["result"]["loaded"] = loaded
            self.save("xeme", {"rows": [row]})
            with self.subTest(loaded=loaded), self.assertRaises(ValueError):
                check_w3c(self.directory, self.baseline)

    def test_conformance_summary_cannot_hide_raw_failures(self) -> None:
        self.summary["mismatches"]["xeme"] = []
        self.save("summary", self.summary)
        with self.assertRaises(ValueError):
            check_w3c(self.directory, self.baseline)

    def test_corpus_drift_duplicate_rows_and_resolver_failure_are_rejected(
        self,
    ) -> None:
        self.save("xeme", {"rows": [self.row, self.row]})
        with self.assertRaises(ValueError):
            check_w3c(self.directory, self.baseline)
        self.row["result"]["resolver_errors"] = ["file not found"]
        self.save("xeme", {"rows": [self.row]})
        with self.assertRaises(ValueError):
            check_w3c(self.directory, self.baseline)
        self.summary["catalog_source_sha256"]["name.xml"] = "different"
        self.save("summary", self.summary)
        with self.assertRaises(ValueError):
            check_w3c(self.directory, self.baseline)


if __name__ == "__main__":
    unittest.main()
