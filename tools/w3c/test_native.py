"""Check that native standards failures cannot be hidden by compatibility results."""

import copy
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from w3c import native


class NativeGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.catalog = [
            {"ID": kind, "TYPE": kind, "path": f"{kind}.xml"}
            for kind in ("valid", "invalid", "not-wf", "error")
        ]
        self.sources = {case["path"]: "hash" for case in self.catalog}
        self.pin = {
            "source_digest": native.digest(self.sources),
            "catalog_digest": native.digest(self.catalog),
            "catalog_descriptors": 4,
            "selected_descriptors": 4,
            "chunks": [1, 7, 4096],
            "inventory_digest": native.digest(
                [(case["ID"], chunk) for case in self.catalog for chunk in (1, 7, 4096)]
            ),
            "expectation_corrections": {},
        }
        self.expected = native.selected_rows(self.catalog, self.sources, self.pin)
        self.rows: list[dict[str, Any]] = [
            {
                **case,
                "result": {
                    "status": int(case["expected"] == "accept"),
                    "name_rules": "FifthEdition",
                    "error": None if case["expected"] == "accept" else "InvalidToken",
                    "index": 0,
                    "loaded": [{"path": case["path"], "sha256": "hash"}],
                    "children": [],
                    "resolver_errors": [],
                },
            }
            for case in self.expected
        ]

    def evaluate(self) -> dict:
        return native.evaluate(self.rows, self.expected, self.sources)

    def test_nonvalidating_expectations_and_optional_observations(self) -> None:
        report = self.evaluate()
        self.assertTrue(report["passed"])
        self.assertEqual(report["mandatory_pass"], 9)
        self.assertEqual(report["optional_observations"], 3)
        self.rows[-1]["result"].update(status=1, error=None)
        self.assertTrue(self.evaluate()["passed"])
        for index in (0, 3, 6):
            with self.subTest(type=self.rows[index]["type"]):
                previous = copy.deepcopy(self.rows[index]["result"])
                accepted = not previous["status"]
                self.rows[index]["result"].update(
                    status=int(accepted), error=None if accepted else "InvalidToken"
                )
                report = self.evaluate()
                self.assertFalse(report["passed"])
                self.assertEqual(report["mandatory_fail"], 1)
                self.rows[index]["result"] = previous

    def test_incomplete_duplicate_or_relabelled_runs_fail(self) -> None:
        for mutation in (
            "missing",
            "duplicate",
            "namespaces",
            "path",
            "type",
            "expected",
            "profile",
        ):
            rows = copy.deepcopy(self.rows)
            if mutation == "missing":
                rows.pop()
            elif mutation == "duplicate":
                rows.append(rows[0])
            elif mutation == "profile":
                rows[0]["result"]["name_rules"] = "FourthEdition"
            else:
                rows[0][mutation] = "changed"
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                native.evaluate(rows, self.expected, self.sources)

    def test_resource_and_resolver_failures_never_count_as_rejections(self) -> None:
        for kind in ("LimitExceeded", "NoMemory", "resolver", "child"):
            rows = copy.deepcopy(self.rows)
            result = rows[6]["result"]  # A required rejection.
            if kind == "resolver":
                result["resolver_errors"] = ["missing file"]
            elif kind == "child":
                result["children"] = [
                    {"path": rows[6]["path"], "status": 0, "error": "LimitExceeded"}
                ]
            else:
                result["error"] = kind
            with self.subTest(kind=kind):
                report = native.evaluate(rows, self.expected, self.sources)
                self.assertFalse(report["passed"])
                self.assertEqual(len(report["inconclusive"]), 1)
                self.assertEqual(report["mandatory_fail"], 0)

    def test_changed_corpus_selection_and_loaded_bytes_fail(self) -> None:
        for field in (
            "source_digest",
            "catalog_digest",
            "inventory_digest",
            "selected_descriptors",
        ):
            pin = {**self.pin, field: "changed"}
            with self.subTest(field=field), self.assertRaises(ValueError):
                native.selected_rows(self.catalog, self.sources, pin)
        for loaded in (
            [],
            [{"path": "wrong.xml", "sha256": "hash"}],
            [{"path": "valid.xml", "sha256": "changed"}],
        ):
            rows = copy.deepcopy(self.rows)
            rows[0]["result"]["loaded"] = loaded
            with self.subTest(loaded=loaded), self.assertRaises(ValueError):
                native.evaluate(rows, self.expected, self.sources)

    def test_accepted_parent_cannot_hide_failed_child(self) -> None:
        self.rows[0]["result"]["children"] = [
            {"path": "valid.xml", "status": 0, "error": "InvalidToken"}
        ]
        with self.assertRaises(ValueError):
            self.evaluate()

    def test_standard_correction_requires_acceptance_and_retains_original_failure(
        self,
    ) -> None:
        self.pin["expectation_corrections"] = {
            "not-wf": {"catalog_type": "not-wf", "expected": "accept"}
        }
        self.expected = native.selected_rows(self.catalog, self.sources, self.pin)
        self.rows = [
            {**expected, "result": row["result"]}
            for row, expected in zip(self.rows, self.expected, strict=True)
        ]
        self.assertEqual(self.evaluate()["mandatory_fail"], 3)
        for row in self.rows[6:9]:
            row["result"].update(status=1, error=None)
        report = self.evaluate()
        self.assertTrue(report["passed"])
        self.assertEqual(len(report["raw_catalog_mismatches"]), 3)
        self.assertTrue(
            all(row["type"] == "not-wf" for row in report["raw_catalog_mismatches"])
        )


