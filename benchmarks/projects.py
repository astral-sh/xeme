#!/usr/bin/env python3
"""Paired native XML parser measurements on a pinned, unmodified project corpus."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
import platform
import random
import statistics
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from xml_abi import Expat


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def native_output(events: list) -> tuple[str, int, int]:
    """Reproduce the C driver's byte hash from the full normalized callback stream."""
    value = 14695981039346656037
    elements = text_bytes = 0
    for event in events:
        data = b""
        if event[0] == "start":
            data = b"\xffS" + event[1].encode()
            for name, attribute in event[2]:
                data += b"\0" + name.encode() + b"\0" + attribute.encode()
            data += b"\0"
            elements += 1
        elif event[0] == "end":
            data = b"\xffE" + event[1].encode() + b"\0"
        elif event[0] == "text":
            data = event[1].encode()
            text_bytes += len(data)
        for byte in data:
            value = ((value ^ byte) * 1099511628211) & 0xFFFFFFFFFFFFFFFF
    return f"{value:016x}", elements, text_bytes


def worker(spec_path: Path) -> None:
    spec = json.loads(spec_path.read_text())
    results = {}
    for engine, library in spec["libraries"].items():
        results[engine] = Expat(library).parse(
            Path(spec["input"]).read_bytes(), spec["chunk"], spec["namespaces"]
        )
    with gzip.open(spec_path.with_suffix(".callbacks.json.gz"), "wt") as stream:
        json.dump(results, stream, separators=(",", ":"))
    passed = all(
        r["status"] == 1 and r["error"] == 0 and not r["callback_errors"]
        for r in results.values()
    ) and all(
        result["normalized_events"] == results["expat"]["normalized_events"]
        for result in results.values()
    )
    summary = {
        "passed": passed,
        "observations": {
            engine: {
                "status": result["status"],
                "error": result["error"],
                "position": result["position"],
                "callback_errors": result["callback_errors"],
                "normalized_events_sha256": hashlib.sha256(
                    json.dumps(
                        result["normalized_events"], separators=(",", ":")
                    ).encode()
                ).hexdigest(),
                "native_output": native_output(result["normalized_events"]),
            }
            for engine, result in results.items()
        },
    }
    spec_path.with_suffix(".result.json").write_text(
        json.dumps(summary, indent=2) + "\n"
    )
    if not passed:
        raise RuntimeError(f"normalized callback preflight failed: {spec_path}")


