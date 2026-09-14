#!/usr/bin/env python3
"""Run generated XML workloads through the shared native benchmark runner."""

from __future__ import annotations

import argparse
import ctypes.util
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import projects
from corpus import workloads


def resolve_library(value: str) -> Path:
    path = Path(value)
    if path.is_file():
        return path.resolve()
    listing = subprocess.run(
        ["ldconfig", "-p"], check=True, capture_output=True, text=True
    ).stdout
    candidates = {
        Path(fields[-1]).resolve()
        for line in listing.splitlines()
        if (fields := line.split()) and fields[0] == value
    }
    if len(candidates) == 1:
        return candidates.pop()
    raise ValueError(
        f"cannot resolve one library path for {value}; supply an absolute path"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--reference", default=ctypes.util.find_library("expat"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--pairs", type=int, default=7)
    parser.add_argument("--iterations", type=int, default=10)
    parser.add_argument("--size", type=int, default=10000)
    parser.add_argument("--chunks", type=int, nargs="+", default=[64, 4096, 1048576])
    parser.add_argument("--seed", type=int, default=20260910)
    parser.add_argument(
        "--namespaces",
        action="store_true",
        help="Enable namespace expansion in both libraries",
    )
    parser.add_argument("--cc", default="cc")
    parser.add_argument("--build-manifest", type=Path, action="append", default=[])
    args = parser.parse_args(argv)
    if args.pairs < 3 or args.iterations < 3 or args.size < 1 or min(args.chunks) < 1:
        parser.error(
            "at least three pairs/iterations, positive size and chunk sizes required"
        )
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    inputs = []
    for name, data in workloads(args.size).items():
        path = output / f"{name}.xml"
        path.write_bytes(data)
        inputs.append(
            {
                "name": name,
                "files": [
                    {
                        "role": "input",
                        "path": path.name,
                        "sha256": hashlib.sha256(data).hexdigest(),
                    }
                ],
            }
        )
    corpus = output / "corpus.json"
    corpus.write_text(
        json.dumps(
            {"kind": "generated", "size": args.size, "projects": inputs}, indent=2
        )
        + "\n"
    )
    sources = [
        Path(__file__).resolve(),
        Path(__file__).resolve().parents[1] / "tools/corpus.py",
    ]
    for index, manifest in enumerate(args.build_manifest):
        copied = output / f"build-manifest-{index}.json"
        shutil.copy2(manifest, copied)
        sources.append(copied)
    return projects.main(
        [
            "--library",
            str(resolve_library(str(args.library))),
            "--reference",
            str(resolve_library(args.reference)),
            "--corpus",
            str(corpus),
            "--output",
            str(output / "results"),
            "--pairs",
            str(args.pairs),
            "--iterations",
            str(args.iterations),
            "--chunks",
            *map(str, args.chunks),
            "--seed",
            str(args.seed),
            "--namespaces",
            "on" if args.namespaces else "off",
            "--cc",
            args.cc,
            *[
                argument
                for path in args.build_manifest
                for argument in ("--build-manifest", str(path))
            ],
        ],
        source_files=sources,
    )


if __name__ == "__main__":
    raise SystemExit(main())
