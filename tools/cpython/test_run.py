"""Keep the optional callback-fragmentation gate closed on unrelated failures."""

import unittest

from run import TEXT_FRAGMENTATION_FAILURES, only_text_fragmentation


class FragmentationGateTests(unittest.TestCase):
    def test_complete_known_assertions(self) -> None:
        log = self.log()
        self.assertTrue(only_text_fragmentation(log, 2, 6))
        for exit_code in [0, 1, -9]:
            self.assertFalse(only_text_fragmentation(log, exit_code, 6))

    def test_unknown_error_incomplete_run_and_mismatched_counts(self) -> None:
        log = self.log()
        for altered in [
            log.replace("FAIL:", "ERROR:", 1),
            log.replace("BufferTextTest.test1", "BufferTextTest.test2"),
            log.replace("failures=2", "failures=3"),
            log.replace("failures=2", "failures=2 errors=1"),
            log.replace("run=6/6", "run=5/6"),
            log.replace("failed=2", "failed=3"),
            log.split("Total tests:")[0],
        ]:
            with self.subTest(log=altered):
                self.assertFalse(only_text_fragmentation(altered, 2, 6))
        self.assertFalse(only_text_fragmentation(log, 2, 7))

    @staticmethod
    def log() -> str:
        return "\n".join(
            [
                *(f"FAIL: {name}" for name in sorted(TEXT_FRAGMENTATION_FAILURES)),
                "Total tests: run=803 failures=2 skipped=31",
                "Total test files: run=6/6 failed=2",
            ]
        )


if __name__ == "__main__":
    unittest.main()
