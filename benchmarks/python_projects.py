#!/usr/bin/env python3
"""Matched CPython ElementTree and pyexpat consumers on pinned project XML."""

from __future__ import annotations

import argparse
import ctypes
import gc
import hashlib
import importlib.util
import json
import os
import platform
import random
import statistics
import subprocess
import sys
import sysconfig
import time
from pathlib import Path
from typing import Any


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_consumers(directory: Path) -> dict:
    """Override built-in PBS extensions explicitly and prove the loaded parser origin."""
    result = {}
    for name in ["pyexpat", "_elementtree"]:
        path = directory / f"{name}{sysconfig.get_config_var('EXT_SUFFIX')}"
        spec = importlib.util.spec_from_file_location(name, path)
        if spec is None or spec.loader is None:
            raise ImportError(str(path))
        module = importlib.util.module_from_spec(spec)
        sys.modules[name] = module
        spec.loader.exec_module(module)
        if module.__file__ is None or Path(module.__file__).resolve() != path.resolve():
            raise ImportError("wrong consumer origin")
        result[name] = {"path": str(path), "sha256": digest(path)}

    class DlInfo(ctypes.Structure):
        _fields_ = [
            ("filename", ctypes.c_char_p),
            ("base", ctypes.c_void_p),
            ("symbol", ctypes.c_char_p),
            ("address", ctypes.c_void_p),
        ]

    extension = ctypes.CDLL(result["pyexpat"]["path"])
    process = ctypes.CDLL(None)
    process.dladdr.argtypes = [ctypes.c_void_p, ctypes.POINTER(DlInfo)]
    process.dladdr.restype = ctypes.c_int
    info = DlInfo()
    if (
        process.dladdr(
            ctypes.cast(extension.XML_Parse, ctypes.c_void_p), ctypes.byref(info)
        )
        != 1
        or info.filename is None
    ):
        raise RuntimeError("cannot resolve XML_Parse origin")
    library = Path(os.fsdecode(info.filename)).resolve(strict=True)
    if library.parent != directory.resolve():
        raise RuntimeError(
            f"XML_Parse resolves outside frozen consumer directory: {library}"
        )
    result["parser_library"] = {"path": str(library), "sha256": digest(library)}
    result["expat_version"] = sys.modules["pyexpat"].EXPAT_VERSION
    return result


def parse(mode: str, chunks: list[bytes]):
    if mode == "elementtree":
        import xml.etree.ElementTree as ET

        parser = ET.XMLParser()
        for chunk in chunks:
            parser.feed(chunk)
        return parser.close()
    import pyexpat

    events = []
    parser = pyexpat.ParserCreate(namespace_separator="|")
    parser.StartElementHandler = lambda name, attrs: events.append(
        ["start", name, list(attrs.items())]
    )
    parser.EndElementHandler = lambda name: events.append(["end", name])
    parser.CharacterDataHandler = lambda value: events.append(["text", value])
    for i, chunk in enumerate(chunks):
        parser.Parse(chunk, i == len(chunks) - 1)
    return events


