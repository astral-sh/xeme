#!/usr/bin/env python3
"""Preserve the first index and add the omitted, byte-identical wrapper alias."""
import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
index_path = root / 'members.json.gz'
old = index_path.read_bytes()
assert hashlib.sha256(old).hexdigest() == '0f08ef4ba9faa4e21fb7b3c8c762e45832179b71a9e75fc1c7b6ec25cf6b64c0'
index = json.loads(gzip.decompress(old))
assert index['archive_sha256'] == '86007d0ffd7cc7ca8fd47b3b9bbc4b640b536c5154a11c414e0b334fa9b86769'
source = '/tmp/oriole-allocator-bolt-native-study/run.py'
alias = '/tmp/oriole-context-text-frame-fixed-native-study/pgo/run.py'
assert alias not in {row['path'] for row in index['origins']}
row = next(row for row in index['origins'] if row['path'] == source)
data = Path(alias).read_bytes()
assert hashlib.sha256(data).hexdigest() == row['sha256'] == '5f41a419e800e95954424e2a85d2ee988609d830db18dc85c469af801cd8aef5'
assert len(data) == row['bytes']
index['origins'].append({**row, 'path': alias, 'origin': alias,
                         'reason': 'Inherited wrapper byte comparison read by original auditor; explicit dependency omitted from its hash map'})
index['origins'].sort(key=lambda item: item['path'])
(root / 'attempts').mkdir(exist_ok=True)
saved = root / 'attempts/members-attempt01.json.gz'
assert not saved.exists()
saved.write_bytes(old)
new = gzip.compress((json.dumps(index, indent=2) + '\n').encode(), compresslevel=9, mtime=0)
index_path.write_bytes(new)
receipt = {'status': 'corrected_one_missing_alias_archive_unchanged',
           'first_verifier_exit': 1,
           'failure': 'KeyError for inherited run.py; original auditor read this exact byte-comparison dependency without adding it to its pin map',
           'added_origin': alias, 'same_payload_as': source, 'sha256': row['sha256'],
           'archive_sha256_unchanged': index['archive_sha256'],
           'old_index_sha256': hashlib.sha256(old).hexdigest(),
           'new_index_sha256': hashlib.sha256(new).hexdigest(),
           'origins': len(index['origins']), 'stored_members': index['stored_members'],
           'aliases': len(index['origins']) - index['stored_members'],
           'numeric_assertions_changed': False}
(root / 'index-correction.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
