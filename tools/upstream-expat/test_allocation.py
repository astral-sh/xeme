"""Check allocation diagnostics independently of cached upstream checkouts."""

import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import allocation_behavior

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


class AllocationAdapterTests(unittest.TestCase):
    def test_pinned_selection_includes_namespaces_and_supplemental_tests(self) -> None:
        public = set(
            json.loads((ROOT / "tools/compatibility-baseline.json").read_text())["api"][
                "tests"
            ]
        )
        selected = allocation_behavior.selected_tests(public)
        self.assertEqual(len(selected), 84)
        self.assertEqual(sum(name.startswith("test_nsalloc_") for name in selected), 27)
        self.assertTrue(allocation_behavior.EXTRA_TESTS <= set(selected))
        for changed in (
            public - {selected[0]},
            public | {"test_alloc_unreviewed"},
            (public - {selected[0]}) | {"test_alloc_renamed"},
        ):
            with self.subTest(changed=changed ^ public), self.assertRaises(ValueError):
                allocation_behavior.selected_tests(changed)

    def test_every_source_hash_is_checked_before_any_file_is_changed(self) -> None:
        originals = {
            name: f"/* Synthetic {name} fixture. */\n"
            for name in allocation_behavior.SOURCE_SHA256
        }
        hashes = {
            name: hashlib.sha256(text.encode()).hexdigest()
            for name, text in originals.items()
        }
        for changed in originals:
            with (
                self.subTest(changed=changed),
                tempfile.TemporaryDirectory() as temporary,
            ):
                directory = Path(temporary) / "adapted"
                directory.mkdir()
                sources = {**originals, changed: originals[changed] + "/* drift */\n"}
                for name, text in sources.items():
                    (directory / name).write_text(text)
                with (
                    patch.object(allocation_behavior, "SOURCE_SHA256", hashes),
                    self.assertRaisesRegex(ValueError, f"pristine pinned {changed}"),
                ):
                    allocation_behavior.adapt_sources(directory)
                self.assertEqual(
                    {name: (directory / name).read_text() for name in sources}, sources
                )
                self.assertFalse(
                    (directory.parent / "allocation-behavior.patch").exists()
                )

    def test_matching_hashes_do_not_allow_missing_rewrite_anchors(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "adapted"
            directory.mkdir()
            source = "/* Expected allocation fixture is absent. */\n"
            hashes = dict.fromkeys(
                allocation_behavior.SOURCE_SHA256,
                hashlib.sha256(source.encode()).hexdigest(),
            )
            for name in hashes:
                (directory / name).write_text(source)
            with (
                patch.object(allocation_behavior, "SOURCE_SHA256", hashes),
                self.assertRaisesRegex(ValueError, "expected 1 matches"),
            ):
                allocation_behavior.adapt_sources(directory)
            self.assertTrue(
                all((directory / name).read_text() == source for name in hashes)
            )
            self.assertFalse((directory.parent / "allocation-behavior.patch").exists())


@unittest.skipUnless(
    sys.platform.startswith("linux") and shutil.which("cc"), "requires Linux cc"
)
class AllocationDiagnosticTests(unittest.TestCase):
    def run_selftest(
        self, name: str, bridge: bool = False
    ) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            # The discarded test-registration code needs only this public callback type.
            (directory / "minicheck.h").write_text(
                "typedef void (*tcase_test_function)(void);\n"
            )
            executable = directory / "selftest"
            command = [
                "cc",
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-I",
                str(HERE),
                "-I",
                str(ROOT / "include"),
                "-I",
                str(directory),
                str(HERE / name),
                str(HERE / "allocation_tracker.c"),
                "-o",
                str(executable),
            ]
            if bridge:
                command.extend(
                    [
                        "-DXEME_ALLOCATION_BEHAVIOR",
                        "-ffunction-sections",
                        "-fdata-sections",
                        str(HERE / "bridge.c"),
                        "-Wl,--gc-sections",
                        "-ldl",
                    ]
                )
            compiled = subprocess.run(
                command, text=True, capture_output=True, check=False
            )
            self.assertEqual(compiled.returncode, 0, compiled.stdout + compiled.stderr)
            result = subprocess.run(
                [str(executable)],
                text=True,
                capture_output=True,
                timeout=30,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            return result

    def test_tracker_ownership_failures_and_failed_realloc_preserve_state(self) -> None:
        result = self.run_selftest("allocation_tracker_selftest.c")
        self.assertIn("self-checks passed: 6 child scenarios", result.stdout)
        audits = [
            json.loads(line.split("\t", 1)[1])
            for line in result.stdout.splitlines()
            if line.startswith("XEME_ALLOCATION_AUDIT\t")
        ]
        self.assertEqual(len(audits), 1)
        self.assertEqual(audits[0]["live_blocks"], 0)
        # This selftest deliberately asks libc for SIZE_MAX to check failed realloc.
        self.assertEqual(audits[0]["system_failures"], 1)
        self.assertEqual(audits[0]["injected_malloc_failures"], 1)
        self.assertEqual(audits[0]["injected_realloc_failures"], 1)
        self.assertEqual(result.stderr.count("XEME_ALLOCATION_ERROR\t"), 5)
        self.assertIn("allocations remain after teardown", result.stderr)
        self.assertIn("pointer is not a live allocation", result.stderr)

    def test_parse_bridge_preserves_results_and_tracks_default_constructors(
        self,
    ) -> None:
        result = self.run_selftest("allocation_bridge_selftest.c", bridge=True)
        self.assertIn(
            "self-checks passed: 14 child scenarios, 4 constructors", result.stdout
        )
        self.assertEqual(
            result.stderr.count("injected failure reported wrong parse error"), 2
        )


if __name__ == "__main__":
    unittest.main()
