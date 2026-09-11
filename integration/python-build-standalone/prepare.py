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

import pgo_bundle

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
            "integration/python-build-standalone/prepare.py",
            "integration/python-build-standalone/pgo_bundle.py",
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
    parser.add_argument(
        "--pgo", action="store_true", help="Train and bundle a fresh PGO library"
    )
    parser.add_argument(
        "--llvm-profdata",
        type=Path,
        help="Installed profiler matching the PGO compiler's LLVM version",
    )
    parser.add_argument(
        "--cargo-arg",
        action="append",
        default=[],
        type=pgo_bundle.build.cargo_option,
        help="Global Cargo option for PGO, e.g. --cargo-arg=-Zohm-defaults=no",
    )
    args = parser.parse_args()
    if args.pgo != (args.llvm_profdata is not None):
        parser.error("--pgo and --llvm-profdata must be supplied together")
    if args.cargo_arg and not args.pgo:
        parser.error("--cargo-arg requires --pgo")
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("the initial recipe supports native Linux x86_64 builds only")
    if os.environ.get("CARGO_ENCODED_RUSTFLAGS"):
        parser.error(
            "unset CARGO_ENCODED_RUSTFLAGS so the required PIC/unwind flags take effect"
        )
    output = args.output.resolve()
    if args.pgo:
        if output.is_relative_to(ROOT) or ROOT.is_relative_to(output):
            parser.error("PGO output must be outside the Oriole source tree")
        if os.environ.get("RUSTFLAGS") or any(
            key.startswith("CARGO_PROFILE_") for key in os.environ
        ):
            parser.error(
                "unset RUSTFLAGS and CARGO_PROFILE_* overrides; PBS PGO verifies fixed PIC/unwind/ThinLTO settings"
            )
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
    pgo_output = output / "pgo"
    build_command = command
    if args.pgo:
        build_command = [
            sys.executable,
            "-I",
            "-S",
            str(ROOT / "tools/pgo/build.py"),
            "--source",
            str(ROOT),
            "--output",
            str(pgo_output),
            "--llvm-profdata",
            str(args.llvm_profdata.resolve()),
            "--native-static-libs",
            *(["--toolchain", args.toolchain] if args.toolchain else []),
            *[f"--cargo-arg={arg}" for arg in args.cargo_arg],
        ]
    result = subprocess.run(
        build_command,
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
    pgo = None
    archive = target_dir / TARGET / "release/liboriole_expat.a"
    if args.pgo:
        archive, libraries, pgo, command = pgo_bundle.verify(
            pgo_output, ROOT, env, args.toolchain, args.cargo_arg
        )
    else:
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
    shutil.copyfile(archive, output / "libexpat.a")
    if pgo is not None:
        pgo_bundle.require(
            digest(output / "libexpat.a")
            == pgo["manifest"]["libraries_sha256"]["use/liboriole_expat.a"],
            "Copied PBS PGO archive checksum mismatch",
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
                *args.cargo_arg,
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
    rustc_version = subprocess.check_output([*rustc, "-vV"], text=True)
    if pgo is not None:
        pgo_bundle.require(
            str((sysroot / "bin/rustc").resolve()) == pgo["compiler"],
            "PBS license and PGO compiler sysroots differ",
        )
        pgo_bundle.require(
            rustc_version == pgo["manifest"]["rustc_version"],
            "PBS and PGO compiler version mismatch",
        )
        pgo_bundle.verify(pgo_output, ROOT, env, args.toolchain, args.cargo_arg)
    if sources() != before:
        raise RuntimeError("Oriole bundle inputs changed during packaging")
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
        "rustc": rustc_version,
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
    if pgo is not None:
        manifest["pgo"] = pgo
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Bundle: {output}")


if __name__ == "__main__":
    main()
