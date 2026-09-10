#!/usr/bin/env python3
"""Measure the actual unmodified Wayland scanner process and generated output."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import resource
import statistics
import subprocess
import time
from pathlib import Path
from typing import Any


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-manifest", type=Path, required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--pairs", type=int, default=7)
    parser.add_argument("--iterations", type=int, default=10)
    parser.add_argument("--seed", type=int, default=20260910)
    args = parser.parse_args()
    if args.pairs < 3 or args.iterations < 3:
        parser.error("requires >=3 pairs and iterations")
    build_path = args.build_manifest.resolve(strict=True)
    build = json.loads(build_path.read_text())
    if build["status"] != "passed":
        parser.error("build did not pass")
    source = args.input.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binaries = build["binaries"]
    for entry in binaries.values():
        if (
            digest(Path(entry["path"])) != entry["sha256"]
            or digest(Path(entry["library"])) != entry["library_sha256"]
        ):
            parser.error("binary/library differs from build manifest")
    observed = [
        Path(__file__).resolve(),
        build_path,
        source,
        *[Path(entry["path"]) for entry in binaries.values()],
        *[Path(entry["library"]) for entry in binaries.values()],
    ]
    hashes = {str(p): digest(p) for p in observed}
    report: dict[str, Any] = {
        "status": "failed",
        "method": "Actual unmodified Wayland 1.23.90 scanner processes compile the pinned wayland.xml into client-header and private-code. Full process wall time includes ELF/library loading, opening/reading input, XML_GetBuffer/XML_ParseBuffer, protocol semantic processing, generating/writing the output file, and process exit. Python launch/wait overhead is included. Generated file hashing is outside timing. All outputs must exactly equal the reference preflight. One warmup process per engine/mode is discarded; seven randomized pairs of ten processes by default. Paired ratios use medians of full-process wall times within each pair.",
        "limitations": "Linux process timings include startup and warm filesystem I/O. Optional libxml DTD validation is disabled in both builds; this isolates the Expat-backed normal scanner path. This is an actual project command on one protocol input, not the whole Wayland build. Shared host CPU frequency/load and memory bandwidth are uncontrolled.",
        "affinity": sorted(os.sched_getaffinity(0)),
        "build": build,
        "input": str(source),
        "pairs": args.pairs,
        "iterations": args.iterations,
        "seed": args.seed,
        "sha256_before": hashes,
        "preflights": [],
        "rows": [],
        "summary": {},
    }
    expected = {}

    def execute(engine: str, mode: str, pair: int, iteration: int, warmup: bool):
        generated = output / f"{mode}-{engine}.out"
        command = [binaries[engine]["path"], mode, str(source), str(generated)]
        row = {
            "engine": engine,
            "mode": mode,
            "pair": pair,
            "iteration": iteration,
            "warmup": warmup,
            "command": command,
        }
        destination = report["preflights"] if warmup else report["rows"]
        destination.append(row)
        usage = resource.getrusage(resource.RUSAGE_CHILDREN)
        before = time.perf_counter_ns()
        try:
            done = subprocess.run(
                command, capture_output=True, text=True, check=False, timeout=30
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            row["failure"] = repr(error)
            raise
        after = time.perf_counter_ns()
        finished = resource.getrusage(resource.RUSAGE_CHILDREN)
        row.update(
            wall_seconds=(after - before) / 1e9,
            user_seconds=finished.ru_utime - usage.ru_utime,
            system_seconds=finished.ru_stime - usage.ru_stime,
            returncode=done.returncode,
            stdout=done.stdout,
            stderr=done.stderr,
        )
        if done.returncode:
            raise RuntimeError(f"{engine}/{mode} exited {done.returncode}")
        value = digest(generated)
        row.update(output_sha256=value, output_bytes=generated.stat().st_size)
        if mode in expected and expected[mode] != value:
            raise RuntimeError(f"generated output differs: {engine}/{mode}")
        expected[mode] = value
        return row

    try:
        for mode in ["client-header", "private-code"]:
            for engine in ["expat", "oriole"]:
                execute(engine, mode, -1, 0, True)
        rng = random.Random(args.seed)
        for pair in range(args.pairs):
            jobs = [
                (mode, iteration)
                for mode in ["client-header", "private-code"]
                for iteration in range(args.iterations)
            ]
            rng.shuffle(jobs)
            for mode, iteration in jobs:
                order = list(binaries)
                rng.shuffle(order)
                for position, engine in enumerate(order):
                    row = execute(engine, mode, pair, iteration, False)
                    row["order"] = position
        for mode in ["client-header", "private-code"]:
            medians = {
                engine: [
                    statistics.median(
                        row["wall_seconds"]
                        for row in report["rows"]
                        if row["mode"] == mode
                        and row["engine"] == engine
                        and row["pair"] == pair
                    )
                    for pair in range(args.pairs)
                ]
                for engine in binaries
            }
            ratios = [
                medians["expat"][p] / medians["oriole"][p] for p in range(args.pairs)
            ]
            report["summary"][mode] = {
                "process_pair_medians_seconds": medians,
                "median_seconds": {
                    engine: statistics.median(values)
                    for engine, values in medians.items()
                },
                "paired_expat_over_oriole": ratios,
                "median_expat_over_oriole": statistics.median(ratios),
                "range_expat_over_oriole": [min(ratios), max(ratios)],
            }
        report["status"] = "passed"
    except (RuntimeError, ValueError, OSError, subprocess.SubprocessError) as error:
        report["status"] = "failed"
        report["failure"] = repr(error)
    finally:
        report["sha256_after"] = {str(p): digest(p) for p in observed}
        if hashes != report["sha256_after"]:
            report["status"] = "failed"
            report["failure"] = "source/input/binary changed during measurement"
        (output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {
                "status": report["status"],
                "processes": len(report["rows"]),
                "summary": report["summary"],
                "failure": report.get("failure"),
            },
            indent=2,
        )
    )
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
