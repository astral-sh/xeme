#!/usr/bin/env python3
"""Build real CPython XML extensions against Oriole and run upstream tests.

Run with a CPython 3.12.13 interpreter and its development headers. Sources are
pinned and never edited in place. The optional system-allocator adaptation is an
explicit consumer change; results from it are not drop-in compatibility evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import sysconfig
from pathlib import Path

REVISION = "3bb231a6a5dc02b95658877318bf61501a7209e9"
TESTS = [
    "test_pyexpat",
    "test_xml_etree",
    "test_xml_etree_c",
    "test_minidom",
    "test_sax",
    "test_pulldom",
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--system-allocator", action="store_true")
    parser.add_argument(
        "--native-library",
        action="append",
        default=[],
        help="Native library name required by a Rust static archive; repeat in linker order",
    )
    parser.add_argument("--tests", nargs="+", default=TESTS)
    args = parser.parse_args()
    if sys.version_info[:3] != (3, 12, 13):
        parser.error("use CPython 3.12.13 to match the pinned source")
    root = Path(__file__).resolve().parents[2]
    source, library, output = (
        p.resolve() for p in (args.source, args.library, args.output)
    )
    revision = subprocess.check_output(
        ["git", "-C", str(source), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != REVISION:
        parser.error(f"expected CPython {REVISION}, got {revision}")
    if subprocess.check_output(["git", "-C", str(source), "status", "--porcelain"]):
        parser.error("CPython source must be clean")
    output.mkdir(parents=True, exist_ok=True)
    frozen_library = output / library.name
    shutil.copy2(library, frozen_library)
    library = frozen_library
    # The dynamic loader resolves ELF SONAMEs, not necessarily the path supplied
    # to the linker. Preserve that alias beside the frozen library so a system
    # libexpat cannot silently satisfy the extension's dependency instead.
    dynamic = (
        subprocess.check_output(["readelf", "-d", str(library)], text=True)
        if library.suffix != ".a"
        else ""
    )
    soname = re.search(r"\(SONAME\).*\[([^]]+)\]", dynamic)
    if soname:
        name = soname.group(1)
        if Path(name).name != name:
            parser.error("library SONAME must be a file name")
        alias = output / name
        if alias != library and not alias.exists():
            alias.symlink_to(library.name)
    module_source = source / "Modules" / "pyexpat.c"
    text = module_source.read_text()
    if args.system_allocator:
        marker = '#include "pyexpat.h"'
        assert text.count(marker) == 1
        text = text.replace(
            marker,
            marker
            + """

/* Explicit Oriole opt-in: parser storage uses the Rust system allocator. */
static XML_Parser
oriole_create_system(const XML_Char *encoding,
                     const XML_Memory_Handling_Suite *suite,
                     const XML_Char *separator)
{
    (void)suite;
    return XML_ParserCreate_MM(encoding, NULL, separator);
}
#define XML_ParserCreate_MM oriole_create_system
""",
        )
    adapted = output / "pyexpat.c"
    adapted.write_text(text)
    compiler = shlex.split(os.environ.get("CC", "cc"))
    includes = [
        root / "include",
        Path(sysconfig.get_path("include")),
        Path(sysconfig.get_path("include")) / "internal",
        source / "Modules" / "expat",
        source / "Modules",
    ]
    flags = ["-shared", "-fPIC", "-O2", *[f"-I{p}" for p in includes]]
    suffix = sysconfig.get_config_var("EXT_SUFFIX")
    commands = []
    for name, path in [
        ("pyexpat", adapted),
        ("_elementtree", source / "Modules" / "_elementtree.c"),
    ]:
        command = (
            compiler
            + flags
            + [
                str(path),
                str(library),
                *[f"-l{name}" for name in args.native_library],
                f"-Wl,-rpath,{library.parent}",
                "-o",
                str(output / f"{name}{suffix}"),
            ]
        )
        commands.append(command)
        result = subprocess.run(command, text=True, capture_output=True, check=False)
        (output / f"{name}-build.log").write_text(result.stdout + result.stderr)
        if result.returncode:
            print(f"{name} build failed; see {output / f'{name}-build.log'}")
            return result.returncode
    env = os.environ.copy()
    env["PYTHONPATH"] = os.pathsep.join([str(output), str(source / "Lib")])
    # PBS may compile these modules into the interpreter. Builtins precede
    # PYTHONPATH, so explicitly load our extensions at startup in every worker.
    (output / "sitecustomize.py").write_text("""
import importlib.util
from pathlib import Path
import sys
import sysconfig
root = Path(__file__).resolve().parent
for name in ('pyexpat', '_elementtree'):
    path = root / (name + sysconfig.get_config_var('EXT_SUFFIX'))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    assert Path(module.__file__).resolve() == path
""")
    executable = getattr(sys, "_base_executable", sys.executable)
    probe = subprocess.run(
        [
            executable,
            "-s",
            "-c",
            (
                "import pyexpat, _elementtree; "
                "print(pyexpat.__file__); print(_elementtree.__file__); "
                "print(pyexpat.EXPAT_VERSION); "
                "p=pyexpat.ParserCreate(); p.Parse(b'<root/>', True)"
            ),
        ],
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    (output / "probe.log").write_text(probe.stdout + probe.stderr)
    # Fail closed if startup used a builtin or failed to load an extension.
    origin_probe = subprocess.run(
        [
            executable,
            "-s",
            "-c",
            (
                "import pyexpat, _elementtree; from pathlib import Path; "
                f"root=Path({str(output)!r}); "
                "assert Path(pyexpat.__file__).parent == root; "
                "assert Path(_elementtree.__file__).parent == root"
            ),
        ],
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    if origin_probe.returncode:
        (output / "origin-failure.log").write_text(
            origin_probe.stdout + origin_probe.stderr
        )
        print(f"extension origin check failed; see {output / 'origin-failure.log'}")
        return 1
    env["TMPDIR"] = str(output / "tmp")
    Path(env["TMPDIR"]).mkdir(exist_ok=True)
    command = [
        executable,
        "-s",
        "-m",
        "test",
        "-v",
        "-j1",
        "--timeout",
        "120",
        *args.tests,
    ]
    with (output / "tests.log").open("w") as log:
        result = subprocess.run(
            command,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            timeout=900,
            check=False,
        )
    evidence = {
        "schema_version": 1,
        "cpython_revision": revision,
        "python": sys.version,
        "library_sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
        "linkage": "static" if library.suffix == ".a" else "shared",
        "consumer_adaptation": "system allocator" if args.system_allocator else None,
        "commands": commands,
        "test_command": command,
        "probe_exit_code": probe.returncode,
        "tests_exit_code": result.returncode,
        "source_sha256": hashlib.sha256(module_source.read_bytes()).hexdigest(),
        "compiled_source_sha256": hashlib.sha256(adapted.read_bytes()).hexdigest(),
    }
    (output / "summary.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps(evidence, indent=2))
    return result.returncode or probe.returncode


if __name__ == "__main__":
    raise SystemExit(main())
