#!/usr/bin/env python3
"""Build a traceable, position-independent archive for the experimental PBS overlay."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TARGET = "x86_64-unknown-linux-gnu"
PBS_REVISION = "a4553880293fe9d1bb62747d34ab0e5121d3554f"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources() -> dict[str, str]:
    paths = [
        ROOT / name
        for name in (
            "Cargo.toml",
            "Cargo.lock",
            "include/expat.h",
            "LICENSE-MIT",
            "LICENSE-APACHE",
            "licenses/cpython.txt",
            "integration/python-build-standalone/consumer-fix/cpython-3.12.13-external-parser.patch",
            "integration/python-build-standalone/consumer-fix/provenance.json",
        )
    ]
    for crate in ("oriole", "oriole_storage", "oriole_expat"):
        paths.extend((ROOT / "crates" / crate).rglob("*.rs"))
        paths.append(ROOT / "crates" / crate / "Cargo.toml")
    return {str(path.relative_to(ROOT)): digest(path) for path in sorted(paths)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--toolchain", help="Optional rustup toolchain, e.g. ohm for local development"
    )
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("the initial recipe supports native Linux x86_64 builds only")
    if os.environ.get("CARGO_ENCODED_RUSTFLAGS"):
        parser.error(
            "unset CARGO_ENCODED_RUSTFLAGS so the required PIC/unwind flags take effect"
        )
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    before = sources()
    cargo = ["cargo", *([f"+{args.toolchain}"] if args.toolchain else [])]
    rustc = ["rustc", *([f"+{args.toolchain}"] if args.toolchain else [])]
    env = dict(os.environ)
    # Even an empty encoded value would override the required RUSTFLAGS below.
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    # CI may force ANSI colors; native-static-libs is machine-read below.
    env["CARGO_TERM_COLOR"] = "never"
    target_dir = Path(env.get("CARGO_TARGET_DIR", ROOT / "target" / "pbs")).resolve()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    env["RUSTFLAGS"] = (
        f"{env.get('RUSTFLAGS', '')} -C relocation-model=pic -C panic=unwind".strip()
    )
    command = [
        *cargo,
        "rustc",
        "--locked",
        "--release",
        "--target",
        TARGET,
        "-p",
        "oriole_expat",
        "--lib",
        "--crate-type",
        "cdylib,staticlib",
        "--verbose",
        "--",
        "--print=native-static-libs",
    ]
    result = subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    (output / "build.log").write_text(result.stdout)
    sys.stdout.write(result.stdout)
    result.check_returncode()
    matches = re.findall(r"native-static-libs:\s*([^\n]+)", result.stdout)
    if not matches:
        raise RuntimeError(
            "rustc did not report native libraries; use a fresh CARGO_TARGET_DIR and retry"
        )
    libraries = matches[-1].split()
    if not libraries or any(
        re.fullmatch(r"-l[A-Za-z0-9_]+", library) is None for library in libraries
    ):
        raise RuntimeError(f"native linker arguments need review: {libraries!r}")
    if sources() != before:
        raise RuntimeError(
            "Oriole sources changed during the build; discard this bundle and retry"
        )
    shutil.copyfile(
        target_dir / TARGET / "release/liboriole_expat.a", output / "libexpat.a"
    )
    # Only an optional weak reference can safely select Rust's null-hook path.
    symbols = subprocess.check_output(
        ["nm", "--undefined-only", "--format=posix", str(output / "libexpat.a")],
        text=True,
    )
    references = [
        fields[1]
        for line in symbols.splitlines()
        if (fields := line.split()) and fields[0] == "__cxa_thread_atexit_impl"
    ]
    if not references or any(kind not in ("w", "v") for kind in references):
        raise RuntimeError("Rust's optional TLS destructor hook requires review")
    shutil.copyfile(ROOT / "include/expat.h", output / "expat.h")
    consumer_fix = Path(__file__).resolve().parent / "consumer-fix"
    provenance = json.loads((consumer_fix / "provenance.json").read_text())
    consumer_patch = consumer_fix / "cpython-3.12.13-external-parser.patch"
    if digest(consumer_patch) != provenance["patch_sha256"]:
        raise RuntimeError("CPython backport does not match its reviewed provenance")
    shutil.copyfile(consumer_patch, output / "cpython-external-parser.patch")
    (output / "native-static-libs.txt").write_text(" ".join(libraries) + "\n")
    header = (output / "expat.h").read_text()
    version_parts = []
    for part in ("MAJOR", "MINOR", "MICRO"):
        match = re.search(rf"#\s*define\s+XML_{part}_VERSION\s+(\d+)", header)
        if match is None:
            raise RuntimeError(f"missing XML_{part}_VERSION in the public header")
        version_parts.append(match[1])
    api_version = ".".join(version_parts)
    (output / "expat.pc").write_text(
        "prefix=/tools/deps\nexec_prefix=${prefix}\nlibdir=${prefix}/lib\nincludedir=${prefix}/include\n"
        "implementation=oriole\n\nName: Oriole Expat interface\n"
        "Description: Experimental Oriole implementation of the Expat C ABI\n"
        f"Version: {api_version}\nLibs: -L${{libdir}} -lexpat\n"
        f"Libs.private: {' '.join(libraries)}\nCflags: -I${{includedir}}\n"
    )
    metadata = json.loads(
        subprocess.check_output(
            [
                *cargo,
                "metadata",
                "--locked",
                "--format-version=1",
                "--filter-platform",
                TARGET,
            ],
            cwd=ROOT,
            env=env,
        )
    )
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [
        package_id
        for package_id, package in packages.items()
        if package["name"] == "oriole_expat"
    ]
    used = set()
    while pending:
        package_id = pending.pop()
        if package_id not in used:
            used.add(package_id)
            pending.extend(nodes[package_id]["dependencies"])
    notices = []
    for package_id in sorted(used):
        package = packages[package_id]
        directory = Path(package["manifest_path"]).parent
        license_paths = sorted(directory.glob("LICENSE*"))
        if not license_paths and directory.is_relative_to(ROOT):
            license_paths = [ROOT / "LICENSE-MIT", ROOT / "LICENSE-APACHE"]
        if not license_paths:
            raise RuntimeError(f"missing license text for {package['name']}")
        for path in license_paths:
            if path.is_file():
                notices.append(
                    f"\n=== {package['name']} {package['version']} / {path.name} ===\n{path.read_text()}"
                )
    notices.append(
        "\n=== Expat header notice ===\n" + header.split("*/", 1)[0] + "*/\n"
    )
    notices.append(
        "\n=== CPython cleanup backport / "
        + provenance["upstream_commit"]
        + " ===\n"
        + (ROOT / "licenses/cpython.txt").read_text()
    )
    sysroot = Path(
        subprocess.check_output([*rustc, "--print", "sysroot"], text=True).strip()
    )
    rust_notices = sysroot / "share/doc/rust/COPYRIGHT-library.html"
    if not rust_notices.is_file():
        raise RuntimeError(
            "install the matching Rust documentation component to retain standard-library notices"
        )
    notices.append(
        "\n=== Rust standard library notices ===\n" + rust_notices.read_text()
    )
    for path in sorted((sysroot / "share/doc/rust/licenses").glob("*")):
        if path.is_file():
            notices.append(f"\n=== Rust / {path.name} ===\n{path.read_text()}")
    (output / "LICENSE.oriole.txt").write_text("\n".join(notices))
    manifest = {
        "format": 1,
        "pbs_revision": PBS_REVISION,
        "target": TARGET,
        "api_version": api_version,
        "implementation_version": next(
            package["version"]
            for package in packages.values()
            if package["name"] == "oriole_expat"
        ),
        "rustc": subprocess.check_output([*rustc, "-vV"], text=True),
        "command": command,
        "rustflags": env["RUSTFLAGS"],
        "sources": before,
        "consumer_adaptation": provenance,
        "files": {
            name: digest(output / name)
            for name in (
                "libexpat.a",
                "expat.h",
                "expat.pc",
                "native-static-libs.txt",
                "LICENSE.oriole.txt",
                "cpython-external-parser.patch",
            )
        },
        "validation": "Archive built locally; PBS target-sysroot linking and complete distribution validation remain required.",
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Bundle: {output}")


if __name__ == "__main__":
    main()
