#!/usr/bin/env python3
"""Compare the Expat C ABI on a deterministic corpus in isolated worker processes."""

from __future__ import annotations

import argparse
import base64
import ctypes
import ctypes.util
import hashlib
import json
import os
import platform
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from corpus import Case, cases
from xml_abi import Expat


def library_identity(engine: Expat) -> dict[str, str]:
    """Identify the shared object that actually provides XML_Parse."""

    class DlInfo(ctypes.Structure):
        _fields_ = [
            ("filename", ctypes.c_char_p),
            ("base", ctypes.c_void_p),
            ("symbol", ctypes.c_char_p),
            ("address", ctypes.c_void_p),
        ]

    dladdr = ctypes.CDLL(None).dladdr
    dladdr.argtypes = [ctypes.c_void_p, ctypes.POINTER(DlInfo)]
    dladdr.restype = ctypes.c_int
    info = DlInfo()
    if (
        not dladdr(
            ctypes.cast(engine.lib.XML_Parse, ctypes.c_void_p), ctypes.byref(info)
        )
        or not info.filename
    ):
        raise RuntimeError("cannot identify the loaded XML_Parse library")
    path = Path(os.fsdecode(info.filename)).resolve(strict=True)
    return {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def case_identity(row: dict) -> tuple:
    """Bind a result to its requested input bytes, chunk width and namespace mode."""
    return (row["name"], row["chunk_size"], row["namespaces"], row["sha256"])


def check_worker(
    observed: dict,
    corpus: list[dict],
    corpus_sha256: str,
    library: str,
    library_sha256: str | None,
    capture: bool,
) -> None:
    """Reject common truncation, input substitution and wrong-library false greens."""
    expected = [case_identity(case) for case in corpus]
    actual = [case_identity(row) for row in observed["results"]]
    if not expected or len(set(expected)) != len(expected) or actual != expected:
        raise RuntimeError("worker case inventory differs from the requested corpus")
    if (
        observed["corpus_sha256"] != corpus_sha256
        or observed["capture_callbacks"] != capture
    ):
        raise RuntimeError("worker corpus or callback mode differs from the request")
    identity = observed["library"]
    if identity != observed["library_after"]:
        raise RuntimeError("loaded worker library changed during parsing")
    if observed["requested_library"] != library:
        raise RuntimeError("worker used a different library selector")
    if library_sha256 is not None and (
        identity["path"] != str(Path(library).resolve(strict=True))
        or identity["sha256"] != library_sha256
    ):
        raise RuntimeError("worker XML_Parse origin differs from the selected library")


def worker(library: str, corpus: Path, output: Path, capture: bool) -> None:
    engine = Expat(library)
    identity = library_identity(engine)
    corpus_bytes = corpus.read_bytes()
    rows = []
    for case in json.loads(corpus_bytes):
        output.with_suffix(".progress.json").write_text(
            json.dumps({"name": case["name"], "chunk_size": case["chunk_size"]}) + "\n"
        )
        data = base64.b64decode(case["base64"], validate=True)
        input_sha256 = hashlib.sha256(data).hexdigest()
        if input_sha256 != case["sha256"]:
            raise RuntimeError("corpus input bytes do not match their recorded hash")
        result = engine.parse(
            data, case["chunk_size"], case["namespaces"], capture=capture
        )
        rows.append(
            {
                "name": case["name"],
                "chunk_size": case["chunk_size"],
                "namespaces": case["namespaces"],
                "sha256": input_sha256,
                "result": result,
            }
        )
    output.write_text(
        json.dumps(
            {
                "version": engine.version,
                "requested_library": library,
                "library": identity,
                "library_after": library_identity(engine),
                "corpus_sha256": hashlib.sha256(corpus_bytes).hexdigest(),
                "capture_callbacks": capture,
                "results": rows,
            },
            indent=2,
        )
        + "\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", required=True)
    parser.add_argument("--reference", default=ctypes.util.find_library("expat"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--generated", type=int, default=100)
    parser.add_argument(
        "--corpus-dir",
        type=Path,
        help="Also compare every .xml file below this directory",
    )
    parser.add_argument(
        "--corpus-namespaces",
        action="store_true",
        help="Enable namespace processing for imported .xml files",
    )
    parser.add_argument("--seed", type=int, default=20260910)
    parser.add_argument("--chunks", type=int, nargs="+", default=[1, 2, 3, 7, 64, 4096])
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument(
        "--acceptance-only",
        action="store_true",
        help="Compare status, error code, and location without registering callbacks",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Fail on callback fragmentation and location differences too",
    )
    parser.add_argument("--worker", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.worker:
        worker(args.library, args.worker, args.output, not args.acceptance_only)
        return 0
    if not args.reference or args.generated < 0 or min(args.chunks) < 1:
        parser.error(
            "reference library, nonnegative generated count, and positive chunk sizes required"
        )
    args.output.mkdir(parents=True, exist_ok=False)
    selected_cases = cases(args.seed, args.generated)
    if args.corpus_dir:
        corpus_root = args.corpus_dir.resolve(strict=True)
        selected_cases.extend(
            Case(
                f"imported/{path.relative_to(corpus_root)}",
                path.read_bytes(),
                args.corpus_namespaces,
                "imported",
            )
            for path in sorted(corpus_root.rglob("*.xml"))
            if path.is_file()
        )
    corpus = [
        {
            "name": case.name,
            "category": case.category,
            "base64": base64.b64encode(case.data).decode(),
            "sha256": hashlib.sha256(case.data).hexdigest(),
            "namespaces": case.namespaces,
            "chunk_size": chunk,
        }
        for case in selected_cases
        for chunk in args.chunks
    ]
    corpus_path = args.output / "corpus.json"
    corpus_path.write_text(json.dumps(corpus, indent=2) + "\n")
    corpus_sha256 = hashlib.sha256(corpus_path.read_bytes()).hexdigest()
    report = {
        "status": "failed",
        "platform": platform.platform(),
        "seed": args.seed,
        "cases": len(corpus),
        "corpus_sha256": corpus_sha256,
        "scope": (
            "Full-document status, error code, and final location only. No callbacks registered; this is not a callback compatibility gate."
            if args.acceptance_only
            else "Full-document status, error code, callback stream, and final location. Adjacent text callbacks are coalesced only in the separately reported semantic comparison. No differences are waived."
        ),
        "capture_callbacks": not args.acceptance_only,
        "libraries": {},
        "differences": [],
    }
    try:
        for name, library in (("reference", args.reference), ("xeme", args.library)):
            path = Path(library)
            metadata = {
                "path": library,
                "sha256_before": hashlib.sha256(path.read_bytes()).hexdigest()
                if path.is_file()
                else None,
            }
            report["libraries"][name] = metadata
            command = [
                sys.executable,
                "-I",
                "-S",
                str(Path(__file__).resolve()),
                "--library",
                library,
                "--worker",
                str(corpus_path),
                "--output",
                str(args.output / f"{name}.json"),
            ]
            if args.acceptance_only:
                command.append("--acceptance-only")
            environment = os.environ.copy()
            for key in ("LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT"):
                environment.pop(key, None)
            completed = subprocess.run(
                command,
                env=environment,
                timeout=args.timeout,
                capture_output=True,
                text=True,
                check=False,
            )
            metadata.update(
                {
                    "command": command,
                    "returncode": completed.returncode,
                    "stderr": completed.stderr,
                }
            )
            if completed.returncode:
                raise RuntimeError(
                    f"{name} worker exited {completed.returncode}: {completed.stderr}"
                )
            metadata["sha256_after"] = (
                hashlib.sha256(path.read_bytes()).hexdigest()
                if path.is_file()
                else None
            )
            if metadata["sha256_before"] != metadata["sha256_after"]:
                raise RuntimeError(f"{name} library changed during the run")
        reference = json.loads((args.output / "reference.json").read_text())
        actual = json.loads((args.output / "xeme.json").read_text())
        report["versions"] = {
            "reference": reference["version"],
            "xeme": actual["version"],
        }
        for name, observed in (("reference", reference), ("xeme", actual)):
            metadata = report["libraries"][name]
            check_worker(
                observed,
                corpus,
                corpus_sha256,
                metadata["path"],
                metadata["sha256_before"],
                not args.acceptance_only,
            )
            metadata["loaded_library"] = observed["library"]
        if hashlib.sha256(corpus_path.read_bytes()).hexdigest() != corpus_sha256:
            raise RuntimeError("requested corpus changed during the run")
        counts = {
            "status": 0,
            "error": 0,
            "normalized_events": 0,
            "events": 0,
            "position": 0,
            "callback_errors": 0,
        }
        for expected, observed in zip(
            reference["results"], actual["results"], strict=True
        ):
            if (expected["name"], expected["chunk_size"]) != (
                observed["name"],
                observed["chunk_size"],
            ):
                raise RuntimeError("worker case order differs")
            fields = [
                field
                for field in counts
                if expected["result"][field] != observed["result"][field]
            ]
            if (
                expected["result"]["callback_errors"]
                or observed["result"]["callback_errors"]
            ) and "callback_errors" not in fields:
                fields.append("callback_errors")
            if fields:
                report["differences"].append(
                    {
                        "name": expected["name"],
                        "chunk_size": expected["chunk_size"],
                        "fields": fields,
                        "reference": expected["result"],
                        "xeme": observed["result"],
                    }
                )
                for field in fields:
                    counts[field] += 1
        report["difference_counts"] = counts
        report["semantic_pass"] = not any(
            counts[key]
            for key in ("status", "error", "normalized_events", "callback_errors")
        )
        report["exact_pass"] = not any(counts.values())
        report["status"] = (
            "passed"
            if report["exact_pass" if args.strict else "semantic_pass"]
            else "failed"
        )
    except (
        RuntimeError,
        ValueError,
        KeyError,
        TypeError,
        subprocess.TimeoutExpired,
        OSError,
    ) as error:
        report["failure"] = str(error)
    finally:
        (args.output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {
                key: value
                for key, value in report.items()
                if key not in {"differences", "libraries"}
            },
            indent=2,
        )
    )
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
