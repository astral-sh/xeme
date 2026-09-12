"""Combined Start, End, namespace and declaration normal CPython screen with separate correctness and timing phases."""

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
import time
from pathlib import Path
from typing import Any

ENGINES = ("control", "candidate", "expat")
RATIOS = (("candidate", "control"), ("control", "expat"), ("candidate", "expat"))
ITERATIONS = {
    "vulkan": 3,
    "wayland": 16,
    "maven": 32,
    "batik": 128,
    "gtk": 64,
    "docbook": 64,
}
WORKER = Path(__file__).with_name("python_worker.py").resolve()


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path: Path, value: Any) -> None:
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def run_process(
    report: dict, output: Path, label: str, command: list[str], timeout: int
) -> tuple[dict, str]:
    record: dict[str, Any] = {
        "label": label,
        "command": command,
        "status": "incomplete",
        "timeout_seconds": timeout,
    }
    report["processes"].append(record)
    stdout, stderr = "", ""
    started = time.perf_counter_ns()
    try:
        result = subprocess.run(
            command, capture_output=True, text=True, timeout=timeout
        )
        record.update(
            returncode=result.returncode,
            wall_seconds=(time.perf_counter_ns() - started) / 1e9,
        )
        stdout, stderr = result.stdout, result.stderr
    except subprocess.TimeoutExpired as exc:
        record.update(returncode=None, failure=repr(exc))
        stdout = (
            (exc.stdout or b"").decode(errors="replace")
            if isinstance(exc.stdout, bytes)
            else exc.stdout or ""
        )
        stderr = (
            (exc.stderr or b"").decode(errors="replace")
            if isinstance(exc.stderr, bytes)
            else exc.stderr or ""
        )
        raise
    except OSError as exc:
        record.update(returncode=None, failure=repr(exc))
        stdout, stderr = "", str(exc)
        raise
    finally:
        for stream, text in (("stdout", stdout), ("stderr", stderr)):
            filename = f"{label}-{stream}.gz"
            with gzip.open(output / filename, "wt") as handle:
                handle.write(text)
            record[stream] = filename
            record[f"{stream}_sha256"] = digest(output / filename)
    if result.returncode != 0:
        raise RuntimeError(f"{label} exited {result.returncode}")
    record["status"] = "passed"
    return record, stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kind", choices=("python",), required=True)
    parser.add_argument("--phase", choices=("prepare", "time"), required=True)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument(
        "--input",
        type=Path,
        required=True,
        help="Pinned original XML corpus manifest",
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.phase == "prepare" and os.sched_getaffinity(0) != {3}:
        parser.error("Preflights require CPU3")
    if args.phase == "time" and os.sched_getaffinity(0) != {0}:
        parser.error("Timings require the coordinated CPU0 reservation")
    output = args.output.resolve()
    build_path, input_path = args.build.resolve(), args.input.resolve()
    build = json.loads(build_path.read_text())
    if build["status"] != "passed":
        parser.error("Consumer build did not pass")
    variants = build["consumers" if args.kind == "python" else "binaries"]
    if set(variants) != set(ENGINES):
        parser.error("Expected selected raw-view control, Combined Start, End, namespace and declaration candidate and Expat engines")
    files = [
        Path(__file__).resolve(),
        WORKER,
        Path(sys.executable).resolve(),
        build_path,
        input_path,
    ]
    conditions: list[dict[str, Any]] = []
    if args.kind == "python":
        corpus = json.loads(input_path.read_text())
        for project in corpus["projects"]:
            entry = next(row for row in project["files"] if row["role"] == "input")
            path = (input_path.parent / entry["path"]).resolve()
            if digest(path) != entry["sha256"]:
                raise RuntimeError("Corpus input hash changed")
            files.append(path)
            for chunk in (4096, 65536):
                for mode in ("elementtree", "pyexpat-events"):
                    conditions.append(
                        {
                            "name": project["name"],
                            "chunk": chunk,
                            "mode": mode,
                            "input": str(path),
                            "iterations": ITERATIONS[project["name"]],
                        }
                    )
        if len(conditions) != 24:
            raise RuntimeError(
                "Expected six original projects and 24 Python conditions"
            )
        for variant in variants.values():
            for path, expected in variant["files"].items():
                if digest(Path(path)) != expected:
                    raise RuntimeError("Consumer differs from build manifest")
                files.append(Path(path))
    else:
        for mode in ("client-header", "private-code"):
            conditions.append(
                {
                    "name": "wayland",
                    "mode": mode,
                    "input": str(input_path),
                    "iterations": 20,
                }
            )
        for variant in variants.values():
            for key in ("path", "library"):
                path = Path(variant[key])
                expected = variant["sha256" if key == "path" else "library_sha256"]
                if digest(path) != expected:
                    raise RuntimeError("Scanner/library differs from build manifest")
                files.append(path)
    hashes = {str(path): digest(path) for path in files}
    if args.phase == "prepare":
        output.mkdir(parents=True, exist_ok=False)
        report: dict[str, Any] = {
            "status": "incomplete",
            "kind": args.kind,
            "method": "Matched normal-release parser cohort: explicit-host selected raw-view control and Combined Start, End, namespace and declaration candidate, plus pinned Expat2.8.4 normal. Both Oriole libraries use -O3, ThinLTO and one codegen unit. All24 original CPython conditions, seven fixed seeded cohorts,504 timed workers after72 preflights. Unchanged reviewed worker times creation/feed/finalization/callbacks and explicit destruction, with canonical checks/imports/input reads/gc.collect outside. One discarded warmup per worker; automatic GC remains enabled. Six original project XML inputs. Extensions use identical -O2 without LTO; no profile campaign is prepared or run.",
            "limitations": "Shared host frequency/cache/memory bandwidth uncontrolled. These are original real XML fixtures processed by unmodified CPython3.12.13 ElementTree and pyexpat consumers, not full project executions. No Wayland generator or external DTD loading. The selected runtime has two known strict CPython callback-grouping failures. Strict-suite compatibility is evaluated separately; every canonical fixture output in this benchmark must match Expat. This benchmark does not establish full-suite compatibility.",
            "python_version": sys.version,
            "platform": platform.platform(),
            "pairs": 7,
            "seed": 202609104412,
            "hashes_before": hashes,
            "conditions": conditions,
            "processes": [],
            "preflights": [],
            "rows": [],
            "expected": {},
        }
    else:
        report = json.loads((output / "preflight.json").read_text())
        if (
            report["status"] != "preflight_passed"
            or report["hashes_before"] != hashes
            or report["conditions"] != conditions
            or report["kind"] != args.kind
        ):
            raise RuntimeError("Preflight source/input/configuration mismatch")
        report["preflight_sha256"] = digest(output / "preflight.json")
        report["status"] = "incomplete"
        report["processes"] = []
    report["affinity"] = sorted(os.sched_getaffinity(0))

    def execute(
        condition: dict, engine: str, pair: int, iteration: int, prepare: bool
    ) -> dict:
        key = f"{condition['name']}/{condition.get('chunk', 0)}/{condition['mode']}"
        label = f"{key.replace('/', '-')}-{engine}-{pair}-{iteration}"
        if args.kind == "python":
            variant = variants[engine]
            spec = {
                "input": condition["input"],
                "input_sha256": hashes[condition["input"]],
                "chunk": condition["chunk"],
                "mode": condition["mode"],
                "consumer": variant["directory"],
                "library_sha256": digest(Path(variant["library"])),
                "iterations": 0 if prepare else condition["iterations"],
            }
            spec_path = output / f"{label}.json"
            save(spec_path, spec)
            process, stdout = run_process(
                report,
                output,
                label,
                [sys.executable, "-I", "-S", str(WORKER), "--worker", str(spec_path)],
                300,
            )
            result = json.loads(stdout)
            samples = result["samples"]
            if (
                len(samples) != spec["iterations"] + 1
                or result["identity"]["parser_library"]["sha256"]
                != spec["library_sha256"]
            ):
                raise RuntimeError("Wrong sample count or parser origin")
            values = []
            for index, sample in enumerate(samples):
                if (
                    sample["iteration"] != index
                    or sample["warmup"] != (index == 0)
                    or sample["seconds"] <= 0
                    or sample["seconds"]
                    != (sample["parse_ns"] + sample["destruction_ns"]) / 1e9
                ):
                    raise RuntimeError("Invalid Python timing record")
                actual = [sample["output_sha256"], sample["output_rows"]]
                if key in report["expected"] and actual != report["expected"][key]:
                    raise RuntimeError(
                        f"Canonical consumer output differs: {key}/{engine}"
                    )
                report["expected"][key] = actual
                if index > 0:
                    values.append(sample["seconds"])
            return {
                "key": key,
                "engine": engine,
                "pair": pair,
                "process": process["label"],
                "identity": result["identity"],
                "median_seconds": statistics.median(values) if values else None,
                "samples": samples,
            }
        generated = output / f"{condition['mode']}-{engine}.out"
        generated.unlink(missing_ok=True)
        command = [
            variants[engine]["path"],
            condition["mode"],
            condition["input"],
            str(generated),
        ]
        process, _ = run_process(report, output, label, command, 30)
        actual = [digest(generated), generated.stat().st_size]
        if key in report["expected"] and actual != report["expected"][key]:
            raise RuntimeError(f"Generated scanner output differs: {engine}")
        report["expected"][key] = actual
        return {
            "key": key,
            "engine": engine,
            "pair": pair,
            "iteration": iteration,
            "process": process["label"],
            "seconds": process["wall_seconds"],
            "output": str(generated),
            "output_fresh": True,
            "output_sha256": actual[0],
            "output_bytes": actual[1],
        }

    try:
        if args.phase == "prepare":
            for condition in conditions:
                for engine in ENGINES:
                    report["preflights"].append(execute(condition, engine, -1, 0, True))
            report["status"] = "preflight_passed"
        else:
            rng = random.Random(report["seed"])
            jobs = [
                (condition, pair)
                for condition in conditions
                for pair in range(report["pairs"])
            ]
            rng.shuffle(jobs)
            for condition, pair in jobs:
                repetitions = 1 if args.kind == "python" else condition["iterations"]
                for iteration in range(repetitions):
                    order = list(ENGINES)
                    rng.shuffle(order)
                    for position, engine in enumerate(order):
                        row = execute(condition, engine, pair, iteration, False)
                        row["order"] = position
                        report["rows"].append(row)
                save(output / "results.json", report)
            report["summary"] = []
            for condition in conditions:
                key = f"{condition['name']}/{condition.get('chunk', 0)}/{condition['mode']}"
                medians = {
                    engine: [
                        statistics.median(
                            row["median_seconds"]
                            if args.kind == "python"
                            else row["seconds"]
                            for row in report["rows"]
                            if row["engine"] == engine
                            and row["pair"] == pair
                            and row["key"] == key
                        )
                        for pair in range(report["pairs"])
                    ]
                    for engine in ENGINES
                }
                ratios = {
                    f"{a}_over_{b}": [
                        medians[a][pair] / medians[b][pair]
                        for pair in range(report["pairs"])
                    ]
                    for a, b in RATIOS
                }
                report["summary"].append(
                    {
                        "condition": condition,
                        "process_pair_medians_seconds": medians,
                        "paired_ratios": ratios,
                        "median_ratios": {
                            name: statistics.median(values)
                            for name, values in ratios.items()
                        },
                    }
                )
            if digest(output / "preflight.json") != report["preflight_sha256"]:
                raise RuntimeError("Preflight report changed")
            report["status"] = "passed"
    except BaseException as exc:
        report.update(status="failed", failure=repr(exc))
        raise
    finally:
        report["hashes_after"] = {}
        report["hash_errors"] = {}
        for path in hashes:
            try:
                report["hashes_after"][path] = digest(Path(path))
            except OSError as exc:
                report["hashes_after"][path] = None
                report["hash_errors"][path] = repr(exc)
        if report["hashes_after"] != hashes:
            report.update(
                status="failed",
                failure="Source/input/consumer changed or became unreadable",
            )
        save(
            output / ("preflight.json" if args.phase == "prepare" else "results.json"),
            report,
        )
    print(
        json.dumps(
            {
                "status": report["status"],
                "kind": args.kind,
                "preflights": len(report["preflights"]),
                "timed_rows": len(report["rows"]),
            }
        )
    )
    return 0 if report["status"] in ("preflight_passed", "passed") else 1


if __name__ == "__main__":
    raise SystemExit(main())
