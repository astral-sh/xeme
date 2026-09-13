# /// script
# requires-python = ">=3.11"
# dependencies = []
# [tool.uv]
# no-build = true
# ///
"""Build Xeme with a fresh, generated-input PGO profile (offline, opt-in)."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import tomllib
from collections.abc import Iterator
from contextlib import contextmanager, suppress
from pathlib import Path
from typing import Any

# Support Python safe-path mode while importing only this tool's own module.
sys.path.insert(0, str(Path(__file__).resolve().parent))
# Support direct script execution without importing unrelated corpus modules.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from pgo.corpus import generate

SCRIPT_DIRECTORY = Path(__file__).resolve().parent
# Deliberately exclude registry credentials and unrelated application secrets.
BUILD_ENVIRONMENT = (
    "PATH",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "CARGO_BUILD_BUILD_DIR",
    "CARGO_BUILD_JOBS",
    "CARGO_TERM_COLOR",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CC",
    "CXX",
    "AR",
    "CFLAGS",
    "CXXFLAGS",
    "LDFLAGS",
    "MACOSX_DEPLOYMENT_TARGET",
    "SDKROOT",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
)


def cargo_option(value: str) -> str:
    """Accept one global option without allowing a replacement subcommand."""
    if not value.startswith("-") or value in ("-", "--", "-Z"):
        raise argparse.ArgumentTypeError(
            "expected a global Cargo option; attach its value, e.g. -Zohm-defaults=no"
        )
    if value.startswith("-C") or (
        value.startswith("-") and not value.startswith(("--", "-Z")) and "C" in value
    ):
        raise argparse.ArgumentTypeError("Cargo directory changes are not supported")
    if value.startswith("--config"):
        try:
            if not value.startswith("--config="):
                raise ValueError("missing inline configuration")
            config = tomllib.loads(value.removeprefix("--config="))
            if not config or "include" in config:
                raise ValueError("additional configuration files are unsupported")
        except (ValueError, tomllib.TOMLDecodeError) as error:
            raise argparse.ArgumentTypeError(
                "Cargo configuration must be inline --config=KEY=VALUE, without includes"
            ) from error
    return value


class BuildError(Exception):
    """A failed or unverifiable build; its run manifest must remain incomplete."""


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path: Path, data: Any) -> None:
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
    temporary.replace(path)


def files_below(directory: Path) -> dict[str, str]:
    return {
        str(path.relative_to(directory)): digest(path)
        for path in sorted(directory.rglob("*"))
        if path.is_file() and "__pycache__" not in path.parts
    }


def source_files(source: Path) -> dict[str, str]:
    """Record build inputs, including optional local Cargo configuration."""
    result = {}
    for name in ("Cargo.toml", "Cargo.lock"):
        result[name] = digest(source / name)
    for name in (
        "rust-toolchain",
        "rust-toolchain.toml",
        "build.rs",
        ".cargo/config",
        ".cargo/config.toml",
    ):
        if (source / name).is_file():
            result[name] = digest(source / name)
    for name in ("crates", "include"):
        result.update(
            {
                f"{name}/{relative}": value
                for relative, value in files_below(source / name).items()
            }
        )
    if "crates/xeme_expat/Cargo.toml" not in result:
        raise BuildError(
            "--source must be an Xeme workspace with crates/xeme_expat"
        )
    return result


def cargo_configs(source: Path, env: dict[str, str]) -> dict[str, str]:
    directories = [source, *source.parents]
    cargo_home = Path(env.get("CARGO_HOME", str(Path.home() / ".cargo"))).expanduser()
    candidates = [
        directory / ".cargo" / name
        for directory in directories
        for name in ("config", "config.toml")
    ]
    candidates += [cargo_home / name for name in ("config", "config.toml")]
    return {str(path.resolve()): digest(path) for path in candidates if path.is_file()}


def llvm_version(text: str) -> str:
    match = re.search(r"LLVM version:?\s+(\d+\.\d+\.\d+)", text)
    if not match:
        raise BuildError("Cannot identify the LLVM major.minor.patch version")
    return match[1]


def base_flags(env: dict[str, str]) -> list[str]:
    if "CARGO_ENCODED_RUSTFLAGS" in env:
        encoded = env["CARGO_ENCODED_RUSTFLAGS"]
        flags = encoded.split("\x1f") if encoded else []
    else:
        flags = env.get("RUSTFLAGS", "").split()
    if any(
        "profile-use" in flag or "profile-generate" in flag or "pgo-" in flag
        for flag in flags
    ):
        raise BuildError(
            "Remove inherited PGO flags; every run creates its own profile"
        )
    return flags


@contextmanager
def output_lock(output: Path) -> Iterator[None]:
    output.mkdir(parents=True, exist_ok=True)
    lock = output / ".lock"
    try:
        with lock.open("x") as stream:
            stream.write(f"{os.getpid()}\n")
    except FileExistsError as exc:
        raise BuildError(
            f"Output is locked: {lock}; do not run overlapping builds"
        ) from exc
    try:
        yield
    finally:
        lock.unlink()


class Run:
    def __init__(self, directory: Path, source: Path, timeout: float):
        self.directory = directory
        self.source = source
        self.timeout = timeout
        self.manifest: dict[str, Any] = {
            "schema": 1,
            "status": "incomplete",
            "source": str(source),
            "run": str(directory),
            "commands": [],
            "started_unix": time.time(),
            "invocation": [sys.executable, *sys.argv],
            "invocation_cwd": str(Path.cwd()),
            "python_version": sys.version,
            "cpu_affinity": sorted(os.sched_getaffinity(0))
            if hasattr(os, "sched_getaffinity")
            else None,
        }
        self.save()

    def save(self) -> None:
        write_json(self.directory / "manifest.json", self.manifest)

    def command(self, label: str, arguments: list[str], env: dict[str, str]) -> str:
        log = self.directory / f"{len(self.manifest['commands']):02d}-{label}.log"
        record: dict[str, Any] = {
            "label": label,
            "argv": arguments,
            "cwd": str(self.source),
            "log": log.name,
            "status": "incomplete",
            "timeout_seconds": self.timeout,
            "environment": {
                key: env[key]
                for key in (
                    *BUILD_ENVIRONMENT,
                    "RUSTC",
                    "CARGO_TARGET_DIR",
                    "CARGO_INCREMENTAL",
                    "RUSTUP_AUTO_INSTALL",
                    "LLVM_PROFILE_FILE",
                )
                if key in env
            },
        }
        self.manifest["commands"].append(record)
        self.save()
        process = None
        started = time.monotonic()
        try:
            with log.open("wb") as stream:
                process = subprocess.Popen(
                    arguments,
                    cwd=self.source,
                    env=env,
                    stdout=stream,
                    stderr=subprocess.STDOUT,
                    start_new_session=True,
                )
                try:
                    record["returncode"] = process.wait(timeout=self.timeout)
                except BaseException:
                    # Cargo can have compiler children. Do not release the output lock
                    # until the entire command's process group has been stopped.
                    with suppress(ProcessLookupError):
                        os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                    raise
            if record["returncode"] != 0:
                with suppress(ProcessLookupError):
                    os.killpg(process.pid, signal.SIGKILL)
                raise BuildError(f"{label} failed; see {log}")
            record["status"] = "passed"
        except BaseException as exc:
            record["failure"] = repr(exc)
            raise
        finally:
            record["returncode"] = process.returncode if process is not None else None
            record["elapsed_seconds"] = time.monotonic() - started
            if log.exists():
                record["log_sha256"] = digest(log)
            self.save()
        return log.read_text(errors="replace")


def check_profile_output(text: str) -> None:
    # Missing-function warnings are enabled explicitly. Be conservative: a new
    # compiler warning in this optional release pipeline needs investigation.
    if re.search(
        r"(?im)^\s*warning[:\[]|\b(?:hash mismatch|counter mismatch|malformed instrumentation profile|no profile data available)\b",
        text,
    ):
        raise BuildError(
            "Profile-use build emitted a warning or profile mismatch; inspect its raw log"
        )


def native_static_libraries(text: str) -> list[str]:
    """Read the exact use build's native dependencies for the Linux PBS bundle."""
    matches = re.findall(r"native-static-libs:\s*([^\n]+)", text)
    libraries = matches[-1].split() if matches else []
    if not libraries or any(
        re.fullmatch(r"-l[A-Za-z0-9_]+", library) is None for library in libraries
    ):
        raise BuildError("Missing or unsupported native-static-libs in the use build")
    return libraries