def main(argv: list[str] | None = None, *, source_files: Sequence[Path] = ()) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--baseline", type=Path, help="Optional third parser library")
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--chunks", nargs="+", type=int, default=[4096, 65536])
    parser.add_argument("--pairs", type=int, default=7)
    parser.add_argument("--iterations", type=int, default=20)
    parser.add_argument("--seed", type=int, default=20260910)
    parser.add_argument("--cc", default="cc")
    parser.add_argument("--build-manifest", type=Path, action="append", default=[])
    parser.add_argument("--preflight-only", action="store_true")
    parser.add_argument("--namespaces", choices=["off", "on", "both"], default="both")
    args = parser.parse_args(argv)
    if (
        args.pairs < 3
        or args.iterations < 3
        or not args.chunks
        or min(args.chunks) < 1
        or max(args.chunks) > 2**31 - 1
    ):
        parser.error(
            "at least three pairs/iterations and positive chunks <= INT_MAX required"
        )
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    source = Path(__file__).with_name("native_driver.c").resolve()
    binary = output / "native-driver"
    corpus = args.corpus.resolve()
    manifest = json.loads(corpus.read_text())
    inputs = {}
    for project in manifest["projects"]:
        files = [f for f in project["files"] if f["role"] == "input"]
        if len(files) != 1 or project["name"] in inputs:
            raise ValueError("expected exactly one input per unique project")
        entry = files[0]
        path = (corpus.parent / entry["path"]).resolve(strict=True)
        if digest(path) != entry["sha256"]:
            raise ValueError(f"corpus hash mismatch: {path}")
        inputs[project["name"]] = path
    libraries = {
        "xeme": args.library.resolve(strict=True),
        "expat": args.reference.resolve(strict=True),
    }
    if args.baseline is not None:
        libraries["baseline"] = args.baseline.resolve(strict=True)
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
    build = subprocess.run(command, capture_output=True, text=True, check=False)
    (output / "build.log").write_text(build.stdout + build.stderr)
    if build.returncode:
        return build.returncode
    observed = [
        Path(__file__).resolve(),
        source,
        binary,
        Path(__file__).resolve().parents[1] / "tools/xml_abi.py",
        corpus,
        *inputs.values(),
        *libraries.values(),
        *(p.resolve(strict=True) for p in args.build_manifest),
        *(p.resolve(strict=True) for p in source_files),
    ]
    hashes = {str(path): digest(path) for path in observed}
    report: dict[str, Any] = {
        "status": "failed",
        "schema_version": 1,
        "corpus_manifest": manifest,
        "method": "Seven matched rounds by default, randomizing all engines within each condition; per-process median of parse samples after one discarded warmup. Creation, callback registration, parse, native callback hashing and free are timed. File/library loading and process startup are excluded. Full normalized callback streams are compared before timing and derive the expected native output hash. Each measured sample must match its independent preflight hash/counts.",
        "limitations": "These are parser microbenchmarks on the supplied corpus, not end-to-end project build, rendering or code generation. No external entity handler is installed; no runtime network or file resolution occurs. Batik external DTD is skipped by both parsers. XSL includes are XML data, not resolved transformations. Shared host CPU frequency/load/memory bandwidth are uncontrolled. Warm filesystem caches.",
        "platform": platform.platform(),
        "cpu": next(
            (
                line.split(":", 1)[1].strip()
                for line in Path("/proc/cpuinfo").read_text().splitlines()
                if line.startswith("model name")
            ),
            None,
        ),
        "affinity": sorted(os.sched_getaffinity(0)),
        "compiler": subprocess.check_output([args.cc, "--version"], text=True),
        "build_command": command,
        "pairs": args.pairs,
        "iterations": args.iterations,
        "seed": args.seed,
        "sha256_before": hashes,
        "preflights": [],
        "processes": [],
        "summary": {},
    }
    namespaces = {"off": [False], "on": [True], "both": [False, True]}[args.namespaces]
    jobs = [
        (name, chunk, ns)
        for name in inputs
        for chunk in args.chunks
        for ns in namespaces
    ]
    expected = {}

    def execute(command: list[str], label: str, timeout: int = 120):
        row = {"label": label, "command": command}
        report["processes"].append(row)
        try:
            done = subprocess.run(
                command,
                capture_output=True,
                text=True,
                check=False,
                timeout=timeout,
                env={
                    key: value
                    for key, value in os.environ.items()
                    if key not in {"LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT"}
                },
            )
            row.update(
                returncode=done.returncode, stdout=done.stdout, stderr=done.stderr
            )
            if done.returncode:
                raise RuntimeError(f"worker {label} exited {done.returncode}")
            return done.stdout
        except (OSError, subprocess.TimeoutExpired) as error:
            row["failure"] = repr(error)
            raise

    try:
        for name, chunk, ns in jobs:
            key = f"{name}/{chunk}/namespaces-{int(ns)}"
            path = output / f"preflight-{name}-{chunk}-{int(ns)}.json"
            path.write_text(
                json.dumps(
                    {
                        "libraries": {k: str(v) for k, v in libraries.items()},
                        "input": str(inputs[name]),
                        "chunk": chunk,
                        "namespaces": ns,
                    },
                    indent=2,
                )
                + "\n"
            )
            execute(
                [
                    sys.executable,
                    "-I",
                    "-S",
                    str(Path(__file__).resolve()),
                    "--worker",
                    str(path),
                ],
                key,
            )
            result = json.loads(path.with_suffix(".result.json").read_text())
            report["preflights"].append(
                {
                    "key": key,
                    "specification": str(path),
                    "callbacks": str(path.with_suffix(".callbacks.json.gz")),
                    "callbacks_sha256": digest(path.with_suffix(".callbacks.json.gz")),
                    **result,
                }
            )
            expected[key] = tuple(result["observations"]["expat"]["native_output"])
        if not args.preflight_only:
            randomizer = random.Random(args.seed)
            rows: list[dict[str, Any]] = []
            report["rows"] = rows
            for pair in range(args.pairs):
                randomizer.shuffle(jobs)
                for name, chunk, ns in jobs:
                    key = f"{name}/{chunk}/namespaces-{int(ns)}"
                    order = list(libraries)
                    randomizer.shuffle(order)
                    for position, engine in enumerate(order):
                        command = [
                            str(binary),
                            str(libraries[engine]),
                            str(inputs[name]),
                            str(chunk),
                            str(args.iterations),
                            *(["namespaces"] if ns else []),
                        ]
                        result = json.loads(
                            execute(command, f"{key}/pair-{pair}/{engine}")
                        )
                        if len(result["samples"]) != args.iterations + 1:
                            raise ValueError("unexpected sample count")
                        for i, sample in enumerate(result["samples"]):
                            if (
                                sample["iteration"] != i
                                or sample["warmup"] != (i == 0)
                                or not (sample["seconds"] > 0)
                            ):
                                raise ValueError("invalid sample timing or iteration")
                            if (
                                sample["hash"],
                                sample["elements"],
                                sample["text_bytes"],
                            ) != expected[key]:
                                raise ValueError(
                                    f"measured output mismatch: {key}/{engine}"
                                )
                        rows.append(
                            {
                                "key": key,
                                "workload": name,
                                "chunk": chunk,
                                "namespaces": ns,
                                "pair": pair,
                                "order": position,
                                "engine": engine,
                                **result,
                            }
                        )
            for name, chunk, ns in jobs:
                key = f"{name}/{chunk}/namespaces-{int(ns)}"
                medians = {
                    engine: [
                        statistics.median(
                            s["seconds"]
                            for s in next(
                                r
                                for r in rows
                                if r["key"] == key
                                and r["pair"] == pair
                                and r["engine"] == engine
                            )["samples"]
                            if not s["warmup"]
                        )
                        for pair in range(args.pairs)
                    ]
                    for engine in libraries
                }
                ratios = [
                    medians["expat"][p] / medians["xeme"][p]
                    for p in range(args.pairs)
                ]
                report["summary"][key] = {
                    "input_bytes": inputs[name].stat().st_size,
                    "process_medians_seconds": medians,
                    "median_seconds": {
                        e: statistics.median(v) for e, v in medians.items()
                    },
                    "paired_expat_over_xeme": ratios,
                    "median_expat_over_xeme": statistics.median(ratios),
                    "range_expat_over_xeme": [min(ratios), max(ratios)],
                }
        report["status"] = "preflight-passed" if args.preflight_only else "passed"
    except (
        RuntimeError,
        ValueError,
        KeyError,
        OSError,
        subprocess.SubprocessError,
    ) as error:
        report["status"] = "failed"
        report["failure"] = repr(error)
    finally:
        report["sha256_after"] = {str(path): digest(path) for path in observed}
        if hashes != report["sha256_after"]:
            report["status"] = "failed"
            report["failure"] = (
                "source, input, manifest or binary changed during measurement"
            )
        (output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {
                "status": report["status"],
                "preflights": len(report["preflights"]),
                "timed_processes": len(report.get("rows", [])),
                "failure": report.get("failure"),
            },
            indent=2,
        )
    )
    return 0 if report["status"] != "failed" else 1


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--worker":
        worker(Path(sys.argv[2]))
    else:
        raise SystemExit(main())
