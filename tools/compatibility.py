"""Run pinned compatibility suites and reject changes to reviewed failure boundaries."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "tools/compatibility-baseline.json"


def digest(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def api_assertions(log: str) -> dict[tuple[str, str], list[str]]:
    """Associate assertion locations with their isolated test invocation."""
    records: dict[tuple[str, str], list[str]] = {}
    current = None
    for line in log.splitlines():
        if line.startswith("XEME_BEGIN\t"):
            _, context, name = line.split("\t")
            current = (context, name)
            if current in records:
                raise ValueError(f"duplicate API invocation: {current}")
            records[current] = []
        elif line.startswith("ASSERTION:"):
            match = re.fullmatch(r"ASSERTION: (\w+) at (.+):(\d+)", line)
            if current is None or match is None:
                raise ValueError("unattributed API assertion")
            records[current].append(f"{match[1]} at {Path(match[2]).name}:{match[3]}")
    return records


def check_api(result: dict, log: str, baseline: dict, reference: bool) -> dict:
    """Keep all passing configurations and each known failing assertion observable."""
    expected = {
        (context, name)
        for context in baseline["contexts"]
        for name in baseline["tests"]
    }
    rows = result["results"]
    keys = [(row["context"], row["test"]) for row in rows]
    assertions = api_assertions(log)
    if len(keys) != len(set(keys)) or set(keys) != expected:
        raise ValueError("API configuration inventory changed or is incomplete")
    if set(assertions) != expected:
        raise ValueError("API log inventory differs from structured results")
    if (
        not result["library_origin_verified"]
        or not result["selection_complete"]
        or result["timed_out"]
        or result["returncode"] not in (0, 1)
    ):
        raise ValueError("API runner failed or did not verify the selected library")
    failures = sum(row["outcome"] != "pass" for row in rows)
    if (
        result["failed"] != failures
        or result["passed"] != len(rows) - failures
        or result["returncode"] != int(bool(failures))
    ):
        raise ValueError("API result counts or exit status are inconsistent")
    regressions = []
    improvements = []
    known_failures = 0
    for row, key in zip(rows, keys, strict=True):
        known = None if reference else baseline["failures"].get(row["test"])
        allowed = known is not None and row["context"] in known["contexts"]
        if row["outcome"] == "pass" and row["code"] == 0 and not assertions[key]:
            if allowed:
                improvements.append(row)
        elif (
            allowed
            and row["outcome"] == "fail"
            and row["code"] == 100
            and assertions[key] == known["assertions"]
        ):
            known_failures += 1
        else:
            regressions.append({**row, "assertions": assertions[key]})
    return {
        "passed": not regressions,
        "configurations": len(rows),
        "known_failures": known_failures,
        "regressions": regressions,
        "improvements": improvements,
    }


def check_w3c(directory: Path, baseline: dict) -> dict:
    """Compare complete acceptance and child outcomes without waiving conformance."""
    summary = json.loads((directory / "summary.json").read_text())
    catalog = json.loads((directory / "catalog.json").read_text())
    if digest(catalog) != baseline["catalog_digest"]:
        raise ValueError("W3C catalog descriptors changed")
    expected_rows = [
        {
            "id": case["ID"],
            "path": case["path"],
            "type": case["TYPE"],
            "chunk": chunk,
            "namespaces": case.get("NAMESPACE", "yes") != "no",
        }
        for case in catalog
        if not case.get("skip")
        for chunk in baseline["chunks"]
    ]
    if summary["worker_failures"] or any(summary["inconclusive"].values()):
        raise ValueError("W3C workers failed or produced inconclusive results")
    if digest(summary["catalog_source_sha256"]) != baseline["source_digest"]:
        raise ValueError("W3C pinned corpus changed")
    if (
        summary["catalog_descriptors"] != baseline["catalog_descriptors"]
        or summary["selected_descriptors"] != baseline["selected_descriptors"]
        or summary["chunks"] != baseline["chunks"]
    ):
        raise ValueError("W3C catalog selection changed")
    expected_mismatches = {
        (name, chunk)
        for name in baseline["catalog_failures"]
        for chunk in baseline["chunks"]
    }
    improvements = {}
    rows = {}
    for engine in ("reference", "xeme"):
        current = json.loads((directory / f"{engine}.json").read_text())["rows"]
        keys = [(row["id"], row["chunk"]) for row in current]
        if len(keys) != len(set(keys)) or digest(keys) != baseline["inventory_digest"]:
            raise ValueError(f"{engine}: W3C row inventory changed")
        if [
            {key: value for key, value in row.items() if key != "result"}
            for row in current
        ] != expected_rows:
            raise ValueError(f"{engine}: W3C case selection differs from catalog")
        for row in current:
            loaded = row["result"]["loaded"]
            if not loaded or loaded[0]["path"] != row["path"]:
                raise ValueError(f"{engine}: W3C root input differs from catalog")
            if any(
                item["sha256"] != summary["catalog_source_sha256"].get(item["path"])
                for item in loaded
            ):
                raise ValueError(
                    f"{engine}: W3C loaded bytes differ from pinned corpus"
                )
        mismatches = {
            (row["id"], row["chunk"]) for row in summary["mismatches"][engine]
        }
        observed_mismatches = [
            row
            for row in current
            if row["type"] != "error"
            and (
                (row["result"]["status"] == 1) != (row["type"] in ("valid", "invalid"))
            )
        ]
        if summary["mismatches"][engine] != observed_mismatches:
            raise ValueError(f"{engine}: W3C summary differs from raw outcomes")
        if mismatches - expected_mismatches:
            raise ValueError(f"{engine}: new mandatory W3C conformance failures")
        improvements[engine] = sorted(expected_mismatches - mismatches)
        rows[engine] = current
    differences = []
    for expected, actual in zip(rows["reference"], rows["xeme"], strict=True):
        for row in (expected, actual):
            if row["result"]["resolver_errors"]:
                raise ValueError("W3C resolver failed")

        # Error codes and positions remain diagnostic differences. Acceptance,
        # loaded bytes, and child success/failure are separate semantic contracts.
        def semantic(row: dict) -> tuple:
            result = row["result"]
            return (
                result["status"],
                result["loaded"],
                [(child["path"], child["status"]) for child in result["children"]],
            )

        if semantic(expected) != semantic(actual):
            differences.append({"reference": expected, "xeme": actual})
    return {
        "passed": not differences,
        "rows_per_engine": len(rows["reference"]),
        "conformance_failures": {
            engine: len(summary["mismatches"][engine]) for engine in rows
        },
        "differences": differences,
        "improvements": improvements,
    }


def run(command: list[str], output: Path, accepted: tuple[int, ...]) -> None:
    environment = os.environ.copy()
    for key in ("LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT", "PYTHONPATH"):
        environment.pop(key, None)
    with output.open("w") as stream:
        result = subprocess.run(
            command,
            env=environment,
            stdout=stream,
            stderr=subprocess.STDOUT,
            timeout=600,
            check=False,
        )
    if result.returncode not in accepted:
        raise ValueError(f"suite exited {result.returncode}; see {output}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("suite", choices=("api", "w3c", "differential"))
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--config", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.suite in ("api", "w3c") and args.source is None:
        parser.error("--source is required for API and W3C suites")
    if args.suite == "api" and args.config is None:
        parser.error("--config is required for the API suite")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    baseline = json.loads(BASELINE.read_text())
    libraries = {
        label: path.resolve(strict=True)
        for label, path in (("xeme", args.library), ("reference", args.reference))
    }
    hashes = {
        name: hashlib.sha256(path.read_bytes()).hexdigest()
        for name, path in libraries.items()
    }
    report: dict[str, Any] = {
        "suite": args.suite,
        "passed": False,
        "library_sha256": hashes,
        "baseline_sha256": hashlib.sha256(BASELINE.read_bytes()).hexdigest(),
        "commands": [],
    }
    try:
        if args.suite == "api":
            report["engines"] = {}
            for label, library in libraries.items():
                directory = args.output / label
                command = [
                    sys.executable,
                    "-I",
                    "-S",
                    str(ROOT / "tools/upstream-expat/run.py"),
                    "--source",
                    str(args.source.resolve()),
                    "--config",
                    str(args.config.resolve()),
                    "--library",
                    str(library),
                    "--output",
                    str(directory),
                ]
                if label == "reference":
                    # Upstream reserves a 1 GiB buffer and streams over 2 GiB.
                    # Keep the RSS cap, but allow these reference tests to finish.
                    command.extend(["--memory-mib", "4096", "--test-timeout", "30"])
                report["commands"].append(command)
                run(
                    command,
                    args.output / f"{label}.log",
                    (0,) if label == "reference" else (0, 1),
                )
                report["engines"][label] = check_api(
                    json.loads((directory / "results.json").read_text()),
                    (directory / "tests.log").read_text(),
                    baseline["api"],
                    label == "reference",
                )
            report["passed"] = all(
                result["passed"] for result in report["engines"].values()
            )
        else:
            directory = args.output / "results"
            script = (
                "tools/w3c/run.py" if args.suite == "w3c" else "tools/differential.py"
            )
            command = [
                sys.executable,
                "-I",
                "-S",
                str(ROOT / script),
                "--library",
                str(libraries["xeme"]),
                "--reference",
                str(libraries["reference"]),
                "--output",
                str(directory),
            ]
            if args.suite == "w3c":
                command.extend(["--suite", str(args.source.resolve())])
            else:
                command.extend(["--generated", "200", "--seed", "20260910"])
            report["commands"].append(command)
            run(
                command,
                args.output / "suite.log",
                (0, 1) if args.suite == "w3c" else (0,),
            )
            if args.suite == "w3c":
                report.update(check_w3c(directory, baseline["w3c"]))
            else:
                report["passed"] = True
        if hashes != {
            name: hashlib.sha256(path.read_bytes()).hexdigest()
            for name, path in libraries.items()
        }:
            raise ValueError("selected library changed during the suite")
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        report["passed"] = False
        report["error"] = str(error)
    finally:
        (args.output / "gate.json").write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {
                key: value
                for key, value in report.items()
                if key not in ("commands", "engines")
            }
        )
    )
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