class WorkerTests(unittest.TestCase):
    def test_resolver_failure_is_retained_even_if_adapter_omits_it(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            suite = Path(directory)
            root = suite / "root.xml"
            root.write_text("<r/>")
            native.corpus.SUITE = suite
            engine = object.__new__(native.NativeEngine)
            engine.suite = suite
            engine.process = Mock(
                stdin=io.StringIO(),
                stdout=io.StringIO(
                    "\n".join(
                        json.dumps(message)
                        for message in [
                            {
                                "op": "resolve",
                                "base": root.as_uri(),
                                "system": "missing.ent",
                            },
                            {
                                "op": "result",
                                "status": 0,
                                "children": [],
                                "resolver_errors": [],
                            },
                        ]
                    )
                    + "\n"
                ),
            )
            result = engine.parse({"path": "root.xml", "chunk": 1, "namespaces": True})
            self.assertEqual(len(result["resolver_errors"]), 1)
            self.assertIn("FileNotFoundError", result["resolver_errors"][0])

    def test_runner_crashes_fail_the_gate_and_preserve_partial_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = root / "suite"
            suite.mkdir()
            (suite / "xmlconf.xml").write_text(
                '<TESTSUITE><TEST ID="one" TYPE="valid" URI="one.xml"/></TESTSUITE>'
            )
            (suite / "one.xml").write_text("<r/>")
            native.corpus.SUITE = suite
            catalog = native.corpus.selected_catalog()
            pin = root / "pin.json"
            native.save(
                pin,
                {
                    "source_digest": native.digest(native.source_hashes(suite)),
                    "catalog_digest": native.digest(catalog),
                    "catalog_descriptors": 1,
                    "selected_descriptors": 1,
                    "chunks": [1],
                    "inventory_digest": native.digest([("one", 1)]),
                    "expectation_corrections": {},
                },
            )
            runner = root / "crashing-parser"
            runner.write_text("#!/bin/sh\nexit 7\n")
            runner.chmod(0o755)
            output = root / "results"
            with (
                patch.object(native, "PIN", pin),
                patch.object(
                    sys,
                    "argv",
                    [
                        "native.py",
                        "--suite",
                        str(suite),
                        "--runner",
                        str(runner),
                        "--output",
                        str(output),
                    ],
                ),
                patch("builtins.print"),
            ):
                self.assertEqual(native.main(), 1)
            report = json.loads((output / "summary.json").read_text())
            self.assertFalse(report["passed"])
            self.assertNotEqual(report["worker_returncode"], 0)
            self.assertTrue((output / "worker.log").read_text())
            self.assertEqual(
                json.loads((output / "progress.json").read_text())["id"], "one"
            )

    def test_resolver_rejects_escape_and_bounds_reads(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            suite = root / "suite"
            suite.mkdir()
            (root / "outside.xml").write_text("<r/>")
            (suite / "link.xml").symlink_to(root / "outside.xml")
            native.corpus.SUITE = suite
            for value in ("../outside.xml", "link.xml", "https://example.org/entity"):
                with self.subTest(value=value), self.assertRaises(ValueError):
                    native.corpus.resolve((suite / "root.xml").as_uri(), value)
            large = suite / "large.xml"
            large.write_bytes(b"x" * 9)
            # No subprocess is needed to exercise the Python-owned file resolver.
            engine = object.__new__(native.NativeEngine)
            engine.suite = suite
            loaded: list[dict] = []
            with patch.object(native, "FILE_BYTES", 8), self.assertRaises(ValueError):
                engine.read_file(large, loaded)
            self.assertEqual(loaded, [])


if __name__ == "__main__":
    unittest.main()
