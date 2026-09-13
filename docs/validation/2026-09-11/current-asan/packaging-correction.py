"""Complete the packaging-only correction after a read-only copy could not move across filesystems."""
from pathlib import Path
import hashlib
import json
import tarfile

original = Path('/tmp/oriole-finalize-current-asan-docs.py').read_text()
scope = {}
exec(original[:original.index('archives = {')].replace("assert not target.exists()", "assert not target.exists()"), scope)
destination = scope['destination']
handoff = scope['handoff']
archives = {}
for name, source in [
    ('producer-handoff.tar.gz', handoff),
    ('independent-review.tar.gz', scope['final_review']),
    ('draft-prose-review.tar.gz', Path('/tmp/oriole-current-asan-prose-independent-review')),
    ('draft-structure-review.tar.gz', Path('/tmp/oriole-current-asan-doc-structure-independent-review')),
]:
    expected = {str(p.relative_to(source)): {'sha256': scope['digest'](p), 'size': p.stat().st_size} for p in sorted(source.rglob('*')) if p.is_file()}
    with tarfile.open(destination / name) as archive:
        actual = {p.name: {'sha256': hashlib.sha256(archive.extractfile(p).read()).hexdigest(), 'size': p.size} for p in archive.getmembers() if p.isfile()}
    assert actual == expected
    archives[name] = {'sha256': scope['digest'](destination / name), 'size': (destination / name).stat().st_size, 'members': expected}
for copy in [destination / 'data', Path('/tmp/oriole-current-asan-publication-uncompressed-copy')]:
    assert {str(p.relative_to(copy)): scope['digest'](p) for p in copy.rglob('*') if p.is_file()} == {str(p.relative_to(handoff)): scope['digest'](p) for p in handoff.rglob('*') if p.is_file()}
retained = Path('/home/dev-user/code/oss/oriole-current-asan-uncompressed-draft')
assert not retained.exists()
(destination / 'data').chmod(0o755)
(destination / 'data').rename(retained)
scope.update(archives=archives, __file__='/tmp/oriole-finalize-current-asan-docs.py')
exec(original[original.index("text = (destination / 'README.md').read_text()"):], scope)
(destination / 'packaging-correction.py').write_text(Path(__file__).read_text())
(destination / 'packaging-correction.json').write_text(json.dumps({
    'status': 'passed',
    'initial_attempt': 'Cross-filesystem shutil.move copied all 12 sealed files, then could not remove the read-only source directory. Four archives and three report copies had already completed.',
    'correction': 'Verified all four archives and both complete uncompressed copies against their immutable sources. A same-filesystem rename also required write permission on the copied directory. Made that generated directory writable, renamed it, and completed the document update. Original immutable handoff permissions and bytes remain unchanged.',
    'sources_changed': False,
    'targets_rerun': False,
}, indent=2) + '\n')
