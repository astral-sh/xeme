"""Require complete passing runs and bound the historical fragmentation exception."""

import unittest
from pathlib import Path

from run import (
    FRAGMENTATION_ASSERTIONS,
    only_text_fragmentation,
    successful_test_run,
    test_inventory,
)


class StrictGateTests(unittest.TestCase):
    def test_success_requires_complete_inventory_and_clean_result(self) -> None:
        cases = ["test.test_pyexpat.BufferTextTest.test1"]
        log = (
            "test1 (test.test_pyexpat.BufferTextTest.test1) ... ok\n"
            "Total tests: run=1 skipped=0\n"
            "Total test files: run=6/6\n"
        )
        self.assertTrue(successful_test_run(log, 0, 6, cases))
        for altered in [
            log.replace("run=6/6", "run=5/6"),
            log.replace("run=1 skipped", "run=2 skipped"),
            log.replace("test1", "test2"),
            log.replace("skipped=0", "failures=1"),
            log + "ERROR: unexpected failure\n",
        ]:
            with self.subTest(log=altered):
                self.assertFalse(successful_test_run(altered, 0, 6, cases))
        self.assertFalse(successful_test_run(log, 2, 6, cases))
        self.assertFalse(successful_test_run(log, 0, 6, [*cases, "test.missing.test"]))

    def test_historical_fragmentation_failures_are_rejected(self) -> None:
        log, cases = FragmentationGateTests.log()
        for exit_code in [0, 2]:
            self.assertFalse(successful_test_run(log, exit_code, 6, cases))

    def test_class_skip_accounts_for_unexecuted_cases(self) -> None:
        cases = ["test.test_sax.Skipped.test_example"]
        log = (
            "setUpClass (test.test_sax.Skipped) ... skipped 'resource unavailable'\n"
            "Total tests: run=0 skipped=1\n"
            "Total test files: run=6/6\n"
        )
        self.assertTrue(successful_test_run(log, 0, 6, cases))


class FragmentationGateTests(unittest.TestCase):
    def test_complete_known_assertions(self) -> None:
        log, cases = self.log()
        self.assertTrue(only_text_fragmentation(log, 2, 6, cases))
        for exit_code in [0, 1, -9]:
            self.assertFalse(only_text_fragmentation(log, exit_code, 6, cases))

    def test_same_method_does_not_waive_other_assertions_or_payloads(self) -> None:
        log, cases = self.log()
        for altered in [
            log.replace("Parseable character data", "corrupted character data", 1),
            log.replace("line 1543", "line 1544"),
            log.replace("in characters", "in test_handlers"),
            log.replace(
                "h.assertEqual(t[0], content)", "h.assertEqual(t[1], h.in_cdata)"
            ),
            log.replace(
                "AssertionError: Lists differ:", "AssertionError: arbitrary failure:"
            ),
            log.replace("/Lib/test/test_pyexpat.py", "/other/test_pyexpat.py"),
            log.replace("'2\\n3'", "'wrong'", 1),
        ]:
            with self.subTest(log=altered):
                self.assertFalse(only_text_fragmentation(altered, 2, 6, cases))

    def test_unknown_error_incomplete_run_and_mismatched_counts(self) -> None:
        log, cases = self.log()
        for altered in [
            log.replace("FAIL:", "ERROR:", 1),
            log.replace("BufferTextTest.test1", "BufferTextTest.test2"),
            log.replace("failures=2", "failures=3"),
            log.replace("failures=2", "failures=2 errors=1"),
            log.replace("run=6/6", "run=5/6"),
            log.replace("failed=2", "failed=3"),
            log.replace("run=2 failures", "run=3 failures"),
            log.split("Total tests:")[0],
            log.replace(" ... FAIL\n", " ... FAIL\n" + log.splitlines()[0] + "\n", 1),
        ]:
            with self.subTest(log=altered):
                self.assertFalse(only_text_fragmentation(altered, 2, 6, cases))
        self.assertFalse(only_text_fragmentation(log, 2, 7, cases))
        self.assertFalse(
            only_text_fragmentation(
                log, 2, 6, [*cases, "test.test_sax.Other.test_missing"]
            )
        )

    def test_class_skip_accounts_for_unexecuted_cases(self) -> None:
        log, cases = self.log()
        cases.append("test.test_sax.Skipped.test_example")
        self.assertFalse(only_text_fragmentation(log, 2, 6, cases))
        log = (
            "setUpClass (test.test_sax.Skipped) ... skipped 'resource unavailable'\n"
            + log
        )
        self.assertTrue(only_text_fragmentation(log, 2, 6, cases))
        self.assertFalse(
            only_text_fragmentation(log.replace("skipped", "ERROR"), 2, 6, cases)
        )

    def test_inventory_hash_rejects_changed_or_reduced_suite(self) -> None:
        for text in ["", "test.test_sax.CDATAHandlerTest.test_handlers\n"]:
            with self.assertRaisesRegex(
                ValueError, "unexpected CPython XML test inventory"
            ):
                test_inventory(text)

    def test_installed_tracebacks_must_use_the_verified_fixture_directory(self) -> None:
        log, cases = self.log()
        directory = Path("/python/install/lib/python3.12/test")
        log = log.replace("/cpython/Lib/test", str(directory))
        self.assertTrue(only_text_fragmentation(log, 2, 6, cases, directory))
        self.assertFalse(only_text_fragmentation(log, 2, 6, cases, Path("/other/test")))

    @staticmethod
    def log() -> tuple[str, list[str]]:
        cases = [name.split("(", 1)[1][:-1] for name in FRAGMENTATION_ASSERTIONS]
        lines = [f"{name} ... FAIL" for name in FRAGMENTATION_ASSERTIONS]
        for name, (
            filename,
            line,
            function,
            source,
            assertion,
        ) in FRAGMENTATION_ASSERTIONS.items():
            lines.extend(
                [
                    "=" * 70,
                    f"FAIL: {name}",
                    "-" * 70,
                    "Traceback (most recent call last):",
                    f'  File "/cpython/Lib/test/{filename}", line {line}, in {function}',
                    f"    {source}",
                    assertion,
                    "-" * 70,
                ]
            )
        lines.extend(
            [
                "Total tests: run=2 failures=2 skipped=0",
                "Total test files: run=6/6 failed=2",
            ]
        )
        return "\n".join(lines), cases


if __name__ == "__main__":
    unittest.main()
