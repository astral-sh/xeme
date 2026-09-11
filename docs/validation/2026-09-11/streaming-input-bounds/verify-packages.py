"""Verify the saved evidence without extracting archives or running targets."""

import argparse
import gzip
import hashlib
import json
from pathlib import Path, PurePosixPath
import tarfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def verify(root, label, check_originals):
    wrapper_path = root / f"{label}-archive.json"
    wrapper = json.loads(wrapper_path.read_bytes())
    archive = root / wrapper["archive"]
    index_bytes = (root / wrapper["member_index"]).read_bytes()
    assert digest(index_bytes) == wrapper["member_index_sha256"]
    index = json.loads(gzip.decompress(index_bytes))
    assert digest(archive.read_bytes()) == wrapper["archive_sha256"] == index["archive_sha256"]
    expected = {row["destination"]: row for row in index["members"]}
    assert len(expected) == len(index["members"]) == wrapper["members"]
    seen = set()
    total = 0
    with tarfile.open(archive, "r:gz") as stream:
        for member in stream:
            path = PurePosixPath(member.name)
            assert member.isfile() and not path.is_absolute() and ".." not in path.parts
            assert member.name not in seen and member.name in expected
            row = expected[member.name]
            data = stream.extractfile(member).read()
            assert len(data) == row["bytes"] == member.size
            assert digest(data) == row["sha256"]
            assert not data.startswith(b"\x7fELF")
            assert not member.name.endswith((".tar.gz", ".tar.zst"))
            if check_originals:
                assert digest(Path(row["source"]).read_bytes()) == row["sha256"]
            total += len(data)
            seen.add(member.name)
    assert seen == set(expected) and total == wrapper["uncompressed_bytes"]
    assert len(index["aliases"]) == wrapper["duplicate_source_aliases"]
    for alias in index["aliases"]:
        canonical = expected[alias["same_bytes_as_destination"]]
        assert alias["sha256"] == canonical["sha256"]
        if check_originals:
            assert digest(Path(alias["source"]).read_bytes()) == alias["sha256"]
    return {
        "archive": archive.name,
        "archive_sha256": wrapper["archive_sha256"],
        "index_sha256": wrapper["member_index_sha256"],
        "members": len(seen),
        "aliases": len(index["aliases"]),
        "uncompressed_bytes": total,
        "original_paths_checked": check_originals,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-originals", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    print(json.dumps({
        "status": "passed",
        "scope": "Independent root archive/member/alias readback; no target execution or new numerical claim.",
        "archives": [verify(root, label, args.check_originals)
                     for label in ("evidence", "benchmark-evidence")],
    }, indent=2))
