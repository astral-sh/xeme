"""Deterministic generated XML; this module never reads project fixtures."""

import hashlib
import json
import random
from pathlib import Path

SEED = 0x4F52494F4C45


def documents() -> dict[str, tuple[bytes, str]]:
    random_source = random.Random(SEED)
    docs = {
        "small": "<root>"
        + "".join(
            f'<item id="{i}" kind="k{i % 7}">value {i}</item>' for i in range(1500)
        )
        + "</root>",
        "attributes": "<root>"
        + "".join(
            "<item "
            + " ".join(
                f'a{j}="token-{random_source.randrange(10000)} and &#xE9; &amp; end"'
                for j in range(24)
            )
            + "/>"
            for _ in range(300)
        )
        + "</root>",
        "text": "<root>"
        + "".join(
            "<section>"
            + ("abcdefghijklmnopqrstuvwxyz é中😀 " * 120)
            + "\r\n"
            + ("long-line-" * 1000)
            + "</section>"
            for _ in range(16)
        )
        + "</root>",
        "mixed": "<root>"
        + "".join(
            f'<!-- note {i} --><?worker step="{i}"?><entry>one &lt; two &amp; three <![CDATA[raw <x> & data]]>\r\n<empty/></entry>'
            for i in range(800)
        )
        + "</root>",
        "namespaces": '<root xmlns="urn:generated:root" xmlns:p="urn:generated:p">'
        + "".join(
            f'<p:item p:id="{i}" xmlns:q="urn:q:{i % 17}"><q:child xml:lang="en">μ{i}</q:child></p:item>'
            for i in range(1000)
        )
        + "</root>",
        "deep": "<root>"
        + "".join(
            '<node a="v">' * 64 + f"<leaf>{i}</leaf>" + "</node>" * 64
            for i in range(40)
        )
        + "</root>",
    }
    attributes = " ".join(
        f"a{i} "
        + (
            "(x|y|z) 'x'"
            if i % 3 == 0
            else "CDATA 'default &e;'"
            if i % 3 == 1
            else "IDREF #IMPLIED"
        )
        for i in range(128)
    )
    docs["dtd"] = (
        '<!DOCTYPE root [<!ENTITY e "value"><!ENTITY joined "&e; &e;"><!ENTITY % extra "<!ATTLIST item extra NMTOKENS \' one   two \' >">%extra;<!ELEMENT root (item*)><!ELEMENT item (#PCDATA)><!ATTLIST item '
        + attributes
        + ">]><root>"
        + "".join(f'<item a1="explicit{i}">&joined;</item>' for i in range(300))
        + "</root>"
    )
    docs["line_endings"] = (
        "<root>"
        + "".join(
            f'<item a="a\r\nb\rc\nd\te">L{i}\r\nX\rY\nZ</item>' for i in range(1200)
        )
        + "</root>"
    )
    encoded = {name: (text.encode(), "UTF-8") for name, text in docs.items()}
    for name, encoding, bom in [
        ("mixed", "utf-16-le", b"\xff\xfe"),
        ("namespaces", "utf-16-be", b"\xfe\xff"),
        ("line_endings", "utf-16-le", b"\xff\xfe"),
    ]:
        encoded[f"{name}-{encoding}"] = (bom + docs[name].encode(encoding), encoding)
    latin = (
        '<?xml version="1.0" encoding="ISO-8859-1"?><root>'
        + "".join(f'<item id="{i}">café à bientôt</item>' for i in range(900))
        + "</root>"
    )
    encoded["latin1"] = (latin.encode("latin1"), "ISO-8859-1")
    return encoded


def generate(directory: Path) -> dict:
    """Write the fixed corpus and its manifest to a new, empty directory."""
    directory.mkdir()
    rows = []
    for name, (data, encoding) in documents().items():
        filename = f"{name}.xml"
        (directory / filename).write_bytes(data)
        rows.append(
            {
                "name": name,
                "file": filename,
                "encoding": encoding,
                "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
                "chunks": [1 if name == "latin1" else 17, 4096, 65536],
                "namespaces": [False, True],
            }
        )
    manifest = {
        "seed": SEED,
        "iterations": 2,
        "handlers": ["minimal", "full"],
        "method": "Generated XML only; Python ctypes callbacks. No external files or network input.",
        "rows": rows,
    }
    (directory / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest
