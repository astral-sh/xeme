"""Gate native Rust XML 1.0 Fifth Edition acceptance against the pinned W3C corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import resource
import signal
import subprocess
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from w3c import run as corpus

PIN = Path(__file__).with_name("native-corpus.json")
FILE_BYTES = 2 * 1024 * 1024
WALL_SECONDS = 120


def digest(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def save(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n")


def source_hashes(suite: Path) -> dict[str, str]:
    return {
        str(path.relative_to(suite)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in suite.rglob("*")
        if path.is_file()
    }


def selected_rows(catalog: list[dict], sources: dict, pin: dict) -> list[dict]:
    """Bind the entire corpus and selection, independently of Expat outcomes."""
    if digest(sources) != pin["source_digest"]:
        raise ValueError("W3C pinned corpus changed")
    if digest(catalog) != pin["catalog_digest"]:
        raise ValueError("W3C catalog descriptors changed")
    selected = [case for case in catalog if not case.get("skip")]
    if (
        len(catalog) != pin["catalog_descriptors"]
        or len(selected) != pin["selected_descriptors"]
    ):
        raise ValueError("W3C catalog selection changed")
    expectations = {
        "valid": "accept",
        "invalid": "accept",
        "not-wf": "reject",
        "error": "optional",
    }
    corrections = pin["expectation_corrections"]
    if not corrections.keys() <= {case["ID"] for case in selected}:
        raise ValueError("standard correction refers to an unselected case")
    rows = []
    for case in selected:
        expected = expectations[case["TYPE"]]
        if correction := corrections.get(case["ID"]):
            if correction["catalog_type"] != case["TYPE"] or correction[
                "expected"
            ] not in ("accept", "reject"):
                raise ValueError("standard correction no longer matches the catalog")
            expected = correction["expected"]
        for chunk in pin["chunks"]:
            rows.append(
                {
                    "id": case["ID"],
                    "path": case["path"],
                    "type": case["TYPE"],
                    "chunk": chunk,
                    "namespaces": case.get("NAMESPACE", "yes") != "no",
                    "expected": expected,
                }
            )
    keys = [(row["id"], row["chunk"]) for row in rows]
    if len(keys) != len(set(keys)) or digest(keys) != pin["inventory_digest"]:
        raise ValueError("W3C row inventory changed")
    return rows


class NativeEngine:
    """Supply bounded local resources to the native parser's external children."""

    def __init__(self, runner: Path, suite: Path):
        self.suite = suite
        self.process = subprocess.Popen(
            [str(runner)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            encoding="utf-8",
        )

    def send(self, value: dict) -> None:
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(value, separators=(",", ":")) + "\n")
        self.process.stdin.flush()

    def read_file(self, path: Path, loaded: list[dict]) -> dict:
        path = path.resolve(strict=True)
        if not path.is_relative_to(self.suite):
            raise ValueError("URI outside suite")
        with path.open("rb") as stream:
            data = stream.read(FILE_BYTES + 1)
        if len(data) > FILE_BYTES:
            raise ValueError("file budget")
        loaded.append(
            {
                "path": str(path.relative_to(self.suite)),
                "sha256": hashlib.sha256(data).hexdigest(),
            }
        )
        return {"base": path.as_uri(), "data": list(data)}

    def parse(self, case: dict) -> dict:
        loaded: list[dict] = []
        resolver_errors: list[str] = []
        self.send(
            {
                "op": "parse",
                "chunk": case["chunk"],
                "namespaces": case["namespaces"],
                **self.read_file(self.suite / case["path"], loaded),
            }
        )
        assert self.process.stdout is not None
        while True:
            line = self.process.stdout.readline()
            if not line:
                raise ValueError("native parser exited without a result")
            message = json.loads(line)
            if message.get("op") == "resolve":
                try:
                    path = corpus.resolve(message["base"], message["system"])
                    reply = self.read_file(path, loaded)
                except (OSError, ValueError) as error:
                    error_message = f"{type(error).__name__}: {error}"
                    resolver_errors.append(error_message)
                    reply = {"error": error_message}
                self.send(reply)
            elif message.get("op") == "result":
                del message["op"]
                message["loaded"] = loaded
                message["resolver_errors"] = list(
                    dict.fromkeys([*resolver_errors, *message["resolver_errors"]])
                )
                for child in message["children"]:
                    child["path"] = str(
                        corpus.resolve(child.pop("base"), "").relative_to(self.suite)
                    )
                return message
            else:
                raise ValueError("unexpected native parser protocol message")

    def close(self) -> None:
        if self.process.stdin:
            self.process.stdin.close()
        try:
            status = self.process.wait(timeout=5)
            if status:
                raise ValueError(f"native parser exited {status}")
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
            raise
        finally:
            if self.process.stdout:
                self.process.stdout.close()


def worker(runner: Path, suite: Path, output: Path) -> None:
    engine = NativeEngine(runner, suite)
    try:
        with (output / "native.jsonl").open("w") as stream:
            for case in json.loads((output / "cases.json").read_text()):
                save(output / "progress.json", case)
                row = {**case, "result": engine.parse(case)}
                stream.write(json.dumps(row, separators=(",", ":")) + "\n")
                stream.flush()
    finally:
        engine.close()


