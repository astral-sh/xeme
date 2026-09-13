#!/usr/bin/env python3
"""Generate bounded declaration-composition and token-scanning workloads."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--external-grammar", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = {}
    for size in (64, 256, 1024):
        cases = {
            "closing-pe": "<!ENTITY % end '>'>"
            + "".join(f"<!ELEMENT e{i} EMPTY %end;" for i in range(size)),
            "header-pe": "<!ENTITY % begin 'INCLUDE['>"
            + "".join(f"<![%begin;<!ELEMENT e{i} EMPTY>]]>" for i in range(size)),
            "empty-internal": "<!ENTITY % e ''><!ATTLIST r a CDATA "
            + "%e; " * size
            + '"v">',
            "literal": "<!ENTITY % t 'CDATA'><!ATTLIST r a %t; '"
            + "x" * (size * 64)
            + "'>",
        }
        if args.external_grammar:
            # Include the outer DTD child while remaining below the family cap.
            cases["empty-external"] = (
                "<!ENTITY % e SYSTEM 'empty'><!ATTLIST r a CDATA "
                + "%e; " * (size // 4)
                + '"v">'
            )
        for kind, text in cases.items():
            data = text.encode()
            path = args.output / f"{kind}-{size:04}.dtd"
            path.write_bytes(data)
            manifest[path.name] = {
                "kind": kind,
                "size": size,
                "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
                "expected_declarations": size + 1
                if kind in ("closing-pe", "header-pe")
                else 2,
                "expected_external_calls": size // 4 + 1
                if kind == "empty-external"
                else 1,
            }
    (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
