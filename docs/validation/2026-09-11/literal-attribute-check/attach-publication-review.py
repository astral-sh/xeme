from pathlib import Path
import hashlib
import json
import shutil
import tarfile

destination = Path('/home/dev-user/code/oss/oriole-literal-attribute-check/docs/validation/2026-09-11/literal-attribute-check')
source = Path('/tmp/oriole-literal-attribute-publication-independent-review')
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(source / 'review.json') == 'e2354d747399040fc8eed1097ab4e29be92e421a01f8ef16faae458f6998746a'
members = {str(p.relative_to(source)): {'sha256': sha(p), 'size': p.stat().st_size} for p in sorted(source.rglob('*')) if p.is_file()}
with tarfile.open(destination / 'publication-review.tar.gz', 'w:gz') as archive:
    for name in members:
        archive.add(source / name, arcname=name, recursive=False)
with tarfile.open(destination / 'publication-review.tar.gz') as archive:
    actual = {p.name: {'sha256': hashlib.sha256(archive.extractfile(p).read()).hexdigest(), 'size': p.size} for p in archive if p.isfile()}
assert actual == members
assert all(sha(source / name) == item['sha256'] for name, item in members.items())
shutil.copyfile(source / 'review.json', destination / 'publication-review.json')
(destination / 'publication-review-members.json').write_text(json.dumps(members, indent=2) + '\n')
readme = destination / 'README.md'
readme.write_text(readme.read_text() + '\nThe [final publication review](publication-review.json) verifies the committed source identity, current README benchmark table, result claims, evidence links and preserved warning/license structure. Its [complete receipt](publication-review.tar.gz) retains the reviewed snapshots; the final attachment and file-index update are separate packaging steps.\n')
shutil.copyfile(__file__, destination / 'attach-publication-review.py')
(destination / 'files.json').write_text(json.dumps({p.name: {'sha256': sha(p), 'size': p.stat().st_size} for p in sorted(destination.iterdir()) if p.is_file() and p.name != 'files.json'}, indent=2) + '\n')
print(json.dumps({'review_members': len(members), 'archive_sha256': sha(destination / 'publication-review.tar.gz'), 'files_index_sha256': sha(destination / 'files.json')}))
