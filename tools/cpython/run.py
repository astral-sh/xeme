#!/usr/bin/env python3
"""Build real CPython XML extensions against Xeme and run upstream tests.

Run with a CPython 3.12.13 interpreter and its development headers. Sources are
pinned and never edited in place. Optional consumer adaptations are recorded
explicitly; their results are separate from unmodified-consumer compatibility.
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
import tempfile
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

TEXT_FRAGMENTATION_FAILURES = {
    "test1 (test.test_pyexpat.BufferTextTest.test1)",
    "test_handlers (test.test_sax.CDATAHandlerTest.test_handlers)",
}

# The optional exception is deliberately tied to these exact upstream assertions,
# rather than every failure that happens to occur in the same test methods.
FRAGMENTATION_ASSERTIONS = {
    "test1 (test.test_pyexpat.BufferTextTest.test1)": (
        "test_pyexpat.py",
        401,
        "test1",
        "self.assertEqual(self.stuff,",
        "AssertionError: Lists differ: ['<a>', '1', '<b>', '2\\n3', '<c>', '4\\n5'] != "
        "['<a>', '1', '<b>', '2', '\\n', '3', '<c>', '4\\n5']",
    ),
    "test_handlers (test.test_sax.CDATAHandlerTest.test_handlers)": (
        "test_sax.py",
        1543,
        "characters",
        "h.assertEqual(t[0], content)",
        "AssertionError: 'Parseable character data' != '\\nParseable character data\\n'",
    ),
}
FRAGMENTATION_SOURCE_HASHES = {
    "test_pyexpat.py": "44129616745434f065948aa9cee6a9be4546aa7cea93b952259e140cbc3fe844",
    "test_sax.py": "b061db0792bb838bf2568ff145b770db0b89cbf7bfa995664f4d392d5e41ec34",
}
# --list-cases includes cases skipped in setUpClass. Their identities are stable
# across the supported 3.12.13 environments even when resource skips differ.
TEST_INVENTORY_SHA256 = (
    "78757aa1f8927a3eabd8ab26acdc16d037d9bf936ebd5a3e8c420e4b4b59db68"
)


def test_inventory(text: str) -> list[str]:
    """Validate every pinned case identity, including cases omitted by class skips."""
    cases = text.splitlines()
    digest = hashlib.sha256(("\n".join(sorted(cases)) + "\n").encode()).hexdigest()
    if digest != TEST_INVENTORY_SHA256 or len(cases) != len(set(cases)):
        raise ValueError("unexpected CPython XML test inventory")
    return cases


def complete_inventory(log: str, cases: list[str], reported_count: int) -> bool:
    """Require each expected test or an explicit skip of its entire class."""
    records = re.findall(r"^(\w+) \((test\.[^()\n]+)\)([^\n]*)$", log, re.MULTILINE)
    observed = [identity for method, identity, _ in records if method != "setUpClass"]
    skipped_classes = {
        identity
        for method, identity, tail in records
        if method == "setUpClass" and tail.startswith(" ... skipped ")
    }
    omitted = {case for case in cases if case.rsplit(".", 1)[0] in skipped_classes}
    return (
        len(observed) == len(set(observed))
        and set(observed) == set(cases) - omitted
        and reported_count == len(observed)
        and all(
            any(case.startswith(cls + ".") for case in omitted)
            for cls in skipped_classes
        )
    )


def successful_test_run(
    log: str, returncode: int, file_count: int, cases: list[str]
) -> bool:
    """Require a successful upstream run with every pinned case accounted for."""
    total = re.search(r"^Total tests: run=(\d+)(?: skipped=\d+)?$", log, re.MULTILINE)
    return (
        returncode == 0
        and total is not None
        and f"Total test files: run={file_count}/{file_count}" in log.splitlines()
        and not re.search(r"^(FAIL|ERROR): ", log, re.MULTILINE)
        and complete_inventory(log, cases, int(total[1]))
    )


def only_text_fragmentation(
    log: str,
    returncode: int,
    file_count: int,
    cases: list[str],
    test_directory: Path | None = None,
) -> bool:
    """Recognize only the two pinned upstream callback-boundary assertions."""
    failures = re.findall(r"^(FAIL|ERROR): (.+)$", log, re.MULTILINE)
    total = re.search(
        r"^Total tests: run=(\d+) failures=(\d+)(?: skipped=\d+)?$", log, re.MULTILINE
    )
    complete = (
        f"Total test files: run={file_count}/{file_count} failed={len(failures)}"
        in log.splitlines()
    )
    valid = (
        returncode == 2
        and bool(failures)
        and total is not None
        and int(total[2]) == len(failures)
        and complete
        and complete_inventory(log, cases, int(total[1]))
        and len({name for _, name in failures}) == len(failures)
        and all(
            kind == "FAIL" and name in TEXT_FRAGMENTATION_FAILURES
            for kind, name in failures
        )
    )
    if not valid:
        return False
    for _, name in failures:
        section = re.search(
            r"^FAIL: " + re.escape(name) + r"\n-+\n(.*?)\n-+\n",
            log,
            re.MULTILINE | re.DOTALL,
        )
        if section is None:
            return False
        traceback = section[1]
        filename, line, function, source_line, assertion = FRAGMENTATION_ASSERTIONS[
            name
        ]
        frames = re.findall(
            r'^  File "([^"\n]+)", line (\d+), in ([^\n]+)\n    ([^\n]+)',
            traceback,
            re.MULTILINE,
        )
        assertions = re.findall(r"^AssertionError: [^\n]*$", traceback, re.MULTILINE)
        if (
            not frames
            or (
                Path(frames[-1][0]).resolve() != (test_directory / filename).resolve()
                if test_directory is not None
                else not frames[-1][0].endswith(f"/Lib/test/{filename}")
            )
            or frames[-1][1:] != (str(line), function, source_line)
            or assertions != [assertion]
        ):
            return False
    return True


def apply_consumer_fix(text: str, root: Path, output: Path) -> tuple[str, dict]:
    """Apply the pinned upstream allocation-failure backport to a temporary copy."""
    directory = root / "tools/cpython/consumer-fix"
    patch = directory / "cpython-3.12.13-external-parser.patch"
    provenance = json.loads((directory / "provenance.json").read_text())
    if hashlib.sha256(text.encode()).hexdigest() != provenance["source_sha256"]:
        raise ValueError("consumer fix requires the pinned unmodified pyexpat.c")
    if hashlib.sha256(patch.read_bytes()).hexdigest() != provenance["patch_sha256"]:
        raise ValueError("consumer fix patch does not match its provenance")
    with tempfile.TemporaryDirectory(prefix="consumer-fix-", dir=output) as temporary:
        tree = Path(temporary)
        copied = tree / "Modules/pyexpat.c"
        copied.parent.mkdir()
        copied.write_text(text)
        command = ["patch", "--batch", "--forward", "--fuzz=0", "-p1", "-i", str(patch)]
        result = subprocess.run(
            command, cwd=tree, text=True, capture_output=True, check=False
        )
        (output / "consumer-fix.log").write_text(result.stdout + result.stderr)
        if result.returncode:
            raise ValueError("consumer fix failed; see consumer-fix.log")
        fixed = copied.read_text()
    if (
        hashlib.sha256(fixed.encode()).hexdigest()
        != provenance["patched_source_sha256"]
    ):
        raise ValueError("patched pyexpat.c does not match the pinned backport")
    return fixed, provenance


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--system-allocator", action="store_true")
    parser.add_argument(
        "--allow-text-fragmentation",
        action="store_true",
        help="Reproduce the historical two-assertion exception; not a current release gate",
    )
    parser.add_argument(
        "--consumer-fix",
        action="store_true",
        help="Explicitly backport CPython's upstream pyexpat allocation-failure fix",
    )
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
    full_suite = sorted(args.tests) == sorted(TESTS)
    if args.allow_text_fragmentation and not full_suite:
        parser.error("the fragmentation exception requires all six XML test modules")
    if full_suite:
        for filename, expected in FRAGMENTATION_SOURCE_HASHES.items():
            if (
                hashlib.sha256(
                    (source / "Lib/test" / filename).read_bytes()
                ).hexdigest()
                != expected
            ):
                parser.error(f"upstream fixture source changed: {filename}")
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
    consumer_fix = None
    adaptations = []
    if args.consumer_fix:
        text, consumer_fix = apply_consumer_fix(text, root, output)
        adaptations.append("CPython upstream allocation-failure fix")
    if args.system_allocator:
        adaptations.append("system allocator")
        marker = '#include "pyexpat.h"'
        assert text.count(marker) == 1
        text = text.replace(
            marker,
            marker
            + """

