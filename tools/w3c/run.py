"""Read the original W3C catalog and compare nonvalidating XML 1.0 acceptance."""

import argparse
import collections
import ctypes as C
import hashlib
import json
import resource
import subprocess
import sys
import urllib.parse
import xml.parsers.expat
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from xml_abi import load_library

SUITE = Path()
OUT = Path()


def resolve(base, value):
    uri = urllib.parse.urlsplit(urllib.parse.urljoin(base, value))
    if uri.scheme != "file" or uri.netloc not in ("", "localhost"):
        raise ValueError("nonlocal URI: " + uri.geturl())
    path = Path(urllib.parse.unquote(uri.path)).resolve()
    if not path.is_relative_to(SUITE):
        raise ValueError("URI outside suite: " + str(path))
    return path


def catalog():
    stack = [(SUITE / "xmlconf.xml").as_uri()]
    rows = []

    def start(name, attrs):
        base = urllib.parse.urljoin(stack[-1], attrs.get("xml:base", ""))
        stack.append(base)
        if name == "TEST":
            row = dict(attrs)
            path = resolve(base, attrs["URI"])
            # The original catalog has this documented base-URI typo. Retain
            # both paths; do not edit the suite or use the mirror's cleaned XML.
            if "/eduni/namespaces/misc/" in str(path):
                row["catalog_uri_correction"] = str(path.relative_to(SUITE))
                path = Path(
                    str(path).replace("/eduni/namespaces/misc/", "/eduni/misc/")
                )
            row["path"] = str(path.relative_to(SUITE))
            rows.append(row)

    def load(parser, path):
        parser.SetBase(path.as_uri())
        parser.SetParamEntityParsing(xml.parsers.expat.XML_PARAM_ENTITY_PARSING_ALWAYS)
        parser.StartElementHandler = start
        parser.EndElementHandler = lambda name: stack.pop()

        def external(context, base, system, public):
            child = parser.ExternalEntityParserCreate(context)
            load(child, resolve(base, system))
            return 1

        parser.ExternalEntityRefHandler = external
        parser.Parse(path.read_bytes(), True)

    load(xml.parsers.expat.ParserCreate(), SUITE / "xmlconf.xml")
    return rows


EXT = C.CFUNCTYPE(C.c_int, C.c_void_p, C.c_void_p, C.c_char_p, C.c_char_p, C.c_char_p)


