"""Exercise only the fixed generated corpus in a separate training process."""

import argparse
import ctypes as c
import hashlib
import json
import os
import sys
from contextlib import contextmanager
from pathlib import Path

# Support isolated script execution while importing this tool's package.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from pgo.corpus import documents


class DynamicLibraryInfo(c.Structure):
    _fields_ = [
        ("filename", c.c_char_p),
        ("base", c.c_void_p),
        ("symbol", c.c_char_p),
        ("address", c.c_void_p),
    ]


def verify_origin(function, expected: Path) -> dict[str, str]:
    """Resolve a C function's actual shared library before invoking the parser."""
    dladdr = c.CDLL(None).dladdr
    dladdr.argtypes = [c.c_void_p, c.POINTER(DynamicLibraryInfo)]
    dladdr.restype = c.c_int
    info = DynamicLibraryInfo()
    if dladdr(c.cast(function, c.c_void_p), c.byref(info)) == 0 or not info.filename:
        raise RuntimeError("Cannot identify XML_Parse's loaded library")
    actual = Path(os.fsdecode(info.filename)).resolve()
    if actual != expected.resolve(strict=True):
        raise RuntimeError(f"XML_Parse loaded from {actual}, expected {expected}")
    return {
        "path": str(actual),
        "sha256": hashlib.sha256(actual.read_bytes()).hexdigest(),
    }


