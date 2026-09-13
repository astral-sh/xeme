#!/usr/bin/env python3
"""Generate bounded DTD attribute-lookup workloads and retain their hashes."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def document(attributes: int, rows: int) -> bytes:
    names = [f"a{index:04}" for index in range(attributes)]
    declarations = " ".join(
        f"{name} {'ID' if index + 1 == attributes else 'NMTOKENS'} #IMPLIED"
        for index, name in enumerate(names)
    )
    nodes = []
    for row in range(rows):
        values = " ".join(
            f'{name}="{f"row{row}" if index + 1 == attributes else "a  b"}"'
            for index, name in enumerate(names)
        )
        nodes.append(f"<item {values}/>")
    return (
        "<!DOCTYPE root [<!ELEMENT root (item*)><!ELEMENT item EMPTY>"
        f"<!ATTLIST item {declarations}>]><root>{''.join(nodes)}</root>"
    ).encode()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = {}
    for attributes in [128, 256, 512, 1024]:
        for rows in [0, 16]:
            data = document(attributes, rows)
            name = f"dtd-{attributes:04}-{rows:02}.xml"
            (args.output / name).write_bytes(data)
            manifest[name] = {
                "attributes": attributes,
                "rows": rows,
                "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
            }
    (args.output / "inputs.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
