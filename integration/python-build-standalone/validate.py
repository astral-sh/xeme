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
import tempfile
from pathlib import Path
from unittest.mock import patch

DIRECTORY = Path(__file__).resolve().parent
REVISION = "a4553880293fe9d1bb62747d34ab0e5121d3554f"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--pbs", type=Path, required=True, help="Clean pinned PBS source directory"
    )
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="oriole-pbs-validation-") as temporary:
        checkout = Path(temporary) / "pbs"
        checkout.mkdir()
        for name in (
            "cpython-unix/build.py",
            "cpython-unix/Makefile",
            "cpython-unix/build-cpython.sh",
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
                f"-L{tools}/deps/lib -lexpat -lgcc_s -lpthread -ldl -lm -lc",
            ]
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
            "Patch application, Python compilation, shell syntax, default path, overlay staging, native linker flags, target guards, and checksum rejection passed."
        )
        print(
            "No Rust archive was built or linked by these fixture checks; no PBS distribution was built."
        )


if __name__ == "__main__":
    main()
