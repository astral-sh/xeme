"""Arithmetic and failure gates for the benchmark entrypoint; no timing workloads."""

from __future__ import annotations

import argparse
import copy
import io
import json
import math
import os
import shlex
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import hillclimb
import projects


def fixture() -> dict:
    durations = {
        "xeme": [1.0, 100.0, 3.0],
        "baseline": [1.0, 2.0, 3.0],
        "expat": [2.0, 4.0, 6.0],
    }
    return {
        "status": "passed",
        "sha256_before": {"input": "hash"},
        "sha256_after": {"input": "hash"},
        "pairs": 3,
        "iterations": 3,
        "summary": {"project/4096/namespaces-0": {}},
        "rows": [
            {
                "key": "project/4096/namespaces-0",
                "pair": pair,
                "engine": engine,
                "samples": [
                    {
                        "iteration": i,
                        "warmup": i == 0,
                        "seconds": 1e9 if i == 0 else value,
                    }
                    for i in range(4)
                ],
            }
            for engine, values in durations.items()
            for pair, value in enumerate(values)
        ],
    }


class FreshBuildTests(unittest.TestCase):
    def checkout(self, directory: Path, name: str) -> argparse.Namespace:
        checkout = directory / name
        checkout.mkdir()
        (checkout / "Cargo.toml").write_text("workspace")
        (checkout / "Cargo.lock").write_text("locked")
        for crate in ["xeme_storage", "xeme", "xeme_expat"]:
            source = checkout / "crates" / crate / "src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text(name)
        return argparse.Namespace(
            checkout=checkout,
            output=directory / f"{name}-build",
            target_dir=directory / f"{name}-target",
        )

    def compiler_log(self, stream, checkout: Path, intermediates: Path):
        for crate in ["xeme_storage", "xeme", "xeme_expat"]:
            command = [
                f"CARGO_MANIFEST_DIR={checkout / 'crates' / crate}",
                "rustc",
                "--crate-name",
                crate,
                f"crates/{crate}/src/lib.rs",
                "--out-dir",
                str(intermediates / "release/deps"),
            ]
            stream.write(f"Running `{shlex.join(command)}`\n")

    def test_multiline_dependency_metadata_is_not_parsed_as_a_workspace_command(self):
        with tempfile.TemporaryDirectory() as temporary:
            args = self.checkout(Path(temporary), "candidate")
            intermediates = args.output / "intermediates"
            stream = io.StringIO()
            # The actual memchr description spans three verbose-log lines.
            stream.write(
                "Running `CARGO_CRATE_NAME=memchr CARGO_PKG_DESCRIPTION='Provides "
                "extremely fast (uses SIMD on x86_64, aarch64 and wasm32) routines for\n"
                "1, 2 or 3 byte search and single substring search.\n"
                "' rustc --crate-name memchr src/lib.rs`\n"
            )
            self.compiler_log(stream, args.checkout, intermediates)
            compiled = hillclimb.workspace_compilations(
                stream.getvalue(), args.checkout, intermediates
            )
            self.assertEqual(
                set(compiled), {"xeme_storage", "xeme", "xeme_expat"}
            )

    def test_distinct_sources_cannot_reuse_the_shared_intermediate_artifact(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            shared = directory / "shared"
            shared.mkdir()
            cached = shared / "old-artifact"
            cached.write_bytes(b"wrong candidate cache")
            baseline = self.checkout(directory, "baseline")
            candidate = self.checkout(directory, "candidate")

            def compiler(command, *, cwd, env, stdout, **kwargs):
                target = (
                    Path(command[command.index("--target-dir") + 1])
                    / "release/libxeme_expat.so"
                )
                target.parent.mkdir(parents=True, exist_ok=True)
                self.assertNotIn("RUSTC", env)
                self.assertNotIn("CARGO_BUILD_RUSTC", env)
                intermediate = Path(env["CARGO_BUILD_BUILD_DIR"])
                if intermediate == shared:
                    stdout.write(
                        "Fresh xeme_storage\nFresh xeme\nFresh xeme_expat\n"
                    )
                    target.write_bytes(cached.read_bytes())
                else:
                    self.assertEqual(list(intermediate.iterdir()), [])
                    self.compiler_log(stdout, cwd, intermediate)
                    target.write_bytes((cwd / "crates/xeme/src/lib.rs").read_bytes())

            with (
                patch.dict(
                    os.environ,
                    {
                        "CARGO_BUILD_BUILD_DIR": str(shared),
                        "RUSTC": "/wrong/compiler",
                        "CARGO_BUILD_RUSTC": "/wrong/compiler",
                    },
                ),
                patch.object(hillclimb.subprocess, "check_output", return_value="test"),
                patch.object(hillclimb.subprocess, "run", side_effect=compiler),
            ):
                hillclimb.build(baseline)
                hillclimb.build(candidate)
                self.assertEqual(os.environ["CARGO_BUILD_BUILD_DIR"], str(shared))
            self.assertEqual(
                (baseline.output / "libxeme_expat.so").read_bytes(), b"baseline"
            )
            self.assertEqual(
                (candidate.output / "libxeme_expat.so").read_bytes(), b"candidate"
            )
            self.assertEqual(cached.read_bytes(), b"wrong candidate cache")
            for args in [baseline, candidate]:
                report = json.loads((args.output / "build.json").read_text())
                self.assertEqual(report["status"], "passed")
                self.assertEqual(
                    set(report["workspace_compilations"]),
                    {"xeme_storage", "xeme", "xeme_expat"},
                )

    def test_cached_or_foreign_workspace_commands_cannot_pass(self):
        for mode in ["cached", "foreign"]:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                args = self.checkout(Path(temporary), "candidate")

                def compiler(command, *, cwd, env, stdout, mode=mode, **kwargs):
                    if mode == "cached":
                        stdout.write(
                            "Fresh xeme_storage\nFresh xeme\nFresh xeme_expat\n"
                        )
                    else:
                        self.compiler_log(
                            stdout,
                            cwd.parent / "another-checkout",
                            Path(env["CARGO_BUILD_BUILD_DIR"]),
                        )

                with (
                    patch.object(
                        hillclimb.subprocess, "check_output", return_value="test"
                    ),
                    patch.object(hillclimb.subprocess, "run", side_effect=compiler),
                    self.assertRaises(ValueError),
                ):
                    hillclimb.build(args)
                self.assertEqual(
                    json.loads((args.output / "build.json").read_text())["status"],
                    "failed",
                )
                self.assertFalse((args.output / "libxeme_expat.so").exists())


class BuildBindingTests(unittest.TestCase):
    def test_only_a_successful_matching_build_can_supply_a_library(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            baseline = directory / "baseline.so"
            baseline.write_bytes(b"baseline")
            candidate = directory / "candidate.so"
            candidate.write_bytes(b"candidate")
            manifest = directory / "build.json"
            record = {"status": "passed", "library_sha256": hillclimb.digest(baseline)}
            manifest.write_text(json.dumps(record))
            self.assertEqual(hillclimb.validate_build(manifest, baseline), record)
            with self.assertRaisesRegex(ValueError, "does not match library"):
                hillclimb.validate_build(manifest, candidate)
            baseline.write_bytes(b"stale library")
            with self.assertRaisesRegex(ValueError, "does not match library"):
                hillclimb.validate_build(manifest, baseline)
            record.update(status="failed", library_sha256=hillclimb.digest(baseline))
            manifest.write_text(json.dumps(record))
            with self.assertRaisesRegex(ValueError, "build did not pass"):
                hillclimb.validate_build(manifest, baseline)

    def test_runner_isolates_children_and_preserves_a_failed_campaign(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            library = directory / "parser.so"
            library.write_bytes(b"parser")
            manifest = directory / "build.json"
            manifest.write_text(
                json.dumps(
                    {"status": "passed", "library_sha256": hillclimb.digest(library)}
                )
            )
            args = argparse.Namespace(
                cpu=6,
                output=directory / "run",
                candidate=library,
                baseline=library,
                expat=library,
                baseline_build=manifest,
                candidate_build=manifest,
                build_manifest=[],
                consumers=None,
                mode="screen",
                seed=1,
                native_iterations=None,
                python_iterations=None,
                python=Path("python3"),
            )
            injected = dict.fromkeys(
                ["LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT"], "/untrusted"
            )
            with (
                patch.dict(os.environ, injected),
                patch.object(os, "sched_getaffinity", return_value={6}),
                patch.object(hillclimb.subprocess, "run") as child,
            ):
                child.side_effect = subprocess.CalledProcessError(1, ["worker"])
                with self.assertRaises(subprocess.CalledProcessError):
                    hillclimb.run(args)
            child.assert_called_once()
            command = child.call_args.args[0]
            self.assertEqual(command[3:6], ["python3", "-I", "-S"])
            self.assertTrue(injected.keys().isdisjoint(child.call_args.kwargs["env"]))
            self.assertEqual(
                json.loads((args.output / "report.json").read_text())["status"],
                "failed",
            )


class SummaryTests(unittest.TestCase):
    def test_median_of_within_round_ratios_discards_warmups(self):
        row = hillclimb.summarize(fixture())[0]
        self.assertEqual(row["candidate_over_baseline"], 1.0)
        self.assertEqual(row["candidate_over_expat"], 0.5)
        self.assertEqual(row["regression_percent"], 0.0)

    def test_failures_and_changed_inputs_cannot_be_reported(self):
        for field, value in [("status", "failed"), ("sha256_after", {})]:
            report = fixture()
            report[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                hillclimb.summarize(report)

    def test_missing_duplicate_and_extra_workers_are_rejected(self):
        original = fixture()
        missing = copy.deepcopy(original)
        missing["rows"].pop()
        duplicate = copy.deepcopy(original)
        duplicate["rows"].append(duplicate["rows"][0])
        extra = copy.deepcopy(original)
        extra["rows"][0]["engine"] = "unexpected"
        for report in [missing, duplicate, extra]:
            with self.subTest(rows=report["rows"]), self.assertRaises(ValueError):
                hillclimb.summarize(report)

    def test_invalid_and_missing_samples_are_rejected(self):
        for value in [0, -1, math.nan, math.inf]:
            report = fixture()
            report["rows"][0]["samples"][1]["seconds"] = value
            with self.subTest(seconds=value), self.assertRaises(ValueError):
                hillclimb.summarize(report)
        report = fixture()
        report["rows"][0]["samples"].pop()
        with self.assertRaises(ValueError):
            hillclimb.summarize(report)

    def test_baseline_callback_mismatch_fails_native_preflight(self):
        def result(text):
            return {
                "status": 1,
                "error": 0,
                "callback_errors": [],
                "position": {},
                "normalized_events": [["text", text]],
            }

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            data = directory / "input.xml"
            data.write_bytes(b"<r/>")
            spec = directory / "preflight.json"
            spec.write_text(
                json.dumps(
                    {
                        "libraries": {"xeme": "a", "expat": "b", "baseline": "c"},
                        "input": str(data),
                        "chunk": 4096,
                        "namespaces": False,
                    }
                )
            )
            with patch.object(projects, "Expat") as expat:
                expat.return_value.parse.side_effect = [
                    result("same"),
                    result("same"),
                    result("wrong"),
                ]
                with self.assertRaisesRegex(RuntimeError, "callback preflight failed"):
                    projects.worker(spec)
            self.assertFalse(
                json.loads(spec.with_suffix(".result.json").read_text())["passed"]
            )


if __name__ == "__main__":
    unittest.main()
