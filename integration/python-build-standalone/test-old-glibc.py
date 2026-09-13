"""Exercise the actual installed interpreter and parser on the minimum glibc."""

import concurrent.futures
import json
import platform
import subprocess
import sys
import xml.parsers.expat

assert platform.libc_ver() == ("glibc", "2.17"), platform.libc_ver()
assert xml.parsers.expat.EXPAT_VERSION.startswith("xeme_")


def parse_document(_: int) -> None:
    parser = xml.parsers.expat.ParserCreate(namespace_separator="|")
    events = []
    parser.StartElementHandler = lambda name, attributes: events.append(
        (name, attributes)
    )
    parser.Parse(b"<r xmlns='urn:example'><child a='value'/>text&amp;</r>", True)
    assert events == [("urn:example|r", {}), ("urn:example|child", {"a": "value"})]


for _ in range(8):
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        list(pool.map(parse_document, range(128)))
print(
    json.dumps(
        {
            "glibc": platform.libc_ver(),
            "expat": xml.parsers.expat.EXPAT_VERSION,
            "threaded_parses": 1024,
        }
    ),
    flush=True,
)
subprocess.run(
    [
        sys.executable,
        "-I",
        "-m",
        "test",
        "test_pyexpat",
        "test_xml_etree",
        "test_xml_etree_c",
        "test_minidom",
        "test_sax",
        "test_pulldom",
    ],
    check=True,
)
