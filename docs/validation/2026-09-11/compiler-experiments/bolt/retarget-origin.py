#!/usr/bin/env python3
"""Keep the archived first verifier's origin attached to its retained bytes."""
import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
path = root / 'members.json.gz'
old = path.read_bytes()
assert hashlib.sha256(old).hexdigest() == 'b65a1166ba9f0edbce4ea63d9f6ac97344637f483a6103db7d9e6b3d9c585890'
index = json.loads(gzip.decompress(old))
row = next(row for row in index['origins'] if row['path'] == str(root / 'verify.py'))
retained = root / 'attempts/verify-attempt01.py'
assert hashlib.sha256(retained.read_bytes()).hexdigest() == row['sha256']
row['origin'] = str(retained)
row['reason'] = 'Original archived verifier retained before the one-line original-auditor hash guard; final verifier is a separately indexed sidecar'
assert not (root / 'attempts/members-attempt02.json.gz').exists()
(root / 'attempts/members-attempt02.json.gz').write_bytes(old)
new = gzip.compress((json.dumps(index, indent=2) + '\n').encode(), compresslevel=9, mtime=0)
path.write_bytes(new)
receipt = {'status': 'retargeted_historical_verifier_origin_only',
           'old_index_sha256': hashlib.sha256(old).hexdigest(),
           'new_index_sha256': hashlib.sha256(new).hexdigest(),
           'path_key_sha256_member_bytes_unchanged': True,
           'archive_unchanged': True,
           'reason': 'Top-level verifier gained a source identity guard; archived first-version origin now points to its exact retained snapshot.',
           'independent_source_review': '0a5b73c70ffa0d662a76d2ca1c6fd9ad1a333cbfebe039a853b80f25a02ff96b binds the same final verifier; its member-index input is the retained attempt02 index.'}
(root / 'origin-correction.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
