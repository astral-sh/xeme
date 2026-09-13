"""Package sealed sanitizer evidence for a compact, reviewable Git diff."""
from pathlib import Path
import hashlib
import json
import shutil
import tarfile

destination = Path('/home/dev-user/code/oss/oriole-current-asan/docs/validation/2026-09-11/current-asan')
handoff = Path('/tmp/oriole-be22-asan-handoff')
final_review = Path('/tmp/oriole-be22-asan-final-independent-review')

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def pack(source, name):
    paths = sorted(p for p in source.rglob('*') if p.is_file())
    expected = {str(p.relative_to(source)): {'sha256': digest(p), 'size': p.stat().st_size} for p in paths}
    target = destination / name
    assert not target.exists()
    with tarfile.open(target, 'w:gz') as archive:
        for path in paths:
            archive.add(path, arcname=str(path.relative_to(source)), recursive=False)
    with tarfile.open(target) as archive:
        actual = {p.name: {'sha256': hashlib.sha256(archive.extractfile(p).read()).hexdigest(), 'size': p.size} for p in archive.getmembers() if p.isfile()}
    assert actual == expected
    assert all(digest(source / name) == item['sha256'] for name, item in expected.items())
    return {'sha256': digest(target), 'size': target.stat().st_size, 'members': expected}

assert digest(final_review / 'review.json') == '2c8ee226e4aa494c25a751ed9e184de281600eed8bc3d506601bd4fb5e3e7899'
archives = {
    'producer-handoff.tar.gz': pack(handoff, 'producer-handoff.tar.gz'),
    'independent-review.tar.gz': pack(final_review, 'independent-review.tar.gz'),
    'draft-prose-review.tar.gz': pack(Path('/tmp/oriole-current-asan-prose-independent-review'), 'draft-prose-review.tar.gz'),
    'draft-structure-review.tar.gz': pack(Path('/tmp/oriole-current-asan-doc-structure-independent-review'), 'draft-structure-review.tar.gz'),
}
for source, name in [(handoff / 'report.json', 'report.json'), (handoff / 'readiness-review.json', 'readiness-review.json'), (final_review / 'review.json', 'independent-review.json')]:
    shutil.copyfile(source, destination / name)
    assert digest(destination / name) == digest(source)
retained = Path('/tmp/oriole-current-asan-publication-uncompressed-copy')
assert not retained.exists()
shutil.move(destination / 'data', retained)
text = (destination / 'README.md').read_text()
old = '''The [report](data/report.json), [complete evidence](data/evidence.tar.gz), [final corpus](data/final-corpus.tar.gz) and [member indexes](data/files.json) retain commands, sources, compiler records, binary identities, complete logs, inputs, provenance and preparation failures. Compiled executables are excluded and separately hashed. The [handoff guide](data/README.md) describes archive membership and deduplication.

The [independent readiness review](data/readiness-review.json) verifies source, instrumentation and all input origins. Its scope precedes campaign outcomes. Final campaign and package review is recorded separately before publication.'''
new = '''The [report](report.json) and [complete producer handoff](producer-handoff.tar.gz) retain commands, sources, compiler records, binary identities, complete logs, original and final inputs, provenance and preparation failures. Compiled executables are excluded and separately hashed. Every original handoff file is archived unchanged, including its guide and member indexes. The [publication index](files.json) records file hashes, and the [archive member index](archive-members.json) records member hashes. Large per-input maps remain compressed.

The [independent readiness review](readiness-review.json) verifies source, instrumentation and all input origins. Its scope precedes campaign outcomes. The [independent final review](independent-review.json) reconstructs all 12 executions, totals and package origins from saved evidence: 676,701 checks across 57,597 unchanged inputs, with no findings. Its [complete receipt and audit](independent-review.tar.gz) are retained separately. The producer guide's pending-review sentence describes its earlier sealing time; this final review supersedes that status.

The [draft prose review](draft-prose-review.tar.gz) and [draft structure review](draft-structure-review.tar.gz) retain their exact earlier document snapshots. They exclude this final packaging and review-link update; the [final publication review](publication-review.json) covers those changes.'''
assert text.count(old) == 1
(destination / 'README.md').write_text(text.replace(old, new))
shutil.copyfile('/tmp/oriole-publish-current-asan-docs.py', destination / 'assemble-draft.py')
shutil.copyfile(__file__, destination / 'package-publication.py')
(destination / 'archive-members.json').write_text(json.dumps(archives, indent=2) + '\n')
print(json.dumps({name: {'bytes': value['size'], 'members': len(value['members']), 'sha256': value['sha256']} for name, value in archives.items()}, indent=2))