def main() -> None:
    parser_args = argparse.ArgumentParser(description=__doc__)
    parser_args.add_argument("--library", type=Path, required=True)
    parser_args.add_argument("--inputs", type=Path, required=True)
    parser_args.add_argument("--report", type=Path, required=True)
    args = parser_args.parse_args()
    libpath = args.library.resolve()
    lib = c.CDLL(str(libpath))
    P = c.c_void_p
    S = c.c_char_p
    Int = c.c_int

    def sha(path: Path) -> str:
        return hashlib.sha256(path.read_bytes()).hexdigest()

    def api(name, ret, args):
        f = getattr(lib, name)
        f.restype = ret
        f.argtypes = args
        return f

    create = api("XML_ParserCreate", P, [S])
    create_ns = api("XML_ParserCreateNS", P, [S, c.c_char])
    free = api("XML_ParserFree", None, [P])
    parse = api("XML_Parse", Int, [P, S, Int, Int])
    origin = verify_origin(parse, libpath)
    error = api("XML_GetErrorCode", Int, [P])
    param = api("XML_SetParamEntityParsing", Int, [P, Int])
    free_model = api("XML_FreeContentModel", None, [P, P])
    start_t = c.CFUNCTYPE(None, P, S, c.POINTER(S))
    end_t = c.CFUNCTYPE(None, P, S)
    text_t = c.CFUNCTYPE(None, P, P, Int)
    pair_t = c.CFUNCTYPE(None, P, S, S)
    void_t = c.CFUNCTYPE(None, P)
    doctype_t = c.CFUNCTYPE(None, P, S, S, S, Int)
    decl_t = c.CFUNCTYPE(None, P, S, Int, P, Int, S, S, S, S)
    att_t = c.CFUNCTYPE(None, P, S, S, S, S, Int)
    notation_t = c.CFUNCTYPE(None, P, S, S, S, S)
    model_t = c.CFUNCTYPE(None, P, S, P)
    set_elements = api("XML_SetElementHandler", None, [P, start_t, end_t])
    set_text = api("XML_SetCharacterDataHandler", None, [P, text_t])
    set_default = api("XML_SetDefaultHandlerExpand", None, [P, text_t])
    set_comment = api("XML_SetCommentHandler", None, [P, end_t])
    set_pi = api("XML_SetProcessingInstructionHandler", None, [P, pair_t])
    set_att = api("XML_SetAttlistDeclHandler", None, [P, att_t])
    set_decl = api("XML_SetEntityDeclHandler", None, [P, decl_t])
    set_model = api("XML_SetElementDeclHandler", None, [P, model_t])
    set_notation = api("XML_SetNotationDeclHandler", None, [P, notation_t])
    set_doctype = api("XML_SetDoctypeDeclHandler", None, [P, doctype_t, void_t])
    set_ns = api("XML_SetNamespaceDeclHandler", None, [P, pair_t, end_t])

    @contextmanager
    def parser_scope(namespaces):
        parser = create_ns(None, b"|") if namespaces else create(None)
        if not parser:
            raise RuntimeError("Parser allocation failed")
        try:
            yield parser
        finally:
            free(parser)

    def exercise(data, name, ns, chunk, handlers, iteration):
        h = hashlib.sha256()
        counts = {}
        faults = []
        with parser_scope(ns) as parser:

            def add(tag, *values):
                counts[tag] = counts.get(tag, 0) + 1
                h.update(tag.encode() + b":")
                for value in values:
                    if value is None:
                        h.update(b"N;")
                    elif isinstance(value, int):
                        h.update(str(value).encode() + b";")
                    else:
                        h.update(str(len(value)).encode() + b":" + value)

            def guarded(function):
                def call(*args):
                    try:
                        return function(*args)
                    except BaseException as exc:  # noqa: BLE001 - ctypes cannot propagate callback exceptions
                        faults.append(repr(exc))

                return call

            def start(_, name, attrs):
                values = []
                index = 0
                while attrs[index] is not None:
                    values.append(attrs[index])
                    index += 1
                add("start", name, *values)

            callbacks = [
                start_t(guarded(start)),
                end_t(guarded(lambda _, name: add("end", name))),
                text_t(guarded(lambda _, text, n: add("text", c.string_at(text, n)))),
            ]
            set_elements(parser, *callbacks[:2])
            set_text(parser, callbacks[2])
            if not param(parser, 2):
                raise RuntimeError("Parameter entity mode setup failed")
            if handlers == "full":
                default = text_t(
                    guarded(lambda _, text, n: add("default", c.string_at(text, n)))
                )
                comment = end_t(guarded(lambda _, text: add("comment", text)))
                pi = pair_t(guarded(lambda _, target, text: add("pi", target, text)))
                att = att_t(
                    guarded(
                        lambda _, el, name, kind, value, required: add(
                            "attlist",
                            el,
                            name,
                            kind,
                            value,
                            required,
                        )
                    )
                )
                decl = decl_t(
                    guarded(
                        lambda _, name, parameter, value, n, base, system, public, notation: (
                            add(
                                "entity",
                                name,
                                parameter,
                                c.string_at(value, n) if value else None,
                                base,
                                system,
                                public,
                                notation,
                            )
                        )
                    )
                )

                def element(_, name, model):
                    try:
                        add("element", name)
                    finally:
                        free_model(parser, model)

                model = model_t(guarded(element))
                notation = notation_t(
                    guarded(
                        lambda _, name, base, system, public: add(
                            "notation", name, base, system, public
                        )
                    )
                )
                doc = doctype_t(
                    guarded(
                        lambda _, name, system, public, internal: add(
                            "doctype",
                            name,
                            system,
                            public,
                            internal,
                        )
                    )
                )
                docend = void_t(guarded(lambda _: add("doctype-end")))
                nsstart = pair_t(
                    guarded(lambda _, prefix, uri: add("ns-start", prefix, uri))
                )
                nsend = end_t(guarded(lambda _, prefix: add("ns-end", prefix)))
                callbacks += [
                    default,
                    comment,
                    pi,
                    att,
                    decl,
                    model,
                    notation,
                    doc,
                    docend,
                    nsstart,
                    nsend,
                ]
                set_default(parser, default)
                set_comment(parser, comment)
                set_pi(parser, pi)
                set_att(parser, att)
                set_decl(parser, decl)
                set_model(parser, model)
                set_notation(parser, notation)
                set_doctype(parser, doc, docend)
                set_ns(parser, nsstart, nsend)
            status = 1
            for offset in range(0, len(data), chunk):
                part = data[offset : offset + chunk]
                status = parse(
                    parser,
                    part,
                    len(part),
                    int(offset + chunk >= len(data)),
                )
                if status != 1:
                    break
            record = {
                "fixture": name,
                "namespaces": ns,
                "chunk": chunk,
                "handlers": handlers,
                "iteration": iteration,
                "status": status,
                "error": error(parser),
                "callback_sha256": h.hexdigest(),
                "counts": counts,
                "callback_exceptions": faults,
            }
            return record

    inputs = json.loads((args.inputs / "manifest.json").read_text())
    generated = documents()
    if len(inputs["rows"]) != len(generated) or {
        row["name"] for row in inputs["rows"]
    } != set(generated):
        raise ValueError("Training corpus must be the fixed generated corpus")
    report = {
        "status": "incomplete",
        "library_sha256": sha(libpath),
        "xml_parse_origin": origin,
        "inputs_sha256": sha(args.inputs / "manifest.json"),
        "script_sha256": sha(Path(__file__)),
        "profile_file_environment": os.environ.get("LLVM_PROFILE_FILE"),
        "rows": [],
        "scope": "Generated training inputs only; minimal and full callback modes, both namespace modes, three chunk widths. Each input is parsed twice with fresh parsers. The Python driver is not instrumented; its runtime is not a benchmark.",
    }
    try:
        for fixture in inputs["rows"]:
            name = fixture["name"]
            if (
                fixture["file"] != f"{name}.xml"
                or fixture["namespaces"] != [False, True]
                or fixture["chunks"] != [1 if name == "latin1" else 17, 4096, 65536]
            ):
                raise ValueError("Training corpus configuration changed")
            data = (args.inputs / fixture["file"]).read_bytes()
            if (
                data != generated[name][0]
                or hashlib.sha256(data).hexdigest() != fixture["sha256"]
            ):
                raise ValueError("Training corpus content changed")
            for ns in fixture["namespaces"]:
                for chunk in fixture["chunks"]:
                    for handlers in ["minimal", "full"]:
                        expected = None
                        for iteration in range(2):
                            record = exercise(
                                data, name, ns, chunk, handlers, iteration
                            )
                            report["rows"].append(record)
                            if (
                                record["status"] != 1
                                or record["error"] != 0
                                or record["callback_exceptions"]
                            ):
                                raise RuntimeError(record)
                            normalized = {
                                k: v for k, v in record.items() if k != "iteration"
                            }
                            if expected is None:
                                expected = normalized
                            else:
                                if normalized != expected:
                                    raise RuntimeError("Repeated parser results differ")
        report["status"] = "passed"
    finally:
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"status": report["status"], "parses": len(report["rows"])}))


if __name__ == "__main__":
    main()
