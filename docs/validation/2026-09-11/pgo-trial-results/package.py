"""Package saved PR126 evidence; never execute a compiler or downloaded target."""
import gzip
import hashlib
import io
import json
import tarfile
import zipfile
from pathlib import Path

ROOT = Path('/tmp/oriole-pr126-pbs-results')
MONITOR = Path('/tmp/oriole-pr126-ci-monitor')
OUTPUT = Path('/tmp/oriole-pr126-pbs-handoff')
SOURCE = Path('/home/dev-user/code/oss/oriole-pbs-pgo-bundle')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def dump(path, data):
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + '\n')


def main():
    assert not OUTPUT.exists(), 'Keep first package attempts intact.'
    OUTPUT.mkdir()
    payloads = {}
    origins = {}

    def add(name, path, compressed=False):
        data = path.read_bytes()
        origins[name] = {'path': str(path), 'source_bytes': len(data), 'source_sha256': digest(data)}
        if compressed:
            data = gzip.compress(data, compresslevel=3, mtime=0)
            origins[name]['transformation'] = 'gzip level 3, mtime 0'
        payloads[name] = data

    for name in (
        'README.md', 'report.json', 'audit.py', 'audit.stdout', 'audit.stderr',
        'audit.py.before-wording-correction', 'report.json.before-wording-correction',
        'wording-correction.json', 'run.json', 'jobs.json', 'artifacts.json',
        'validation.zip', 'pbs.yml', 'test-old-glibc.py',
        'python-dynamic.txt', 'libpython-dynamic.txt',
        'selected/python/PYTHON.json', 'selected/python/build/run_tests.py',
        'selected/python/licenses/LICENSE.oriole-build.txt',
        'selected/python/licenses/LICENSE.oriole.txt',
    ):
        add(name, ROOT / name)
    for name in ('workflow.log', 'distribution-members.json', 'python-dynsym.txt', 'libpython-dynsym.txt'):
        add(name + '.gz', ROOT / name, compressed=True)
    for name in ('ci-receipt.json', 'ci-run-final.json', 'ci-jobs-final.json', 'log-retrieval-note.json'):
        add('regular-ci/' + name, MONITOR / name)
    add('regular-ci/ci-workflow.log.gz', MONITOR / 'ci-workflow.log', compressed=True)
    add('root-review.json', Path('/tmp/oriole-pr126-pbs-root-review.json'))
    add('root-review.py', Path('/tmp/oriole-pr126-pbs-root-review.py'))
    add('selected-streaming-source.json', Path('/tmp/oriole-streaming-work-pgo-study-attempt02/source.json'))
    for name in ('LICENSE-APACHE', 'LICENSE-MIT'):
        add('source/' + name, SOURCE / name)
    for name in ('README.md', '__init__.py', 'build.py', 'corpus.py', 'test_build.py', 'train.py'):
        add('source/tools/pgo/' + name, SOURCE / 'tools/pgo' / name)
    for name in ('prepare.py', 'pgo_bundle.py'):
        add('source/integration/python-build-standalone/' + name, SOURCE / 'integration/python-build-standalone' / name)
    add('package.py', Path(__file__))

    # Retain the untouched first-party ZIP, and identify every nested raw member.
    raw_members = []
    with zipfile.ZipFile(io.BytesIO(payloads['validation.zip'])) as original:
        assert original.testzip() is None
        for member in original.infolist():
            assert not member.is_dir()
            data = original.read(member.filename)
            assert data == (ROOT / 'validation' / member.filename).read_bytes()
            raw_members.append({'name': member.filename, 'bytes': len(data), 'sha256': digest(data)})
    assert len(raw_members) == 42
    payloads['validation-members.json'] = (json.dumps(raw_members, indent=2, sort_keys=True) + '\n').encode()
    origins['validation-members.json'] = {'derived_from': 'validation.zip', 'scope': 'Read every original ZIP member and match extracted bytes; no target execution.'}
    assert digest(payloads['report.json']) == 'd084a9e4f7da27444931adb47c4c98d2c427048e6689731b8cccc6defa600425'
    assert digest(payloads['validation.zip']) == 'bd92e2d3af1b2518e03adf5403e079d998e29ba5c13ed66594118b0fc394b9e6'
    for name in payloads:
        assert name not in ('distribution.zip',) and not name.endswith(('.so', '.a', '.tar.zst'))

    members = []
    tar_bytes = io.BytesIO()
    with tarfile.open(fileobj=tar_bytes, mode='w', format=tarfile.PAX_FORMAT) as archive:
        for name, data in sorted(payloads.items()):
            entry = tarfile.TarInfo(name)
            entry.size = len(data)
            entry.mode = 0o644
            entry.uid = entry.gid = entry.mtime = 0
            archive.addfile(entry, io.BytesIO(data))
            members.append({'name': name, 'bytes': len(data), 'sha256': digest(data), 'origin': origins[name]})
    packed = gzip.compress(tar_bytes.getvalue(), compresslevel=3, mtime=0)
    (OUTPUT / 'evidence.tar.gz').write_bytes(packed)
    with tarfile.open(fileobj=io.BytesIO(packed), mode='r:gz') as archive:
        assert archive.getnames() == sorted(payloads)
        for member in archive:
            assert archive.extractfile(member).read() == payloads[member.name]
    report = json.loads(payloads['report.json'])
    dump(OUTPUT / 'archive-members.json', {
        'archive': 'evidence.tar.gz', 'bytes': len(packed), 'sha256': digest(packed),
        'members': members, 'raw_validation_members': len(raw_members),
        'source_run': report['run']['url'],
        'binary_exclusions': report['binary_publication_exclusions'],
        'readback': 'Every archive member read back and matched byte for byte; all 42 members of the retained original validation ZIP also matched their saved originals.',
    })
    for name in ('README.md', 'report.json', 'root-review.json', 'package.py'):
        (OUTPUT / name).write_bytes(payloads[name])
    dump(OUTPUT / 'files.json', {'files': [
        {'name': p.name, 'bytes': p.stat().st_size, 'sha256': digest(p.read_bytes())}
        for p in sorted(OUTPUT.iterdir()) if p.name != 'files.json'
    ]})
    print(json.dumps({'output': str(OUTPUT), 'archive_sha256': digest(packed), 'archive_bytes': len(packed), 'members': len(members), 'nested_raw_members': len(raw_members)}))


if __name__ == '__main__':
    main()
