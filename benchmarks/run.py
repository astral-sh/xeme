#!/usr/bin/env python3
"""Capture paired native C ABI benchmarks, refusing unequal callback output."""

from __future__ import annotations

import argparse
import ctypes.util
import hashlib
import json
import os
import platform
import random
import shutil
import statistics
import subprocess
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from corpus import workloads
from xml_abi import Expat


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def resolve_library(value: str) -> Path:
    path = Path(value)
    if path.is_file():
        return path.resolve()
    listing = subprocess.run(
        ["ldconfig", "-p"], check=True, capture_output=True, text=True
    ).stdout
    candidates = {
        Path(fields[-1]).resolve()
        for line in listing.splitlines()
        if (fields := line.split()) and fields[0] == value
    }
    if len(candidates) == 1:
        return candidates.pop()
    raise ValueError(
        f"cannot resolve one library path for {value}; supply an absolute path"
    )


def preflight(path: Path) -> None:
    specification = json.loads(path.read_text())
    engines = {
        name: Expat(library) for name, library in specification["libraries"].items()
    }
    results = []
    for name, location in specification["inputs"].items():
        data = Path(location).read_bytes()
        for chunk in specification["chunks"]:
            namespaces = specification.get("namespaces", False)
            expected = engines["expat"].parse(data, chunk, namespaces)
            actual = engines["oriole"].parse(data, chunk, namespaces)
            if (
                expected["status"] != 1
                or actual["status"] != 1
                or expected["callback_errors"]
                or actual["callback_errors"]
                or expected["normalized_events"] != actual["normalized_events"]
            ):
                raise RuntimeError(
                    f"semantic preflight failed for {name}, chunk {chunk}"
                )
            results.append(
                {
                    "workload": name,
                    "chunk": chunk,
                    "normalized_events_sha256": hashlib.sha256(
                        json.dumps(actual["normalized_events"]).encode()
                    ).hexdigest(),
                }
            )
    path.with_name("preflight-results.json").write_text(
        json.dumps(results, indent=2) + "\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--reference", default=ctypes.util.find_library("expat"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--pairs", type=int, default=7)
    parser.add_argument("--iterations", type=int, default=10)
    parser.add_argument("--size", type=int, default=10000)
    parser.add_argument("--chunks", type=int, nargs="+", default=[64, 4096, 1048576])
    parser.add_argument("--seed", type=int, default=20260910)
    parser.add_argument(
        "--namespaces",
        action="store_true",
        help="Enable namespace expansion in both libraries",
    )
    parser.add_argument("--cc", default="cc")
    parser.add_argument("--build-manifest", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.pairs < 3 or args.iterations < 3 or args.size < 1 or min(args.chunks) < 1:
        parser.error(
            "at least three pairs/iterations, positive size and chunk sizes required"
        )
    args.output.mkdir(parents=True, exist_ok=False)
    source = Path(__file__).with_name("native_driver.c").resolve()
    binary = (args.output / "native-driver").resolve()
    libraries = {
        "oriole": resolve_library(str(args.library)),
        "expat": resolve_library(args.reference),
    }
    command = [
        args.cc,
        "-std=c11",
        "-O3",
        "-Wall",
        "-Wextra",
        "-Werror",
        str(source),
        "-ldl",
        "-o",
        str(binary),
    ]
    subprocess.run(command, check=True)
    inputs = {}
    for name, data in workloads(args.size).items():
        path = args.output / f"{name}.xml"
        path.write_bytes(data)
        inputs[name] = path.resolve()
    helper_root = Path(__file__).resolve().parents[1] / "tools"
    observed_paths = [
        Path(__file__).resolve(),
        source,
        binary,
        helper_root / "xml_abi.py",
        helper_root / "corpus.py",
        *libraries.values(),
        *inputs.values(),
        *(path.resolve(strict=True) for path in args.build_manifest),
    ]
    hashes = {str(path): digest(path) for path in observed_paths}
    rows: list[dict[str, Any]] = []
    report = {
        "status": "failed",
        "method": "Median of native-driver process medians; speedup is median of paired Expat/Oriole ratios. Each process discards one warmup. Parser creation, callback registration, parse, native callbacks, and parser free are timed. Library/input loading and process startup are excluded. Native callbacks hash names, attributes, and character bytes independent of text fragmentation.",
        "limitations": "Shared host: CPU frequency, host load, and memory bandwidth are uncontrolled. Warm filesystem caches. The FNV-1a callback digest is an output consistency check; the separate untimed differential preflight compares complete normalized callback streams. These generated workloads do not establish CPython application performance.",
        "platform": platform.platform(),
        "cpu": next(
            (
                line.split(":", 1)[1].strip()
                for line in Path("/proc/cpuinfo").read_text().splitlines()
                if line.startswith("model name")
            ),
            None,
        ),
        "affinity": sorted(os.sched_getaffinity(0))
        if hasattr(os, "sched_getaffinity")
        else None,
        "compiler": subprocess.run(
            [args.cc, "--version"], check=True, capture_output=True, text=True
        ).stdout,
        "build_command": command,
        "seed": args.seed,
        "pairs": args.pairs,
        "iterations": args.iterations,
        "namespaces": args.namespaces,
        "sha256_before": hashes,
        "rows": rows,
        "summary": {},
        "build_manifests": [],
    }
    for index, path in enumerate(args.build_manifest):
        copy = args.output / f"build-manifest-{index}.json"
        shutil.copy2(path, copy)
        report["build_manifests"].append(
            {
                "original": str(path.resolve()),
                "copy": str(copy.resolve()),
                "sha256": digest(copy),
            }
        )
    randomizer = random.Random(args.seed)
    try:
        preflight_path = (args.output / "preflight.json").resolve()
        preflight_path.write_text(
            json.dumps(
                {
                    "libraries": {name: str(path) for name, path in libraries.items()},
                    "inputs": {name: str(path) for name, path in inputs.items()},
                    "chunks": args.chunks,
                    "namespaces": args.namespaces,
                },
                indent=2,
            )
            + "\n"
        )
        command = [
            sys.executable,
            str(Path(__file__).resolve()),
            "--preflight",
            str(preflight_path),
        ]
        report["preflight_command"] = command
        completed = subprocess.run(
            command, check=True, capture_output=True, text=True, timeout=120
        )
        (args.output / "preflight.log").write_text(completed.stdout + completed.stderr)
        for pair in range(args.pairs):
            jobs = [(name, chunk) for name in inputs for chunk in args.chunks]
            randomizer.shuffle(jobs)
            for name, chunk in jobs:
                order = list(libraries)
                randomizer.shuffle(order)
                for position, engine in enumerate(order):
                    command = [
                        str(binary),
                        str(libraries[engine]),
                        str(inputs[name]),
                        str(chunk),
                        str(args.iterations),
                        *(["namespaces"] if args.namespaces else []),
                    ]
                    completed = subprocess.run(
                        command, check=True, capture_output=True, text=True, timeout=120
                    )
                    result = json.loads(completed.stdout)
                    rows.append(
                        {
                            "pair": pair,
                            "order": position,
                            "engine": engine,
                            "workload": name,
                            "chunk_size": chunk,
                            "command": command,
                            **result,
                        }
                    )
        for name, path in inputs.items():
            for chunk in args.chunks:
                selected = [
                    row
                    for row in rows
                    if row["workload"] == name and row["chunk_size"] == chunk
                ]
                outputs = {
                    (sample["hash"], sample["elements"], sample["text_bytes"])
                    for row in selected
                    for sample in row["samples"]
                }
                if len(outputs) != 1:
                    raise RuntimeError(
                        f"native callback output differs for {name}, chunk {chunk}"
                    )
                medians = {
                    engine: [
                        statistics.median(
                            sample["seconds"]
                            for sample in next(
                                row
                                for row in selected
                                if row["engine"] == engine and row["pair"] == pair
                            )["samples"]
                            if not sample["warmup"]
                        )
                        for pair in range(args.pairs)
                    ]
                    for engine in libraries
                }
                ratios = [
                    medians["expat"][pair] / medians["oriole"][pair]
                    for pair in range(args.pairs)
                ]
                report["summary"][f"{name}/{chunk}"] = {
                    "input_bytes": path.stat().st_size,
                    "process_medians_seconds": medians,
                    "median_seconds": {
                        engine: statistics.median(values)
                        for engine, values in medians.items()
                    },
                    "paired_expat_over_oriole": ratios,
                    "median_expat_over_oriole": statistics.median(ratios),
                    "range_expat_over_oriole": [min(ratios), max(ratios)],
                }
        report["sha256_after"] = {str(path): digest(path) for path in observed_paths}
        if hashes != report["sha256_after"]:
            raise RuntimeError("benchmark files changed during measurement")
        report["status"] = "passed"
    except (RuntimeError, subprocess.SubprocessError, OSError) as error:
        report["failure"] = str(error)
    finally:
        (args.output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {
                "status": report["status"],
                "failure": report.get("failure"),
                "summary": report["summary"],
            },
            indent=2,
        )
    )
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--preflight":
        preflight(Path(sys.argv[2]))
        raise SystemExit(0)
    raise SystemExit(main())
