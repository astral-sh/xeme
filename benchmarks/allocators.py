#!/usr/bin/env python3
"""Compare frozen safe-core executables, with allocation counts measured separately."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import random
import shutil
import statistics
import subprocess
from pathlib import Path
from typing import Any


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary", action="append", required=True, metavar="LABEL=PATH"
    )
    parser.add_argument("--counts", type=Path)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--pairs", type=int, default=7)
    parser.add_argument("--iterations", type=int, default=10)
    parser.add_argument("--chunks", type=int, nargs="+", default=[64, 4096, 1048576])
    parser.add_argument("--seed", type=int, default=20260910)
    parser.add_argument("--build-manifest", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.pairs < 3 or args.iterations < 3 or min(args.chunks) < 1:
        parser.error("at least three pairs/iterations and positive chunks required")
    binaries: dict[str, Path] = {}
    for specification in args.binary:
        label, separator, value = specification.partition("=")
        if not separator or not label or label in binaries:
            parser.error("each binary needs a unique LABEL=PATH")
        binaries[label] = Path(value).resolve(strict=True)
    inputs = {path.stem: path.resolve() for path in sorted(args.inputs.glob("*.xml"))}
    if not inputs:
        parser.error("input directory must contain XML files")
    args.output.mkdir(parents=True, exist_ok=False)
    observed = [
        Path(__file__).resolve(),
        *binaries.values(),
        *inputs.values(),
        *(path.resolve(strict=True) for path in args.build_manifest),
    ]
    if args.counts:
        args.counts = args.counts.resolve(strict=True)
        observed.append(args.counts)
    hashes = {str(path): digest(path) for path in observed}
    rows: list[dict[str, Any]] = []
    counts: list[dict[str, Any]] = []
    report: dict[str, Any] = {
        "status": "failed",
        "method": "Randomized paired processes, each with one discarded warmup. Each timed sample creates, feeds, drains, and drops a safe-core parser. Event hashing is included; file loading, process startup, and JSON output are excluded. The first binary is the comparison baseline. Instrumented allocation counts run separately and never contribute timing samples.",
        "limitations": "Generated workloads on a shared host; frequency and competing load are uncontrolled. Requested bytes sum allocation and reallocation requests, not peak live memory. This compares safe-core executables, not CPython application performance.",
        "platform": platform.platform(),
        "affinity": sorted(os.sched_getaffinity(0)),
        "seed": args.seed,
        "pairs": args.pairs,
        "iterations": args.iterations,
        "sha256_before": hashes,
        "rows": rows,
        "allocation_counts": counts,
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
    expected: dict[tuple[str, int], tuple[Any, ...]] = {}

    def run(label: str, binary: Path, name: str, chunk: int, instrumented: bool):
        command = [str(binary), str(inputs[name]), str(chunk), str(args.iterations)]
        result = json.loads(
            subprocess.run(
                command, check=True, capture_output=True, text=True, timeout=120
            ).stdout
        )
        if result["instrumented"] != instrumented:
            raise RuntimeError(f"incorrect instrumentation mode: {label}")
        for sample in result["samples"]:
            output = (sample["hash"], sample["elements"], sample["text_bytes"])
            prior = expected.setdefault((name, chunk), output)
            if prior != output:
                raise RuntimeError(f"output mismatch: {label}/{name}/{chunk}")
        return {
            "label": label,
            "workload": name,
            "chunk": chunk,
            "command": command,
            **result,
        }

    randomizer = random.Random(args.seed)
    try:
        for pair in range(args.pairs):
            jobs = [(name, chunk) for name in inputs for chunk in args.chunks]
            randomizer.shuffle(jobs)
            for name, chunk in jobs:
                order = list(binaries)
                randomizer.shuffle(order)
                for position, label in enumerate(order):
                    rows.append(
                        {
                            "pair": pair,
                            "order": position,
                            **run(label, binaries[label], name, chunk, False),
                        }
                    )
        baseline = next(iter(binaries))
        for name, input_path in inputs.items():
            for chunk in args.chunks:
                medians = {
                    label: [
                        statistics.median(
                            sample["seconds"]
                            for sample in row["samples"]
                            if not sample["warmup"]
                        )
                        for row in rows
                        if row["label"] == label
                        and row["workload"] == name
                        and row["chunk"] == chunk
                    ]
                    for label in binaries
                }
                report["summary"][f"{name}/{chunk}"] = {
                    "input_bytes": input_path.stat().st_size,
                    "process_medians_seconds": medians,
                    "median_seconds": {
                        label: statistics.median(values)
                        for label, values in medians.items()
                    },
                    "paired_baseline_over_candidate": {
                        label: [
                            left / right
                            for left, right in zip(
                                medians[baseline], values, strict=True
                            )
                        ]
                        for label, values in medians.items()
                    },
                }
                if args.counts:
                    counts.append(run("counts", args.counts, name, chunk, True))
        report["sha256_after"] = {str(path): digest(path) for path in observed}
        if hashes != report["sha256_after"]:
            raise RuntimeError("benchmark files changed during measurement")
        report["status"] = "passed"
    except (RuntimeError, subprocess.SubprocessError, OSError, ValueError) as error:
        report["failure"] = str(error)
    finally:
        (args.output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ("status", "summary")}, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
