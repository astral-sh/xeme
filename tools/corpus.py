"""Deterministic XML coverage and mutation corpus; generated inputs are retained in reports."""

from __future__ import annotations

import random
from dataclasses import dataclass


@dataclass(frozen=True)
class Case:
    name: str
    data: bytes
    namespaces: bool = False
    category: str = "xml"


def cases(seed: int = 20260910, generated: int = 100) -> list[Case]:
    documents = {
        "empty-element": b"<root/>",
        "nested": b'<root a="1" b="two"><child>text &amp; more</child><empty/></root>',
        "unicode": '<根 café="é">Καλημέρα 😀</根>'.encode(),
        "declaration": b'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><r/>',
        "misc": b"<?before yes?><!--before--><r><?target value?><![CDATA[<hello>&x]]><!--a--></r><!--after-->",
        "line-endings": b'<r a="one\r\ntwo\tthree\rfour">a\r\nb\rc\n</r>',
        # Reduced CPython BufferTextTest and SAX CDATAHandlerTest inputs.
        "python-buffer-text-lines": b"<a>1<b buffer-text='no'/>2\n3<c buffer-text='yes'/>4\n5</a>",
        "python-sax-cdata-lines": b"<r>\nParseable character data\n<![CDATA[<> &% - assorted other XML junk.]]>\n</r>",
        "character-references": b'<r a="&#x9;&#10;&#13;">&#65;&#x1F600;&lt;&gt;&apos;&quot;&amp;</r>',
        "utf8-bom": b"\xef\xbb\xbf<r>ok</r>",
        "utf16-le": b"\xff\xfe" + '<r a="é">😀</r>'.encode("utf-16-le"),
        "utf16-be": b"\xfe\xff" + '<r a="é">😀</r>'.encode("utf-16-be"),
        "latin1": b'<?xml version="1.0" encoding="ISO-8859-1"?><r>caf\xe9</r>',
        "internal-entity": b'<!DOCTYPE r [<!ENTITY word "hello">]><r>&word; &word;</r>',
        "nested-entities": b'<!DOCTYPE r [<!ENTITY a "A"><!ENTITY b "&a;&a;">]><r>&b;</r>',
        "default-attribute": b'<!DOCTYPE r [<!ATTLIST r color CDATA "blue">]><r/>',
        "tokenized-attribute": b'<!DOCTYPE r [<!ATTLIST r list NMTOKENS #IMPLIED>]><r list=" a  b\tc "/>',
        "external-doctype": b'<!DOCTYPE r SYSTEM "file:///definitely-not-an-xeme-file"><r/>',
        "empty": b"",
        "whitespace": b" \n\t",
        "missing-close": b"<r>",
        "mismatched-close": b"<r><a></r>",
        "multiple-roots": b"<r/><s/>",
        "duplicate-attribute": b'<r a="1" a="2"/>',
        "unquoted-attribute": b"<r a=1/>",
        "missing-space": b'<r a="1"b="2"/>',
        "bad-name": b"<1r/>",
        "raw-less-than": b'<r a="<"/>',
        "raw-ampersand": b"<r>&</r>",
        "unknown-entity": b"<r>&unknown;</r>",
        "zero-reference": b"<r>&#0;</r>",
        "surrogate-reference": b"<r>&#xD800;</r>",
        "overflow-reference": b"<r>&#99999999999999999999999999999999;</r>",
        "out-of-range-reference": b"<r>&#x110000;</r>",
        "embedded-null": b"<r>a\x00b</r>",
        "bad-utf8": b"<r>\xc0\x80</r>",
        "surrogate-utf8": b"<r>\xed\xa0\x80</r>",
        "truncated-utf8": b"<r>\xf0\x9f",
        "unclosed-comment": b"<r><!-- x</r>",
        "double-hyphen-comment": b"<r><!-- a--b --></r>",
        "trailing-comment-hyphen": b"<r><!-- a---></r>",
        "unclosed-cdata": b"<r><![CDATA[x</r>",
        "cdata-end-in-text": b"<r>]]></r>",
        "misplaced-declaration": b' <r><?xml version="1.0"?></r>',
        "bad-declaration": b'<?xml encoding="UTF-8"?><r/>',
        "recursive-entity": b'<!DOCTYPE r [<!ENTITY x "&x;">]><r>&x;</r>',
        "mutual-entity": b'<!DOCTYPE r [<!ENTITY x "&y;"><!ENTITY y "&x;">]><r>&x;</r>',
    }
    result = [Case(name, data) for name, data in documents.items()]
    namespaces = {
        "default-namespace": b'<r xmlns="urn:one"><a x="1"/><b xmlns=""/></r>',
        "prefixed-namespace": b'<p:r xmlns:p="urn:p" xmlns:q="urn:q" q:a="v"><q:c/></p:r>',
        "namespace-shadow": b'<r xmlns:p="urn:one"><p:a xmlns:p="urn:two"/><p:b/></r>',
        "unbound-prefix": b"<p:r/>",
        "duplicate-expanded-attribute": b'<r xmlns:a="urn:x" xmlns:b="urn:x" a:x="1" b:x="2"/>',
        "reserved-xml": b'<r xmlns:xml="urn:bad"/>',
        "reserved-xmlns": b'<r xmlns:xmlns="urn:bad"/>',
        "undeclare-prefix": b'<r xmlns:p=""/>',
    }
    result.extend(
        Case(name, data, True, "namespaces") for name, data in namespaces.items()
    )
    randomizer = random.Random(seed)
    for index in range(generated):
        width = randomizer.randrange(1, 12)
        nodes = [
            f'<n id="{i}" flag="{randomizer.choice(["a", "b", "c"])}">{randomizer.choice(["text", "&amp;", "&#x41;", "é", ""])}</n>'
            for i in range(width)
        ]
        data = ("<root>" + "".join(nodes) + "</root>").encode()
        result.append(Case(f"generated-{index}", data, category="generated"))
        mutation = bytearray(data)
        operation = index % 4
        position = randomizer.randrange(len(mutation))
        if operation == 0:
            del mutation[position:]
        elif operation == 1:
            mutation[position] = randomizer.choice(
                [0, 0xFF, ord("<"), ord("&"), ord('"')]
            )
        elif operation == 2:
            mutation[position:position] = b"]]>"
        else:
            mutation[position : position + 1] = b""
        result.append(Case(f"mutation-{index}", bytes(mutation), category="mutation"))
    return result


def workloads(size: int = 10000) -> dict[str, bytes]:
    return {
        "elements": b"<root>" + b'<item id="123" enabled="true"/>' * size + b"</root>",
        "text": b"<root>"
        + b"the quick brown fox jumps over the lazy dog\n" * size
        + b"</root>",
        "entities": b"<root>"
        + b"alpha &amp; beta &#65; &lt; gamma\n" * size
        + b"</root>",
        "namespaces": b'<root xmlns:p="urn:xeme">'
        + b'<p:item p:id="123">value</p:item>' * size
        + b"</root>",
    }
