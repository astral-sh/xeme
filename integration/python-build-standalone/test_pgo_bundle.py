"""Exercise the bundle handoff using saved-build fixtures, without a compiler."""

import json
import os
import shlex
import subprocess
import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest.mock import patch

import pbs_target
import pgo_bundle
from pgo import build


class PgoBundleTests(unittest.TestCase):
    def setUp(self):
        self.target_cpu = None
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / "source"
        self.output = self.root / "output"
        self.run_directory = self.output / "runs/run-fixture"
        self.run_directory.mkdir(parents=True)
        self.env = {"CARGO_HOME": str(self.root / "cargo")}
        self.compiler = self.root / "sysroot/bin/rustc"
        self.compiler.parent.mkdir(parents=True)
        self.compiler.write_text("fixture compiler; never executed")
        for name in ("Cargo.toml", "Cargo.lock", "crates/oriole_expat/Cargo.toml"):
            path = self.source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fixture source")
        config = Path(self.env["CARGO_HOME"]) / "config.toml"
        config.parent.mkdir()
        config.write_text("[net]\noffline = true\n")
        for name in (
            "inputs/manifest.json",
            "inputs/text.xml",
            "raw-profiles/one.profraw",
            "merged.profdata",
            "generate/liboriole_expat.so",
            "generate/liboriole_expat.a",
            "use/liboriole_expat.so",
            "use/liboriole_expat.a",
        ):
            path = self.run_directory / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name)
        self.commands = []
        self.add_command("rustc-sysroot", str(self.compiler.parent.parent))
        for phase in ("generate", "use"):
            text = "\n".join(
                self.vector(phase, name)
                for name in ("oriole_storage", "oriole", "oriole_expat")
            )
            if phase == "use":
                text += "\nnote: native-static-libs: -lgcc_s -lpthread -lc\n"
            self.add_command(f"build-{phase}", text)
            library = self.run_directory / phase / "liboriole_expat.so"
            report = {
                "status": "passed",
                "rows": [
                    {"fixture": index, "digest": "matching callback digest"}
                    for index in range(288)
                ],
                "xml_parse_origin": {
                    "path": str(library),
                    "sha256": build.digest(library),
                },
                "library_sha256": build.digest(library),
                "inputs_sha256": build.digest(
                    self.run_directory / "inputs/manifest.json"
                ),
            }
            build.write_json(self.run_directory / f"{phase}-training.json", report)
        self.manifest: dict[str, Any] = {
            "schema": 1,
            "status": "passed",
            "source": str(self.source),
            "run": str(self.run_directory),
            "host": pgo_bundle.TARGET,
            "toolchain": None,
            "cargo_args": [],
            "base_rustflags": pgo_bundle.FLAGS,
            "source_sha256": build.source_files(self.source),
            "cargo_config_sha256": build.cargo_configs(self.source, self.env),
            "script_sha256": build.files_below(build.SCRIPT_DIRECTORY),
            "tools_sha256": {str(self.compiler): build.digest(self.compiler)},
            "inputs_sha256": build.files_below(self.run_directory / "inputs"),
            "profiles_sha256": build.files_below(self.run_directory / "raw-profiles")
            | {"merged.profdata": build.digest(self.run_directory / "merged.profdata")},
            "libraries_sha256": {
                f"{phase}/{name}": build.digest(self.run_directory / phase / name)
                for phase in ("generate", "use")
                for name in ("liboriole_expat.so", "liboriole_expat.a")
            },
            "commands": self.commands,
            "native_static_libraries": ["-lgcc_s", "-lpthread", "-lc"],
            "training_sha256": {
                phase: build.digest(self.run_directory / f"{phase}-training.json")
                for phase in ("generate", "use")
            },
        }
        self.save()

    def add_command(self, label, text):
        path = self.run_directory / f"{label}.log"
        path.write_text(text)
        self.commands.append(
            {
                "label": label,
                "argv": ["fixture command", label],
                "log": path.name,
                "log_sha256": build.digest(path),
                "status": "passed",
                "returncode": 0,
            }
        )

    def vector(self, phase, name):
        arguments = [
            str(self.compiler),
            "--crate-name",
            name,
            "--target",
            pgo_bundle.TARGET,
        ]
        for kind in ["cdylib", "staticlib"] if name == "oriole_expat" else ["lib"]:
            arguments += ["--crate-type", kind]
        options = [
            "opt-level=3",
            "lto=thin" if name == "oriole_expat" else "linker-plugin-lto",
            "codegen-units=1",
            "metadata=fixture",
            "extra-filename=-fixture",
            "strip=debuginfo",
            "relocation-model=pic",
            "panic=unwind",
        ]
        if self.target_cpu:
            options += [f"target-cpu={self.target_cpu}"]
        options += (
            [f"profile-generate={self.run_directory / 'raw-profiles'}"]
            if phase == "generate"
            else []
            if phase == "normal"
            else [
                f"profile-use={self.run_directory / 'merged.profdata'}",
                "llvm-args=-pgo-warn-missing-function",
            ]
        )
        for option in options:
            arguments += ["-C", option]
        return f"Running `{shlex.join(arguments)}`"

    def save(self):
        path = self.run_directory / "manifest.json"
        build.write_json(path, self.manifest)
        build.write_json(
            self.output / "latest.json",
            {"manifest": str(path), "sha256": build.digest(path)},
        )

    def verify(self):
        return pgo_bundle.verify(
            self.output, self.source, self.env, None, [], self.target_cpu
        )

    def test_v3_requires_matching_requested_and_both_effective_phase_flags(self):
        self.target_cpu = "x86-64-v3"
        self.manifest["base_rustflags"] = pgo_bundle.flags(self.target_cpu)
        for phase in ("generate", "use"):
            text = "\n".join(
                self.vector(phase, name)
                for name in ("oriole_storage", "oriole", "oriole_expat")
            )
            if phase == "use":
                text += "\nnote: native-static-libs: -lgcc_s -lpthread -lc\n"
            command = next(c for c in self.commands if c["label"] == f"build-{phase}")
            log = self.run_directory / command["log"]
            log.write_text(text)
            command["log_sha256"] = build.digest(log)
        self.save()
        archive, _, provenance, _ = self.verify()
        self.assertEqual(archive, self.run_directory / "use/liboriole_expat.a")
        self.assertEqual(set(provenance["compiler_vectors"]), {"generate", "use"})
        with self.assertRaises(build.BuildError):
            pgo_bundle.verify(self.output, self.source, self.env, None, [])
        for phase in ("generate", "use"):
            command = next(c for c in self.commands if c["label"] == f"build-{phase}")
            log = self.run_directory / command["log"]
            original = log.read_text()
            log.write_text(
                original.replace("target-cpu=x86-64-v3", "target-cpu=native")
            )
            command["log_sha256"] = build.digest(log)
            self.save()
            with self.assertRaises(build.BuildError):
                self.verify()
            log.write_text(original)
            command["log_sha256"] = build.digest(log)
            self.save()

    def test_normal_vectors_check_generic_and_v3_without_profile_options(self):
        for cpu in (None, "x86-64-v3"):
            self.target_cpu = cpu
            text = "\n".join(
                self.vector("normal", name)
                for name in ("oriole_storage", "oriole", "oriole_expat")
            )
            self.assertEqual(
                len(
                    pgo_bundle.verify_vectors(
                        text, "normal", self.run_directory, self.compiler, cpu
                    )
                ),
                3,
            )
            for option in (
                "target-feature=+avx2",
                "target-cpu=native",
                "profile-use=/old",
                "target-cpu=x86-64-v3",
            ):
                with (
                    self.subTest(cpu=cpu, option=option),
                    self.assertRaises(build.BuildError),
                ):
                    pgo_bundle.verify_vectors(
                        text.replace("panic=unwind", f"panic=unwind -C {option}"),
                        "normal",
                        self.run_directory,
                        self.compiler,
                        cpu,
                    )

    def test_hidden_rust_overrides_are_rejected_before_build(self):
        for key in (
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_RUSTFLAGS",
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
            "CARGO_PROFILE_RELEASE_LTO",
            "RUSTC_WRAPPER",
        ):
            with self.subTest(key=key), self.assertRaises(build.BuildError):
                pgo_bundle.reject_overrides(
                    self.source, {**self.env, key: "unreviewed"}
                )
        config = Path(self.env["CARGO_HOME"]) / "config.toml"
        config.write_text(
            '[target.x86_64-unknown-linux-gnu]\nrustflags = ["-Ctarget-cpu=native"]\n'
        )
        with self.assertRaises(build.BuildError):
            pgo_bundle.reject_overrides(self.source, self.env)

    def test_selects_exact_use_archive_and_native_dependencies(self):
        archive, libraries, provenance, command = self.verify()
        self.assertEqual(archive, self.run_directory / "use/liboriole_expat.a")
        self.assertEqual(libraries, ["-lgcc_s", "-lpthread", "-lc"])
        self.assertEqual(provenance["manifest"], self.manifest)
        self.assertEqual(provenance["compiler"], str(self.compiler))
        self.assertEqual(command, self.commands[-1]["argv"])

    def test_changed_inputs_are_rejected(self):
        paths = [
            self.source / "Cargo.lock",
            Path(self.env["CARGO_HOME"]) / "config.toml",
            self.compiler,
        ]
        paths += [
            self.run_directory / name
            for name in (
                "inputs/text.xml",
                "raw-profiles/one.profraw",
                "merged.profdata",
                "use/liboriole_expat.a",
                "generate/liboriole_expat.so",
                "build-use.log",
                "use-training.json",
            )
        ]
        for path in paths:
            with self.subTest(path=path):
                original = path.read_bytes()
                path.write_bytes(original + b"changed")
                try:
                    with self.assertRaises(build.BuildError):
                        self.verify()
                finally:
                    path.write_bytes(original)

    def test_rejects_failed_or_mismatched_run(self):
        for key, value in (
            ("status", "incomplete"),
            ("host", "other-target"),
            ("toolchain", "other"),
            ("cargo_args", ["--offline"]),
            ("base_rustflags", []),
            ("native_static_libraries", ["-ldl"]),
        ):
            with self.subTest(key=key):
                original = self.manifest[key]
                self.manifest[key] = value
                self.save()
                try:
                    with self.assertRaises(build.BuildError):
                        self.verify()
                finally:
                    self.manifest[key] = original
        self.save()

    def test_rejects_manifest_outside_fresh_output(self):
        path = self.root / "old-manifest.json"
        build.write_json(path, self.manifest)
        build.write_json(
            self.output / "latest.json",
            {"manifest": str(path), "sha256": build.digest(path)},
        )
        with self.assertRaisesRegex(build.BuildError, "fresh output"):
            self.verify()

    def test_rejects_wrong_origin_and_callback_difference_even_with_new_report_hash(
        self,
    ):
        path = self.run_directory / "use-training.json"
        original = path.read_text()
        for key, value in (
            (
                "xml_parse_origin",
                {
                    "path": str(self.run_directory / "generate/liboriole_expat.so"),
                    "sha256": build.digest(
                        self.run_directory / "generate/liboriole_expat.so"
                    ),
                },
            ),
            ("rows", [{"wrong callbacks": True}] * 288),
        ):
            with self.subTest(key=key):
                report = json.loads(original)
                report[key] = value
                build.write_json(path, report)
                self.manifest["training_sha256"]["use"] = build.digest(path)
                self.save()
                with self.assertRaises(build.BuildError):
                    self.verify()

    def test_effective_flags_reject_config_overrides_and_experimental_defaults(self):
        text = (self.run_directory / "build-use.log").read_text()
        changes = (
            ("lto=thin", "lto=fat"),
            ("codegen-units=1", "codegen-units=16"),
            ("relocation-model=pic", "relocation-model=static"),
            ("panic=unwind", "panic=abort"),
            ("profile-use=", "profile-use=/stale/"),
            ("--crate-name", "-Zrelink-only --crate-name"),
        )
        for before, after in changes:
            with self.subTest(after=after), self.assertRaises(build.BuildError):
                pgo_bundle.verify_vectors(
                    text.replace(before, after),
                    "use",
                    self.run_directory,
                    self.compiler,
                )
        with self.assertRaisesRegex(build.BuildError, "fresh workspace"):
            pgo_bundle.verify_vectors(
                "Fresh oriole", "use", self.run_directory, self.compiler
            )


