from pathlib import Path
import hashlib
import json
import shutil
import tarfile

destination = Path('/home/dev-user/code/oss/oriole-current-pbs/docs/validation/2026-09-11/current-pbs')
source = Path('/tmp/oriole-be22-pbs-independent-review')
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(source / 'review.json') == '99d896600da91192636203558c02028c657687cb3b120f469be7650c88ca0760'
members = {str(p.relative_to(source)): {'sha256': sha(p), 'size': p.stat().st_size} for p in sorted(source.rglob('*')) if p.is_file()}
with tarfile.open(destination / 'independent-review.tar.gz', 'w:gz') as archive:
    for name in members:
        archive.add(source / name, arcname=name, recursive=False)
with tarfile.open(destination / 'independent-review.tar.gz') as archive:
    actual = {p.name: {'sha256': hashlib.sha256(archive.extractfile(p).read()).hexdigest(), 'size': p.size} for p in archive if p.isfile()}
assert actual == members
assert all(sha(source / name) == item['sha256'] for name, item in members.items())
shutil.copyfile(source / 'review.json', destination / 'independent-review.json')
(destination / 'review-members.json').write_text(json.dumps(members, indent=2) + '\n')
readme = destination / 'README.md'
text = readme.read_text()
old = 'The [independent review](independent-review.json) and [publication index](files.json) provide the final review and file identities.'
new = 'The [independent review](independent-review.json), [complete review receipt](independent-review.tar.gz) and [publication index](files.json) provide the final review and file identities.'
assert text.count(old) == 1
readme.write_text(text.replace(old, new))
shutil.copyfile(__file__, destination / 'attach-review.py')
(destination / 'files.json').write_text(json.dumps({p.name: {'sha256': sha(p), 'size': p.stat().st_size} for p in sorted(destination.iterdir()) if p.is_file() and p.name != 'files.json'}, indent=2) + '\n')
print(json.dumps({'review_members': len(members), 'archive_sha256': sha(destination / 'independent-review.tar.gz'), 'files_index_sha256': sha(destination / 'files.json')}))
