#!/usr/bin/env python3
"""Compare pinned unmodified and fixed CPython extensions under child-create failure.

Linux only: the interposer forces XML_ExternalEntityParserCreate to return NULL.
Every extension runs in a separate, bounded process. Original crashes are recorded
as crashes; a successful diagnostic verifies that the patched extensions recover.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shlex
import signal
import subprocess
import sys
from pathlib import Path


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--original-extension", type=Path, action="append", required=True
    )
    parser.add_argument("--fixed-extension", type=Path, action="append", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != "linux" or sys.version_info[:3] != (3, 12, 13):
        parser.error("use Linux CPython 3.12.13 matching the compiled extensions")
    output = args.output.resolve()
    if output.exists() and any(output.iterdir()):
        parser.error("output must be empty")
    output.mkdir(parents=True, exist_ok=True)
    directory = Path(__file__).resolve().parent
    root = directory.parents[2]
    shim = output / "fail-child.so"
    command = [
        *shlex.split(os.environ.get("CC", "cc")),
        "-shared",
        "-fPIC",
        "-Wall",
        "-Wextra",
        "-Werror",
        f"-I{root / 'include'}",
        str(directory / "fail-child.c"),
        "-o",
        str(shim),
    ]
    build = subprocess.run(
        command, text=True, capture_output=True, timeout=30, check=False
    )
    (output / "build.log").write_text(build.stdout + build.stderr)
    if build.returncode:
        return build.returncode
    results = []
    success = True
    for kind, extensions in [
        ("original", args.original_extension),
        ("fixed", args.fixed_extension),
    ]:
        for index, extension in enumerate(extensions):
            extension = extension.resolve(strict=True)
            label = f"{kind}-{index}"
            command = [
                sys.executable,
                "-I",
                "-S",
                "-X",
                "faulthandler",
                str(directory / "probe.py"),
                str(extension),
                str(shim),
            ]
            env = dict(os.environ, LD_PRELOAD=str(shim), PYTHONPATH="")
            timed_out = False
            try:
                result = subprocess.run(
                    command,
                    env=env,
                    text=True,
                    capture_output=True,
                    timeout=15,
                    check=False,
                )
                code, stdout, stderr = result.returncode, result.stdout, result.stderr
            except subprocess.TimeoutExpired:
                code, stdout, stderr = None, "", "probe exceeded 15 seconds\n"
                timed_out = True
            (output / f"{label}.log").write_text(stdout + stderr)
            observed = json.loads(stdout) if stdout.strip() else None
            expected = not timed_out and (
                code == -signal.SIGSEGV
                if kind == "original"
                else code == 0
                and observed is not None
                and observed["delta"] == 0
                and observed["memory_error"]
                and observed["shim_calls"] == 1
            )
            success &= expected
            results.append(
                {
                    "kind": kind,
                    "extension": str(extension),
                    "extension_sha256": sha256(extension),
                    "command": command,
                    "exit_code": code,
                    "timed_out": timed_out,
                    "probe": observed,
                    "matches_expected_failure_or_recovery": expected,
                }
            )
    manifest = {
        "provenance": json.loads((directory / "provenance.json").read_text()),
        "python": sys.version,
        "build_command": build.args,
        "shim_sha256": sha256(shim),
        "source_sha256": {
            path.name: sha256(path)
            for path in [
                directory / "fail-child.c",
                directory / "probe.py",
                Path(__file__),
            ]
        },
        "results": results,
        "scope": "NULL parser cleanup only; buffer/handler allocation branches are not fault-injected.",
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest, indent=2))
    return 0 if success else 1


if __name__ == "__main__":
    raise SystemExit(main())
