"""Replay external-entity base URI regressions against two Expat ABI libraries."""

from __future__ import annotations

import argparse
import ctypes as c
from pathlib import Path

from xml_abi import Expat, I, P, S

EXTERNAL = c.CFUNCTYPE(I, P, S, S, S, S)
DOCTYPE = c.CFUNCTYPE(None, P, S, S, S, I)
DATA = (
    Path(__file__).resolve().parents[1] / "crates/xeme_expat/tests/data/entity-base"
)


def run(path: str, mode: str, initial: bytes | None, buffered: bool) -> list:
    lib = Expat(path).lib
    for name, result, args in [
        ("XML_ExternalEntityParserCreate", P, [P, S, S]),
        ("XML_SetBase", I, [P, S]),
        ("XML_UseForeignDTD", I, [P, c.c_ubyte]),
        ("XML_SetParamEntityParsing", I, [P, I]),
        ("XML_GetBuffer", P, [P, I]),
        ("XML_ParseBuffer", I, [P, I, I]),
        ("XML_SetExternalEntityRefHandler", None, [P, EXTERNAL]),
        ("XML_SetStartDoctypeDeclHandler", None, [P, DOCTYPE]),
    ]:
        function = getattr(lib, name)
        function.restype, function.argtypes = result, args
    events = []
    errors = []

    def feed(parser, data, final):
        if buffered:
            buffer = lib.XML_GetBuffer(parser, len(data))
            if not buffer:
                raise AssertionError("GetBuffer failed")
            c.memmove(buffer, data, len(data))
            status = lib.XML_ParseBuffer(parser, len(data), final)
        else:
            status = lib.XML_Parse(parser, data, len(data), final)
        if status != 1:
            raise AssertionError(
                (mode, "parse failed", lib.XML_GetErrorCode(parser), errors)
            )

    @EXTERNAL
    def external(parser, context, base, system, public):
        events.append((base, system, public))
        child = None
        try:
            if system is None or system == b"data.txt":
                return 1
            child = lib.XML_ExternalEntityParserCreate(parser, context, None)
            assert child
            child_base = b"/dtd/" + system
            assert lib.XML_SetBase(child, child_base) == 1
            if system == b"entities.dtd" and mode in {"parameter", "grammar", "value"}:
                feed(child, b'<!ENTITY % p SYSTEM "nested.dtd">', 0)
                assert lib.XML_SetBase(child, b"/changed/dtd") == 1
                body = {
                    "parameter": b"%p;",
                    "grammar": b"<!ELEMENT r %p;EMPTY>",
                    "value": b'<!ENTITY i "%p;">',
                }[mode]
            elif system == b"nested.dtd":
                body = {"parameter": b"", "grammar": b"", "value": b"value"}[mode]
            elif system == b"entities.dtd" and mode == "general-child":
                body = b'<!ENTITY wrapper SYSTEM "wrapper.txt"><!ENTITY external SYSTEM "data.txt">'
            elif system == b"wrapper.txt":
                body = b"&external;"
            else:
                body = (DATA / "entities.dtd").read_bytes()
            feed(child, body, 1)
            return 1
        except BaseException as error:  # noqa: BLE001 — ctypes callbacks must not unwind
            errors.append(repr(error))
            return 0
        finally:
            if child:
                lib.XML_ParserFree(child)

    @DOCTYPE
    def doctype(parser, *_):
        if lib.XML_SetBase(parser, b"/changed/doctype") != 1:
            errors.append("SetBase in doctype callback failed")

    parser = lib.XML_ParserCreate(None)
    try:
        assert parser
        lib.XML_SetExternalEntityRefHandler(parser, external)
        assert lib.XML_SetParamEntityParsing(parser, 2) == 1
        assert lib.XML_SetBase(parser, initial) == 1
        if mode == "internal":
            feed(parser, b'<!DOCTYPE r [<!ENTITY external SYSTEM "data.txt">]>', 0)
            assert lib.XML_SetBase(parser, b"/changed/root") == 1
            body = b"<r>&external;</r>"
        elif mode in {"doctype", "foreign"}:
            lib.XML_SetUserData(parser, parser)
            lib.XML_SetStartDoctypeDeclHandler(parser, doctype)
            if mode == "foreign":
                assert lib.XML_UseForeignDTD(parser, 1) == 0
                body = b"<!DOCTYPE r><r/>"
            else:
                body = (DATA / "root.xml").read_bytes()
        elif mode == "general-child":
            body = b'<!DOCTYPE r SYSTEM "entities.dtd"><r>&wrapper;</r>'
        elif mode in {"parameter", "grammar", "value"}:
            body = b'<!DOCTYPE r SYSTEM "entities.dtd"><r/>'
        else:
            body = (DATA / "root.xml").read_bytes()
        feed(parser, body, 1)
        assert not errors, errors
        return events
    finally:
        lib.XML_ParserFree(parser)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference")
    parser.add_argument("candidate")
    args = parser.parse_args()
    count = 0
    for mode in [
        "internal",
        "external",
        "doctype",
        "general-child",
        "parameter",
        "grammar",
        "value",
        "foreign",
    ]:
        for initial in [None, b"", b"/root/\xff.xml"]:
            for buffered in [False, True]:
                expected = run(args.reference, mode, initial, buffered)
                actual = run(args.candidate, mode, initial, buffered)
                assert actual == expected, (mode, initial, buffered, expected, actual)
                count += 1
    print(
        f"PASS: {count} external entity base scenarios agree with {Expat(args.reference).version}"
    )


if __name__ == "__main__":
    main()
