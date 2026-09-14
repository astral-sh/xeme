"""Require complete passing runs of the pinned CPython XML suites."""

import unittest

from run import successful_test_run, test_inventory


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
        cases = [
            "test.test_pyexpat.BufferTextTest.test1",
            "test.test_sax.CDATAHandlerTest.test_handlers",
        ]
        log = (
            "test1 (test.test_pyexpat.BufferTextTest.test1) ... FAIL\n"
            "test_handlers (test.test_sax.CDATAHandlerTest.test_handlers) ... FAIL\n"
            "FAIL: test1 (test.test_pyexpat.BufferTextTest.test1)\n"
            "FAIL: test_handlers (test.test_sax.CDATAHandlerTest.test_handlers)\n"
            "Total tests: run=2 failures=2 skipped=0\n"
            "Total test files: run=6/6 failed=2\n"
        )
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

    def test_inventory_hash_rejects_changed_or_reduced_suite(self) -> None:
        for text in ["", "test.test_sax.CDATAHandlerTest.test_handlers\n"]:
            with self.assertRaisesRegex(
                ValueError, "unexpected CPython XML test inventory"
            ):
                test_inventory(text)


if __name__ == "__main__":
    unittest.main()
