#!/usr/bin/env python3
"""Measure external DTD scaling after comparing complete callback metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import random
import statistics
import subprocess
from pathlib import Path
from typing import Any


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--driver", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--pairs", type=int, default=7)
    parser.add_argument("--iterations", type=int, default=5)
    parser.add_argument("--build-manifest", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=30)
    args = parser.parse_args()
    if args.pairs < 3 or args.iterations < 3:
        parser.error("at least three pairs and iterations are required")
    if not args.timeout > 0:
        parser.error("a positive worker timeout is required")
    args.output.mkdir(parents=True, exist_ok=False)
    libraries = {"oriole": args.library.resolve(), "expat": args.reference.resolve()}
    inputs = sorted(args.inputs.glob("*.dtd"))
    if not inputs:
        parser.error("expected generated DTD inputs")
    paths = [Path(__file__), args.driver, *libraries.values(), *inputs]
    paths.extend([args.inputs / "manifest.json", args.build_manifest])
    specification = json.loads((args.inputs / "manifest.json").read_text())
    for path in inputs:
        if sha(path) != specification[path.name]["sha256"]:
            parser.error(f"input hash changed: {path.name}")
    observed = {str(p): sha(p) for p in paths}
    report: dict[str, Any] = {
        "status": "incomplete",
        "platform": platform.platform(),
        "cwd": str(Path.cwd()),
        "affinity": sorted(os.sched_getaffinity(0)),
        "pairs": args.pairs,
        "iterations": args.iterations,
        "sha256_before": observed,
        "preflight": [],
        "rows": [],
        "summary": {},
        "method": "Native C callbacks include entities, ATTLIST defaults, full content models, external requests and root elements/attributes. The local resolver reads preloaded memory only. Each process discards one warmup; creation, child handling, callback hashing and destruction are timed. File/library loading and startup are excluded. Trace preflight serializes every observed callback field before timing.",
        "limitations": "Generated DTDs on a shared host; frequency and host contention are uncontrolled. These measurements do not establish general XML or application performance. Timing excludes trace output; the FNV digest is only a repeatability check after exact metadata preflight. Source positions and Default callbacks are outside this benchmark contract.",
    }
    rng = random.Random(20260910)
    expected_outputs = {}

    def run(label: str, path: Path, chunk: int, trace: bool = False) -> dict:
        command = [
            str(args.driver),
            str(libraries[label]),
            str(path),
            str(chunk),
            str(1 if trace else args.iterations),
        ]
        if trace:
            command.append("--trace")
        row: dict[str, Any] = {
            "engine": label,
            "file": path.name,
            "chunk": chunk,
            "command": command,
        }
        try:
            result = subprocess.run(
                command, capture_output=True, text=True, timeout=args.timeout
            )
        except subprocess.TimeoutExpired as error:
            row["failure"] = "timeout"
            row["timeout_seconds"] = error.timeout
            # TimeoutExpired retains bytes even when text=True was requested.
            for key, value in (("stdout", error.stdout), ("stderr", error.stderr)):
                row[key] = (
                    value.decode(errors="replace")
                    if isinstance(value, bytes)
                    else value
                )
            report["rows"].append(row)
            raise
        except OSError as error:
            row["failure"] = "launch"
            row["error"] = str(error)
            report["rows"].append(row)
            raise
        row.update(exit_code=result.returncode, stderr=result.stderr)
        if result.returncode:
            row["stdout"] = result.stdout
            report["rows"].append(row)
            raise RuntimeError(f"native driver failed for {label}/{path.name}/{chunk}")
        try:
            row["result"] = json.loads(result.stdout)
        except json.JSONDecodeError:
            row["failure"] = "invalid-json"
            row["stdout"] = result.stdout
            report["rows"].append(row)
            raise
        return row

    try:
        for path in inputs:
            for chunk in (1, 64, 4096):
                a, b = [run(label, path, chunk, True) for label in libraries]
                report["preflight"].extend([a, b])
                expected = a["result"]["samples"][0]
                expected_outputs[path.name, chunk] = {
                    key: expected[key]
                    for key in ("hash", "declarations", "external_calls")
                }
                for key in ("declarations", "external_calls"):
                    if expected[key] != specification[path.name]["expected_" + key]:
                        raise RuntimeError(
                            f"fixture count failed for {path.name}: {key}"
                        )
                for row in (a, b):
                    for sample in row["result"]["samples"]:
                        for key in ("trace", "hash", "declarations", "external_calls"):
                            if sample[key] != expected[key]:
                                raise RuntimeError(
                                    f"metadata preflight failed for {path.name}/{chunk}: {key}"
                                )
        for path in inputs:
            for chunk in (1, 64, 4096):
                medians = {label: [] for label in libraries}
                for pair in range(args.pairs):
                    order = list(libraries)
                    rng.shuffle(order)
                    for label in order:
                        row = run(label, path, chunk)
                        row["pair"] = pair
                        report["rows"].append(row)
                        samples = row["result"]["samples"]
                        for sample in samples:
                            for key, value in expected_outputs[
                                path.name, chunk
                            ].items():
                                if sample[key] != value:
                                    raise RuntimeError(
                                        f"timed metadata changed for {label}/{path.name}/{chunk}: {key}"
                                    )
                        medians[label].append(
                            statistics.median(
                                s["seconds"] for s in samples if not s["warmup"]
                            )
                        )
                report["summary"][f"{path.name}/{chunk}"] = {
                    "process_medians_seconds": medians,
                    "median_seconds": {
                        label: statistics.median(values)
                        for label, values in medians.items()
                    },
                    "median_expat_over_oriole": statistics.median(
                        a / b
                        for a, b in zip(
                            medians["expat"], medians["oriole"], strict=True
                        )
                    ),
                }
        report["sha256_after"] = {str(p): sha(p) for p in paths}
        if report["sha256_after"] != observed:
            raise RuntimeError("measured source, input or binary changed")
        report["status"] = "passed"
    except Exception as error:
        report["failure"] = {"type": type(error).__name__, "message": str(error)}
        raise
    finally:
        (args.output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report["summary"], indent=2))


if __name__ == "__main__":
    main()
