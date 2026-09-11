"""Verify a relocated allocator evidence package and replay saved native data.

No compiler, library or parser is loaded. Only the trusted, hash-bound Python
records-only reader is executed; archive members are never blindly extracted.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tarfile
import tempfile


def sha(data):
    return hashlib.sha256(data).hexdigest()


def safe(name):
    path = PurePosixPath(name)
    assert not path.is_absolute() and '..' not in path.parts and str(path) == name


def main():
    if not __debug__:
        raise RuntimeError('Assertions must remain enabled')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert not args.output.exists()
    root = args.package
    manifest = json.loads((root / 'files.json').read_bytes())
    assert manifest['format'] == 'sha256-file-manifest-v1'
    for name, identity in manifest['files'].items():
        safe(name)
        data = (root / name).read_bytes()
        assert (sha(data), len(data)) == (identity['sha256'], identity['bytes']), name
    index_data = (root / 'archive-index.json').read_bytes()
    index = json.loads(index_data)
    assert index['format'] == 'sha256-object-archive-v1'
    records = {}
    for item in index['files']:
        safe(item['path'])
        assert item['path'] not in records
        records[item['path']] = item
    sizes = {}
    for row in records.values():
        assert row['sha256'] not in sizes or sizes[row['sha256']] == row['bytes']
        sizes[row['sha256']] = row['bytes']
    assert len(sizes) == index['objects']
    assert sum(sizes.values()) == index['unique_bytes'] <= 256 * 1024**2
    objects = {}
    expected = {'index.json'} | {'objects/' + digest for digest in sizes}
    seen = set()
    with tarfile.open(root / 'evidence.tar.gz', 'r:gz') as archive:
        for member in archive:
            assert member.name in expected and member.name not in seen and member.isfile()
            seen.add(member.name)
            assert member.size <= 256 * 1024**2
            data = archive.extractfile(member).read()
            if member.name == 'index.json':
                assert data == index_data
            else:
                digest = member.name.removeprefix('objects/')
                assert sha(data) == digest and len(data) == sizes[digest]
                assert not data.startswith((b'\x7fELF', b'!<arch>\n'))
                objects[digest] = data
    assert seen == expected

    def get(name):
        return objects[records[name]['sha256']]

    all_paths = set(records)
    for group in ('excluded_binaries_and_caches', 'native_worker_aliases'):
        for row in index[group]:
            safe(row['path'])
            assert row['path'] not in all_paths
            all_paths.add(row['path'])
            assert len(row['sha256']) == 64 and int(row['sha256'], 16) >= 0 and row['bytes'] >= 0
    for name in ('notices/expat-2.8.4/COPYING', 'notices/expat-2.8.4/expat/COPYING',
                 'notices/cpython-3.12.13/LICENSE', 'notices/oriole/LICENSE-APACHE',
                 'notices/oriole/LICENSE-MIT', 'notices/oriole/tests/c/UPSTREAM-NOTICES.txt'):
        assert len(get(name)) > 0
    native = 'allocator-provenance-native-study'
    audit = 'allocator-provenance-native-independent-review'
    containers = {(mode, name): json.loads(get(f'{native}/{mode}/native-screen/{name}.json'))
                  for mode in ('normal', 'pgo') for name in ('preflight', 'results')}
    alias_list = []
    for alias in index['native_worker_aliases']:
        target = alias['reconstruct']
        parts = PurePosixPath(alias['path']).parts
        assert parts[0] == native and parts[2] == 'native-screen'
        mode = parts[1]
        ordinal = int(parts[3].removeprefix('worker-').removesuffix('.json'))
        assert alias['path'] == f'{native}/{mode}/native-screen/worker-{ordinal:04d}.json'
        kind = PurePosixPath(target['container']).stem
        assert target['container'] == f'{native}/{mode}/native-screen/{kind}.json'
        row = containers[(mode, kind)]['rows'][target['row_index']]
        if target['process_index'] is not None:
            assert kind == 'results'
            row = row['processes'][target['process_index']]
        else:
            assert kind == 'preflight'
        raw = {k: v for k, v in row.items() if k not in ('observations', 'median_seconds')}
        data = (json.dumps(raw, indent=2) + '\n').encode()
        assert sha(data) == alias['sha256'] == target['sha256']
        assert len(data) == alias['bytes'] == target['bytes']
        alias_list.append([mode, ordinal, kind, target['row_index'], target['process_index'], len(data), sha(data)])
    assert sorted(alias_list) == sorted(json.loads(get(audit + '/worker-aliases.json'))['aliases'])
    assert len(alias_list) == 1344

    source = json.loads(get('allocator-provenance-pgo-study/source.json'))['source_sha256']
    assert len(source) == 70
    source_seen = set()
    with tarfile.open(fileobj=io.BytesIO(get('allocator-provenance-pgo-study/source.tar.gz')), mode='r:gz') as archive:
        for member in archive:
            assert member.isfile() and member.name in source and member.name not in source_seen
            assert sha(archive.extractfile(member).read()) == source[member.name]
            source_seen.add(member.name)
    assert source_seen == set(source)
    assert (root / 'replay_native.py').read_bytes() == get(audit + '/replay.py')
    assert (root / 'conditions.csv').read_bytes() == get(audit + '/conditions.csv')
    with tempfile.TemporaryDirectory(prefix='oriole-native-records-') as temporary:
        temp = Path(temporary)
        for mode in ('normal', 'pgo'):
            folder = temp / mode / 'native-screen'
            folder.mkdir(parents=True)
            for name in ('protocol.json', 'preflight.json', 'results.json'):
                (folder / name).write_bytes(get(f'{native}/{mode}/native-screen/{name}'))
        (temp / 'review.json').write_bytes(get(audit + '/review.json'))
        command = [sys.executable, '-I', '-S', str((root / 'replay_native.py').resolve()),
                   '--evidence-root', str(temp), '--review', str(temp / 'review.json'),
                   '--output', str(temp / 'replayed.json')]
        outcome = subprocess.run(command, capture_output=True, text=True, timeout=120, check=False)
        assert outcome.returncode == 0 and outcome.stderr == '', (outcome.returncode, outcome.stderr)
        replay = json.loads((temp / 'replayed.json').read_bytes())
    report = json.loads((root / 'report.json').read_bytes())
    assert report['archive_sha256'] == manifest['files']['evidence.tar.gz']['sha256']
    assert report['archive_index_sha256'] == sha(index_data)
    assert report['source_manifest_sha256'] == sha(get('allocator-provenance-pgo-study/source.json'))
    assert report['logical_files'] == len(records) and report['unique_files'] == len(objects)
    assert report['excluded_binaries_and_caches'] == len(index['excluded_binaries_and_caches'])
    assert report['native_worker_aliases'] == len(alias_list)
    assert report['native_totals']['workers'] == replay['totals']['workers'] == 1344
    assert report['native_totals']['samples'] == replay['totals']['samples'] == 212352
    assert report['native'] == {mode: replay['modes'][mode]['groups'] for mode in ('normal', 'pgo')}
    receipt = {'status': 'passed', 'scope': 'Portable archive integrity, source snapshot, worker aliases and saved native arithmetic only; no targets or live dependencies checked.',
               'files_manifest_sha256': sha((root / 'files.json').read_bytes()),
               'archive_sha256': report['archive_sha256'], 'logical_files': len(records),
               'unique_objects': len(objects), 'excluded_identities': len(index['excluded_binaries_and_caches']),
               'worker_aliases': len(alias_list), 'source_files': len(source),
               'native': replay, 'reader_stdout': outcome.stdout}
    with args.output.open('x') as output:
        json.dump(receipt, output, indent=2)
        output.write('\n')
    print(json.dumps({'status': 'passed', 'receipt': str(args.output), 'sha256': sha(args.output.read_bytes()),
                      'objects': len(objects), 'aliases': len(alias_list), 'native_totals': replay['totals']}))


if __name__ == '__main__':
    main()