class Engine:
    def __init__(self, library):
        self.lib = load_library(library)
        for name, args, result in [
            ("XML_ParserCreate", [C.c_char_p], C.c_void_p),
            ("XML_ParserCreateNS", [C.c_char_p, C.c_char], C.c_void_p),
            ("XML_ParserFree", [C.c_void_p], None),
            (
                "XML_ExternalEntityParserCreate",
                [C.c_void_p, C.c_void_p, C.c_char_p],
                C.c_void_p,
            ),
            ("XML_SetExternalEntityRefHandler", [C.c_void_p, EXT], None),
            ("XML_SetParamEntityParsing", [C.c_void_p, C.c_int], C.c_int),
            ("XML_SetBase", [C.c_void_p, C.c_char_p], C.c_int),
            ("XML_Parse", [C.c_void_p, C.c_char_p, C.c_int, C.c_int], C.c_int),
            ("XML_GetErrorCode", [C.c_void_p], C.c_int),
            ("XML_GetCurrentByteIndex", [C.c_void_p], C.c_long),
            ("XML_ExpatVersion", [], C.c_char_p),
        ]:
            fn = getattr(self.lib, name)
            fn.argtypes = args
            fn.restype = result
        self.version = self.lib.XML_ExpatVersion().decode()

    def parse(self, path, chunk, namespaces):
        lib = self.lib
        parser = (
            lib.XML_ParserCreateNS(None, b"|")
            if namespaces
            else lib.XML_ParserCreate(None)
        )
        if not parser:
            raise MemoryError("parser creation")
        errors = []
        loaded = []
        children = []
        depth = 0
        requests = 0

        def feed(handle, file):
            data = file.read_bytes()
            if len(data) > 2 * 1024 * 1024:
                raise ValueError("file budget")
            loaded.append(
                {
                    "path": str(file.relative_to(SUITE)),
                    "sha256": hashlib.sha256(data).hexdigest(),
                }
            )
            if not lib.XML_SetBase(handle, file.as_uri().encode()):
                raise MemoryError("setbase")
            lib.XML_SetParamEntityParsing(handle, 2)
            lib.XML_SetExternalEntityRefHandler(handle, external)
            if not data:
                return lib.XML_Parse(handle, b"", 0, 1)
            for offset in range(0, len(data), chunk):
                part = data[offset : offset + chunk]
                status = lib.XML_Parse(
                    handle, part, len(part), int(offset + chunk >= len(data))
                )
                if status != 1:
                    return status
            return 1

        @EXT
        def external(parent, context, base, system, public):
            nonlocal depth, requests
            child = None
            try:
                depth += 1
                requests += 1
                if depth > 32 or requests > 1024:
                    raise ValueError("external family budget")
                file = resolve(base.decode(), system.decode())
                child = lib.XML_ExternalEntityParserCreate(parent, context, None)
                if not child:
                    raise MemoryError("child creation")
                status = feed(child, file)
                children.append(
                    {
                        "path": str(file.relative_to(SUITE)),
                        "status": status,
                        "error": lib.XML_GetErrorCode(child),
                    }
                )
                return int(status == 1)
            except Exception as exc:  # noqa: BLE001 - never let a Python exception escape a C callback
                errors.append(type(exc).__name__ + ": " + str(exc))
                return 0
            finally:
                if child:
                    lib.XML_ParserFree(child)
                depth -= 1

        try:
            status = feed(parser, path)
            return {
                "status": status,
                "error": lib.XML_GetErrorCode(parser),
                "index": lib.XML_GetCurrentByteIndex(parser),
                "resolver_errors": errors,
                "loaded": loaded,
                "children": children,
            }
        finally:
            lib.XML_ParserFree(parser)


def worker(library, label):
    engine = Engine(library)
    rows = []
    for case in json.loads((OUT / "catalog.json").read_text()):
        if case.get("skip"):
            continue
        for chunk in (1, 7, 4096):
            row = {
                "id": case["ID"],
                "path": case["path"],
                "type": case["TYPE"],
                "chunk": chunk,
                "namespaces": case.get("NAMESPACE", "yes") != "no",
            }
            (OUT / (label + "-progress.json")).write_text(json.dumps(row) + "\n")
            row["result"] = engine.parse(SUITE / case["path"], chunk, row["namespaces"])
            rows.append(row)
    (OUT / (label + ".json")).write_text(
        json.dumps(
            {
                "library": library,
                "sha256": hashlib.sha256(Path(library).read_bytes()).hexdigest(),
                "version": engine.version,
                "rows": rows,
            },
            separators=(",", ":"),
        )
        + "\n"
    )
    print(label, len(rows), flush=True)


