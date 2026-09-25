#!/usr/bin/env python3
"""Run Expat 2.8.5 public API tests in isolated children against a chosen library."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import signal
import subprocess
import sys
from pathlib import Path

REVISION = "4b3f0b06f39fb5529cead381694f8929901bc273"
EXCLUDED = {
    "test_hash_table": "Expat's private hash table implementation",
    "test_siphash_self": "Expat's private SipHash implementation",
    "test_siphash_spec": "Expat's private SipHash implementation",
    "test_utf8_auto_align": "private _INTERNAL_trim_to_complete_utf8_characters",
    "test_big_tokens_scale_linearly": "private g_bytesScanned counter",
    "test_varying_buffer_fills": "private g_bytesScanned counter",
    "test_accounting_precision": "private direct/indirect accounting getters",
    "test_helper_unsigned_char_to_printable": "private diagnostic formatting helper",
    "test_alloc_tracker_size_recorded": "private expat_malloc/realloc/free",
    "test_alloc_tracker_pointer_alignment": "private expat_malloc/free",
    "test_alloc_tracker_maximum_amplification": "private expat_malloc/free",
    "test_alloc_tracker_threshold": "private expat_malloc/free",
    "test_alloc_tracker_getbuffer_unlimited": "private expat_malloc",
}
TEST_BLOCK = re.compile(r"^START_TEST\((\w+)\).*?^END_TEST", re.MULTILINE | re.DOTALL)
REGISTRATION = re.compile(
    r"\b(tcase_add_test(?:__ifdef_xml_dtd|__if_xml_ge)?)\(\s*(\w+)\s*,\s*(\w+)\s*\);"
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--chunks", default="0,1,2,3,4,5")
    parser.add_argument(
        "--tests", nargs="+", help="exact public test names for a focused run"
    )
    parser.add_argument(
        "--allocation-behavior",
        action="store_true",
        help="separately audit allocation behavior with recorded test adaptations",
    )
    parser.add_argument("--deferral", default="0,1")
    parser.add_argument("--test-timeout", type=int, default=3)
    parser.add_argument("--memory-mib", type=int, default=1024)
    parser.add_argument("--rss-mib", type=int, default=768)
    parser.add_argument("--timeout", type=int, default=240)
    args = parser.parse_args()
    source, config, library, output = (
        path.resolve() for path in (args.source, args.config, args.library, args.output)
    )
    revision = subprocess.check_output(
        ["git", "-C", str(source), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != REVISION:
        parser.error(f"expected Expat {REVISION}, got {revision}")
    if subprocess.check_output(["git", "-C", str(source), "status", "--porcelain"]):
        parser.error("upstream source must be clean")
    chunks = [int(value) for value in args.chunks.split(",")]
    modes = [int(value) for value in args.deferral.split(",")]
    if not chunks or not modes or any(value not in range(6) for value in chunks):
        parser.error("chunks must be comma-separated values from 0 through 5")
    if any(value not in (0, 1) for value in modes):
        parser.error("deferral must be 0 and/or 1")
    if any(
        value <= 0
        for value in (args.test_timeout, args.timeout, args.memory_mib, args.rss_mib)
    ):
        parser.error("resource and time limits must be positive")
    if output.exists() and any(output.iterdir()):
        parser.error("output directory must be empty to preserve earlier evidence")
    output.mkdir(parents=True, exist_ok=True)
    adapted = output / "adapted"
    adapted.mkdir(exist_ok=True)
    upstream = source / "expat"
    # Test utilities include ../lib/xcsinc.c and private formatting headers. The
    # parser implementation files are copied for include resolution, never compiled.
    shutil.copytree(upstream / "lib", output / "lib", dirs_exist_ok=True)
    for path in (upstream / "tests").glob("*.[ch]"):
        shutil.copy2(path, adapted / path.name)
    for name in ("expat.h", "expat_external.h", "internal.h", "siphash.h"):
        shutil.copy2(upstream / "lib" / name, adapted / name)
    shutil.copy2(config, adapted / "expat_config.h")
    here = Path(__file__).resolve().parent
    for name in ("bridge.h", "bridge.c"):
        shutil.copy2(here / name, adapted / name)
    allocation_manifest = None
    if args.allocation_behavior:
        # The runner also works with Python's isolated mode (-I).
        sys.path.insert(0, str(here))
        from allocation_behavior import adapt_sources, selected_tests

        allocation_manifest = adapt_sources(adapted)
    seen_exclusions: set[str] = set()
    public_tests: set[str] = set()
    for path in adapted.glob("*.c"):
        if path.name == "bridge.c":
            continue
        text = path.read_text()

        def remove_internal(match: re.Match[str]) -> str:
            name = match[1]
            if name in EXCLUDED:
                seen_exclusions.add(name)
                return f"/* Excluded internal-only test: {name}. */"
            public_tests.add(name)
            return match[0]

        text = TEST_BLOCK.sub(remove_internal, text)

        def register(match: re.Match[str]) -> str:
            _function, _case, name = match.groups()
            if name in EXCLUDED:
                return f"/* Excluded registration: {name}. */"
            if name.startswith("test_"):
                return f'xeme_register_test({name}, "{name}");\n  {match[0]}'
            return match[0]

        text = REGISTRATION.sub(register, text)
        # Preserve every public test body; only append the adapter after includes.
        includes = list(re.finditer(r"^#include [^\n]+$", text, re.MULTILINE))
        offset = includes[-1].end()
        text = text[:offset] + '\n#include "bridge.h"' + text[offset:]
        if path.name == "minicheck.c":
            if args.allocation_behavior:
                text = '#include "allocation_tracker.h"\n' + text
            start = text.index("void\nsrunner_run_all(")
            end = text.index("void\nsrunner_summarize(", start)
            runner = (here / "runner.c").read_text()
            if args.allocation_behavior:
                assert runner.count("        fflush(NULL);\n        _Exit(0);") == 1
                runner = runner.replace(
                    "        fflush(NULL);\n        _Exit(0);",
                    "        xeme_audit_finish();\n        fflush(NULL);\n        _Exit(0);",
                )
            text = text[:start] + runner + "\n" + text[end:]
            text = text.replace(
                "longjmp(env, 1);",
                'fprintf(stderr, "ASSERTION: %s at %s:%d\\n", '
                "_check_current_function, file, line);\n  fflush(NULL);\n  _Exit(100);",
            )
            text = (
                "#include <errno.h>\n#include <sys/resource.h>\n"
                "#include <sys/wait.h>\n#include <unistd.h>\n#include <signal.h>\n"
                + text
            )
        if path.name == "runtests.c":
            text = text.replace(
                "int i, nf;", "int i, nf;\n  xeme_print_library();"
            ).replace(
                "char context[100];",
                "if (!xeme_context_enabled(g_chunkSize, enabled)) continue;\n"
                "      char context[100];",
            )
        path.write_text(text)
    if seen_exclusions != EXCLUDED.keys():
        parser.error(
            f"exclusion set drifted: {sorted(EXCLUDED.keys() - seen_exclusions)}"
        )
    if args.allocation_behavior:
        allocation_tests = selected_tests(public_tests)
        if args.tests and set(args.tests) - set(allocation_tests):
            parser.error(
                "allocation behavior mode only accepts its audited test inventory"
            )
        args.tests = args.tests or allocation_tests
        for name in ("allocation_tracker.c", "allocation_tracker.h"):
            shutil.copy2(here / name, adapted / name)
    if args.tests and set(args.tests) - public_tests:
        parser.error(f"unknown public tests: {sorted(set(args.tests) - public_tests)}")
    frozen_library = output / library.name
    shutil.copy2(library, frozen_library)
    dynamic = subprocess.check_output(["readelf", "-d", str(frozen_library)], text=True)
    soname = re.search(r"\(SONAME\).*\[([^]]+)\]", dynamic)
    if soname and soname[1] != frozen_library.name:
        if Path(soname[1]).name != soname[1]:
            parser.error("library SONAME must be a filename")
        alias = output / soname[1]
        if not alias.exists():
            alias.symlink_to(frozen_library.name)
    binary = output / "runtests"
    command = (
        shlex.split(os.environ.get("CC", "cc"))
        + [
            "-O1",
            "-DXML_TESTING",
            "-D_GNU_SOURCE",
            *(["-DXEME_ALLOCATION_BEHAVIOR"] if args.allocation_behavior else []),
            f"-I{adapted}",
            f"-I{output / 'lib'}",
        ]
        + [str(path) for path in sorted(adapted.glob("*.c"))]
        + [
            str(frozen_library),
            f"-Wl,-rpath,{output}",
            "-lm",
            "-ldl",
            "-o",
            str(binary),
        ]
    )
    manifest = {
        "revision": revision,
        "library_sha256": digest(frozen_library),
        "config_sha256": digest(config),
        "adapted_sources": {
            path.name: digest(path) for path in sorted(adapted.iterdir())
        },
        "upstream_include_sources": {
            str(path.relative_to(output)): digest(path)
            for path in sorted((output / "lib").rglob("*"))
            if path.is_file()
        },
        "adapter_sources": {
            path.name: digest(path) for path in sorted(here.iterdir()) if path.is_file()
        },
        "compile_command": command,
        "excluded_internal_tests": EXCLUDED,
        "public_tests_in_source": sorted(public_tests),
        "selected_tests": args.tests,
        "chunks": chunks,
        "deferral": modes,
        "per_test_seconds": args.test_timeout,
        "per_test_address_space_bytes": args.memory_mib * 1024**2,
        "per_test_rss_limit_bytes": args.rss_mib * 1024**2,
        "total_timeout_seconds": args.timeout,
        "allocation_behavior": allocation_manifest,
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    build = subprocess.run(
        command, text=True, capture_output=True, timeout=120, check=False
    )
    (output / "build.log").write_text(build.stdout + build.stderr)
    if build.returncode:
        print(f"Build failed; see {output / 'build.log'}")
        return build.returncode
    manifest["binary_sha256"] = digest(binary)
    env = os.environ.copy()
    env.update(
        XEME_CHUNK_MASK=str(sum(1 << value for value in set(chunks))),
        XEME_DEFERRAL_MASK=str(sum(1 << value for value in set(modes))),
        XEME_TEST_TIMEOUT=str(args.test_timeout),
        XEME_MEMORY_MIB=str(args.memory_mib),
        XEME_RSS_MIB=str(args.rss_mib),
        XEME_SELECTED_TESTS=",".join(args.tests or []),
    )
    with (output / "tests.log").open("w") as log:
        process = subprocess.Popen(
            [str(binary), "--verbose"],
            stdout=log,
            stderr=subprocess.STDOUT,
            env=env,
            start_new_session=True,
        )
        try:
            code = process.wait(timeout=args.timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            code = 124
    log = (output / "tests.log").read_text(errors="replace")
    origins = re.findall(r"^XEME_LIBRARY\t(.+)$", log, re.MULTILINE)
    versions = re.findall(r"^XEME_VERSION\t(.+)$", log, re.MULTILINE)
    origin_verified = bool(origins) and all(
        Path(origin).resolve() == frozen_library for origin in origins
    )
    results = [
        {"context": context, "test": test, "outcome": outcome, "code": int(result_code)}
        for context, test, outcome, result_code in re.findall(
            r"^XEME_RESULT\t([^\t]+)\t([^\t]+)\t([^\t]+)\t(\d+)$", log, re.MULTILINE
        )
    ]
    selection_complete = not args.tests or all(
        any(
            result["test"] == name
            and result["context"] == f"chunksize={chunk} deferral={mode}"
            for result in results
        )
        for name in args.tests
        for chunk in chunks
        for mode in modes
    )
    summary = {
        "version": versions[0] if len(versions) == 1 else None,
        "returncode": code,
        "selection_complete": selection_complete,
        "library_origin_verified": origin_verified,
        "results": results,
        "passed": sum(result["outcome"] == "pass" for result in results),
        "failed": sum(result["outcome"] != "pass" for result in results),
        "timed_out": code == 124,
        "excluded_internal_tests": EXCLUDED,
        "allocation_behavior": args.allocation_behavior,
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (output / "results.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(
        json.dumps(
            {
                key: value
                for key, value in summary.items()
                if key not in ("results", "excluded_internal_tests")
            }
        )
    )
    return code if origin_verified and results and selection_complete else 2


if __name__ == "__main__":
    raise SystemExit(main())
