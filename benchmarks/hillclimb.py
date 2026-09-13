#!/usr/bin/env python3
"""Build frozen libraries and compare a candidate, its baseline and Expat on Linux."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import shlex
import shutil
import statistics
import subprocess
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from corpus import workloads

ROOT = Path(__file__).resolve().parents[1]
ENGINES = {"xeme", "baseline", "expat"}
TARGET = "x86_64-unknown-linux-gnu"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n")


def source_hashes(checkout: Path) -> dict[str, str]:
    paths = [checkout / "Cargo.toml", checkout / "Cargo.lock"]
    paths.extend(path for path in (checkout / "crates").rglob("*") if path.is_file())
    paths.extend(path for path in (checkout / "include").rglob("*") if path.is_file())
    paths.extend(path for path in (checkout / ".cargo").rglob("*") if path.is_file())
    return {str(p.relative_to(checkout)): digest(p) for p in sorted(paths)}


def workspace_compilations(log: str, checkout: Path, intermediates: Path) -> dict:
    """Reject cached workspace results or compiler inputs from another checkout."""
    crates = {"xeme_storage", "xeme", "xeme_expat"}
    compiled = {}
    for line in log.splitlines():
        if not line.strip().startswith("Running `") or not any(
            f" --crate-name {crate} " in line for crate in crates
        ):
            continue
        command = shlex.split(line.strip().removeprefix("Running `").removesuffix("`"))
        if "--crate-name" not in command:
            continue
        crate = command[command.index("--crate-name") + 1]
        if crate not in crates:
            continue
        source = checkout / "crates" / crate / "src/lib.rs"
        manifest = f"CARGO_MANIFEST_DIR={source.parent.parent}"
        out_dir = Path(command[command.index("--out-dir") + 1]).resolve()
        if (
            manifest not in command
            or not {str(source), str(source.relative_to(checkout))}.intersection(
                command
            )
            or not out_dir.is_relative_to(intermediates)
            or "--target" not in command
            or command[command.index("--target") + 1] != TARGET
        ):
            raise ValueError(f"unexpected workspace compiler input/output: {crate}")
        compiled[crate] = {
            "source": str(source),
            "out_dir": str(out_dir),
            "command": command,
        }
    if set(compiled) != crates:
        raise ValueError(
            f"missing fresh workspace compilations: {sorted(crates - compiled.keys())}"
        )
    return compiled


def library_artifact(messages: str, checkout: Path, target_dir: Path) -> Path:
    """Select the library Cargo emitted for this checkout and explicit target."""
    artifacts = []
    for line in messages.splitlines():
        message = json.loads(line)
        if (
            message.get("reason") != "compiler-artifact"
            or message["target"]["name"] != "xeme_expat"
        ):
            continue
        if (
            message["fresh"]
            or Path(message["manifest_path"]).resolve()
            != checkout / "crates/xeme_expat/Cargo.toml"
            or Path(message["target"]["src_path"]).resolve()
            != checkout / "crates/xeme_expat/src/lib.rs"
        ):
            raise ValueError("unexpected or cached C library artifact")
        artifacts.extend(
            Path(filename).resolve(strict=True)
            for filename in message["filenames"]
            if Path(filename).name == "libxeme_expat.so"
        )
    expected = target_dir / TARGET / "release/libxeme_expat.so"
    if artifacts != [expected]:
        raise ValueError(f"expected one emitted library at {expected}: {artifacts}")
    return artifacts[0]


def build(args: argparse.Namespace) -> None:
    checkout = args.checkout.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    intermediates = output / "intermediates"
    intermediates.mkdir()
    toolchain = [f"+{args.toolchain}"] if args.toolchain else []
    command = [
        "cargo",
        *toolchain,
        *(["-Zohm-defaults=no"] if args.toolchain == "ohm" else []),
        "rustc",
        "--locked",
        "--release",
        "--target",
        TARGET,
        "--manifest-path",
        str(checkout / "Cargo.toml"),
        "--target-dir",
        str(args.target_dir.resolve()),
        "--config",
        "profile.release.opt-level=3",
        "--config",
        'profile.release.lto="thin"',
        "--config",
        "profile.release.codegen-units=1",
        "-p",
        "xeme_expat",
        "--crate-type",
        "cdylib,staticlib",
        "--message-format=json-render-diagnostics",
        "-vv",
    ]
    environment = os.environ.copy()
    for key in list(environment):
        if key.startswith("CARGO_PROFILE_RELEASE_") or key in {
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_TARGET",
            "CARGO_UNSTABLE_OHM_PROC_MACRO_TRUST",
            "CARGO_UNSTABLE_OHM_NATIVE_TOOL_TRUST",
            "RUSTC",
            "CARGO_BUILD_RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
        }:
            environment.pop(key)
    environment["RUSTFLAGS"] = "-Ctarget-cpu=x86-64"
    environment["CARGO_BUILD_BUILD_DIR"] = str(intermediates)
    environment["CARGO_TERM_COLOR"] = "never"
    before = source_hashes(checkout)
    report: dict[str, Any] = {
        "status": "failed",
        "command": command,
        "checkout": str(checkout),
        "builder_sha256": digest(Path(__file__).resolve()),
        "rustflags": environment["RUSTFLAGS"],
        "intermediates": str(intermediates),
        "inherited_shared_build_dir": os.environ.get("CARGO_BUILD_BUILD_DIR"),
        "source_sha256": before,
        "revision": subprocess.check_output(
            ["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True
        ).strip(),
        "compiler": subprocess.check_output(
            ["rustc", *toolchain, "-Vv"], text=True, env=environment, cwd=checkout
        ),
        "target": TARGET,
    }
    try:
        messages = output / "cargo-messages.jsonl"
        with messages.open("w") as stdout, (output / "build.log").open("w") as stderr:
            subprocess.run(
                command,
                cwd=checkout,
                env=environment,
                stdout=stdout,
                stderr=stderr,
                check=True,
            )
        report["workspace_compilations"] = workspace_compilations(
            (output / "build.log").read_text(), checkout, intermediates
        )
        if source_hashes(checkout) != before:
            raise ValueError("source changed while building")
        artifact = library_artifact(
            messages.read_text(), checkout, args.target_dir.resolve()
        )
        artifact_sha256 = digest(artifact)
        library = output / "libxeme_expat.so"
        shutil.copy2(artifact, library)
        if digest(library) != artifact_sha256 or digest(artifact) != artifact_sha256:
            raise ValueError("emitted library changed while copying")
        report.update(
            status="passed",
            library=str(library),
            library_sha256=artifact_sha256,
            emitted_library=str(artifact),
            cargo_messages_sha256=digest(messages),
        )
    finally:
        write_json(output / "build.json", report)


def validate_build(path: Path, library: Path) -> dict[str, Any]:
    """Bind a completed Xeme build record to the actual benchmark library."""
    record = json.loads(path.read_text())
    if record.get("status") != "passed":
        raise ValueError(f"build did not pass: {path}")
    if record.get("library_sha256") != digest(library):
        raise ValueError(f"build record does not match library: {path} / {library}")
    return record


def summarize(report: dict[str, Any]) -> list[dict[str, Any]]:
    """Reconstruct matched ratios from complete worker samples, excluding warmups."""
    if (
        report["status"] != "passed"
        or report["sha256_before"] != report["sha256_after"]
    ):
        raise ValueError("failed run or changed inputs")
    pairs, iterations = report["pairs"], report["iterations"]
    if pairs < 3 or iterations < 3 or not report["summary"]:
        raise ValueError("empty or incomplete measurement")
    medians: dict[tuple[str, int, str], float] = {}
    expected = {
        (key, pair, engine)
        for key in report["summary"]
        for pair in range(pairs)
        for engine in ENGINES
    }
    for row in report["rows"]:
        key = (row["key"], row["pair"], row["engine"])
        if key in medians or key not in expected:
            raise ValueError("duplicate or unexpected worker")
        samples = row["samples"]
        if len(samples) != iterations + 1 or any(
            s["iteration"] != i
            or s["warmup"] != (i == 0)
            or not math.isfinite(s["seconds"])
            or s["seconds"] <= 0
            for i, s in enumerate(samples)
        ):
            raise ValueError("invalid or missing timing samples")
        medians[key] = statistics.median(s["seconds"] for s in samples[1:])
    if medians.keys() != expected:
        raise ValueError("missing worker")
    rows = []
    for key in sorted(report["summary"]):
        row: dict[str, Any] = {"condition": key}
        for control in ["baseline", "expat"]:
            ratios = [
                medians[key, p, "xeme"] / medians[key, p, control]
                for p in range(pairs)
            ]
            if any(not math.isfinite(ratio) or ratio <= 0 for ratio in ratios):
                raise ValueError("invalid paired ratio")
            row[f"candidate_over_{control}"] = statistics.median(ratios)
        row["regression_percent"] = 100 * (row["candidate_over_baseline"] - 1)
        rows.append(row)
    return rows


def generated_corpus(output: Path) -> Path:
    directory = output / "generated"
    directory.mkdir()
    projects = []
    for name, data in workloads().items():
        path = directory / f"{name}.xml"
        path.write_bytes(data)
        projects.append(
            {
                "name": name,
                "files": [{"role": "input", "path": path.name, "sha256": digest(path)}],
            }
        )
    manifest = directory / "corpus.json"
    write_json(manifest, {"projects": projects})
    return manifest


def run(args: argparse.Namespace) -> None:
    if args.cpu not in os.sched_getaffinity(0):
        raise ValueError("requested CPU is outside current affinity")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    libraries = {
        name: getattr(args, name).resolve(strict=True)
        for name in ["candidate", "baseline", "expat"]
    }
    builds = {
        name: getattr(args, f"{name}_build").resolve(strict=True)
        for name in ["baseline", "candidate"]
    }
    build_records = {
        name: validate_build(path, libraries[name]) for name, path in builds.items()
    }
    manifests = [
        *builds.values(),
        *(p.resolve(strict=True) for p in args.build_manifest),
    ]
    observed = [
        *libraries.values(),
        *manifests,
        Path(__file__).resolve(),
        ROOT / "tools/corpus.py",
    ]
    observed.extend(
        ROOT / path
        for path in [
            "benchmarks/projects.py",
            "benchmarks/python_projects.py",
            "benchmarks/native_driver.c",
            "tools/xml_abi.py",
            "benchmarks/projects/corpus-manifest.json",
        ]
    )
    if args.consumers:
        consumers = args.consumers.resolve(strict=True)
        bundle = json.loads(consumers.read_text())
        if bundle["status"] != "passed" or set(bundle["consumers"]) != ENGINES:
            raise ValueError("requires a passed three-engine consumer build")
        observed.append(consumers)
        for consumer in bundle["consumers"].values():
            for name, expected_hash in consumer["files"].items():
                path = Path(name)
                if digest(path) != expected_hash:
                    raise ValueError(f"consumer changed since build: {path}")
                observed.append(path)
    before = {str(p): digest(p) for p in observed}
    report: dict[str, Any] = {
        "status": "failed",
        "mode": args.mode,
        "cpu": args.cpu,
        "seed": args.seed,
        "libraries": {k: str(v) for k, v in libraries.items()},
        "build_records": build_records,
        "sha256_before": before,
        "commands": [],
        "groups": {},
    }
    pairs = 3 if args.mode == "screen" else 7
    iterations = {
        "native": args.native_iterations or (3 if args.mode == "screen" else 20),
        "python": args.python_iterations or (3 if args.mode == "screen" else 10),
    }
    report.update(pairs=pairs, iterations=iterations)
    rows = []
    try:
        corpora = [
            ("real", ROOT / "benchmarks/projects/corpus-manifest.json"),
            ("generated", generated_corpus(output)),
        ]
        jobs = [("native", group, corpus) for group, corpus in corpora]
        if args.consumers:
            consumers = args.consumers.resolve(strict=True)
            bundle = json.loads(consumers.read_text())
            if bundle["status"] != "passed" or set(bundle["consumers"]) != ENGINES:
                raise ValueError("requires a passed three-engine consumer build")
            for engine, label in [
                ("xeme", "candidate"),
                ("baseline", "baseline"),
                ("expat", "expat"),
            ]:
                if digest(Path(bundle["consumers"][engine]["library"])) != digest(
                    libraries[label]
                ):
                    raise ValueError(f"consumer library differs: {engine}")
            jobs.append(("python", "real", corpora[0][1]))
        for consumer, group, corpus in jobs:
            destination = output / f"{consumer}-{group}"
            script = "projects.py" if consumer == "native" else "python_projects.py"
            command = [
                "taskset",
                "-c",
                str(args.cpu),
                str(args.python),
                "-I",
                "-S",
                str(ROOT / "benchmarks" / script),
                "--corpus",
                str(corpus),
                "--output",
                str(destination),
                "--pairs",
                str(pairs),
                "--iterations",
                str(iterations[consumer]),
                "--seed",
                str(args.seed),
            ]
            if consumer == "native":
                command.extend(
                    [
                        "--library",
                        str(libraries["candidate"]),
                        "--baseline",
                        str(libraries["baseline"]),
                        "--reference",
                        str(libraries["expat"]),
                    ]
                )
                for manifest in manifests:
                    command.extend(["--build-manifest", str(manifest)])
            else:
                command.extend(["--consumers", str(args.consumers.resolve())])
            report["commands"].append(command)
            write_json(output / "report.json", report)
            with (output / f"{consumer}-{group}.log").open("w") as stream:
                subprocess.run(
                    command,
                    stdout=stream,
                    stderr=subprocess.STDOUT,
                    check=True,
                    env={
                        key: value
                        for key, value in os.environ.items()
                        if key not in {"LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT"}
                    },
                )
            summary = destination / "summary.json"
            conditions = summarize(json.loads(summary.read_text()))
            expected_count = 24 if group == "real" else 16
            if len(conditions) != expected_count:
                raise ValueError(
                    f"expected {expected_count} conditions, got {len(conditions)}"
                )
            rows.extend(
                {"consumer": consumer, "corpus": group, **row} for row in conditions
            )
            report["groups"][f"{consumer}-{group}"] = {
                "conditions": len(conditions),
                "summary_sha256": digest(summary),
                "candidate_over_baseline": statistics.geometric_mean(
                    row["candidate_over_baseline"] for row in conditions
                ),
                "candidate_over_expat": statistics.geometric_mean(
                    row["candidate_over_expat"] for row in conditions
                ),
                "regressions": [
                    row for row in conditions if row["regression_percent"] > 0
                ],
            }
        with (output / "conditions.csv").open("w", newline="") as stream:
            writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)
        report.update(
            status="passed", conditions_sha256=digest(output / "conditions.csv")
        )
    finally:
        report["sha256_after"] = {str(p): digest(p) for p in observed}
        if report["sha256_after"] != before:
            report["status"] = "failed"
        write_json(output / "report.json", report)
    if report["status"] != "passed":
        raise ValueError("source or library changed during campaign")
    print(json.dumps(report["groups"], indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    builder = commands.add_parser(
        "build", help="Freeze an ordinary O3/ThinLTO Xeme library"
    )
    builder.add_argument("--checkout", type=Path, required=True)
    builder.add_argument("--target-dir", type=Path, required=True)
    builder.add_argument("--output", type=Path, required=True)
    builder.add_argument(
        "--toolchain",
        help="Optional rustup toolchain (for example stable or ohm); otherwise use the checkout's default",
    )
    runner = commands.add_parser(
        "run", help="Compare three frozen parser libraries sequentially"
    )
    for name in ["candidate", "baseline", "expat", "output"]:
        runner.add_argument(f"--{name}", type=Path, required=True)
    runner.add_argument("--mode", choices=["screen", "confirm"], default="screen")
    runner.add_argument("--cpu", type=int, required=True)
    runner.add_argument("--python", type=Path, default=Path(sys.executable))
    runner.add_argument(
        "--consumers", type=Path, help="Three-engine CPython consumer build.json"
    )
    runner.add_argument(
        "--baseline-build",
        type=Path,
        required=True,
        help="Successful build.json matching --baseline's SHA-256",
    )
    runner.add_argument(
        "--candidate-build",
        type=Path,
        required=True,
        help="Successful build.json matching --candidate's SHA-256",
    )
    runner.add_argument(
        "--build-manifest",
        type=Path,
        action="append",
        default=[],
        help="Additional build evidence, such as the Expat CMake cache",
    )
    runner.add_argument("--seed", type=int, default=20260910)
    runner.add_argument(
        "--native-iterations",
        type=int,
        help="Measured parses per native process (screen: 3, confirm: 20)",
    )
    runner.add_argument(
        "--python-iterations",
        type=int,
        help="Measured parses per Python process (screen: 3, confirm: 10)",
    )
    args = parser.parse_args()
    if not sys.platform.startswith("linux") or os.uname().machine != "x86_64":
        parser.error("this benchmark recipe requires x86-64 Linux")
    if args.command == "build":
        build(args)
    else:
        if any(
            value is not None and value < 3
            for value in [args.native_iterations, args.python_iterations]
        ):
            parser.error("iteration overrides must be at least three")
        run(args)


if __name__ == "__main__":
    main()