def main():
    global SUITE, OUT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", type=Path, required=True)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--worker", choices=("xeme", "reference"), help=argparse.SUPPRESS
    )
    args = parser.parse_args()
    SUITE = args.suite.resolve(strict=True)
    OUT = args.output.resolve()
    if args.worker:
        worker(str(args.library.resolve(strict=True)), args.worker)
        return 0
    if args.reference is None:
        parser.error("--reference is required")
    OUT.mkdir(parents=True, exist_ok=False)
    descriptors = catalog()
    for row in descriptors:
        if (
            row.get("RECOMMENDATION", "XML1.0").startswith(("XML1.1", "NS1.1"))
            or "1.0" not in row.get("VERSION", "1.0").split()
        ):
            row["skip"] = "XML 1.1"
        elif "5" not in row.get("EDITION", "5").split():
            row["skip"] = "Earlier XML edition only"
    (OUT / "catalog.json").write_text(json.dumps(descriptors, indent=2) + "\n")
    report = {
        "scope": "Nonvalidating XML 1.0 Fifth Edition and Namespaces 1.0 acceptance; valid and invalid cases must accept, not-wf must reject, error is optional. Resolver failures are inconclusive and receive no conformance credit. No canonical-output comparison. All referenced external files are read through a local-only resolver.",
        "catalog_descriptors": len(descriptors),
        "selected_descriptors": sum(not row.get("skip") for row in descriptors),
        "chunks": [1, 7, 4096],
        "worker_limits": {
            "address_space_bytes": 1024**3,
            "wall_seconds": 120,
            "core_bytes": 0,
        },
        "resolver_limits": {
            "file_bytes": 2 * 1024 * 1024,
            "depth": 32,
            "requests": 1024,
        },
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "catalog_source_sha256": {
            str(path.relative_to(SUITE)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in SUITE.rglob("*")
            if path.is_file()
        },
        "commands": [],
        "engines": {},
        "mismatches": {},
        "inconclusive": {},
        "worker_failures": {},
    }

    def limits():
        resource.setrlimit(resource.RLIMIT_AS, (1024**3, 1024**3))
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))

    all_rows = {}
    for label, library in (("reference", args.reference), ("xeme", args.library)):
        command = [
            sys.executable,
            "-I",
            "-S",
            str(Path(__file__).resolve()),
            "--suite",
            str(SUITE),
            "--output",
            str(OUT),
            "--library",
            str(library.resolve(strict=True)),
            "--worker",
            label,
        ]
        command_result = {"command": command, "returncode": None, "timed_out": False}
        report["commands"].append(command_result)
        try:
            with (OUT / (label + ".log")).open("w") as log:
                completed = subprocess.run(
                    command,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    preexec_fn=limits,
                    timeout=120,
                    check=False,
                )
            command_result["returncode"] = completed.returncode
            if completed.returncode:
                report["worker_failures"][label] = {
                    "reason": "nonzero exit",
                    "returncode": completed.returncode,
                }
            else:
                observed = json.loads((OUT / (label + ".json")).read_text())
        except subprocess.TimeoutExpired:
            command_result["timed_out"] = True
            report["worker_failures"][label] = {
                "reason": "timeout",
                "wall_seconds": 120,
            }
        except (OSError, subprocess.SubprocessError, json.JSONDecodeError) as exc:
            report["worker_failures"][label] = {
                "reason": type(exc).__name__,
                "message": str(exc),
            }
        if label in report["worker_failures"]:
            (OUT / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
            print(label + " worker failed; see summary and log", file=sys.stderr)
            return 1
        all_rows[label] = observed["rows"]
        counts = collections.Counter(
            mandatory_pass=0,
            mandatory_fail=0,
            optional_observations=0,
            inconclusive=0,
            resolver_errors=0,
        )
        failed = []
        inconclusive = []
        for row in observed["rows"]:
            result = row["result"]
            if result["resolver_errors"]:
                counts["resolver_errors"] += 1
                counts["inconclusive"] += 1
                inconclusive.append(row)
                continue
            if row["type"] == "error":
                counts["optional_observations"] += 1
                continue
            expected_accept = row["type"] in ("valid", "invalid")
            passed = (result["status"] == 1) == expected_accept
            counts["mandatory_pass" if passed else "mandatory_fail"] += 1
            if not passed:
                failed.append(row)
        report["engines"][label] = {
            "library": observed["library"],
            "sha256": observed["sha256"],
            "version": observed["version"],
            **counts,
        }
        report["mismatches"][label] = failed
        report["inconclusive"][label] = inconclusive
    pairs = list(zip(all_rows["reference"], all_rows["xeme"], strict=True))
    report["inconclusive_comparisons"] = [
        {"reference": left, "xeme": right}
        for left, right in pairs
        if left["result"]["resolver_errors"] or right["result"]["resolver_errors"]
    ]
    report["acceptance_differences"] = [
        {"reference": left, "xeme": right}
        for left, right in pairs
        if not left["result"]["resolver_errors"]
        and not right["result"]["resolver_errors"]
        and (left["result"]["status"] == 1) != (right["result"]["status"] == 1)
    ]
    (OUT / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report["engines"], indent=2))
    return int(
        any(
            engine.get("mandatory_fail", 0) or engine.get("resolver_errors", 0)
            for engine in report["engines"].values()
        )
    )


if __name__ == "__main__":
    raise SystemExit(main())
