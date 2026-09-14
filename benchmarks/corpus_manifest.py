"""Shared identity checks for pinned benchmark corpora."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


def corpus_files(manifest_path: Path) -> tuple[dict[str, Path], dict[str, str]]:
    """Verify original input and notice bytes without parsing the XML."""
    manifest_path = manifest_path.resolve(strict=True)
    manifest = json.loads(manifest_path.read_text())
    inputs: dict[str, Path] = {}
    hashes = {
        str(manifest_path): hashlib.sha256(manifest_path.read_bytes()).hexdigest()
    }
    for project in manifest["projects"]:
        if project["name"] in inputs:
            raise ValueError("duplicate corpus project")
        entries = [entry for entry in project["files"] if entry["role"] == "input"]
        if len(entries) != 1:
            raise ValueError("expected one original input per project")
        for entry in project["files"]:
            path = (manifest_path.parent / entry["path"]).resolve(strict=True)
            if not path.is_relative_to(manifest_path.parent) or str(path) in hashes:
                raise ValueError("unsafe or repeated corpus path")
            data = path.read_bytes()
            value = hashlib.sha256(data).hexdigest()
            if value != entry["sha256"] or len(data) != entry.get("bytes", len(data)):
                raise ValueError(f"corpus identity mismatch: {path}")
            if "git_blob_sha1" in entry:
                blob = hashlib.sha1(
                    b"blob " + str(len(data)).encode() + b"\0" + data
                ).hexdigest()
                if blob != entry["git_blob_sha1"]:
                    raise ValueError(f"upstream blob identity mismatch: {path}")
            hashes[str(path)] = value
            if entry["role"] == "input":
                inputs[project["name"]] = path
    if not inputs:
        raise ValueError("empty corpus")
    return inputs, hashes
