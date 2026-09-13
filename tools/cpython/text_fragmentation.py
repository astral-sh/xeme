"""Check the content contracts behind two Expat-specific CPython assertions.

Run with the same extension-loading environment as the unchanged upstream suite.
Only adjacent character callbacks are combined; element and CDATA boundaries,
every whitespace character, and callback-controlled buffering remain observable.
"""

from __future__ import annotations

import pyexpat as expat
import sys
import unittest
from io import StringIO
from pathlib import Path
from xml.sax import make_parser
from xml.sax.handler import ContentHandler, LexicalHandler, property_lexical_handler
from xml.sax.xmlreader import InputSource


class TextFragmentationTests(unittest.TestCase):
    def test_callback_controlled_buffering(self) -> None:
        parser = expat.ParserCreate()
        parser.buffer_text = True
        events: list[tuple[str, str]] = []
        buffering: list[bool] = []

        def text(value: str) -> None:
            if events and events[-1][0] == "text":
                events[-1] = ("text", events[-1][1] + value)
            else:
                events.append(("text", value))

        def start(name: str, attributes: dict[str, str]) -> None:
            events.append(("start", name))
            if "buffer-text" in attributes:
                parser.buffer_text = attributes["buffer-text"] == "yes"
            buffering.append(parser.buffer_text)

        parser.CharacterDataHandler = text
        parser.StartElementHandler = start
        parser.Parse(
            b"<a>1<b buffer-text='no'/>2\n3<c buffer-text='yes'/>4\n5</a>", True
        )
        self.assertEqual(buffering, [True, False, True])
        self.assertEqual(
            events,
            [
                ("start", "a"),
                ("text", "1"),
                ("start", "b"),
                ("text", "2\n3"),
                ("start", "c"),
                ("text", "4\n5"),
            ],
        )

    def test_sax_text_and_cdata_order(self) -> None:
        events: list[tuple[str, str]] = []

        class Handler(ContentHandler, LexicalHandler):
            def startElement(self, name: str, attrs: object) -> None:
                events.append(("start", name))

            def endElement(self, name: str) -> None:
                events.append(("end", name))

            def startCDATA(self) -> None:
                events.append(("cdata", "start"))

            def endCDATA(self) -> None:
                events.append(("cdata", "end"))

            def characters(self, content: str) -> None:
                if events and events[-1][0] == "text":
                    events[-1] = ("text", events[-1][1] + content)
                else:
                    events.append(("text", content))

        handler = Handler()
        parser = make_parser()
        parser.setContentHandler(handler)
        parser.setProperty(property_lexical_handler, handler)
        source = InputSource()
        source.setCharacterStream(
            StringIO(
                "<root_doc>\n<some_pcdata>\nParseable character data\n</some_pcdata>\n"
                "<some_cdata>\n<![CDATA[<> &% - assorted other XML junk.]]>\n"
                "</some_cdata>\n</root_doc>\n"
            )
        )
        parser.parse(source)
        self.assertEqual(
            events,
            [
                ("start", "root_doc"),
                ("text", "\n"),
                ("start", "some_pcdata"),
                ("text", "\nParseable character data\n"),
                ("end", "some_pcdata"),
                ("text", "\n"),
                ("start", "some_cdata"),
                ("text", "\n"),
                ("cdata", "start"),
                ("text", "<> &% - assorted other XML junk."),
                ("cdata", "end"),
                ("text", "\n"),
                ("end", "some_cdata"),
                ("text", "\n"),
                ("end", "root_doc"),
            ],
        )


if __name__ == "__main__":
    expected_directory = Path(sys.argv.pop(1)).resolve()
    native_origin = Path(expat.__file__).parent
    if native_origin != expected_directory:
        raise RuntimeError(f"unexpected pyexpat origin: {native_origin}")
    unittest.main(verbosity=2)
