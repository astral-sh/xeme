from pathlib import Path
import hashlib
import io
import json
import tarfile

repo = Path('/home/dev-user/code/oss/oriole-end-frame-cells')
out = repo / 'docs/validation/2026-09-11/detached-end-frames'
sha = lambda b: hashlib.sha256(b).hexdigest()
dirs = {
    'package': Path('/tmp/oriole-detached-end-frames-package-independent-review'),
    'human': Path('/tmp/oriole-detached-end-frames-human-independent-review'),
}
reviews = {name: directory / 'review.json' for name, directory in dirs.items()}
members = []
with tarfile.open(out / 'final-reviews.tar.gz', 'w:gz', compresslevel=9) as archive:
    for prefix, root in dirs.items():
        for path in sorted(root.rglob('*')):
            if not path.is_file() or path.is_symlink() or '__pycache__' in path.parts:
                continue
            data = path.read_bytes()
            assert not data.startswith((b'\x7fELF', b'!<arch>\n'))
            name = prefix + '/' + str(path.relative_to(root))
            entry = {'path': name, 'source': str(path), 'bytes': len(data), 'sha256': sha(data)}
            info = tarfile.TarInfo(name)
            info.size, info.mode, info.mtime = len(data), 0o644, 0
            archive.addfile(info, io.BytesIO(data))
            members.append(entry)
with tarfile.open(out / 'final-reviews.tar.gz', 'r:gz') as archive:
    actual = archive.getmembers()
    assert len(actual) == len(members)
    for member, entry in zip(actual, members):
        assert member.name == entry['path'] and member.isfile() and member.size == entry['bytes']
        assert sha(archive.extractfile(member).read()) == entry['sha256'] == sha(Path(entry['source']).read_bytes())
(out / 'final-review-members.json').write_text(json.dumps(members, indent=2) + '\n')
summary = {'reviews': {name: {'source': str(path), 'sha256': sha(path.read_bytes()),
                            'status': json.loads(path.read_text())['status']}
                       for name, path in reviews.items()},
           'archive': {'sha256': sha((out / 'final-reviews.tar.gz').read_bytes()),
                       'members': len(members), 'all_members_and_origins_readback': True},
           'scope': 'Final detached End benchmark package, corrected-source/runtime/binary lineage and human claims independently reviewed. Subsequent changes only attach these receipts and refresh the outer hash index; no raw measurement or implementation changes.'}
(out / 'final-reviews.json').write_text(json.dumps(summary, indent=2) + '\n')
(out / Path(__file__).name).write_bytes(Path(__file__).read_bytes())
paths = [p for p in sorted(out.rglob('*')) if p.is_file() and p.name != 'files.json']
index = {'files': {str(p.relative_to(out)): {'bytes': p.stat().st_size, 'sha256': sha(p.read_bytes())} for p in paths}}
(out / 'files.json').write_text(json.dumps(index, indent=2) + '\n')
for name, entry in json.loads((out / 'files.json').read_text())['files'].items():
    data = (out / name).read_bytes()
    assert len(data) == entry['bytes'] and sha(data) == entry['sha256']
print(json.dumps({'reviews': summary, 'outer_files': len(paths), 'index_sha256': sha((out / 'files.json').read_bytes())}, indent=2))
