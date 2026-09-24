"""Public API tests against the installed Python extension."""

import gc
import unittest
import weakref

import xeme


def parse(data, **options):
    parser = xeme.Parser(**options)
    parser.feed(data, final=True)
    return list(parser.read_events())


def feed_chunks(parser, data, width, events):
    for offset in range(0, len(data), width):
        parser.feed(
            data[offset : offset + width], final=offset + width >= len(data)
        )
        events.extend(parser.read_events())


def payloads(events):
    """Compare document content without depending on text event boundaries."""
    result = []
    for event in events:
        if event.kind == "text" and result and result[-1][0] == "text":
            result[-1] = ("text", result[-1][1] + event.data)
        else:
            result.append((event.kind, event.data))
    return result


class ParserTests(unittest.TestCase):
    def assert_parse_error(self, data, kind, **options):
        with self.assertRaises(xeme.ParseError) as raised:
            parse(data, **options)
        self.assertEqual(raised.exception.kind, kind)

    def test_elements_attributes_and_original_byte_positions(self):
        events = parse(b'<root a="1">\nchild</root>')
        self.assertEqual(
            payloads(events),
            [("start", ("root", {"a": "1"})), ("text", "\nchild"), ("end", "root")],
        )
        start, end = events[0], events[-1]
        self.assertIsInstance(start, xeme.Event)
        self.assertIsInstance(start.position, xeme.Position)
        self.assertEqual(
            (
                start.position.line,
                start.position.column,
                start.position.byte_index,
                start.position.byte_count,
            ),
            (1, 0, 0, 12),
        )
        self.assertEqual(
            (
                end.position.line,
                end.position.column,
                end.position.byte_index,
                end.position.byte_count,
            ),
            (2, 5, 18, 7),
        )

    def test_declaration_comments_instructions_and_cdata(self):
        events = parse(
            b'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            b"<!--before--><?target instruction?><r><![CDATA[<raw>&]]></r>"
        )
        self.assertEqual(
            payloads(events),
            [
                ("xml_decl", ("1.0", "UTF-8", True)),
                ("comment", "before"),
                ("pi", ("target", "instruction")),
                ("start", ("r", {})),
                ("start_cdata", None),
                ("text", "<raw>&"),
                ("end_cdata", None),
                ("end", "r"),
            ],
        )
        self.assertEqual(
            parse(b'<?xml version="1.0"?><r/>')[0].data,
            ("1.0", None, None),
        )

    def test_xml_version_grammar(self):
        for version in ["1.0", "1.2", "1.01", "banana", "2.0", "1", "1."]:
            data = f"<?xml version='{version}'?><r/>".encode()
            for width in [1, 7, len(data)]:
                with self.subTest(version=version, width=width):
                    parser = xeme.Parser()
                    events = []

                    if version in ["1.0", "1.2", "1.01"]:
                        feed_chunks(parser, data, width, events)
                        self.assertEqual(events[0].data, (version, None, None))
                    else:
                        with self.assertRaises(xeme.ParseError) as raised:
                            feed_chunks(parser, data, width, events)
                        self.assertEqual(raised.exception.kind, "XmlDeclaration")
                        self.assertEqual(events, [])

    def test_utf8_all_two_chunk_boundaries(self):
        document = '<r a="é">€\r\n𐐀&amp;tail</r>'.encode()
        expected = [("start", ("r", {"a": "é"})), ("text", "€\n𐐀&tail"), ("end", "r")]
        for boundary in range(len(document) + 1):
            with self.subTest(boundary=boundary):
                parser = xeme.Parser()
                parser.feed(document[:boundary])
                events = list(parser.read_events())
                parser.feed(document[boundary:], final=True)
                events.extend(parser.read_events())
                self.assertEqual(payloads(events), expected)

    def test_utf16_bytewise_with_bom_and_surrogate_pairs(self):
        document = '<?xml version="1.0" encoding="UTF-16"?><r>é𐐀</r>'
        for encoding, bom in [("utf-16-le", b"\xff\xfe"), ("utf-16-be", b"\xfe\xff")]:
            with self.subTest(encoding=encoding):
                data = bom + document.encode(encoding)
                parser = xeme.Parser()
                events = []
                for index, byte in enumerate(data):
                    parser.feed(bytes([byte]), final=index == len(data) - 1)
                    events.extend(parser.read_events())
                self.assertEqual(
                    payloads(events),
                    [
                        ("xml_decl", ("1.0", "UTF-16", None)),
                        ("start", ("r", {})),
                        ("text", "é𐐀"),
                        ("end", "r"),
                    ],
                )

    def test_utf16_requires_encoding_evidence(self):
        for encoding, bom in [("utf-16-le", b"\xff\xfe"), ("utf-16-be", b"\xfe\xff")]:
            name = encoding.upper().replace("-LE", "LE").replace("-BE", "BE")
            for evidence in ["none", "bom", "declaration", "override"]:
                document = "<r/>"
                if evidence == "declaration":
                    document = f"<?xml version='1.0' encoding='{name}'?>" + document
                data = document.encode(encoding)
                if evidence == "bom":
                    data = bom + data
                for width in [1, 7, len(data)]:
                    with self.subTest(encoding=encoding, evidence=evidence, width=width):
                        parser = xeme.Parser(encoding=name if evidence == "override" else None)
                        events = []

                        if evidence == "none":
                            with self.assertRaises(xeme.ParseError) as raised:
                                feed_chunks(parser, data, width, events)
                            self.assertEqual(raised.exception.kind, "IncorrectEncoding")
                            self.assertEqual(events, [])
                        else:
                            feed_chunks(parser, data, width, events)
                            self.assertEqual(payloads(events)[-2:], [("start", ("r", {})), ("end", "r")])

    def test_encoding_override(self):
        self.assertEqual(
            payloads(parse(b"<r>caf\xe9</r>", encoding="ISO-8859-1")),
            [("start", ("r", {})), ("text", "café"), ("end", "r")],
        )

    def test_namespaces_and_triplets(self):
        document = b'<r xmlns="urn:default" xmlns:p="urn:p" p:a="1"><p:c/></r>'
        for triplets in [False, True]:
            with self.subTest(triplets=triplets):
                suffix = "|p" if triplets else ""
                events = parse(
                    document, namespace_separator="|", namespace_triplets=triplets
                )
                self.assertEqual(
                    payloads(events),
                    [
                        ("start_ns", (None, "urn:default")),
                        ("start_ns", ("p", "urn:p")),
                        ("start", ("urn:default|r", {f"urn:p|a{suffix}": "1"})),
                        ("start", (f"urn:p|c{suffix}", {})),
                        ("end", f"urn:p|c{suffix}"),
                        ("end", "urn:default|r"),
                        ("end_ns", "p"),
                        ("end_ns", None),
                    ],
                )

    def test_namespace_processing_is_optional_and_accepts_unicode_separator(self):
        document = b'<p:r xmlns:p="urn:p"/>'
        self.assertEqual(
            payloads(parse(document)),
            [("start", ("p:r", {"xmlns:p": "urn:p"})), ("end", "p:r")],
        )
        for separator in ["λ", "\0"]:
            with self.subTest(separator=separator):
                events = parse(document, namespace_separator=separator)
                start = next(event for event in events if event.kind == "start")
                joiner = "" if separator == "\0" else separator
                self.assertEqual(start.data, (f"urn:p{joiner}r", {}))

    def test_invalid_parser_options_and_input_types(self):
        for separator in ["", "ab"]:
            with self.subTest(separator=separator), self.assertRaises(ValueError):
                xeme.Parser(namespace_separator=separator)
        with self.assertRaises(ValueError):
            xeme.Parser(namespace_triplets=True)
        with self.assertRaises(TypeError):
            xeme.Parser("UTF-8")
        for data in ["<r/>", bytearray(b"<r/>"), memoryview(b"<r/>"), None]:
            with self.subTest(data=type(data).__name__), self.assertRaises(TypeError):
                xeme.Parser().feed(data)

    def test_events_must_be_drained_before_feeding(self):
        parser = xeme.Parser()
        parser.feed(b"<r><c/>")
        with self.assertRaises(RuntimeError):
            parser.feed(b"</r>", final=True)
        iterator = parser.read_events()
        self.assertIs(iter(iterator), iterator)
        self.assertEqual(next(iterator).data, ("r", {}))
        with self.assertRaises(RuntimeError):
            parser.feed(b"</r>", final=True)
        self.assertEqual(payloads(iterator), [("start", ("c", {})), ("end", "c")])
        parser.feed(b"</r>", final=True)
        self.assertEqual(payloads(parser.read_events()), [("end", "r")])

    def test_empty_nonfinal_input_and_completed_parser(self):
        parser = xeme.Parser()
        self.assertEqual(list(parser.read_events()), [])
        parser.feed(b"")
        self.assertEqual(list(parser.read_events()), [])
        parser.feed(b"<r/>", final=True)
        self.assertEqual(
            payloads(parser.read_events()), [("start", ("r", {})), ("end", "r")]
        )
        self.assertEqual(list(parser.read_events()), [])
        with self.assertRaises(ValueError):
            parser.feed(b"")

    def test_events_and_positions_survive_parser_progress(self):
        parser = xeme.Parser()
        parser.feed(b'<r><c a="first"/>')
        retained = list(parser.read_events())
        first_child = retained[1]
        position = first_child.position
        parser.feed(b'<c a="second"/></r>', final=True)
        following = list(parser.read_events())
        second_child = following[0]
        del parser
        self.assertEqual(first_child.data, ("c", {"a": "first"}))
        self.assertEqual(second_child.data, ("c", {"a": "second"}))
        first_child.data[1]["a"] = "changed in Python"
        self.assertEqual(second_child.data[1], {"a": "second"})
        self.assertEqual(
            (position.line, position.column, position.byte_index), (1, 3, 3)
        )
        for obj, name, value in [
            (first_child, "kind", "other"),
            (position, "line", 99),
        ]:
            with self.subTest(attribute=name), self.assertRaises(AttributeError):
                setattr(obj, name, value)

    def test_python_payload_cycles_are_collected(self):
        class Marker:
            pass

        event = parse(b"<r/>")[0]
        marker = Marker()
        marker.event = event
        event.data[1]["cycle"] = marker
        reference = weakref.ref(marker)
        del event, marker
        gc.collect()
        self.assertIsNone(reference())

    def test_malformed_truncated_and_empty_documents(self):
        for data, kind in [
            (b"", "NoElements"),
            (b"<r>", "NoElements"),
            (b"<r", "UnclosedToken"),
            (b"<r></wrong>", "TagMismatch"),
            (b'<r a="1" a="2"/>', "DuplicateAttribute"),
            (b"<r>&missing;</r>", "UndefinedEntity"),
            (b"<r/>extra", "JunkAfterDocumentElement"),
        ]:
            with self.subTest(data=data):
                self.assert_parse_error(data, kind)

    def test_error_details_and_sticky_failure(self):
        parser = xeme.Parser()
        with self.assertRaises(xeme.ParseError) as raised:
            parser.feed(b"<root>\n  </other>", final=True)
            list(parser.read_events())
        error = raised.exception
        expected = ("TagMismatch", 2, 4, 11)
        self.assertEqual(
            (error.kind, error.line, error.column, error.byte_index), expected
        )
        self.assertIn("line 2", str(error))
        self.assertIn("column 4", str(error))
        for action in [
            lambda: list(parser.read_events()),
            lambda: parser.feed(b"<ok/>"),
        ]:
            with self.assertRaises(xeme.ParseError) as repeated:
                action()
            error = repeated.exception
            self.assertEqual(
                (error.kind, error.line, error.column, error.byte_index), expected
            )

    def test_internal_entities_and_default_attributes(self):
        events = parse(
            b'<!DOCTYPE r [<!ELEMENT r (#PCDATA)><!ENTITY word "hello">'
            b'<!ATTLIST r greeting CDATA "&word;" explicit CDATA "default">]>'
            b'<r explicit="given">&word; &amp; &#x1F600;</r>'
        )
        self.assertEqual(
            payloads(events),
            [
                ("start_doctype", ("r", None, None, True)),
                ("end_doctype", None),
                ("start", ("r", {"greeting": "hello", "explicit": "given"})),
                ("text", "hello & 😀"),
                ("end", "r"),
            ],
        )

    def test_external_resources_are_rejected(self):
        for document in [
            b'<!DOCTYPE r SYSTEM "file:///does-not-exist.dtd"><r/>',
            b'<!DOCTYPE r PUBLIC "example" "https://example.invalid/test.dtd"><r/>',
            b'<!DOCTYPE r SYSTEM "https://example.invalid/test.dtd" [<!ENTITY x "v">]><r/>',
            b'<!DOCTYPE r [<!ENTITY x SYSTEM "file:///does-not-exist">]><r>&x;</r>',
        ]:
            with self.subTest(document=document):
                self.assert_parse_error(document, "ExternalEntityHandling")

    def test_internal_parameter_entities_supply_default_attributes(self):
        events = parse(
            b"<!DOCTYPE r [<!ENTITY % defs '<!ATTLIST r a CDATA \"v\">'>%defs;]><r/>"
        )
        self.assertEqual(
            payloads(events),
            [
                ("start_doctype", ("r", None, None, True)),
                ("end_doctype", None),
                ("start", ("r", {"a": "v"})),
                ("end", "r"),
            ],
        )

    def test_external_parameter_entities_are_rejected(self):
        self.assert_parse_error(
            b"<!DOCTYPE r [<!ENTITY % ext SYSTEM 'file:///missing'>%ext;]><r/>",
            "ExternalEntityHandling",
        )

    def test_limits_defaults_customization_and_validation(self):
        defaults = {
            "max_depth": 256,
            "max_token_bytes": 16 * 1024 * 1024,
            "max_total_bytes": 256 * 1024 * 1024,
            "max_entity_expansion_bytes": 8 * 1024 * 1024,
            "max_entity_depth": 32,
            "max_attributes": 10_000,
            "max_entities": 10_000,
        }
        limits = xeme.Limits()
        for name, value in defaults.items():
            with self.subTest(name=name):
                self.assertEqual(getattr(limits, name), value)
                self.assertEqual(getattr(xeme.Limits(**{name: 0}), name), 0)
        with self.assertRaises(AttributeError):
            limits.max_depth = 100
        for invalid in [-1, 1 << 128]:
            with self.assertRaises(OverflowError):
                xeme.Limits(max_depth=invalid)
        with self.assertRaises(TypeError):
            xeme.Limits(max_depth=1.5)
        with self.assertRaises(TypeError):
            xeme.Limits(2)

    def test_input_limit_is_cumulative_across_feeds(self):
        parser = xeme.Parser(limits=xeme.Limits(max_total_bytes=6))
        parser.feed(b"<r>")
        list(parser.read_events())
        with self.assertRaises(xeme.ParseError) as raised:
            parser.feed(b"</r>", final=True)
            list(parser.read_events())
        self.assertEqual(raised.exception.kind, "LimitExceeded")

    def test_depth_attribute_token_and_entity_limits(self):
        cases = [
            (b"<r><c/></r>", {"max_depth": 1}),
            (b'<r a="1" b="2"/>', {"max_attributes": 1}),
            (b'<root attribute="long"/>', {"max_token_bytes": 8}),
            (b'<!DOCTYPE r [<!ENTITY a "a"><!ENTITY b "b">]><r/>', {"max_entities": 1}),
            (
                b'<!DOCTYPE r [<!ENTITY a "123456789">]><r>&a;</r>',
                {"max_entity_expansion_bytes": 4},
            ),
            (
                b'<!DOCTYPE r [<!ENTITY a "&b;"><!ENTITY b "b">]><r>&a;</r>',
                {"max_entity_depth": 1},
            ),
        ]
        for data, options in cases:
            with self.subTest(options=options):
                self.assert_parse_error(
                    data, "LimitExceeded", limits=xeme.Limits(**options)
                )
        self.assertEqual(
            payloads(parse(b"<r><c/></r>", limits=xeme.Limits(max_depth=2))),
            [("start", ("r", {})), ("start", ("c", {})), ("end", "c"), ("end", "r")],
        )

    def test_unknown_encoding_is_reported(self):
        self.assert_parse_error(
            b'<?xml version="1.0" encoding="not-an-encoding"?><r/>', "UnknownEncoding"
        )
        with self.assertRaises(xeme.ParseError) as raised:
            parse(b"<r/>", encoding="not-an-encoding")
        self.assertEqual(raised.exception.kind, "UnknownEncoding")


if __name__ == "__main__":
    unittest.main()
