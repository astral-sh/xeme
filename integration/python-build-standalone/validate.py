#!/usr/bin/env python3
"""Check patch application and exercise bundle installation without a PBS build."""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path
from unittest.mock import patch

DIRECTORY = Path(__file__).resolve().parent
REVISION = "a4553880293fe9d1bb62747d34ab0e5121d3554f"


def validate_source_selection(checkout: Path, pbs: Path) -> dict:
    """Check the real download module and Make version generator in both modes."""
    original = json.loads((pbs / "pythonbuild/downloads.json").read_text())
    probe = (
        "import json; from pythonbuild.downloads import DOWNLOADS; "
        "print(json.dumps(DOWNLOADS))"
    )
    environment = {
        "PATH": os.defpath,
        "PYTHONPATH": str(checkout),
        "PYTHONDONTWRITEBYTECODE": "1",
    }
    normal = json.loads(
        subprocess.check_output(
            [sys.executable, "-c", probe], cwd=checkout, env=environment
        )
    )
    assert normal == original
    overlay = json.loads(
        subprocess.check_output(
            [sys.executable, "-c", probe],
            cwd=checkout,
            env={**environment, "PYBUILD_ORIOLE_BUNDLE": "fixture"},
        )
    )
    expected = {
        **original["cpython-3.12"],
        "url": "https://www.python.org/ftp/python/3.12.13/Python-3.12.13.tar.xz",
        "size": 20801708,
        "sha256": "c08bc65a81971c1dd5783182826503369466c7e67374d1646519adf05207b684",
        "version": "3.12.13",
    }
    assert overlay == {**original, "cpython-3.12": expected}
    tree = ast.parse((pbs / "pythonbuild/utils.py").read_text())
    writer = next(
        node
        for node in tree.body
        if isinstance(node, ast.FunctionDef) and node.name == "write_package_versions"
    )
    namespace: dict[str, object] = {
        "pathlib": __import__("pathlib"),
        "DOWNLOADS": overlay,
        "write_if_different": lambda path, data: path.write_bytes(data),
    }
    exec(  # noqa: S102 -- run the pinned version generator without importing PBS dependencies.
        compile(ast.Module(body=[writer], type_ignores=[]), "versions", "exec"),
        namespace,
    )
    generate = namespace["write_package_versions"]
    assert callable(generate)
    versions = checkout / "build/versions"
    generate(versions)
    version_file = versions / "VERSION.cpython-3.12"
    assert version_file.read_text() == "CPYTHON_3.12_VERSION := 3.12.13\n"
    default = next(
        line
        for line in (checkout / "cpython-unix/Makefile").read_text().splitlines()
        if line.startswith("default:")
    )
    # Expand the actual PBS default target using its actual generated version.
    makefile = (
        f"include {version_file}\nOUTDIR := build\nPYTHON_MAJOR_VERSION := 3.12\n"
        "PACKAGE_SUFFIX := x86_64-unknown-linux-gnu-noopt\n"
        f"{default}\n%:\n\t@:\n"
    )
    database = subprocess.check_output(
        [
            "make",
            "--no-builtin-rules",
            "--dry-run",
            "--print-data-base",
            "-f",
            "-",
            "default",
        ],
        input=makefile,
        text=True,
    )
    assert (
        "default: build/cpython-3.12.13-x86_64-unknown-linux-gnu-noopt.tar"
        in database.splitlines()
    )
    print(f"Pinned CPython source: {json.dumps(expected, sort_keys=True)}")
    return expected


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--pbs", type=Path, required=True, help="Clean pinned PBS source directory"
    )
    consumer_options = parser.add_mutually_exclusive_group()
    consumer_options.add_argument(
        "--cpython",
        type=Path,
        help="Also apply the consumer backport to pinned CPython source",
    )
    consumer_options.add_argument(
        "--cpython-archive",
        type=Path,
        help="Verify the pinned download and apply the backport to its pyexpat source",
    )
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="oriole-pbs-validation-") as temporary:
        checkout = Path(temporary) / "pbs"
        checkout.mkdir()
        for name in (
            "cpython-unix/build.py",
            "cpython-unix/Makefile",
            "cpython-unix/build-cpython.sh",
            "pythonbuild/__init__.py",
            "pythonbuild/downloads.py",
            "pythonbuild/downloads.json",
        ):
            destination = checkout / name
            destination.parent.mkdir(exist_ok=True)
            shutil.copyfile(args.pbs / name, destination)
        subprocess.run(
            ["git", "apply", "--check", str(DIRECTORY / "pbs-a455388.patch")],
            cwd=checkout,
            check=True,
        )
        subprocess.run(
            ["git", "apply", str(DIRECTORY / "pbs-a455388.patch")],
            cwd=checkout,
            check=True,
        )
        selected_source = validate_source_selection(checkout, args.pbs)
        cpython = args.cpython
        if args.cpython_archive:
            assert args.cpython_archive.stat().st_size == selected_source["size"]
            assert (
                hashlib.sha256(args.cpython_archive.read_bytes()).hexdigest()
                == selected_source["sha256"]
            )
            cpython = Path(temporary) / "downloaded-consumer"
            (cpython / "Modules").mkdir(parents=True)
            with tarfile.open(args.cpython_archive) as archive:
                source = archive.extractfile("Python-3.12.13/Modules/pyexpat.c")
                assert source is not None
                (cpython / "Modules/pyexpat.c").write_bytes(source.read())
        tree = ast.parse((checkout / "cpython-unix/build.py").read_text())
        compile(tree, "patched-build.py", "exec")
        subprocess.run(["bash", "-n", str(DIRECTORY / "run.sh")], check=True)
        subprocess.run(
            ["bash", "-n", str(checkout / "cpython-unix/build-cpython.sh")], check=True
        )
        function = next(
            node
            for node in tree.body
            if isinstance(node, ast.FunctionDef)
            and node.name == "install_oriole_overlay"
        )
        namespace = {
            "os": os,
            "pathlib": __import__("pathlib"),
            "platform": platform,
            "json": json,
            "hashlib": hashlib,
            "re": re,
            "DOWNLOADS": {
                "expat": {"licenses": ["MIT"], "license_file": "LICENSE.expat.txt"}
            },
        }
        exec(  # noqa: S102 -- exercise the reviewed local patch without importing PBS dependencies.
            compile(ast.Module(body=[function], type_ignores=[]), "overlay", "exec"),
            namespace,
        )
        install = namespace["install_oriole_overlay"]
        assert callable(install)

        class Environment:
            is_isolated = True

            def __init__(self) -> None:
                self.copies = {}
                self.commands = []

            def run(self, command):
                self.commands.append(command)

            def copy_file(self, source, *, dest_path, dest_name):
                self.copies[(dest_path, dest_name)] = source.read_bytes()

        env = Environment()
        with patch.dict(os.environ, {}, clear=True):
            assert (
                install(
                    env, "linux_x86_64", "x86_64-unknown-linux-gnu", "3.12.13", "noopt"
                )
                is None
            )
            assert not env.copies and not env.commands
        bundle = Path(temporary) / "bundle"
        bundle.mkdir()
        files = {
            "libexpat.a": b"fixture archive",
            "expat.h": b"fixture header",
            "expat.pc": b"fixture pkg-config",
            "native-static-libs.txt": b"-lgcc_s -lpthread -ldl -lm -lc\n",
            "LICENSE.oriole.txt": b"fixture notices",
            "cpython-external-parser.patch": (
                DIRECTORY / "consumer-fix/cpython-3.12.13-external-parser.patch"
            ).read_bytes(),
        }
        manifest = {
            "format": 1,
            "pbs_revision": REVISION,
            "target": "x86_64-unknown-linux-gnu",
            "files": {
                name: hashlib.sha256(data).hexdigest() for name, data in files.items()
            },
        }
        for name, data in files.items():
            (bundle / name).write_bytes(data)
        (bundle / "manifest.json").write_text(json.dumps(manifest))
        with (
            patch.dict(os.environ, {"PYBUILD_ORIOLE_BUNDLE": str(bundle)}, clear=True),
            patch.object(platform, "system", return_value="Linux"),
        ):
            install(
                env,
                "linux_x86_64",
                "x86_64-unknown-linux-gnu",
                "3.12.13",
                "noopt",
            )
            assert env.copies["/tools/deps/lib", "libexpat.a"] == files["libexpat.a"]
            assert env.copies["/tools/deps/include", "expat.h"] == files["expat.h"]
            assert (
                env.copies["/tools/deps/share/oriole", "cpython-external-parser.patch"]
                == files["cpython-external-parser.patch"]
            )
            assert (
                namespace["DOWNLOADS"]["expat"]["license_file"] == "LICENSE.oriole.txt"
            )
            shell = (checkout / "cpython-unix/build-cpython.sh").read_text()
            block = shell[
                shell.index("# Oriole's optional overlay") : shell.index(
                    "# configure somehow"
                )
            ]
            probe = (
                block + 'printf "%s\\n%s\\n" "$LIBEXPAT_CFLAGS" "$LIBEXPAT_LDFLAGS"\n'
            )
            tools = Path(temporary) / "tools"
            probe_env = {
                "PATH": os.defpath,
                "TOOLS_PATH": str(tools),
                "LIBEXPAT_CFLAGS": "original-cflags",
                "LIBEXPAT_LDFLAGS": "original-ldflags",
            }
            unchanged = subprocess.check_output(
                ["bash", "-eu", "-c", probe], text=True, env=probe_env
            )
            assert unchanged.splitlines() == ["original-cflags", "original-ldflags"]
            (tools / "deps/lib").mkdir(parents=True)
            (tools / "deps/lib/oriole-native-static-libs.txt").write_bytes(
                files["native-static-libs.txt"]
            )
            configured = subprocess.check_output(
                ["bash", "-eu", "-c", probe], text=True, env=probe_env
            )
            assert configured.splitlines() == [
                f"-I{tools}/deps/include",
                f"-L{tools}/deps/lib -lexpat -lgcc_s -lpthread -ldl -lm -lc "
                "-Wl,--wrap=__cxa_thread_atexit_impl",
            ]
            if cpython:
                consumer = Path(temporary) / "consumer"
                (consumer / "Modules").mkdir(parents=True)
                shutil.copyfile(
                    cpython / "Modules/pyexpat.c", consumer / "Modules/pyexpat.c"
                )
                backport = shell[
                    shell.index("# Oriole's bounded allocations") : shell.index(
                        "# configure doesn't support cross-compiling on Apple."
                    )
                ]
                # No overlay leaves the exact source untouched.
                original = (consumer / "Modules/pyexpat.c").read_bytes()
                subprocess.run(
                    ["bash", "-eu", "-c", backport],
                    cwd=consumer,
                    env={**probe_env, "PYTHON_VERSION": "3.12.13"},
                    check=True,
                )
                assert (consumer / "Modules/pyexpat.c").read_bytes() == original
                (tools / "deps/share/oriole").mkdir(parents=True)
                (tools / "deps/share/oriole/cpython-external-parser.patch").write_bytes(
                    files["cpython-external-parser.patch"]
                )
                subprocess.run(
                    ["bash", "-eu", "-c", backport],
                    cwd=consumer,
                    env={**probe_env, "PYTHON_VERSION": "3.12.13"},
                    check=True,
                )
                # Source hash checks reject reapplication or an unexpected source revision.
                repeated = subprocess.run(
                    ["bash", "-eu", "-c", backport],
                    cwd=consumer,
                    env={**probe_env, "PYTHON_VERSION": "3.12.13"},
                    check=False,
                )
                assert repeated.returncode != 0
            for target, version, options in [
                ("aarch64-unknown-linux-gnu", "3.12.13", "noopt"),
                ("x86_64-unknown-linux-gnu", "3.13.7", "noopt"),
                ("x86_64-unknown-linux-gnu", "3.12.13", "noopt+static"),
            ]:
                try:
                    install(Environment(), "linux_x86_64", target, version, options)
                except ValueError:
                    pass
                else:
                    raise AssertionError("unsupported build was accepted")
            (bundle / "expat.h").write_bytes(b"mismatched header")
            rejected = Environment()
            try:
                install(
                    rejected,
                    "linux_x86_64",
                    "x86_64-unknown-linux-gnu",
                    "3.12.13",
                    "noopt",
                )
            except ValueError as error:
                assert "checksum mismatch" in str(error)
            else:
                raise AssertionError("mismatched bundle was accepted")
            assert not rejected.copies and not rejected.commands
        print(
            "Patch application, source pinning, generated Make target, Python compilation, shell syntax, default path, overlay staging, native linker flags, target guards, and checksum rejection passed."
        )
        print(
            "No Rust archive was built or linked by these fixture checks; no PBS distribution was built."
        )


if __name__ == "__main__":
    main()
