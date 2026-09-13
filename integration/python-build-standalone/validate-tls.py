#!/usr/bin/env python3
"""Check Rust's real thread-local destructors with the PBS null-hook link path."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path

DIRECTORY = Path(__file__).resolve().parent


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--toolchain")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    commands = []

    def run(command: list[str]) -> str:
        commands.append(command)
        result = subprocess.run(command, check=True, capture_output=True, text=True)
        return result.stdout

    rustc = ["rustc", *([f"+{args.toolchain}"] if args.toolchain else [])]
    archive = output / "probe.a"
    run(
        [
            *rustc,
            "--edition=2024",
            "--crate-type=staticlib",
            "-C",
            "relocation-model=pic",
            str(DIRECTORY / "tls-probe.rs"),
            "-o",
            str(archive),
        ]
    )
    fallback = ["-Wl,--wrap=__cxa_thread_atexit_impl"]
    native = ["-lpthread", "-ldl", "-lm"]
    client = DIRECTORY / "tls-probe.c"
    symbols = {}
    for name, flags in [("native", []), ("fallback", fallback)]:
        executable = output / name
        run(["cc", str(client), str(archive), *native, *flags, "-o", str(executable)])
        run([str(executable)])
        symbols[name] = run(["readelf", "--dyn-syms", "--wide", str(executable)])
    library = output / "fallback.so"
    run(
        [
            "cc",
            "-shared",
            "-Wl,--whole-archive",
            str(archive),
            "-Wl,--no-whole-archive",
            *native,
            *fallback,
            "-o",
            str(library),
        ]
    )
    executable = output / "shared-client"
    run(["cc", str(client), str(library), "-lpthread", "-o", str(executable)])
    run([str(executable)])
    symbols["shared"] = run(["readelf", "--dyn-syms", "--wide", str(library)])
    assert "__cxa_thread_atexit_impl" in symbols["native"]
    for name in ["fallback", "shared"]:
        lines = [
            line.split()
            for line in symbols[name].splitlines()
            if "cxa_thread_atexit_impl" in line
        ]
        assert len(lines) == 1
        assert lines[0][-1] == "__wrap___cxa_thread_atexit_impl"
        assert "WEAK" in lines[0] and "UND" in lines[0]
        assert "GLIBC_2.18" not in symbols[name]
    for name, text in symbols.items():
        (output / f"{name}-symbols.txt").write_text(text)
    (output / "manifest.json").write_text(
        json.dumps(
            {
                "rustc": run([*rustc, "-vV"]),
                "commands": commands,
                "threads_per_binary": 32,
                "result": "Native, static fallback, and shared fallback each ran all thread destructors exactly once; fallback retains only an unversioned undefined weak wrapper.",
                "sources": {
                    p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                    for p in [DIRECTORY / "tls-probe.rs", client, Path(__file__)]
                },
                "limits": "Host runtime check; the complete PBS validator and glibc 2.17 container tests remain required.",
            },
            indent=2,
        )
        + "\n"
    )
    print(output)


if __name__ == "__main__":
    main()