/* Explicit Xeme opt-in: parser storage uses the Rust system allocator. */
static XML_Parser
xeme_create_system(const XML_Char *encoding,
                     const XML_Memory_Handling_Suite *suite,
                     const XML_Char *separator)
{
    (void)suite;
    return XML_ParserCreate_MM(encoding, NULL, separator);
}
#define XML_ParserCreate_MM xeme_create_system
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
    # PBS may compile these modules into the interpreter. Keep the override
    # active for the upstream tests' fresh imports as well as worker startup.
    loader = Path(__file__).with_name("extension_loader.py")
    shutil.copy2(loader, output / "sitecustomize.py")
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
    import_check = Path(__file__).with_name("check_extension_imports.py")
    origin_command = [executable, "-s", str(import_check), str(output)]
    origin_probe = subprocess.run(
        origin_command,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    (output / "origin.log").write_text(origin_probe.stdout + origin_probe.stderr)
    if origin_probe.returncode:
        print(f"extension origin check failed; see {output / 'origin.log'}")
        return 1
    env["TMPDIR"] = str(output / "tmp")
    Path(env["TMPDIR"]).mkdir(exist_ok=True)
    cases = None
    inventory_command = [executable, "-s", "-m", "test", "--list-cases", *args.tests]
    if full_suite:
        inventory = subprocess.run(
            inventory_command,
            env=env,
            text=True,
            capture_output=True,
            timeout=120,
            check=False,
        )
        (output / "test-inventory.log").write_text(inventory.stdout)
        (output / "test-inventory.stderr").write_text(inventory.stderr)
        if inventory.returncode:
            print("test inventory failed; see test-inventory.stderr")
            return 1
        try:
            cases = test_inventory(inventory.stdout)
        except ValueError as error:
            print(error)
            return 1
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
    gate_exit_code = result.returncode
    fragmentation = None
    if full_suite:
        assert cases is not None
        tests_log = (output / "tests.log").read_text()
        total = re.search(r"^Total tests: run=(\d+)\b", tests_log, re.MULTILINE)
        inventory_complete = total is not None and complete_inventory(
            tests_log, cases, int(total[1])
        )
        if not successful_test_run(
            tests_log, result.returncode, len(args.tests), cases
        ):
            gate_exit_code = gate_exit_code or 1
        semantic_script = Path(__file__).with_name("text_fragmentation.py")
        semantic_command = [executable, "-s", str(semantic_script), str(output)]
        with (output / "text-fragmentation.log").open("w") as log:
            semantic = subprocess.run(
                semantic_command,
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
                timeout=120,
                check=False,
            )
        known_failures = args.allow_text_fragmentation and only_text_fragmentation(
            tests_log, result.returncode, len(args.tests), cases, source / "Lib/test"
        )
        accepted = known_failures and semantic.returncode == 0
        if accepted:
            gate_exit_code = 0
        if semantic.returncode:
            gate_exit_code = gate_exit_code or semantic.returncode
        fragmentation = {
            "accepted_upstream_failures": accepted,
            "allowed_assertions": (
                sorted(TEXT_FRAGMENTATION_FAILURES)
                if args.allow_text_fragmentation
                else []
            ),
            "semantic_command": semantic_command,
            "semantic_exit_code": semantic.returncode,
            "semantic_source_sha256": hashlib.sha256(
                semantic_script.read_bytes()
            ).hexdigest(),
            "upstream_fixture_sha256": FRAGMENTATION_SOURCE_HASHES,
            "inventory_command": inventory_command,
            "inventory_sha256": TEST_INVENTORY_SHA256,
            "inventory_case_count": len(cases),
            "inventory_complete": inventory_complete,
        }
    evidence = {
        "schema_version": 1,
        "cpython_revision": revision,
        "python": sys.version,
        "library_sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
        "linkage": "static" if library.suffix == ".a" else "shared",
        "consumer_adaptation": ", ".join(adaptations) or None,
        "consumer_fix": consumer_fix,
        "commands": commands,
        "test_command": command,
        "probe_exit_code": probe.returncode,
        "origin_command": origin_command,
        "origin_exit_code": origin_probe.returncode,
        "loader_sha256": hashlib.sha256(loader.read_bytes()).hexdigest(),
        "import_check_sha256": hashlib.sha256(import_check.read_bytes()).hexdigest(),
        "tests_exit_code": result.returncode,
        "gate_exit_code": gate_exit_code,
        "text_fragmentation": fragmentation,
        "source_sha256": hashlib.sha256(module_source.read_bytes()).hexdigest(),
        "compiled_source_sha256": hashlib.sha256(adapted.read_bytes()).hexdigest(),
    }
    (output / "summary.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps(evidence, indent=2))
    return gate_exit_code or probe.returncode


if __name__ == "__main__":
    raise SystemExit(main())
