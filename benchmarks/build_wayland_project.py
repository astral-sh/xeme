#!/usr/bin/env python3
"""Build matched unmodified Wayland scanners against benchmark consumer libraries."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path
from typing import Any


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--consumers", type=Path, required=True)
    parser.add_argument("--header", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cpu", type=int, default=2)
    args = parser.parse_args()
    source = args.source.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    manifest = json.loads((source / "source.json").read_text())
    if manifest["commit"] != "1ab6b693b16e1d9734496fe60c8a6ed277e4dec3":
        parser.error("expected pinned Wayland revision")
    for entry in manifest["files"]:
        if digest(source / Path(entry["path"]).name) != entry["sha256"]:
            parser.error("upstream source hash mismatch")
    meson = (source / "meson.build").read_text()
    match = re.search(r"version: '([0-9]+)\.([0-9]+)\.([0-9]+)'", meson)
    if match is None:
        parser.error("cannot identify version")
    version = ".".join(match.groups())
    header = (source / "wayland-version.h.in").read_text()
    for key, value in zip(["MAJOR", "MINOR", "MICRO"], match.groups(), strict=True):
        header = header.replace(f"@WAYLAND_VERSION_{key}@", value)
    header = header.replace("@WAYLAND_VERSION@", version)
    (output / "wayland-version.h").write_text(header)
    consumers = json.loads(args.consumers.read_text())
    if consumers["status"] != "passed":
        parser.error("consumer builds did not pass")
    observed = [
        Path(__file__).resolve(),
        *source.iterdir(),
        output / "wayland-version.h",
        args.header / "expat.h",
        args.consumers.resolve(),
    ]
    hashes = {str(p): digest(p) for p in observed if p.is_file()}
    report: dict[str, Any] = {
        "status": "failed",
        "configuration": "Pinned unmodified Wayland scanner.c and wayland-util.c. Linux POSIX configuration, HAVE_STRNDUP=1, optional libxml DTD validation disabled (HAVE_LIBXML=0) in both binaries. Version header generated from pinned template and meson.build. Same -O3 C compiler and narrow Expat-compatible header for both engines; parser library/rpath differ.",
        "version": version,
        "compiler": subprocess.check_output(["cc", "--version"], text=True),
        "source_sha256": hashes,
        "binaries": {},
        "commands": [],
    }
    try:
        for engine, consumer in consumers["consumers"].items():
            library = Path(consumer["library"])
            expected = consumer["files"][str(library)]
            if digest(library) != expected:
                raise RuntimeError("consumer library changed")
            binary = output / f"wayland-scanner-{engine}"
            command = [
                "taskset",
                "-c",
                str(args.cpu),
                "cc",
                "-std=c99",
                "-O3",
                "-D_POSIX_C_SOURCE=200809L",
                "-DHAVE_STRNDUP=1",
                "-DHAVE_LIBXML=0",
                f"-I{args.header.resolve()}",
                f"-I{output}",
                f"-I{source}",
                str(source / "scanner.c"),
                str(source / "wayland-util.c"),
                str(library),
                f"-Wl,-rpath,{library.parent}",
                "-o",
                str(binary),
            ]
            done = subprocess.run(command, capture_output=True, text=True, check=False)
            (output / f"{engine}-build.log").write_text(done.stdout + done.stderr)
            report["commands"].append(
                {"command": command, "returncode": done.returncode}
            )
            if done.returncode:
                raise RuntimeError(f"failed {engine} build")
            if digest(library) != expected:
                raise RuntimeError("library changed during build")
            report["binaries"][engine] = {
                "path": str(binary),
                "sha256": digest(binary),
                "library": str(library),
                "library_sha256": digest(library),
                "ldd": subprocess.check_output(["ldd", str(binary)], text=True),
            }
        report["source_sha256_after"] = {p: digest(Path(p)) for p in hashes}
        if hashes != report["source_sha256_after"]:
            raise RuntimeError("source changed during build")
        report["status"] = "passed"
    finally:
        (output / "build.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"status": report["status"], "output": str(output)}, indent=2))


if __name__ == "__main__":
    main()