class TargetTests(unittest.TestCase):
    def test_generic_host_check_does_not_compile_or_execute_a_guard(self):
        with (
            patch.object(pbs_target.platform, "system", return_value="Linux"),
            patch.object(pbs_target.platform, "machine", return_value="x86_64"),
            patch.object(pbs_target.subprocess, "run") as run,
        ):
            result = pbs_target.check_host(pbs_target.RUST_TARGET)
            self.assertIsNone(result["target_cpu"])
            run.assert_not_called()

    def test_explicit_mapping_and_bundle_mismatches(self):
        self.assertIsNone(pbs_target.cpu(pbs_target.RUST_TARGET))
        for target, cpu in pbs_target.TARGETS.items():
            manifest = {
                "target": target,
                "rust_target": pbs_target.RUST_TARGET,
                "target_cpu": cpu,
            }
            pbs_target.validate(manifest, target)
            for key, value in (
                ("target", "other"),
                ("rust_target", target + "-wrong"),
                ("target_cpu", "native"),
                ("target_cpu", None if cpu else "x86-64-v3"),
            ):
                with (
                    self.subTest(target=target, key=key),
                    self.assertRaises(ValueError),
                ):
                    pbs_target.validate({**manifest, key: value}, target)
            del manifest["target_cpu"]
            with self.assertRaises(ValueError):
                pbs_target.validate(manifest, target)
        with self.assertRaises(ValueError):
            pbs_target.cpu("x86_64_v4-unknown-linux-gnu")

    def test_cpu_guard_ignores_injected_compiler_flags_and_rejects_missing_support(
        self,
    ):
        calls = []

        def run(command, **kwargs):
            calls.append((command, kwargs))
            self.assertEqual(kwargs["env"], {"PATH": "/usr/bin:/bin", "LC_ALL": "C"})
            if len(calls) == 1:
                self.assertEqual(
                    command[1:5], ["-std=c11", "-O2", "-march=x86-64", "-mtune=generic"]
                )
                Path(command[-1]).write_bytes(b"fixture guard, never executed")
                return subprocess.CompletedProcess(command, 0, "", "")
            return subprocess.CompletedProcess(command, 1, "", "missing v3 OS support")

        with (
            patch.dict(
                os.environ,
                {
                    "CC": "/untrusted",
                    "CFLAGS": "-march=native",
                    "GCC_EXEC_PREFIX": "/wrong",
                },
            ),
            patch.object(pbs_target.platform, "system", return_value="Linux"),
            patch.object(pbs_target.platform, "machine", return_value="x86_64"),
            patch.object(pbs_target.subprocess, "run", side_effect=run),
        ):
            with self.assertRaisesRegex(RuntimeError, "OS support"):
                pbs_target.check_host("x86_64_v3-unknown-linux-gnu")
        self.assertEqual(len(calls), 2)


if __name__ == "__main__":
    unittest.main()
