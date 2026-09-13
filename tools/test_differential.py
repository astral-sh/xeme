"""Guard differential comparisons against incomplete or substituted workers."""

import base64
import copy
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest.mock import Mock, patch

import differential
from corpus import Case


class WorkerValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.library = self.root / "selected.so"
        self.library.write_bytes(b"selected library")
        self.library_hash = hashlib.sha256(self.library.read_bytes()).hexdigest()
        self.corpus: list[dict[str, Any]] = [
            {
                "name": "first",
                "chunk_size": 1,
                "namespaces": False,
                "sha256": "input-a",
            },
            {
                "name": "second",
                "chunk_size": 7,
                "namespaces": True,
                "sha256": "input-b",
            },
        ]
        identity = {"path": str(self.library), "sha256": self.library_hash}
        self.observed: dict[str, Any] = {
            "requested_library": str(self.library),
            "library": identity,
            "library_after": identity.copy(),
            "corpus_sha256": "corpus-hash",
            "capture_callbacks": True,
            "results": copy.deepcopy(self.corpus),
        }

    def check(self, observed: dict) -> None:
        differential.check_worker(
            observed,
            self.corpus,
            "corpus-hash",
            str(self.library),
            self.library_hash,
            True,
        )

    def test_full_inventory_is_required_even_when_both_workers_agree(self) -> None:
        self.check(self.observed)
        for results in ([], self.corpus[:1], self.corpus[::-1], [self.corpus[0]] * 2):
            observed = copy.deepcopy(self.observed)
            observed["results"] = results
            with self.subTest(results=results), self.assertRaises(RuntimeError):
                self.check(observed)

    def test_input_mode_and_library_substitutions_are_rejected(self) -> None:
        for field, value in (
            ("name", "wrong"),
            ("chunk_size", 99),
            ("namespaces", True),
            ("sha256", "wrong"),
        ):
            observed = copy.deepcopy(self.observed)
            observed["results"][0][field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.check(observed)
        for field, value in (
            ("capture_callbacks", False),
            ("corpus_sha256", "wrong"),
            ("requested_library", "wrong.so"),
        ):
            observed = copy.deepcopy(self.observed)
            observed[field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.check(observed)
        for field, value in (
            ("path", str(self.root / "wrong.so")),
            ("sha256", "wrong"),
        ):
            observed = copy.deepcopy(self.observed)
            observed["library"][field] = value
            observed["library_after"][field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.check(observed)
        observed = copy.deepcopy(self.observed)
        observed["library_after"]["sha256"] = "changed"
        with self.assertRaises(RuntimeError):
            self.check(observed)

    def test_corrupt_input_is_rejected_before_parsing(self) -> None:
        corpus = self.root / "corpus.json"
        corpus.write_text(
            json.dumps(
                [
                    {
                        "name": "corrupted",
                        "base64": base64.b64encode(b"<r/>").decode(),
                        "sha256": "incorrect",
                        "chunk_size": 1,
                        "namespaces": False,
                    }
                ]
            )
        )
        engine = Mock()
        with (
            patch.object(differential, "Expat", return_value=engine),
            patch.object(
                differential, "library_identity", return_value=self.observed["library"]
            ),
            self.assertRaisesRegex(RuntimeError, "input bytes"),
        ):
            differential.worker(
                str(self.library), corpus, self.root / "worker.json", True
            )
        engine.parse.assert_not_called()

    def test_two_empty_successful_workers_fail_the_main_command(self) -> None:
        output = self.root / "report"
        commands = []

        def empty_worker(
            command: list[str], **kwargs: Any
        ) -> subprocess.CompletedProcess:
            commands.append(command)
            self.assertEqual(command[1:3], ["-I", "-S"])
            environment = kwargs["env"]
            self.assertIsInstance(environment, dict)
            for variable in ("LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT"):
                self.assertNotIn(variable, environment)
            Path(command[command.index("--output") + 1]).write_text(
                json.dumps({"version": "test", "results": []})
            )
            return subprocess.CompletedProcess(command, 0, "", "")

        with (
            patch.object(
                sys,
                "argv",
                [
                    "differential.py",
                    "--library",
                    str(self.library),
                    "--reference",
                    str(self.library),
                    "--output",
                    str(output),
                    "--chunks",
                    "1",
                    "--generated",
                    "0",
                ],
            ),
            patch.object(differential, "cases", return_value=[Case("only", b"<r/>")]),
            patch.object(differential.subprocess, "run", side_effect=empty_worker),
            patch.object(differential.platform, "platform", return_value="test"),
            patch.dict(
                os.environ,
                dict.fromkeys(
                    ("LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT"), "unwanted"
                ),
            ),
            patch("builtins.print"),
        ):
            self.assertEqual(differential.main(), 1)
        self.assertEqual(len(commands), 2)
        report = json.loads((output / "summary.json").read_text())
        self.assertEqual(report["cases"], 1)
        self.assertEqual(report["status"], "failed")
        self.assertIn("inventory", report["failure"])


if __name__ == "__main__":
    unittest.main()
