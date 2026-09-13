"""Package the unselected namespace guard and saved results; no targets."""
import csv
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile

ROOT = Path(__file__).resolve().parent
BUILD = Path('/tmp/oriole-default-namespace-guard-pgo-study')
NATIVE = 'default-namespace-guard-native-study'
AUDIT = 'default-namespace-guard-independent-review/native'
REVIEWS = Path('/tmp/oriole-default-namespace-guard-independent-review')
WORK = Path('/home/dev-user/code/oss/oriole-default-namespace-guard')
CONTROL = Path('/tmp/oriole-context-text-frame-fixed-pgo-study')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2) + '\n').encode()


def read(path):
    return json.loads(path.read_bytes())


def main():
    if not __debug__:
        raise RuntimeError('Assertions must remain enabled')
    assert not (ROOT / 'evidence.tar.gz').exists()
    native = read(REVIEWS / 'native/review.json')
    build_review = read(REVIEWS / 'build/review.json')
    assert native['status'] == build_review['status'] == 'passed'
    assert digest((REVIEWS / 'native/review.json').read_bytes()) == 'df0c80bac70689b2c4fcd906d4e37eeabc13f868f509eda3bfa12ac94c0f1a19'
    assert digest((REVIEWS / 'build/review.json').read_bytes()) == 'fdc59331aff03afa911db6fd83fec5e3d20cd0dbb7d6984c064050735c96def5'
    freeze = read(BUILD / 'pair-freeze.json')
    assert freeze['count'] == 73 and freeze['status'] == 'frozen'
    assert digest((BUILD / 'pair-freeze.json').read_bytes()) == '6d4984187aff0646784b6ee383367607ec98675a4a2b5323feb49fb266fbfb00'
    sources = {}
    for name, identity in freeze['files'].items():
        path = BUILD / name
        data = path.read_bytes()
        assert (digest(data), len(data)) == (identity['sha256'], identity['bytes'])
        sources['default-namespace-guard-pgo-study/' + name] = path
    sources['default-namespace-guard-pgo-study/pair-freeze.json'] = BUILD / 'pair-freeze.json'
    for prefix, folder in [(NATIVE, Path('/tmp/oriole-' + NATIVE)),
                           ('default-namespace-guard-independent-review', REVIEWS)]:
        for path in sorted(folder.rglob('*')):
            if path.is_file():
                sources[prefix + '/' + path.relative_to(folder).as_posix()] = path
    # Retain only the selected-control files used to establish the comparison,
    # not its older compatibility, publication or experimental review histories.
    pair = read(CONTROL / 'pair-report.json')
    manifest_path = Path(pair['pgo_manifest']['path'])
    for name in ['source.json', 'source.tar.gz', 'tool-source.json', 'pair-report.json',
                 'pair-freeze.json', 'vector-training-proof.json', 'normal-build.log', 'normal-training.json']:
        sources['selected-control/' + name] = CONTROL / name
    for name in ['manifest.json', '04-build-generate.log', '08-build-use.log', 'generate-training.json', 'use-training.json']:
        sources['selected-control/pgo/' + name] = manifest_path.parent / name
    for mode in ['normal', 'pgo']:
        prior = Path('/tmp/oriole-context-text-frame-fixed-native-study') / mode
        for name in ['native_screen.py', 'run.py', 'adaptation.json', 'native-screen/protocol.json']:
            sources['selected-control/native/' + mode + '/' + name] = prior / name
    corpus = Path('/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json')
    sources['reproduce/benchmarks/projects/corpus-manifest.json'] = corpus
    for project in read(corpus)['projects']:
        for item in project['files']:
            if item['role'] not in ['input', 'license', 'notice']:
                continue
            path = corpus.parent / item['path']
            assert digest(path.read_bytes()) == item['sha256']
            sources['reproduce/benchmarks/projects/' + item['path']] = path
    sources['reproduce/benchmarks/native_driver.c'] = Path('/home/dev-user/code/oss/oriole/benchmarks/native_driver.c')
    for mode in ['normal', 'pgo']:
        protocol = read(Path('/tmp/oriole-' + NATIVE) / mode / 'native-screen/protocol.json')
        for item in protocol['conditions']:
            if item['name'].startswith('generated-'):
                sources['reproduce/generated/' + item['name'] + '.xml'] = Path(item['input'])
    for name in ['COPYING', 'expat/COPYING']:
        sources['notices/expat-2.8.4/' + name] = Path('/home/dev-user/.cache/oriole/upstream/expat-2.8.4') / name
    for name in ['LICENSE-APACHE', 'LICENSE-MIT', 'tests/c/UPSTREAM-NOTICES.txt']:
        sources['notices/oriole/' + name] = WORK / name
    for name in ['package.py', 'verify.py', 'replay_native.py', 'replay-adaptation.patch', 'verify-adaptation.patch']:
        sources['reproduce/' + name] = ROOT / name

    aliases = []
    alias_map = {}
    for mode in ['normal', 'pgo']:
        folder = Path('/tmp/oriole-' + NATIVE) / mode / 'native-screen'
        ordinal = 0
        for kind in ['preflight', 'results']:
            container = read(folder / (kind + '.json'))
            assert container['status'] == 'passed'
            for row_index, row in enumerate(container['rows']):
                rows = [(None, row)] if kind == 'preflight' else list(enumerate(row['processes']))
                for process_index, process in rows:
                    raw = {k: v for k, v in process.items() if k not in ['observations', 'median_seconds']}
                    data = encoded(raw)
                    path = folder / f'worker-{ordinal:04d}.json'
                    assert path.read_bytes() == data
                    logical = f'{NATIVE}/{mode}/native-screen/worker-{ordinal:04d}.json'
                    identity = {'sha256': digest(data), 'bytes': len(data)}
                    alias_map[logical] = identity | {'container': f'{NATIVE}/{mode}/native-screen/{kind}.json',
                        'row_index': row_index, 'process_index': process_index}
                    aliases.append([mode, ordinal, kind, row_index, process_index, len(data), digest(data)])
                    ordinal += 1
        assert ordinal == 672
    assert len(aliases) == 1344
    (ROOT / 'worker-aliases.json').write_bytes(encoded({'aliases': aliases,
        'scope': 'All original worker file bytes were compared before omission; exact JSON serialization reconstruction, not an assumed JSON-value alias.'}))
    sources[AUDIT + '/worker-aliases.json'] = ROOT / 'worker-aliases.json'
    sources[AUDIT + '/replay.py'] = ROOT / 'replay_native.py'
    with (ROOT / 'conditions.csv').open('x', newline='') as output:
        writer = csv.writer(output, lineterminator='\n')
        writer.writerow(['mode', 'name', 'chunk', 'namespaces', 'candidate_over_control', 'candidate_over_expat', 'control_over_expat'])
        for mode in ['normal', 'pgo']:
            for row in native['native'][mode]['all_conditions']:
                condition = row['condition']
                writer.writerow([mode, condition['name'], condition['chunk'], condition['namespaces'],
                    *[row['median_ratios'][key] for key in ['candidate_over_control', 'candidate_over_expat', 'control_over_expat']]])
    sources[AUDIT + '/conditions.csv'] = ROOT / 'conditions.csv'
    files, excluded, worker_aliases, objects = [], [], [], {}
    total_read = 0
    for logical, path in sorted(sources.items()):
        data = path.read_bytes()
        total_read += len(data)
        assert total_read <= 256 * 1024**2
        item = {'path': logical, 'origin': str(path), 'sha256': digest(data), 'bytes': len(data)}
        if logical in alias_map:
            target = alias_map[logical]
            assert (item['sha256'], item['bytes']) == (target['sha256'], target['bytes'])
            worker_aliases.append(item | {'reconstruct': target})
        elif data.startswith((b'\x7fELF', b'!<arch>\n')) or path.suffix in {'.pyc', '.o', '.rlib', '.rmeta'} or '__pycache__' in path.parts:
            excluded.append(item | {'reason': 'Compiled artifact identity only; omitted bytes.'})
        else:
            objects.setdefault(item['sha256'], data)
            files.append(item)
    assert len(worker_aliases) == 1344
    assert sum(map(len, objects.values())) <= 128 * 1024**2
    index = {'format': 'sha256-object-archive-v1', 'files': files, 'excluded_binaries_and_caches': excluded,
             'native_worker_aliases': worker_aliases, 'objects': len(objects), 'unique_bytes': sum(map(len, objects.values())),
             'alias_encoding': 'Remove observations/median_seconds; json.dumps(indent=2)+newline in original insertion order, UTF-8. Every reconstructed byte sequence was checked against its original worker file.',
             'cache_scope': 'Candidate reusable pgo/targets cache is not traversed; the frozen73-file build inventory supplies artifact identities. Older study archives/histories are not copied.'}
    index_bytes = encoded(index)
    with (ROOT / 'evidence.tar.gz').open('xb') as raw:
        with gzip.GzipFile(fileobj=raw, mode='wb', mtime=0, filename='') as gz:
            with tarfile.open(fileobj=gz, mode='w') as archive:
                for name, data in [('index.json', index_bytes), *[(f'objects/{h}', d) for h, d in sorted(objects.items())]]:
                    member = tarfile.TarInfo(name)
                    member.size = len(data)
                    member.mode = 0o644
                    archive.addfile(member, io.BytesIO(data))
    assert (ROOT / 'evidence.tar.gz').stat().st_size <= 32 * 1024**2
    (ROOT / 'archive-index.json').write_bytes(index_bytes)
    report = {'status': 'assembled_negative_experiment', 'decision': 'Unselected: both normal and PGO real-project aggregate ratios regressed.',
              'source_manifest_sha256': digest((BUILD / 'source.json').read_bytes()), 'source_files': 72,
              'runtime_delta': ['crates/oriole/src/lib.rs'], 'test_delta': ['crates/oriole/tests/adapter_frame.rs'],
              'selected_runtime_unchanged': '0f28139f83b04290e62301c38d8ed489f183a88d', 'candidate_base': 'ef0eac091999c2e29e888918ac958480dbdacb4c',
              'local_tests': 438, 'actual_compiler_vectors': 9, 'generated_training_records': 864,
              'native': {mode: native['native'][mode]['groups'] for mode in ['normal', 'pgo']},
              'native_totals': {'workers': 1344, 'samples': 212352, 'measured_timed_samples': 210840, 'timed_warmups': 1176, 'preflight_samples': 336},
              'archive_sha256': digest((ROOT / 'evidence.tar.gz').read_bytes()), 'archive_index_sha256': digest(index_bytes),
              'archive_bytes': (ROOT / 'evidence.tar.gz').stat().st_size, 'logical_files': len(files), 'unique_files': len(objects),
              'excluded_binaries_and_caches': len(excluded), 'native_worker_aliases': len(worker_aliases),
              'scope': 'One fixed native campaign per mode. No fresh Python elapsed, full compatibility, sanitizer or Miri campaign for this rejected optimization. All candidate preparation/build/check/audit/native first outcomes retained; older evidence retained only where needed as a comparison input.'}
    (ROOT / 'report.json').write_bytes(encoded(report))
    names = ['README.md', 'package.py', 'verify.py', 'replay_native.py', 'replay-adaptation.patch', 'verify-adaptation.patch',
             'evidence.tar.gz', 'archive-index.json', 'report.json', 'conditions.csv', 'worker-aliases.json']
    seal = {name: {'sha256': digest((ROOT / name).read_bytes()), 'bytes': (ROOT / name).stat().st_size} for name in names}
    (ROOT / 'files.json').write_bytes(encoded({'format': 'sha256-file-manifest-v1', 'files': seal}))
    print(json.dumps({k: report[k] for k in ['status', 'archive_sha256', 'archive_bytes', 'logical_files', 'unique_files', 'native_worker_aliases']}))


if __name__ == '__main__':
    main()
