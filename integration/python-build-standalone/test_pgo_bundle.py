"""Exercise the bundle handoff using saved-build fixtures, without a compiler."""

import json
import shlex
import tempfile
import unittest
from pathlib import Path
from typing import Any

import pgo_bundle
from pgo import build


class PgoBundleTests(unittest.TestCase):
    def setUp(self):
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
        options += (
            [f"profile-generate={self.run_directory / 'raw-profiles'}"]
            if phase == "generate"
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
        return pgo_bundle.verify(self.output, self.source, self.env, None, [])

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


if __name__ == "__main__":
    unittest.main()
