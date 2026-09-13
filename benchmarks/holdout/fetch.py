#!/usr/bin/env python3
"""Reacquire the frozen original blobs through GitHub CLI; never parse or time XML."""

import argparse
import base64
import hashlib
import json
import re
import subprocess
from pathlib import Path
from urllib.parse import quote


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--gh",
        default="gh",
        help="GitHub CLI executable; use gh-auto on the managed devbox",
    )
    args = parser.parse_args()
    manifest_path = Path(__file__).with_name("corpus-manifest.json")
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    output = args.output.resolve()
    if output.exists():
        parser.error("output must be a new directory")
    # Run from the intended OSS checkout so gh-auto uses its scoped identity.
    subprocess.run([args.gh, "api", "user", "--jq", ".login"], check=True)
    output.mkdir(parents=True)
    for project in manifest["projects"]:
        commit = project["commit"]
        if not re.fullmatch(r"[0-9a-f]{40}", commit):
            raise ValueError("expected a full pinned commit")
        for entry in project["files"]:
            destination = (output / entry["path"]).resolve()
            if not destination.is_relative_to(output) or destination.exists():
                raise ValueError("unsafe or repeated destination")
            endpoint = (
                f"repos/{project['repository']}/contents/"
                f"{quote(entry['upstream_path'], safe='/')}?ref={commit}"
            )
            response = subprocess.run(
                [args.gh, "api", endpoint],
                capture_output=True,
                check=True,
            )
            blob = json.loads(response.stdout)
            if blob["encoding"] != "base64":
                raise ValueError("expected base64 contents API response")
            data = base64.b64decode(blob["content"])
            git_blob = hashlib.sha1(
                b"blob " + str(len(data)).encode() + b"\0" + data
            ).hexdigest()
            if (
                len(data) != entry["bytes"]
                or blob["size"] != entry["bytes"]
                or hashlib.sha256(data).hexdigest() != entry["sha256"]
                or git_blob != entry["git_blob_sha1"]
                or blob["sha"] != entry["git_blob_sha1"]
            ):
                raise ValueError(f"blob identity mismatch: {entry['path']}")
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
            print(entry["path"], flush=True)
    (output / "corpus-manifest.json").write_bytes(manifest_bytes)


if __name__ == "__main__":
    main()
