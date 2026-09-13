"""Small, explicit ctypes binding used only by the independent C ABI probes."""

from __future__ import annotations

import ctypes as c
import os
from typing import Any

P = c.c_void_p
S = c.c_char_p
I = c.c_int
START = c.CFUNCTYPE(None, P, S, c.POINTER(S))
END = c.CFUNCTYPE(None, P, S)
TEXT = c.CFUNCTYPE(None, P, P, I)
PI = c.CFUNCTYPE(None, P, S, S)
VOID = c.CFUNCTYPE(None, P)
NS_START = c.CFUNCTYPE(None, P, S, S)
DECL = c.CFUNCTYPE(None, P, S, S, I)


def load_library(library: str) -> c.CDLL:
    """Keep a probe library's internal calls bound to its own implementation.

    Ubuntu's Python links a system Expat into the process before our probes run.
    A normal dlopen can bind another Expat's internal calls to that first copy,
    even when XML_Parse itself has the requested origin. On ELF platforms that
    support it, deep binding prevents this mixture of private parser layouts.
    These ctypes probes load ordinary builds; sanitizer builds use their own
    harnesses because ASan does not support RTLD_DEEPBIND.
    """
    return c.CDLL(library, mode=c.DEFAULT_MODE | getattr(os, "RTLD_DEEPBIND", 0))


def decode(value: bytes | None) -> str | None:
    return (
        value.decode("utf-8", errors="backslashreplace") if value is not None else None
    )


class Expat:
    def __init__(self, library: str):
        self.lib = load_library(library)
        signatures = {
            "XML_ParserCreate": (P, [S]),
            "XML_ParserCreateNS": (P, [S, c.c_char]),
            "XML_ParserFree": (None, [P]),
            "XML_Parse": (I, [P, P, I, I]),
            "XML_SetUserData": (None, [P, P]),
            "XML_SetElementHandler": (None, [P, START, END]),
            "XML_SetCharacterDataHandler": (None, [P, TEXT]),
            "XML_SetCommentHandler": (None, [P, END]),
            "XML_SetProcessingInstructionHandler": (None, [P, PI]),
            "XML_SetCdataSectionHandler": (None, [P, VOID, VOID]),
            "XML_SetNamespaceDeclHandler": (None, [P, NS_START, END]),
            "XML_SetXmlDeclHandler": (None, [P, DECL]),
            "XML_GetErrorCode": (I, [P]),
            "XML_GetCurrentLineNumber": (c.c_ulong, [P]),
            "XML_GetCurrentColumnNumber": (c.c_ulong, [P]),
            "XML_GetCurrentByteIndex": (c.c_long, [P]),
            "XML_ExpatVersion": (S, []),
        }
        for name, (result, arguments) in signatures.items():
            function = getattr(self.lib, name)
            function.restype = result
            function.argtypes = arguments
        self.version = decode(self.lib.XML_ExpatVersion())

    def parse(
        self,
        data: bytes,
        chunk_size: int,
        namespaces: bool = False,
        *,
        capture: bool = True,
    ) -> dict[str, Any]:
        """Capture callbacks exactly, then separately coalesce adjacent text callbacks."""
        events: list[list[Any]] = []
        callback_errors: list[str] = []
        parser = (
            self.lib.XML_ParserCreateNS(None, b"|")
            if namespaces
            else self.lib.XML_ParserCreate(None)
        )
        if not parser:
            raise MemoryError("XML_ParserCreate returned NULL")

        def start(_: int, name: bytes, attributes: Any) -> None:
            try:
                pairs = []
                index = 0
                while attributes[index] is not None:
                    pairs.append(
                        [decode(attributes[index]), decode(attributes[index + 1])]
                    )
                    index += 2
                events.append(["start", decode(name), pairs])
            except BaseException as error:  # noqa: BLE001 — ctypes callbacks must not unwind
                callback_errors.append(repr(error))

        callbacks = (
            START(start),
            END(lambda _, name: events.append(["end", decode(name)])),
            TEXT(
                lambda _, value, length: events.append(
                    ["text", decode(c.string_at(value, length))]
                )
            ),
            END(lambda _, value: events.append(["comment", decode(value)])),
            PI(
                lambda _, target, value: events.append(
                    ["pi", decode(target), decode(value)]
                )
            ),
            VOID(lambda _: events.append(["cdata-start"])),
            VOID(lambda _: events.append(["cdata-end"])),
            NS_START(
                lambda _, prefix, uri: events.append(
                    ["namespace-start", decode(prefix), decode(uri)]
                )
            ),
            END(lambda _, prefix: events.append(["namespace-end", decode(prefix)])),
            DECL(
                lambda _, version, encoding, standalone: events.append(
                    ["xml-decl", decode(version), decode(encoding), standalone]
                )
            ),
        )
        try:
            if capture:
                self.lib.XML_SetElementHandler(parser, callbacks[0], callbacks[1])
                self.lib.XML_SetCharacterDataHandler(parser, callbacks[2])
                self.lib.XML_SetCommentHandler(parser, callbacks[3])
                self.lib.XML_SetProcessingInstructionHandler(parser, callbacks[4])
                self.lib.XML_SetCdataSectionHandler(parser, callbacks[5], callbacks[6])
                self.lib.XML_SetNamespaceDeclHandler(parser, callbacks[7], callbacks[8])
                self.lib.XML_SetXmlDeclHandler(parser, callbacks[9])
            chunks = [
                data[offset : offset + chunk_size]
                for offset in range(0, len(data), chunk_size)
            ] or [b""]
            status = 1
            for index, chunk in enumerate(chunks):
                status = self.lib.XML_Parse(
                    parser, chunk, len(chunk), int(index == len(chunks) - 1)
                )
                if status != 1:
                    break
            normalized: list[list[Any]] = []
            text_parts = []
            for event in events:
                if event[0] == "text":
                    text_parts.append(event[1])
                else:
                    if text_parts:
                        normalized.append(["text", "".join(text_parts)])
                        text_parts.clear()
                    normalized.append(event.copy())
            if text_parts:
                normalized.append(["text", "".join(text_parts)])
            return {
                "status": status,
                "error": self.lib.XML_GetErrorCode(parser),
                "position": [
                    self.lib.XML_GetCurrentLineNumber(parser),
                    self.lib.XML_GetCurrentColumnNumber(parser),
                    self.lib.XML_GetCurrentByteIndex(parser),
                ],
                "events": events,
                "normalized_events": normalized,
                "callback_errors": callback_errors,
            }
        finally:
            self.lib.XML_ParserFree(parser)
