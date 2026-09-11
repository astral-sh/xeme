"""Failure and provenance checks for the optional PGO workflow."""

import argparse
import ctypes
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from pgo import build
from pgo.corpus import documents, generate
from pgo.train import verify_origin


class PgoTests(unittest.TestCase):
    def test_generated_corpus_identity(self):
        first = documents()
        self.assertEqual(first, documents())
        self.assertEqual(len(first), 12)
        self.assertEqual(sum(len(value[0]) for value in first.values()), 1_325_736)
        # Frozen independent study digest, so refactors cannot silently retrain.
        self.assertEqual(
            build.hashlib.sha256(first["attributes"][0]).hexdigest(),
            "3ed561d909cae17c98634b302262ac0830347d3a745c6f8f86e8dd428bb611c8",
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = generate(root / "inputs")
            self.assertEqual(
                sum(
                    len(row["chunks"]) * len(row["namespaces"]) * 4
                    for row in manifest["rows"]
                ),
                288,
            )
            for row in manifest["rows"]:
                self.assertEqual(
                    row["sha256"], build.digest(root / "inputs" / row["file"])
                )
            with self.assertRaises(FileExistsError):
                generate(root / "inputs")

    def test_versions_and_mismatch(self):
        self.assertEqual(build.llvm_version("LLVM version: 22.1.8\n"), "22.1.8")
        self.assertEqual(
            build.llvm_version("LLVM version 22.1.8-rust-1.98.0-stable"), "22.1.8"
        )
        with self.assertRaises(build.BuildError):
            build.llvm_version("unidentified LLVM")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            (source / "crates/oriole_expat").mkdir(parents=True)
            for filename in (
                "Cargo.toml",
                "Cargo.lock",
                "crates/oriole_expat/Cargo.toml",
            ):
                (source / filename).touch()
            (source / ".cargo").mkdir()
            config = source / ".cargo/config.toml"
            config.write_text(
                '[build]\nrustc-wrapper="untrusted-wrapper"\nrustc-workspace-wrapper="other-wrapper"\n'
            )
            (root / "run").mkdir()
            run = build.Run(root / "run", source, 1)
            args = argparse.Namespace(
                toolchain="installed",
                cargo_arg=["-Zohm-defaults=no", "--config=net.offline=true"],
                llvm_profdata=Path(sys.executable),
                output=root / "output",
            )
            with (
                patch.dict(os.environ, {}, clear=True),
                patch("pgo.build.shutil.which", return_value=sys.executable),
                patch.object(
                    run,
                    "command",
                    side_effect=[
                        "host: x86_64-unknown-linux-gnu\nLLVM version: 22.1.8",
                        "cargo version",
                        str(root),
                        "LLVM version 23.0.0",
                    ],
                ) as commands,
            ):
                with self.assertRaisesRegex(build.BuildError, "same LLVM"):
                    build.execute(args, run)
                self.assertEqual(commands.call_count, 4)
                # Cargo options belong after toolchain selection and before
                # Cargo's operation; they must never reach rustc or profdata.
                self.assertEqual(
                    commands.call_args_list[1].args[1][1:],
                    ["+installed", *args.cargo_arg, "-Vv"],
                )
                for index in (0, 2, 3):
                    self.assertFalse(
                        any(
                            value in commands.call_args_list[index].args[1]
                            for value in args.cargo_arg
                        )
                    )
                for call in commands.call_args_list:
                    self.assertEqual(call.args[2]["RUSTC_WRAPPER"], "")
                    self.assertEqual(call.args[2]["RUSTC_WORKSPACE_WRAPPER"], "")
                self.assertEqual(
                    run.manifest["cargo_config_sha256"][str(config)],
                    build.digest(config),
                )
                self.assertFalse((root / "output").exists())

    def test_cargo_options_reject_subcommands_and_argument_terminators(self):
        for value in (
            "build",
            "rustc",
            "--",
            "-",
            "-Z",
            "-C",
            "-C/other/workspace",
            "-vC/other/workspace",
            "--config",
            "--config=/untracked/config.toml",
            "--config=/untracked/name=value.toml",
            "--config=include='/untracked/config.toml'",
        ):
            with self.subTest(value=value):
                with self.assertRaises(argparse.ArgumentTypeError):
                    build.cargo_option(value)
        self.assertEqual(
            build.cargo_option("--config=build.build-dir='/path with spaces'"),
            "--config=build.build-dir='/path with spaces'",
        )

    def test_encoded_flags_preserve_spaces_and_reject_stale_profiles(self):
        self.assertEqual(
            build.base_flags(
                {
                    "CARGO_ENCODED_RUSTFLAGS": "-Clink-arg=/path with spaces/lib.a\x1f-Copt-level=3",
                    "RUSTFLAGS": "ignored",
                }
            ),
            ["-Clink-arg=/path with spaces/lib.a", "-Copt-level=3"],
        )
        self.assertEqual(
            build.base_flags(
                {"CARGO_ENCODED_RUSTFLAGS": "", "RUSTFLAGS": "-Cprofile-use=ignored"}
            ),
            [],
        )
        for value in (
            "-Cprofile-use=old",
            "-C profile-generate=old",
            "-Cllvm-args=-pgo-warn-missing-function",
        ):
            with self.assertRaises(build.BuildError):
                build.base_flags({"RUSTFLAGS": value})

    def test_profile_warnings_are_fatal(self):
        build.check_profile_output("Compiling oriole\nFinished release\n")
        for text in (
            "warning: profile data may be out of date",
            "warning[E123]: example",
            "function hash mismatch",
            "counter mismatch",
            "no profile data available",
        ):
            with self.assertRaises(build.BuildError):
                build.check_profile_output(text)

    def test_output_lock_is_exclusive_and_preserves_existing_owner(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            with build.output_lock(output):
                original = (output / ".lock").read_bytes()
                with self.assertRaises(build.BuildError):
                    with build.output_lock(output):
                        self.fail("Entered a second output lock")
                self.assertEqual((output / ".lock").read_bytes(), original)
            self.assertFalse((output / ".lock").exists())

    def test_source_inventory_detects_added_and_modified_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / "crates/oriole_expat").mkdir(parents=True)
            for filename in (
                "Cargo.toml",
                "Cargo.lock",
                "crates/oriole_expat/Cargo.toml",
            ):
                (source / filename).touch()
            initial = build.source_files(source)
            (source / "crates/oriole_expat/new.rs").write_text("pub fn added() {}")
            with self.assertRaises(build.BuildError):
                build.require_unchanged(initial, build.source_files(source), "Source")
            initial = build.source_files(source)
            (source / "Cargo.lock").write_text("changed")
            with self.assertRaises(build.BuildError):
                build.require_unchanged(initial, build.source_files(source), "Source")

    def test_failed_command_retains_output_and_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            run = build.Run(root, root, 5)
            with self.assertRaises(build.BuildError):
                run.command(
                    "failure",
                    [
                        sys.executable,
                        "-c",
                        "print('failure evidence', flush=True); raise SystemExit(7)",
                    ],
                    os.environ.copy(),
                )
            saved = json.loads((root / "manifest.json").read_text())
            command = saved["commands"][0]
            self.assertEqual(saved["status"], "incomplete")
            self.assertEqual(command["returncode"], 7)
            self.assertIn("failure evidence", (root / command["log"]).read_text())
            self.assertEqual(command["log_sha256"], build.digest(root / command["log"]))

    def test_timeout_stops_command_before_releasing_lock(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            run = build.Run(root, root, 0.1)
            with self.assertRaises(build.subprocess.TimeoutExpired):
                run.command(
                    "timeout",
                    [
                        sys.executable,
                        "-c",
                        "import time; print('started', flush=True); time.sleep(20)",
                    ],
                    os.environ.copy(),
                )
            command = json.loads((root / "manifest.json").read_text())["commands"][0]
            self.assertIn("TimeoutExpired", command["failure"])
            self.assertIn("started", (root / command["log"]).read_text())

    def test_launch_failure_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            run = build.Run(root, root, 1)
            with self.assertRaises(FileNotFoundError):
                run.command(
                    "missing", [str(root / "not-an-executable")], os.environ.copy()
                )
            command = json.loads((root / "manifest.json").read_text())["commands"][0]
            self.assertIn("FileNotFoundError", command["failure"])
            self.assertEqual(command["status"], "incomplete")

    def test_origin_rejects_an_unrelated_library(self):
        with self.assertRaisesRegex(RuntimeError, "loaded from"):
            verify_origin(ctypes.CDLL(None).malloc, Path(__file__))

    def test_training_child_ignores_site_and_user_import_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "injected"
            (root / "sitecustomize.py").write_text(
                f"from pathlib import Path; Path({str(marker)!r}).touch(); raise SystemExit(99)"
            )
            # The trainer adds its own directory explicitly; an unrelated corpus
            # module in PYTHONPATH must never replace its generated fixtures.
            (root / "corpus.py").write_text("raise RuntimeError('wrong corpus module')")
            result = subprocess.run(
                [
                    sys.executable,
                    "-I",
                    "-S",
                    str(build.SCRIPT_DIRECTORY / "train.py"),
                    "--help",
                ],
                env=os.environ | {"PYTHONPATH": str(root)},
                capture_output=True,
                text=True,
                timeout=5,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(marker.exists())

    def test_failed_parent_stops_background_child(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "late-write"
            child = (
                "import time; from pathlib import Path; print('ready', flush=True); time.sleep(0.3); Path("
                + repr(str(marker))
                + ").touch()"
            )
            parent = (
                "import subprocess,sys; child=subprocess.Popen([sys.executable,'-I','-S','-c',"
                + repr(child)
                + "],stdout=subprocess.PIPE); child.stdout.readline(); raise SystemExit(7)"
            )
            run = build.Run(root, root, 5)
            with self.assertRaises(build.BuildError):
                run.command(
                    "background-failure",
                    [sys.executable, "-I", "-S", "-c", parent],
                    os.environ.copy(),
                )
            time.sleep(0.5)
            self.assertFalse(
                marker.exists(), "Failed command left a child writing after it returned"
            )


if __name__ == "__main__":
    unittest.main()
