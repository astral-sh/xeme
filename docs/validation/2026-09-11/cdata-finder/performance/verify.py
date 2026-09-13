"""Verify the portable Finder package and aliases without any producer paths."""
from pathlib import Path
import gzip, hashlib, json, sys, tarfile

root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent
sha = lambda data: hashlib.sha256(data).hexdigest()
index_path = root / 'members.json.gz'
index = json.loads(gzip.decompress(index_path.read_bytes()))
summary = json.loads((root / 'summary.json').read_text())
archive = root / index['archive']
assert sha(archive.read_bytes()) == index['archive_sha256'] == summary['archive_sha256']
assert sha(index_path.read_bytes()) == summary['member_index_sha256']
expected = {r['destination']: r for r in index['members']}
assert len(expected) == len(index['members'])
seen = set()
with tarfile.open(archive, 'r|gz') as stream:
    for member in stream:
        assert member.isfile() and not member.name.startswith('/') and '..' not in Path(member.name).parts
        assert member.name not in seen
        seen.add(member.name)
        row = expected[member.name]
        data = stream.extractfile(member).read()
        assert len(data) == row['bytes'] and sha(data) == row['sha256']
        assert not data.startswith((b'\x7fELF', b'!<arch>\n'))
assert seen == expected.keys()
alias_names = set()
for alias in index['aliases']:
    target = expected[alias['same_bytes_as_destination']]
    assert alias['destination'] not in seen | alias_names
    alias_names.add(alias['destination'])
    assert alias['sha256'] == target['sha256'] and alias['bytes'] == target['bytes']
assert len(seen) == summary['stored_members']
assert len(alias_names) == summary['aliases']
assert len(seen) + len(alias_names) == summary['selected_sources']
print(json.dumps({'status': 'passed', 'stored_members': len(seen), 'aliases': len(alias_names), 'archive_sha256': summary['archive_sha256'], 'scope': 'Every stored payload and alias verified using only package-local files. No original producer path, target execution or numerical rerun required.'}))
