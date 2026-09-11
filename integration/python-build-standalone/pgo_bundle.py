"""Join the existing fresh PGO build's provenance to a Linux PBS bundle."""

from __future__ import annotations

import json
import shlex
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
from pgo import build

TARGET = "x86_64-unknown-linux-gnu"
FLAGS = ["-C", "relocation-model=pic", "-C", "panic=unwind"]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise build.BuildError(message)


def verify_vectors(text: str, phase: str, run: Path, compiler: Path) -> None:
    """Check the effective workspace options, including Cargo config overrides."""
    crates = []
    for line in text.splitlines():
        if "Running `" not in line or "--crate-name " not in line:
            continue
        arguments = shlex.split(line.split("Running `", 1)[1].rsplit("`", 1)[0])
        name = arguments[arguments.index("--crate-name") + 1]
        if name not in ("oriole_storage", "oriole", "oriole_expat"):
            continue
        crates.append(name)
        require(Path(arguments[0]).resolve() == compiler, "PGO compiler path mismatch")
        require(
            "--target" in arguments
            and arguments[arguments.index("--target") + 1] == TARGET,
            "PGO compiler target mismatch",
        )
        require(
            not any(arg.startswith("-Z") for arg in arguments),
            "Experimental rustc flags in PBS PGO build",
        )
        options = [
            arguments[index + 1] if arg == "-C" else arg[2:]
            for index, arg in enumerate(arguments)
            if arg == "-C" or arg.startswith("-C")
        ]
        options = [
            arg
            for arg in options
            if not arg.startswith(("metadata=", "extra-filename="))
        ]
        expected = [
            "opt-level=3",
            "lto=thin" if name == "oriole_expat" else "linker-plugin-lto",
            "codegen-units=1",
            "strip=debuginfo",
            "relocation-model=pic",
            "panic=unwind",
        ]
        expected += (
            [f"profile-generate={run / 'raw-profiles'}"]
            if phase == "generate"
            else [
                f"profile-use={run / 'merged.profdata'}",
                "llvm-args=-pgo-warn-missing-function",
            ]
        )
        require(
            options == expected,
            f"Unexpected {phase} compiler flags for {name}: {options!r}",
        )
        kinds = [
            arguments[index + 1]
            for index, arg in enumerate(arguments)
            if arg == "--crate-type"
        ]
        require(
            kinds == (["cdylib", "staticlib"] if name == "oriole_expat" else ["lib"]),
            "PGO crate-type mismatch",
        )
    require(
        sorted(crates) == ["oriole", "oriole_expat", "oriole_storage"],
        "Missing fresh workspace PGO compilations",
    )


def verify(
    output: Path,
    source: Path,
    env: dict[str, str],
    toolchain: str | None,
    cargo_args: list[str],
) -> tuple[Path, list[str], dict, list[str]]:
    """Validate a freshly completed run before selecting its exact static archive."""
    latest = json.loads((output / "latest.json").read_text())
    path = Path(latest["manifest"]).resolve(strict=True)
    require(
        path.parent.parent == output.resolve() / "runs"
        and path.name == "manifest.json",
        "PGO manifest is outside this bundle's fresh output",
    )
    require(build.digest(path) == latest["sha256"], "PGO manifest checksum mismatch")
    manifest = json.loads(path.read_text())
    run = path.parent
    require(
        manifest["schema"] == 1 and manifest["status"] == "passed",
        "PGO run did not pass",
    )
    require(
        manifest["source"] == str(source) and manifest["run"] == str(run),
        "PGO source/run path mismatch",
    )
    require(
        manifest["host"] == TARGET and manifest["toolchain"] == toolchain,
        "PGO target/toolchain mismatch",
    )
    require(
        manifest["cargo_args"] == cargo_args and manifest["base_rustflags"] == FLAGS,
        "PGO requested build flags mismatch",
    )
    build.require_unchanged(
        manifest["source_sha256"], build.source_files(source), "PGO source"
    )
    build.require_unchanged(
        manifest["cargo_config_sha256"],
        build.cargo_configs(source, env),
        "PGO Cargo configuration",
    )
    build.require_unchanged(
        manifest["script_sha256"],
        build.files_below(build.SCRIPT_DIRECTORY),
        "PGO pipeline",
    )
    for name, digest in manifest["tools_sha256"].items():
        require(build.digest(Path(name)) == digest, f"PGO tool changed: {name}")
    build.require_unchanged(
        manifest["inputs_sha256"],
        build.files_below(run / "inputs"),
        "PGO training input",
    )
    build.require_unchanged(
        manifest["profiles_sha256"],
        build.files_below(run / "raw-profiles")
        | {"merged.profdata": build.digest(run / "merged.profdata")},
        "PGO profile",
    )
    for name, digest in manifest["libraries_sha256"].items():
        require(build.digest(run / name) == digest, f"PGO library changed: {name}")
    commands = {command["label"]: command for command in manifest["commands"]}
    require(len(commands) == len(manifest["commands"]), "Duplicate PGO command records")
    for command in commands.values():
        require(
            command["status"] == "passed" and command["returncode"] == 0,
            "PGO command did not pass",
        )
        require(
            build.digest(run / command["log"]) == command["log_sha256"],
            "PGO command log changed",
        )
    compiler = (
        Path((run / commands["rustc-sysroot"]["log"]).read_text().strip()) / "bin/rustc"
    )
    compiler = compiler.resolve(strict=True)
    require(str(compiler) in manifest["tools_sha256"], "Untracked PGO compiler")
    for phase in ("generate", "use"):
        verify_vectors(
            (run / commands[f"build-{phase}"]["log"]).read_text(), phase, run, compiler
        )
    use_log = (run / commands["build-use"]["log"]).read_text()
    build.check_profile_output(use_log)
    native = build.native_static_libraries(use_log)
    require(
        native == manifest["native_static_libraries"],
        "PGO native linker metadata mismatch",
    )
    reports = {}
    for phase in ("generate", "use"):
        report_path = run / f"{phase}-training.json"
        require(
            build.digest(report_path) == manifest["training_sha256"][phase],
            "PGO training report changed",
        )
        report = json.loads(report_path.read_text())
        library = run / phase / "liboriole_expat.so"
        require(
            report["status"] == "passed" and len(report["rows"]) == 288,
            "Incomplete PGO training",
        )
        require(
            report["xml_parse_origin"]
            == {"path": str(library), "sha256": build.digest(library)},
            "PGO training library origin mismatch",
        )
        require(
            report["library_sha256"] == build.digest(library)
            and report["inputs_sha256"] == build.digest(run / "inputs/manifest.json"),
            "PGO training input identity mismatch",
        )
        reports[phase] = report
    require(
        reports["generate"]["rows"] == reports["use"]["rows"],
        "PGO generated callbacks differ",
    )
    archive = run / "use/liboriole_expat.a"
    require(
        build.digest(archive) == manifest["libraries_sha256"]["use/liboriole_expat.a"],
        "PGO archive identity mismatch",
    )
    return (
        archive,
        native,
        {
            "manifest_sha256": build.digest(path),
            "compiler": str(compiler),
            "manifest": manifest,
        },
        commands["build-use"]["argv"],
    )
