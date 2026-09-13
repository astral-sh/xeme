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
        "oriole": [1.0, 100.0, 3.0],
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
        for crate in ["oriole_storage", "oriole", "oriole_expat"]:
            source = checkout / "crates" / crate / "src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text(name)
        return argparse.Namespace(
            checkout=checkout,
            output=directory / f"{name}-build",
            target_dir=directory / f"{name}-target",
            toolchain=None,
        )

    def compiler_log(self, stream, checkout: Path, intermediates: Path):
        for crate in ["oriole_storage", "oriole", "oriole_expat"]:
            command = [
                f"CARGO_MANIFEST_DIR={checkout / 'crates' / crate}",
                "rustc",
                "--crate-name",
                crate,
                f"crates/{crate}/src/lib.rs",
                "--target",
                hillclimb.TARGET,
                "--out-dir",
                str(intermediates / hillclimb.TARGET / "release/deps"),
            ]
            stream.write(f"Running `{shlex.join(command)}`\n")

    def artifact_message(self, stream, checkout, library, *, fresh=False):
        stream.write(
            json.dumps(
                {
                    "reason": "compiler-artifact",
                    "manifest_path": str(checkout / "crates/oriole_expat/Cargo.toml"),
                    "target": {
                        "name": "oriole_expat",
                        "src_path": str(checkout / "crates/oriole_expat/src/lib.rs"),
                    },
                    "fresh": fresh,
                    "filenames": [str(library)],
                }
            )
            + "\n"
        )

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
                set(compiled), {"oriole_storage", "oriole", "oriole_expat"}
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

            def compiler(command, *, cwd, env, stdout, stderr, **kwargs):
                target = (
                    Path(command[command.index("--target-dir") + 1])
                    / hillclimb.TARGET
                    / "release/liboriole_expat.so"
                )
                target.parent.mkdir(parents=True, exist_ok=True)
                self.assertNotIn("RUSTC", env)
                self.assertNotIn("CARGO_BUILD_RUSTC", env)
                intermediate = Path(env["CARGO_BUILD_BUILD_DIR"])
                if intermediate == shared:
                    stderr.write(
                        "Fresh oriole_storage\nFresh oriole\nFresh oriole_expat\n"
                    )
                    target.write_bytes(cached.read_bytes())
                else:
                    self.assertEqual(list(intermediate.iterdir()), [])
                    self.compiler_log(stderr, cwd, intermediate)
                    target.write_bytes((cwd / "crates/oriole/src/lib.rs").read_bytes())
                self.artifact_message(stdout, cwd, target)

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
                (baseline.output / "liboriole_expat.so").read_bytes(), b"baseline"
            )
            self.assertEqual(
                (candidate.output / "liboriole_expat.so").read_bytes(), b"candidate"
            )
            self.assertEqual(cached.read_bytes(), b"wrong candidate cache")
            for args in [baseline, candidate]:
                report = json.loads((args.output / "build.json").read_text())
                self.assertEqual(report["status"], "passed")
                self.assertEqual(
                    set(report["workspace_compilations"]),
                    {"oriole_storage", "oriole", "oriole_expat"},
                )

    def test_cached_or_foreign_workspace_commands_cannot_pass(self):
        for mode in ["cached", "foreign"]:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                args = self.checkout(Path(temporary), "candidate")

                def compiler(command, *, cwd, env, stderr, mode=mode, **kwargs):
                    if mode == "cached":
                        stderr.write(
                            "Fresh oriole_storage\nFresh oriole\nFresh oriole_expat\n"
                        )
                    else:
                        self.compiler_log(
                            stderr,
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
                self.assertFalse((args.output / "liboriole_expat.so").exists())

    def test_configured_target_cannot_publish_old_host_artifact(self):
        for toolchain in [None, "ohm"]:
            with (
                self.subTest(toolchain=toolchain),
                tempfile.TemporaryDirectory() as temporary,
            ):
                args = self.checkout(Path(temporary), "candidate")
                args.toolchain = toolchain
                config = args.checkout / ".cargo/config.toml"
                config.parent.mkdir()
                config.write_text('[build]\ntarget = "aarch64-unknown-linux-gnu"\n')
                stale = args.target_dir / "release/liboriole_expat.so"
                stale.parent.mkdir(parents=True)
                stale.write_bytes(b"old baseline")
                fresh = (
                    args.target_dir / hillclimb.TARGET / "release/liboriole_expat.so"
                )

                def compiler(
                    command,
                    *,
                    cwd,
                    env,
                    stdout,
                    stderr,
                    toolchain=toolchain,
                    fresh=fresh,
                    **kwargs,
                ):
                    self.assertEqual(
                        command[command.index("--target") + 1], hillclimb.TARGET
                    )
                    self.assertIn("--message-format=json-render-diagnostics", command)
                    if toolchain == "ohm":
                        self.assertEqual(
                            command[:3], ["cargo", "+ohm", "-Zohm-defaults=no"]
                        )
                    else:
                        self.assertEqual(command[:2], ["cargo", "rustc"])
                    self.compiler_log(stderr, cwd, Path(env["CARGO_BUILD_BUILD_DIR"]))
                    fresh.parent.mkdir(parents=True)
                    fresh.write_bytes(b"fresh candidate")
                    self.artifact_message(stdout, cwd, fresh)

                with (
                    patch.object(
                        hillclimb.subprocess, "check_output", return_value="test"
                    ),
                    patch.object(hillclimb.subprocess, "run", side_effect=compiler),
                ):
                    hillclimb.build(args)
                report = json.loads((args.output / "build.json").read_text())
                self.assertEqual(report["status"], "passed")
                self.assertEqual(report["emitted_library"], str(fresh))
                self.assertEqual(
                    (args.output / "liboriole_expat.so").read_bytes(),
                    b"fresh candidate",
                )
                self.assertEqual(stale.read_bytes(), b"old baseline")

    def test_only_current_uncached_emitted_library_is_accepted(self):
        with tempfile.TemporaryDirectory() as temporary:
            args = self.checkout(Path(temporary), "candidate")
            library = args.target_dir / hillclimb.TARGET / "release/liboriole_expat.so"
            library.parent.mkdir(parents=True)
            library.write_bytes(b"candidate")
            stream = io.StringIO()
            self.artifact_message(stream, args.checkout, library)
            message = json.loads(stream.getvalue())
            variants = [[], [message, message]]
            for key, value in [
                ("fresh", True),
                ("manifest_path", str(args.checkout / "foreign/Cargo.toml")),
                ("filenames", []),
            ]:
                changed = copy.deepcopy(message)
                changed[key] = value
                variants.append([changed])
            for messages in variants:
                with self.subTest(messages=messages), self.assertRaises(ValueError):
                    hillclimb.library_artifact(
                        "\n".join(json.dumps(item) for item in messages),
                        args.checkout,
                        args.target_dir,
                    )


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


class CorpusTests(unittest.TestCase):
    def fixture(self, directory):
        path = directory / "input.xml"
        path.write_bytes(b"<root/>")
        notice = directory / "NOTICE"
        notice.write_bytes(b"Original notice")
        manifest = directory / "corpus.json"
        data = {
            "projects": [
                {
                    "name": "project",
                    "files": [
                        {
                            "role": role,
                            "path": p.name,
                            "sha256": hillclimb.digest(p),
                            "bytes": p.stat().st_size,
                        }
                        for role, p in [("input", path), ("notice", notice)]
                    ],
                }
            ]
        }
        manifest.write_text(json.dumps(data))
        return manifest, data

    def test_reserved_holdout_is_distinct_and_all_original_files_match(self):
        manifest, inputs, hashes = hillclimb.holdout_corpus()
        self.assertEqual(
            set(inputs), {"libreoffice", "dotnet", "hadoop", "qt", "musescore"}
        )
        self.assertEqual(
            len(hashes), 20
        )  # 18 source/notice files plus manifest and freeze.
        self.assertEqual(hashes[str(manifest)], hillclimb.digest(manifest))

    def test_holdout_requires_prior_selection_and_uses_reserved_manifest(self):
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
                output=directory / "missing-note",
                candidate=library,
                baseline=library,
                expat=library,
                baseline_build=manifest,
                candidate_build=manifest,
                build_manifest=[],
                consumers=None,
                mode="holdout",
                selection_note=None,
                seed=1,
                native_iterations=None,
                python_iterations=None,
                python=Path("python3"),
            )
            with (
                patch.object(os, "sched_getaffinity", return_value={6}),
                patch.object(hillclimb.subprocess, "run") as child,
            ):
                with self.assertRaisesRegex(ValueError, "selection-note"):
                    hillclimb.run(args)
                child.assert_not_called()
                args.output = directory / "selected"
                args.selection_note = directory / "selection.md"
                args.selection_note.write_text(
                    "Code selected using tuning results before holdout exposure."
                )
                child.side_effect = subprocess.CalledProcessError(1, ["worker"])
                with self.assertRaises(subprocess.CalledProcessError):
                    hillclimb.run(args)
                command = child.call_args.args[0]
                self.assertEqual(
                    command[command.index("--corpus") + 1],
                    str(hillclimb.ROOT / "benchmarks/holdout/corpus-manifest.json"),
                )
                report = json.loads((args.output / "report.json").read_text())
                self.assertEqual(
                    report["evaluation"], "final holdout after code selection"
                )
                self.assertEqual(
                    report["sha256_before"][str(args.selection_note)],
                    hillclimb.digest(args.selection_note),
                )

    def test_changed_input_or_notice_is_rejected(self):
        for filename in ["input.xml", "NOTICE"]:
            with (
                self.subTest(filename=filename),
                tempfile.TemporaryDirectory() as temporary,
            ):
                directory = Path(temporary)
                manifest, _ = self.fixture(directory)
                inputs, _ = hillclimb.corpus_files(manifest)
                self.assertEqual(set(inputs), {"project"})
                (directory / filename).write_bytes(b"changed")
                with self.assertRaisesRegex(ValueError, "identity mismatch"):
                    hillclimb.corpus_files(manifest)

    def test_duplicate_project_or_multiple_inputs_is_rejected(self):
        for duplicate_project in [False, True]:
            with (
                self.subTest(duplicate_project=duplicate_project),
                tempfile.TemporaryDirectory() as temporary,
            ):
                manifest, data = self.fixture(Path(temporary))
                if duplicate_project:
                    data["projects"].append(copy.deepcopy(data["projects"][0]))
                else:
                    data["projects"][0]["files"][1]["role"] = "input"
                manifest.write_text(json.dumps(data))
                with self.assertRaises(ValueError):
                    hillclimb.corpus_files(manifest)


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
                        "libraries": {"oriole": "a", "expat": "b", "baseline": "c"},
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