def canonical(mode: str, value) -> tuple[str, int]:
    if mode == "elementtree":
        # Include every element's expanded name, ordered attributes, text and tail.
        rows = [
            [e.tag, list(e.attrib.items()), e.text, e.tail, len(e)]
            for e in value.iter()
        ]
    else:
        rows = []
        parts = []
        for event in value:
            if event[0] == "text":
                parts.append(event[1])
            else:
                if parts:
                    rows.append(["text", "".join(parts)])
                    parts.clear()
                rows.append(event)
        if parts:
            rows.append(["text", "".join(parts)])
    return hashlib.sha256(
        json.dumps(rows, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest(), len(rows)


def worker(spec_path: Path) -> None:
    spec = json.loads(spec_path.read_text())
    identity = load_consumers(Path(spec["consumer"]))
    if identity["parser_library"]["sha256"] != spec["library_sha256"]:
        raise RuntimeError("wrong parser binary")
    data = Path(spec["input"]).read_bytes()
    if hashlib.sha256(data).hexdigest() != spec["input_sha256"]:
        raise RuntimeError("input changed")
    chunks = [
        data[i : i + spec["chunk"]] for i in range(0, len(data), spec["chunk"])
    ] or [b""]
    # Import ElementTree before timing; parse construction/feed/close and explicit result
    # destruction remain timed. GC collections and canonical output hashing are untimed.
    import xml.etree.ElementTree as ET

    if ET.Element.__module__ != "xml.etree.ElementTree":
        raise RuntimeError("unexpected ElementTree")
    samples = []
    before_hashes = {
        entry["path"]: entry["sha256"]
        for key, entry in identity.items()
        if isinstance(entry, dict)
    }
    expected = None
    for i in range(spec["iterations"] + 1):
        gc.collect()
        before = time.perf_counter_ns()
        value = parse(spec["mode"], chunks)
        parsed = time.perf_counter_ns() - before
        output = canonical(spec["mode"], value)
        if expected is None:
            expected = output
        if output != expected:
            raise RuntimeError("unstable output")
        before = time.perf_counter_ns()
        del value
        destroyed = time.perf_counter_ns() - before
        samples.append(
            {
                "iteration": i,
                "warmup": i == 0,
                "parse_ns": parsed,
                "destruction_ns": destroyed,
                "seconds": (parsed + destroyed) / 1e9,
                "output_sha256": output[0],
                "output_rows": output[1],
            }
        )
    if before_hashes != {path: digest(Path(path)) for path in before_hashes}:
        raise RuntimeError("consumer changed during measurement")
    print(
        json.dumps(
            {
                "identity": identity,
                "python": sys.version,
                "gc_enabled": gc.isenabled(),
                "samples": samples,
            },
            separators=(",", ":"),
        )
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--consumers", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--chunks", type=int, nargs="+", default=[4096, 65536])
    parser.add_argument("--pairs", type=int, default=7)
    parser.add_argument("--iterations", type=int, default=10)
    parser.add_argument("--seed", type=int, default=20260910)
    parser.add_argument("--preflight-only", action="store_true")
    args = parser.parse_args()
    if sys.version_info[:3] != (3, 12, 13):
        parser.error("requires CPython 3.12.13")
    if args.pairs < 3 or args.iterations < 3 or min(args.chunks) < 1:
        parser.error("requires positive chunks and >=3 pairs/iterations")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    corpus = args.corpus.resolve()
    manifest = json.loads(corpus.read_text())
    build = args.consumers.resolve()
    consumers = json.loads(build.read_text())
    if consumers["status"] != "passed":
        parser.error("consumer build did not pass")
    inputs = {}
    for project in manifest["projects"]:
        entry = next(f for f in project["files"] if f["role"] == "input")
        path = (corpus.parent / entry["path"]).resolve(strict=True)
        if digest(path) != entry["sha256"]:
            parser.error("input hash mismatch")
        inputs[project["name"]] = path
    libraries = {e: Path(c["library"]) for e, c in consumers["consumers"].items()}
    observed = [
        Path(__file__).resolve(),
        Path(sys.executable).resolve(),
        corpus,
        build,
        *inputs.values(),
        *[Path(p) for c in consumers["consumers"].values() for p in c["files"]],
    ]
    hashes = {str(p): digest(p) for p in observed}
    report: dict[str, Any] = {
        "status": "failed",
        "method": "Matched unmodified CPython 3.12.13 extensions, loaded explicitly. ElementTree creates and destroys a complete tree; pyexpat installs Python handlers and creates/destroys an event list. Parser creation, feed, finalization, native/Python callbacks and result destruction are timed. Input loading, process startup, imports, explicit gc.collect and complete canonical output checks are untimed. GC remains enabled during parsing. Every sample is checked against its complete canonical untimed output and all matched engines. One warmup per process is discarded; medians of paired process medians.",
        "limitations": "This measures realistic Python XML consumers over original project inputs, not full project execution. Native extension compilation uses identical -O2 without PGO/LTO. No external DTDs, SVG rendering, XSL includes or project-specific semantic processing. Shared host CPU frequency/load/memory bandwidth uncontrolled. Explicit destruction is timed separately after output validation, which warms tree/event cache lines equally but differs from production lifetimes.",
        "python": sys.version,
        "platform": platform.platform(),
        "affinity": sorted(os.sched_getaffinity(0)),
        "seed": args.seed,
        "pairs": args.pairs,
        "iterations": args.iterations,
        "sha256_before": hashes,
        "preflights": [],
        "rows": [],
        "processes": [],
        "summary": {},
    }
    expected = {}
    jobs = [
        (name, chunk, mode)
        for name in inputs
        for chunk in args.chunks
        for mode in ["elementtree", "pyexpat-events"]
    ]

    def execute(name, chunk, mode, engine, pair, iterations):
        key = f"{name}/{chunk}/{mode}"
        label = f"{name}-{chunk}-{mode}-{engine}-{pair}"
        spec = {
            "input": str(inputs[name]),
            "input_sha256": digest(inputs[name]),
            "chunk": chunk,
            "mode": mode,
            "consumer": consumers["consumers"][engine]["directory"],
            "library_sha256": digest(libraries[engine]),
            "iterations": iterations,
        }
        path = output / f"{label}.json"
        path.write_text(json.dumps(spec, indent=2) + "\n")
        command = [
            sys.executable,
            "-I",
            "-S",
            str(Path(__file__).resolve()),
            "--worker",
            str(path),
        ]
        record = {"label": label, "command": command}
        report["processes"].append(record)
        try:
            done = subprocess.run(
                command,
                capture_output=True,
                text=True,
                check=False,
                timeout=180,
                env={
                    key: value
                    for key, value in os.environ.items()
                    if key not in {"LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT"}
                },
            )
            record.update(
                returncode=done.returncode, stdout=done.stdout, stderr=done.stderr
            )
            if done.returncode:
                raise RuntimeError(f"worker failed: {label}")
            result = json.loads(done.stdout)
            if len(result["samples"]) != iterations + 1:
                raise RuntimeError("wrong number of samples")
            for i, sample in enumerate(result["samples"]):
                if (
                    sample["iteration"] != i
                    or sample["warmup"] != (i == 0)
                    or sample["seconds"] <= 0
                ):
                    raise RuntimeError("invalid timing sample")
                actual = (sample["output_sha256"], sample["output_rows"])
                if key in expected and actual != expected[key]:
                    raise RuntimeError(f"canonical consumer output mismatch: {key}")
                expected[key] = actual
            return {"key": key, "engine": engine, "pair": pair, **result}
        except (OSError, subprocess.TimeoutExpired) as error:
            record["failure"] = repr(error)
            raise

    try:
        for name, chunk, mode in jobs:
            for engine in libraries:
                report["preflights"].append(
                    execute(name, chunk, mode, engine, "preflight", 0)
                )
        if not args.preflight_only:
            randomizer = random.Random(args.seed)
            for pair in range(args.pairs):
                randomizer.shuffle(jobs)
                for name, chunk, mode in jobs:
                    order = list(libraries)
                    randomizer.shuffle(order)
                    for position, engine in enumerate(order):
                        row = execute(name, chunk, mode, engine, pair, args.iterations)
                        row["order"] = position
                        report["rows"].append(row)
            for name, chunk, mode in jobs:
                key = f"{name}/{chunk}/{mode}"
                medians = {
                    engine: [
                        statistics.median(
                            s["seconds"]
                            for s in next(
                                row
                                for row in report["rows"]
                                if row["key"] == key
                                and row["engine"] == engine
                                and row["pair"] == pair
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
        report["sha256_after"] = {str(p): digest(p) for p in observed}
        if hashes != report["sha256_after"]:
            report["status"] = "failed"
            report["failure"] = "input/source/consumer changed during measurement"
        (output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {
                "status": report["status"],
                "preflights": len(report["preflights"]),
                "timed_processes": len(report["rows"]),
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
