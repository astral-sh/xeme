import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import tarfile

root = Path('/tmp/oriole-unique-accounting-handoff')
expected = json.loads((root / 'members.json').read_text())
digest = lambda b: hashlib.sha256(b).hexdigest()
observed = {}
nested = {}
with tarfile.open(root / 'evidence.tar.gz', 'r:gz') as t:
    for member in t:
        assert member.isfile() and not member.issym() and not member.islnk()
        path = PurePosixPath(member.name)
        assert not path.is_absolute() and '..' not in path.parts
        assert member.name not in observed
        data = t.extractfile(member).read()
        assert not data.startswith((b'\x7fELF', b'!<arch>\n'))
        observed[member.name] = {'bytes': len(data), 'sha256': digest(data)}
        assert observed[member.name] == expected[member.name]
        assert digest((Path('/tmp/oriole-unique-accounting-evidence') / member.name).read_bytes()) == digest(data)
        if member.name.endswith('.tar.gz'):
            rows = {}
            with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as inner:
                for item in inner:
                    p = PurePosixPath(item.name)
                    assert not p.is_absolute() and '..' not in p.parts
                    if item.isdir():
                        continue
                    assert item.isfile() and item.name not in rows
                    blob = inner.extractfile(item).read()
                    assert not blob.startswith((b'\x7fELF', b'!<arch>\n'))
                    rows[item.name] = {'bytes': len(blob), 'sha256': digest(blob)}
            nested[member.name] = rows
assert observed == expected
report = {
    'status': 'passed',
    'scope': 'Independent compressed-byte readback, no parser/build/timing reruns.',
    'archive_sha256': digest((root / 'evidence.tar.gz').read_bytes()),
    'outer_regular_members': len(observed),
    'outer_member_bytes': sum(x['bytes'] for x in observed.values()),
    'nested_archives': len(nested),
    'nested_regular_members': sum(map(len, nested.values())),
    'every_outer_member_matches_frozen_source_and_manifest': True,
    'every_nested_member_read_and_hashed': True,
    'no_symlinks_unsafe_paths_or_executable_binary_members': True,
    'nested_members': nested,
}
(root / 'readback.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
(root / 'readback.py').write_bytes(Path(__file__).read_bytes())
manifest = {str(p.relative_to(root)): {'sha256': digest(p.read_bytes()), 'bytes': p.stat().st_size} for p in sorted(root.iterdir()) if p.is_file() and p.name != 'manifest.json'}
(root / 'manifest.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
print(json.dumps({k:v for k,v in report.items() if k != 'nested_members'}))
print('manifest_sha256', digest((root / 'manifest.json').read_bytes()))
