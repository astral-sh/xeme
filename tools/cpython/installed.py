"""Validate the actual installed Xeme CPython with the same narrow exception.

Run under the installed interpreter with -I. No extension loading override or
consumer patch is applied. Distribution structure and threaded parsing need
separate validation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import runpy
import subprocess
import sys
import sysconfig
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    import pyexpat

    if sys.version_info[:3] != (3, 12, 13) or not sys.flags.isolated:
        parser.error("use the installed CPython 3.12.13 interpreter with -I")
    if not pyexpat.EXPAT_VERSION.startswith("xeme_"):
        parser.error("installed interpreter did not load Xeme")
    gate_path = Path(__file__).with_name("run.py")
    gate = runpy.run_path(str(gate_path), run_name="xeme_cpython_gate")
    test_directory = Path(sysconfig.get_path("stdlib")) / "test"
    for filename, expected in gate["FRAGMENTATION_SOURCE_HASHES"].items():
        if (
            hashlib.sha256((test_directory / filename).read_bytes()).hexdigest()
            != expected
        ):
            parser.error(f"installed upstream fixture changed: {filename}")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    temporary = output / "tmp"
    temporary.mkdir()
    env["TMPDIR"] = str(temporary)

    def execute(
        name: str, arguments: list[str], timeout: int
    ) -> subprocess.CompletedProcess:
        with (output / f"{name}.log").open("w") as log:
            return subprocess.run(
                arguments,
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
                timeout=timeout,
                check=False,
            )

    inventory_command = [
        sys.executable,
        "-I",
        "-m",
        "test",
        "--list-cases",
        *gate["TESTS"],
    ]
    inventory = execute("test-inventory", inventory_command, 120)
    if inventory.returncode:
        return inventory.returncode
    cases = gate["test_inventory"]((output / "test-inventory.log").read_text())
    command = [
        sys.executable,
        "-I",
        "-m",
        "test",
        "-v",
        "-j1",
        "--timeout",
        "120",
        *gate["TESTS"],
    ]
    result = execute("tests", command, 900)
    log = (output / "tests.log").read_text()
    total = re.search(r"^Total tests: run=(\d+)\b", log, re.MULTILINE)
    complete = total is not None and gate["complete_inventory"](
        log, cases, int(total[1])
    )
    known = gate["only_text_fragmentation"](
        log, result.returncode, len(gate["TESTS"]), cases, test_directory
    )
    semantic_script = Path(__file__).with_name("text_fragmentation.py")
    semantic_command = [sys.executable, "-I", str(semantic_script), "--installed"]
    semantic = execute("text-fragmentation", semantic_command, 120)
    accepted = (
        complete and semantic.returncode == 0 and (result.returncode == 0 or known)
    )
    report = {
        "schema_version": 1,
        "python": sys.version,
        "executable": sys.executable,
        "expat": pyexpat.EXPAT_VERSION,
        "test_command": command,
        "tests_exit_code": result.returncode,
        "gate_exit_code": 0 if accepted else 1,
        "accepted_upstream_failures": bool(accepted and known),
        "inventory_command": inventory_command,
        "inventory_case_count": len(cases),
        "inventory_sha256": gate["TEST_INVENTORY_SHA256"],
        "inventory_complete": complete,
        "upstream_fixture_sha256": gate["FRAGMENTATION_SOURCE_HASHES"],
        "gate_source_sha256": hashlib.sha256(gate_path.read_bytes()).hexdigest(),
        "semantic_command": semantic_command,
        "semantic_exit_code": semantic.returncode,
        "semantic_source_sha256": hashlib.sha256(
            semantic_script.read_bytes()
        ).hexdigest(),
    }
    (output / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return report["gate_exit_code"]


if __name__ == "__main__":
    raise SystemExit(main())
