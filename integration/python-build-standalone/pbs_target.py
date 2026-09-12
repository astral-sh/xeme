"""Explicit PBS product targets and their Rust ABI/CPU requirements."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import subprocess
import tempfile
from pathlib import Path

RUST_TARGET = "x86_64-unknown-linux-gnu"
TARGETS = {RUST_TARGET: None, "x86_64_v3-unknown-linux-gnu": "x86-64-v3"}


def cpu(target: str) -> str | None:
    if target not in TARGETS:
        raise ValueError(f"Unsupported PBS target: {target}")
    return TARGETS[target]


def validate(manifest: dict, target: str) -> None:
    """Reject a mislabeled bundle before staging or executing its binaries."""
    expected = cpu(target)
    if (
        manifest.get("target") != target
        or manifest.get("rust_target") != RUST_TARGET
        or "target_cpu" not in manifest
        or manifest["target_cpu"] != expected
    ):
        raise ValueError("Oriole bundle PBS target, Rust target or CPU does not match")


def check_host(target: str) -> dict:
    """Run a generic-ISA GCC guard before any opt-in v3 code can execute."""
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise ValueError("PBS overlay requires native Linux x86_64")
    result = {"pbs_target": target, "target_cpu": cpu(target)}
    if cpu(target) is None:
        return {**result, "check": "generic native host"}
    source = Path(__file__).with_name("check-cpu.c")
    # Never inherit CC/CFLAGS, compiler search overrides, or a v3 compiler default.
    compiler = Path("/usr/bin/gcc").resolve(strict=True)
    environment = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    with tempfile.TemporaryDirectory(prefix="oriole-pbs-cpu-") as temporary:
        executable = Path(temporary) / "check-cpu"
        command = [
            str(compiler),
            "-std=c11",
            "-O2",
            "-march=x86-64",
            "-mtune=generic",
            "-Wall",
            "-Wextra",
            "-Werror",
            str(source),
            "-o",
            str(executable),
        ]
        compiled = subprocess.run(
            command, env=environment, capture_output=True, text=True, timeout=60
        )
        if compiled.returncode:
            raise RuntimeError(
                f"Generic CPU guard compilation failed: {compiled.stderr}"
            )
        checked = subprocess.run(
            [str(executable)],
            env=environment,
            capture_output=True,
            text=True,
            timeout=10,
        )
        if checked.returncode:
            raise RuntimeError(checked.stderr or "x86-64-v3 CPU/OS check failed")
        return {
            **result,
            "check": "GCC CPU/OS builtin",
            "command": command,
            "compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(),
            "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
            "binary_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
            "compile_stdout": compiled.stdout,
            "compile_stderr": compiled.stderr,
            "stdout": checked.stdout,
            "stderr": checked.stderr,
        }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pbs-target", choices=TARGETS, default=RUST_TARGET)
    parser.add_argument("--bundle", type=Path)
    parser.add_argument("--check-host", action="store_true")
    args = parser.parse_args()
    if args.bundle:
        validate(
            json.loads((args.bundle / "manifest.json").read_text()), args.pbs_target
        )
    if args.check_host:
        print(json.dumps(check_host(args.pbs_target), indent=2))


if __name__ == "__main__":
    main()