def require_unchanged(
    before: dict[str, str], after: dict[str, str], label: str
) -> None:
    if before != after:
        raise BuildError(
            f"{label} changed during this run; discard the profile and retry"
        )


def train(
    run: Run, library: Path, inputs: Path, label: str, env: dict[str, str]
) -> dict:
    output = run.directory / f"{label}-training.json"
    run.command(
        label,
        [
            sys.executable,
            "-I",
            "-S",
            str(SCRIPT_DIRECTORY / "train.py"),
            "--library",
            str(library),
            "--inputs",
            str(inputs),
            "--report",
            str(output),
        ],
        env,
    )
    report = json.loads(output.read_text())
    if report.get("status") != "passed" or len(report.get("rows", [])) != 288:
        raise BuildError(f"Incomplete training: {output}")
    if report["library_sha256"] != digest(library) or report["inputs_sha256"] != digest(
        inputs / "manifest.json"
    ):
        raise BuildError("Training report input identity mismatch")
    if report.get("xml_parse_origin") != {
        "path": str(library.resolve()),
        "sha256": digest(library),
    }:
        raise BuildError("Training called XML_Parse from an unexpected library")
    return report


def execute(args: argparse.Namespace, run: Run) -> None:
    env = os.environ.copy()
    for key in (
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    ):
        if env.get(key):
            raise BuildError(
                f"Unset {key}; this tool verifies and directly selects its compiler"
            )
    flags = base_flags(env)
    env.pop("RUSTFLAGS", None)
    env.pop("LLVM_PROFILE_FILE", None)
    env.update(
        CARGO_INCREMENTAL="0",
        RUSTUP_AUTO_INSTALL="0",
        CARGO_TERM_COLOR="never",
        RUSTC_WRAPPER="",
        RUSTC_WORKSPACE_WRAPPER="",
    )
    cargo = shutil.which("cargo")
    rustc = shutil.which("rustc")
    if cargo is None or rustc is None:
        raise BuildError("cargo and rustc must already be installed on PATH")
    selection = [f"+{args.toolchain}"] if args.toolchain else []
    cargo_selection = [*selection, *args.cargo_arg]
    source_before = source_files(run.source)
    configs_before = cargo_configs(run.source, env)
    scripts_before = files_below(SCRIPT_DIRECTORY)
    run.manifest.update(
        source_sha256=source_before,
        cargo_config_sha256=configs_before,
        script_sha256=scripts_before,
        base_rustflags=flags,
        environment_scope="Child commands inherit the environment with the recorded overrides. Only an allowlist is recorded to avoid storing credentials. Training additionally uses Python -I -S and verifies XML_Parse origin.",
        toolchain=args.toolchain,
        cargo_args=args.cargo_arg,
        environment={
            key: os.environ[key] for key in BUILD_ENVIRONMENT if key in os.environ
        },
    )
    run.save()
    rust_version = run.command("rustc-version", [rustc, *selection, "-vV"], env)
    cargo_version = run.command("cargo-version", [cargo, *cargo_selection, "-Vv"], env)
    sysroot = Path(
        run.command(
            "rustc-sysroot", [rustc, *selection, "--print", "sysroot"], env
        ).strip()
    )
    compiler = sysroot / "bin" / "rustc"
    profiler = args.llvm_profdata.resolve(strict=True)
    profiler_version = run.command(
        "profdata-version", [str(profiler), "--version"], env
    )
    if llvm_version(rust_version) != llvm_version(profiler_version):
        raise BuildError(
            "rustc and llvm-profdata must use the same LLVM major.minor.patch"
        )
    host_match = re.search(r"(?m)^host: (\S+)$", rust_version)
    if host_match is None:
        raise BuildError("Cannot identify rustc's host target")
    host = host_match[1]
    tools = [
        Path(cargo).resolve(),
        Path(rustc).resolve(),
        compiler,
        profiler,
        Path(sys.executable).resolve(),
        sysroot / "bin" / "cargo",
    ]
    # LLVM is dynamically linked on common Rust distributions. Retain those
    # compiler/profiler dependency identities alongside the executable hashes.
    tools += list((sysroot / "lib").glob("libLLVM*"))
    tools += list((profiler.parent.parent / "lib").glob("libLLVM*"))
    tool_hashes = {
        str(path.resolve()): digest(path) for path in tools if path.is_file()
    }
    run.manifest.update(
        tools_sha256=tool_hashes,
        rustc_version=rust_version,
        cargo_version=cargo_version,
        profdata_version=profiler_version,
        host=host,
    )
    env["RUSTC"] = str(compiler)
    inputs = run.directory / "inputs"
    generate(inputs)
    inputs_before = files_below(inputs)
    run.manifest["inputs_sha256"] = inputs_before
    raw = run.directory / "raw-profiles"
    raw.mkdir()
    profile = run.directory / "merged.profdata"
    libraries = {}
    reports = {}
    profile_hashes: dict[str, str] = {}
    for phase in ("generate", "use"):
        require_unchanged(source_before, source_files(run.source), "Source")
        require_unchanged(
            configs_before, cargo_configs(run.source, env), "Cargo configuration"
        )
        require_unchanged(scripts_before, files_below(SCRIPT_DIRECTORY), "Tool scripts")
        require_unchanged(inputs_before, files_below(inputs), "Training inputs")
        target = args.output / "targets" / phase
        phase_env = env | {
            "CARGO_TARGET_DIR": str(target),
            "CARGO_ENCODED_RUSTFLAGS": "\x1f".join(
                flags
                + (
                    [f"-Cprofile-generate={raw}"]
                    if phase == "generate"
                    else [
                        f"-Cprofile-use={profile}",
                        "-Cllvm-args=-pgo-warn-missing-function",
                    ]
                )
            ),
        }
        log = run.command(
            f"build-{phase}",
            [
                cargo,
                *cargo_selection,
                "rustc",
                "--release",
                "--locked",
                "--offline",
                "--target",
                host,
                "-p",
                "xeme_expat",
                "--lib",
                "--crate-type",
                "cdylib,staticlib",
                "--verbose",
                *(
                    ["--", "--print=native-static-libs"]
                    if phase == "use" and getattr(args, "native_static_libs", False)
                    else []
                ),
            ],
            phase_env,
        )
        if phase == "use":
            check_profile_output(log)
            if getattr(args, "native_static_libs", False):
                run.manifest["native_static_libraries"] = native_static_libraries(log)
        artifact_directory = run.directory / phase
        artifact_directory.mkdir()
        shared = (
            "libxeme_expat.dylib"
            if sys.platform == "darwin"
            else "libxeme_expat.so"
        )
        for name in (shared, "libxeme_expat.a"):
            path = artifact_directory / name
            shutil.copy2(target / host / "release" / name, path)
            libraries[str(path.relative_to(run.directory))] = digest(path)
        run.manifest["libraries_sha256"] = libraries.copy()
        run.save()
        training_env = env.copy()
        if phase == "generate":
            if list(raw.iterdir()):
                raise BuildError("Unexpected raw profiles before training")
            training_env["LLVM_PROFILE_FILE"] = str(raw / "xeme-%m-%p.profraw")
        reports[phase] = train(
            run, artifact_directory / shared, inputs, phase, training_env
        )
        if phase == "generate":
            profiles = sorted(raw.glob("*.profraw"))
            if (
                not profiles
                or any(
                    path.is_symlink() or path.stat().st_size == 0 for path in profiles
                )
                or set(profiles) != set(raw.iterdir())
            ):
                raise BuildError("Training produced no valid isolated raw profiles")
            run.command(
                "profile-merge",
                [
                    str(profiler),
                    "merge",
                    "--num-threads=1",
                    "--failure-mode=any",
                    "-o",
                    str(profile),
                    *map(str, profiles),
                ],
                env,
            )
            run.command(
                "profile-show",
                [str(profiler), "show", "--all-functions", "--counts", str(profile)],
                env,
            )
            profile_hashes = files_below(raw) | {"merged.profdata": digest(profile)}
            run.manifest["profiles_sha256"] = profile_hashes
            run.save()
    if reports["generate"]["rows"] != reports["use"]["rows"]:
        raise BuildError(
            "Optimized generated-input callbacks differ from the instrumented build"
        )
    require_unchanged(source_before, source_files(run.source), "Source")
    require_unchanged(
        configs_before, cargo_configs(run.source, env), "Cargo configuration"
    )
    require_unchanged(scripts_before, files_below(SCRIPT_DIRECTORY), "Tool scripts")
    require_unchanged(inputs_before, files_below(inputs), "Training inputs")
    require_unchanged(
        tool_hashes,
        {path: digest(Path(path)) for path in tool_hashes},
        "Compiler/profiler tools",
    )
    require_unchanged(
        libraries,
        {path: digest(run.directory / path) for path in libraries},
        "Libraries",
    )
    require_unchanged(
        profile_hashes,
        files_below(raw) | {"merged.profdata": digest(profile)},
        "Profile",
    )
    run.manifest.update(
        status="passed",
        libraries_sha256=libraries,
        training_sha256={
            phase: digest(run.directory / f"{phase}-training.json") for phase in reports
        },
        compared_parses=288,
        completed_unix=time.time(),
    )
    run.save()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        required=True,
        type=Path,
        help="Xeme workspace; dependencies must be cached",
    )
    parser.add_argument(
        "--output",
        required=True,
        type=Path,
        help="Build/evidence directory outside the source tree",
    )
    parser.add_argument(
        "--llvm-profdata",
        required=True,
        type=Path,
        help="Installed tool matching rustc's LLVM version",
    )
    parser.add_argument(
        "--toolchain", help="Installed rustup toolchain; e.g. stable or ohm"
    )
    parser.add_argument(
        "--cargo-arg",
        action="append",
        default=[],
        type=cargo_option,
        help="Repeatable global Cargo option, e.g. --cargo-arg=-Zohm-defaults=no",
    )
    parser.add_argument(
        "--native-static-libs",
        action="store_true",
        help="Capture the use build's native linker libraries for a Linux PBS bundle",
    )
    parser.add_argument(
        "--command-timeout",
        type=float,
        default=1800,
        help="Per-command seconds (default: 1800)",
    )
    args = parser.parse_args()
    args.source = args.source.resolve(strict=True)
    args.output = args.output.resolve()
    if sys.platform not in ("linux", "darwin"):
        parser.error("This workflow currently supports Linux and macOS hosts")
    if args.output.is_relative_to(args.source) or args.source.is_relative_to(
        args.output
    ):
        parser.error(
            "--source and --output must be separate, non-overlapping directories"
        )
    if not math.isfinite(args.command_timeout) or args.command_timeout <= 0:
        parser.error("--command-timeout must be positive")
    try:
        with output_lock(args.output):
            runs = args.output / "runs"
            runs.mkdir(exist_ok=True)
            run = Run(
                Path(tempfile.mkdtemp(prefix="run-", dir=runs)),
                args.source,
                args.command_timeout,
            )
            try:
                execute(args, run)
            except BaseException as exc:
                run.manifest.update(failure=repr(exc), completed_unix=time.time())
                run.save()
                raise
            write_json(
                args.output / "latest.json",
                {
                    "manifest": str(run.directory / "manifest.json"),
                    "sha256": digest(run.directory / "manifest.json"),
                },
            )
            print(f"PGO build passed: {run.directory / 'manifest.json'}")
        return 0
    except (BuildError, OSError, subprocess.SubprocessError) as exc:
        print(f"PGO build failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
