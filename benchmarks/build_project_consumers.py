#!/usr/bin/env python3
"""Build matched unmodified CPython XML consumers for the project benchmarks."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import sysconfig
from pathlib import Path
from typing import Any

REVISION = "3bb231a6a5dc02b95658877318bf61501a7209e9"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--header", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if sys.version_info[:3] != (3, 12, 13):
        parser.error("requires CPython 3.12.13")
    source = args.source.resolve(strict=True)
    revision = subprocess.check_output(
        ["git", "-C", str(source), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != REVISION:
        parser.error("CPython source revision differs from pin")
    if subprocess.check_output(["git", "-C", str(source), "status", "--porcelain"]):
        parser.error("CPython source is modified")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    libraries = {
        "oriole": args.library.resolve(strict=True),
        "expat": args.reference.resolve(strict=True),
    }
    includes = [
        args.header.resolve(strict=True),
        Path(sysconfig.get_path("include")),
        Path(sysconfig.get_path("include")) / "internal",
        source / "Modules/expat",
        source / "Modules",
    ]
    inputs = [
        Path(__file__).resolve(),
        source / "Modules/pyexpat.c",
        source / "Modules/_elementtree.c",
        args.header / "expat.h",
        *libraries.values(),
        Path(sys.executable).resolve(),
    ]
    report: dict[str, Any] = {
        "status": "failed",
        "cpython_revision": revision,
        "python": sys.version,
        "compiler": subprocess.check_output(["cc", "--version"], text=True),
        "adaptations": None,
        "method": "Both engines compile identical unmodified CPython 3.12.13 pyexpat.c and _elementtree.c with identical -O2 flags and the same narrow Expat-compatible header. Only the linked parser library and its rpath differ. No source adaptation, alternate allocator, LTO or PGO is applied.",
        "source_sha256_before": {str(p): digest(p) for p in inputs},
        "consumers": {},
    }
    try:
        for engine, library in libraries.items():
            directory = output / engine
            directory.mkdir()
            copied = directory / library.name
            shutil.copy2(library, copied)
            dynamic = subprocess.check_output(["readelf", "-d", str(copied)], text=True)
            match = re.search(r"\(SONAME\).*\[([^]]+)\]", dynamic)
            if match and match[1] != copied.name:
                if Path(match[1]).name != match[1]:
                    raise ValueError("invalid SONAME")
                (directory / match[1]).symlink_to(copied.name)
            commands = []
            files = {str(copied): digest(copied)}
            for module in ["pyexpat", "_elementtree"]:
                destination = (
                    directory / f"{module}{sysconfig.get_config_var('EXT_SUFFIX')}"
                )
                command = [
                    "cc",
                    "-shared",
                    "-fPIC",
                    "-O2",
                    *[f"-I{p}" for p in includes],
                    str(source / f"Modules/{module}.c"),
                    str(copied),
                    f"-Wl,-rpath,{directory}",
                    "-o",
                    str(destination),
                ]
                run = subprocess.run(
                    command, capture_output=True, text=True, check=False
                )
                log = directory / f"{module}-build.log"
                log.write_text(run.stdout + run.stderr)
                commands.append(
                    {
                        "command": command,
                        "returncode": run.returncode,
                        "log_sha256": digest(log),
                    }
                )
                if run.returncode:
                    raise RuntimeError(f"{engine}/{module} failed")
                files[str(destination)] = digest(destination)
            report["consumers"][engine] = {
                "directory": str(directory),
                "library": str(copied),
                "files": files,
                "commands": commands,
            }
        report["source_sha256_after"] = {str(p): digest(p) for p in inputs}
        if report["source_sha256_before"] != report["source_sha256_after"]:
            raise RuntimeError("source changed during build")
        report["status"] = "passed"
    finally:
        (output / "build.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"status": report["status"], "output": str(output)}, indent=2))


if __name__ == "__main__":
    main()
