"""Verify a relocated rejected checkpoint-guard V2 evidence package and replay saved native data.

No compiler, library or parser is loaded. Only the trusted, hash-bound Python
records-only reader is executed; archive members are never blindly extracted.
"""
import argparse
import base64
import csv
import re
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


def validation_records(get, identities):
    """Reconstruct the saved PGO differential and final local-check records."""
    load = lambda name: json.loads(get(name))
    prefix = 'coordinate-checkpoint-guards-differential/'
    summary, corpus, reference, candidate = [load(prefix + n + '.json') for n in ['summary', 'corpus', 'reference', 'oriole']]
    assert summary['status'] == 'passed' and summary['exact_pass'] and summary['semantic_pass'] and summary['capture_callbacks']
    assert summary['cases'] == len(corpus) == len(candidate['results']) == 3318
    assert reference == candidate and not summary['differences'] and not any(summary['difference_counts'].values())
    for case, result in zip(corpus, candidate['results'], strict=True):
        assert (case['name'], case['chunk_size']) == (result['name'], result['chunk_size'])
        assert sha(base64.b64decode(case['base64'])) == case['sha256'] and not result['result']['callback_errors']
    for library in summary['libraries'].values():
        assert library['returncode'] == 0
        assert library['sha256_before'] == library['sha256_after'] == identities[library['path']]
    parent = load('parent-execution-receipt.json')
    assert sha(get('parent-execution-receipt.json')) == '3848f4ed33967314f3ca17143efc8d54aaf1ba91aac534c061b4237826e4ad97'
    assert all(row['exit'] == 0 and row['reaped'] for row in parent['sessions'].values())
    assert parent['differential']['summary_sha256'] == sha(get(prefix + 'summary.json'))
    assert parent['differential']['log_sha256'] == sha(get('coordinate-checkpoint-guards-differential.log'))
    for engine, flag in [('oriole', '--library'), ('reference', '--reference')]:
        argv = parent['differential']['argv']
        assert argv[argv.index(flag) + 1] == summary['libraries'][engine]['path']
    assert '--strict' in argv and argv[argv.index('--generated') + 1] == '250'
    prefix = 'coordinate-checkpoint-guards-pgo-study/local-checks/'
    checks = load(prefix + 'summary.json')
    assert checks['source_file_count'] == 73
    for name, row in checks['reports'].items():
        assert sha(get(prefix + name)) == row['sha256']
        receipt = load(prefix + name)
        assert receipt == {k: value for k, value in row.items() if k != 'sha256'}
        assert receipt['returncode'] == 0
        assert receipt['command'][3:6] == ['cargo', '+ohm', '-Zohm-defaults=no']
        for stream in ['stdout', 'stderr']:
            data = get(prefix + name.removesuffix('.json') + '.' + stream)
            assert (sha(data), len(data)) == (row[stream]['sha256'], row[stream]['bytes'])
    for name, total in [('tests-attempt01', 444), ('doctests-attempt01', 1), ('focused-attempt01', 8)]:
        data = get(prefix + name + '.stdout').decode()
        counts = [tuple(map(int, row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', data)]
        assert sum(row[0] for row in counts) == total and all(row[1] == row[2] == 0 for row in counts)
    # Compare the two archived snapshots, so no live V1 worktree is required.
    source = load('coordinate-checkpoint-guards-pgo-study/source.json')['source_sha256']
    v1 = load('v1-parent/source.json')['source_sha256']
    assert len(v1) == len(source) == 73 and set(v1) == set(source)
    assert [name for name in source if source[name] != v1[name]] == ['crates/oriole/src/encoding.rs']
    seen = set()
    with tarfile.open(fileobj=io.BytesIO(get('v1-parent/source.tar.gz')), mode='r:gz') as archive:
        for member in archive:
            assert member.isfile() and member.name in v1 and member.name not in seen
            assert sha(archive.extractfile(member).read()) == v1[member.name]
            seen.add(member.name)
    assert seen == set(v1)
    review = load('coordinate-checkpoint-guards-source-review.json')
    assert sha(get('guard.patch')) == review['diff_sha256'] == '12102c9e7ea0e0328e10fa1e80e8ae0381b571bf2d366902e892c815729f2f6a'
    assert review['source_sha256'] == source['crates/oriole/src/encoding.rs']
    return {'pgo_exact_differential_cases': 3318, 'local_tests': 444, 'local_doctests': 1, 'focused_tests': 8,
            'source73_delta_vs_v1': ['crates/oriole/src/encoding.rs'],
            'scope': 'Saved differential rows, final local command/stream outcomes and archived source snapshots checked. Parent session receipts remain explicitly parent-reported. No normal native, CPython timing or V2 CI/Miri validation claimed.'}


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
                 'notices/oriole/LICENSE-APACHE',
                 'notices/oriole/LICENSE-MIT', 'notices/oriole/tests/c/UPSTREAM-NOTICES.txt'):
        assert len(get(name)) > 0
    native = 'coordinate-checkpoint-guards-native-study'
    audit = 'coordinate-checkpoint-guards-native-review'
    containers = {(mode, name): json.loads(get(f'{native}/{mode}/native-screen/{name}.json'))
                  for mode in ('pgo',) for name in ('preflight', 'results')}
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
    assert len(alias_list) == 672

    source = json.loads(get('coordinate-checkpoint-guards-pgo-study/source.json'))['source_sha256']
    assert len(source) == 73
    source_seen = set()
    with tarfile.open(fileobj=io.BytesIO(get('coordinate-checkpoint-guards-pgo-study/source.tar.gz')), mode='r:gz') as archive:
        for member in archive:
            assert member.isfile() and member.name in source and member.name not in source_seen
            assert sha(archive.extractfile(member).read()) == source[member.name]
            source_seen.add(member.name)
    assert source_seen == set(source)
    assert (root / 'replay_native.py').read_bytes() == get(audit + '/replay.py')
    assert (root / 'conditions.csv').read_bytes() == get(audit + '/conditions.csv')
    replay_view = json.loads(get(audit + '/replay-review.json'))
    assert get(audit + '/replay-review.json') == get(audit + '/pgo-review.json')
    assert sha(get(audit + '/pgo-review.json')) == '472b2c33032adc6eede82e285d006a1c1cc292ae70b4a2b4625d49d9bca09635'
    expected_csv = [['phase', 'group', 'project', 'chunk', 'namespaces', 'iterations',
                     'candidate_over_control', 'candidate_over_expat', 'control_over_expat']]
    for row in replay_view['native']['pgo']['all_conditions']:
        c = row['condition']
        assert row['median_ratios']['candidate_over_control'] > 1
        expected_csv.append([str(value) for value in ['pgo', 'generated' if c['name'].startswith('generated-') else 'real',
            c['name'], c['chunk'], c['namespaces'], c['iterations'],
            *[row['median_ratios'][k] for k in ['candidate_over_control', 'candidate_over_expat', 'control_over_expat']]]])
    assert list(csv.reader(io.StringIO(get(audit + '/conditions.csv').decode()))) == expected_csv
    assert len(expected_csv) == 29
    identities = {}
    for row in [*index['files'], *index['excluded_binaries_and_caches'], *index['native_worker_aliases']]:
        assert row['origin'] not in identities or identities[row['origin']] == row['sha256']
        identities[row['origin']] = row['sha256']
    validation = validation_records(get, identities)
    with tempfile.TemporaryDirectory(prefix='oriole-native-records-') as temporary:
        temp = Path(temporary)
        for mode in ('pgo',):
            folder = temp / mode / 'native-screen'
            folder.mkdir(parents=True)
            for name in ('protocol.json', 'preflight.json', 'results.json'):
                (folder / name).write_bytes(get(f'{native}/{mode}/native-screen/{name}'))
        (temp / 'review.json').write_bytes(get(audit + '/replay-review.json'))
        command = [sys.executable, '-I', '-S', str((root / 'replay_native.py').resolve()),
                   '--evidence-root', str(temp), '--review', str(temp / 'review.json'),
                   '--output', str(temp / 'replayed.json')]
        outcome = subprocess.run(command, capture_output=True, text=True, timeout=120, check=False)
        assert outcome.returncode == 0 and outcome.stderr == '', (outcome.returncode, outcome.stderr)
        replay = json.loads((temp / 'replayed.json').read_bytes())
    report = json.loads((root / 'report.json').read_bytes())
    assert report['archive_sha256'] == manifest['files']['evidence.tar.gz']['sha256']
    assert report['archive_index_sha256'] == sha(index_data)
    assert report['source_manifest_sha256'] == sha(get('coordinate-checkpoint-guards-pgo-study/source.json'))
    assert report['logical_files'] == len(records) and report['unique_files'] == len(objects)
    assert report['excluded_binaries_and_caches'] == len(index['excluded_binaries_and_caches'])
    assert report['native_worker_aliases'] == len(alias_list)
    assert report['native_totals']['workers'] == replay['totals']['workers'] == 672
    assert report['native_totals']['samples'] == replay['totals']['samples'] == 106176
    assert report['native'] == {mode: replay['modes'][mode]['groups'] for mode in ('pgo',)}
    receipt = {'status': 'passed', 'scope': 'Portable archive integrity, source snapshot, worker aliases, all28 CSV/native arithmetic and saved PGO differential/local-check replay; no targets or live dependencies checked.',
               'files_manifest_sha256': sha((root / 'files.json').read_bytes()),
               'archive_sha256': report['archive_sha256'], 'logical_files': len(records),
               'unique_objects': len(objects), 'excluded_identities': len(index['excluded_binaries_and_caches']),
               'worker_aliases': len(alias_list), 'source_files': len(source),
               'native': replay, 'validation': validation, 'reader_stdout': outcome.stdout}
    with args.output.open('x') as output:
        json.dump(receipt, output, indent=2)
        output.write('\n')
    print(json.dumps({'status': 'passed', 'receipt': str(args.output), 'sha256': sha(args.output.read_bytes()),
                      'objects': len(objects), 'aliases': len(alias_list), 'native_totals': replay['totals']}))


if __name__ == '__main__':
    main()