def evaluate(rows: list[dict], expected: list[dict], sources: dict) -> dict:
    """Require every standard expectation; incomplete or inconclusive runs fail."""
    if [
        {key: value for key, value in row.items() if key != "result"} for row in rows
    ] != expected:
        raise ValueError(
            "native W3C inventory is incomplete or differs from selected cases"
        )
    mismatches, inconclusive, catalog_mismatches = [], [], []
    optional = 0
    for row in rows:
        result = row["result"]
        if (
            result["name_rules"] != "FifthEdition"
            or type(result["status"]) is not int
            or result["status"] not in (0, 1)
        ):
            raise ValueError("invalid native parser mode or status")
        loaded = result["loaded"]
        if (
            not loaded
            or loaded[0]["path"] != row["path"]
            or any(item["sha256"] != sources.get(item["path"]) for item in loaded)
        ):
            raise ValueError("native parser input differs from the pinned corpus")
        children = result["children"]
        if any(
            child["path"] not in {item["path"] for item in loaded}
            or type(child["status"]) is not int
            or child["status"] not in (0, 1)
            for child in children
        ):
            raise ValueError("invalid native external child result")
        if result["status"] == 1 and any(child["status"] == 0 for child in children):
            raise ValueError(
                "native parser accepted a document with a failed external child"
            )
        if any(
            (item["status"] == 1) != (item["error"] is None)
            for item in (result, *children)
        ):
            raise ValueError("native parser status and error disagree")
        if result["resolver_errors"] or any(
            item["error"] in ("LimitExceeded", "NoMemory")
            for item in (result, *children)
        ):
            inconclusive.append(row)
            continue
        accepted = result["status"] == 1
        if row["type"] != "error" and accepted != (row["type"] in ("valid", "invalid")):
            catalog_mismatches.append(row)
        if row["expected"] == "optional":
            optional += 1
        elif accepted != (row["expected"] == "accept"):
            mismatches.append(row)
    return {
        "passed": not mismatches and not inconclusive,
        "rows": len(rows),
        "mandatory_pass": len(rows) - optional - len(mismatches) - len(inconclusive),
        "mandatory_fail": len(mismatches),
        "optional_observations": optional,
        "mismatches": mismatches,
        "inconclusive": inconclusive,
        "raw_catalog_mismatches": catalog_mismatches,
    }


def limits() -> None:
    resource.setrlimit(resource.RLIMIT_AS, (1024**3, 1024**3))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", type=Path, required=True)
    parser.add_argument("--runner", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    suite, runner, output = (
        args.suite.resolve(strict=True),
        args.runner.resolve(strict=True),
        args.output.resolve(),
    )
    corpus.SUITE = suite
    if args.worker:
        worker(runner, suite, output)
        return 0
    output.mkdir(parents=True, exist_ok=False)
    report: dict[str, Any] = {
        "passed": False,
        "interface": "native Rust",
        "name_rules": "FifthEdition",
    }
    try:
        pin = json.loads(PIN.read_text())
        report.update(
            {
                "scope": "Nonvalidating XML 1.0 Fifth Edition and Namespaces 1.0 acceptance; original catalog plus cited standards corrections. No Expat outcomes or failure allowances.",
                "corpus": pin,
                "runner": str(runner),
                "runner_sha256": hashlib.sha256(runner.read_bytes()).hexdigest(),
                "harness_sha256": {
                    path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                    for path in (Path(__file__), Path(corpus.__file__), PIN)
                },
                "worker_limits": {
                    "address_space_bytes": 1024**3,
                    "wall_seconds": WALL_SECONDS,
                    "core_bytes": 0,
                },
                "resolver_limits": {
                    "file_bytes": FILE_BYTES,
                    "depth": 32,
                    "requests": 1024,
                },
            }
        )
        sources = source_hashes(suite)
        catalog = corpus.selected_catalog()
        save(output / "catalog.json", catalog)
        save(output / "sources.json", sources)
        expected = selected_rows(catalog, sources, pin)
        save(output / "cases.json", expected)
        command = [
            sys.executable,
            "-I",
            "-S",
            str(Path(__file__).resolve()),
            "--suite",
            str(suite),
            "--runner",
            str(runner),
            "--output",
            str(output),
            "--worker",
        ]
        report["command"] = command
        environment = os.environ.copy()
        for key in ("LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT", "PYTHONPATH"):
            environment.pop(key, None)
        with (
            (output / "worker.log").open("w") as log,
            subprocess.Popen(
                command,
                stdout=log,
                stderr=subprocess.STDOUT,
                env=environment,
                preexec_fn=limits,
                start_new_session=True,
            ) as process,
        ):
            try:
                report["worker_returncode"] = process.wait(timeout=WALL_SECONDS)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
                raise
        if report["worker_returncode"]:
            raise ValueError(
                "native W3C worker failed; see worker.log and progress.json"
            )
        if hashlib.sha256(runner.read_bytes()).hexdigest() != report["runner_sha256"]:
            raise ValueError("native parser binary changed during the suite")
        rows = [
            json.loads(line)
            for line in (output / "native.jsonl").read_text().splitlines()
        ]
        report.update(evaluate(rows, expected, sources))
    except (
        ValueError,
        OSError,
        KeyError,
        TypeError,
        subprocess.SubprocessError,
    ) as error:
        report.update(passed=False, error=f"{type(error).__name__}: {error}")
    finally:
        save(output / "summary.json", report)
    print(
        json.dumps(
            {
                key: value
                for key, value in report.items()
                if key
                in (
                    "passed",
                    "rows",
                    "mandatory_pass",
                    "mandatory_fail",
                    "optional_observations",
                    "error",
                )
            }
        )
    )
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
