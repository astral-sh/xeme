"""Package the rejected checkpoint-guard V2 prototype and saved results; no targets."""
import csv
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile

ROOT = Path(__file__).resolve().parent
BUILD = Path('/tmp/oriole-coordinate-checkpoint-guards-pgo-study')
NATIVE = 'coordinate-checkpoint-guards-native-study'
AUDIT = 'coordinate-checkpoint-guards-native-review'
REVIEWS = Path('/tmp/oriole-coordinate-checkpoint-guards-native-review')
WORK = Path('/home/dev-user/code/oss/oriole-coordinate-checkpoint-guards')
CONTROL = Path('/tmp/oriole-arena-capacity-bound-pgo-study')


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
    native = read(REVIEWS / 'pgo-review.json')
    assert native['status'] == 'passed' and set(native['native']) == {'pgo'}
    assert digest((REVIEWS / 'pgo-review.json').read_bytes()) == '472b2c33032adc6eede82e285d006a1c1cc292ae70b4a2b4625d49d9bca09635'
    (ROOT / 'replay-review.json').write_bytes((REVIEWS / 'pgo-review.json').read_bytes())
    build_review = read(Path('/tmp/oriole-coordinate-checkpoint-guards-build-review/review.json'))
    assert build_review['status'] == 'passed'
    assert digest(Path('/tmp/oriole-coordinate-checkpoint-guards-build-review/review.json').read_bytes()) == '238baac3be0e6a13165f1f4d7e6f5286fc249a90a2f865a13d2739d417014743'
    freeze = read(BUILD / 'pair-freeze.json')
    assert freeze['count'] == 90 and freeze['status'] == 'frozen'
    assert digest((BUILD / 'pair-freeze.json').read_bytes()) == '636224ca1f94645f9e85b310acc52c57dba5422e144bce769684b698e5ecfa18'
    sources = {}
    for name, identity in freeze['files'].items():
        path = BUILD / name
        data = path.read_bytes()
        assert (digest(data), len(data)) == (identity['sha256'], identity['bytes'])
        sources['coordinate-checkpoint-guards-pgo-study/' + name] = path
    sources['coordinate-checkpoint-guards-pgo-study/pair-freeze.json'] = BUILD / 'pair-freeze.json'
    for prefix, folder in [(NATIVE, Path('/tmp/oriole-' + NATIVE)),
                           (AUDIT, REVIEWS),
                           ('coordinate-checkpoint-guards-build-review', Path('/tmp/oriole-coordinate-checkpoint-guards-build-review')),
                           ('coordinate-checkpoint-guards-differential', Path('/tmp/oriole-coordinate-checkpoint-guards-differential'))]:
        for path in sorted(folder.rglob('*')):
            if path.is_file():
                sources[prefix + '/' + path.relative_to(folder).as_posix()] = path
    for name in ['source-review.json', 'native-prepare.py', 'differential.log']:
        sources['coordinate-checkpoint-guards-' + name] = Path('/tmp/oriole-coordinate-checkpoint-guards-' + name)
    sources[AUDIT + '/replay-review.json'] = ROOT / 'replay-review.json'
    sources['parent-execution-receipt.json'] = ROOT / 'parent-execution-receipt.json'
    sources['guard.patch'] = ROOT / 'guard.patch'
    assert digest((ROOT / 'guard.patch').read_bytes()) == read(Path('/tmp/oriole-coordinate-checkpoint-guards-source-review.json'))['diff_sha256']
    for name in ['tools/differential.py', 'tools/xml_abi.py', 'tools/corpus.py']:
        sources['reproduce/worktree/' + name] = WORK / name
    # Keep only the V1 inputs actually used to establish the V2 change and collector.
    for name in ['source.json', 'source.tar.gz']:
        sources['v1-parent/' + name] = Path('/tmp/oriole-lazy-coordinates-pgo-study') / name
    for name in ['native_screen.py', 'run.py']:
        sources['v1-parent/native/' + name] = Path('/tmp/oriole-lazy-coordinates-native-study/pgo') / name
    # Retain only the selected-control files used to establish the comparison,
    # not its older compatibility, publication or experimental review histories.
    pair = read(CONTROL / 'pair-report.json')
    manifest_path = Path(pair['pgo_manifest']['path'])
    for name in ['source.json', 'source.tar.gz', 'tool-source.json', 'pair-report.json',
                 'pair-freeze.json', 'vector-training-proof.json', 'normal-build.log', 'normal-training.json']:
        sources['selected-control/' + name] = CONTROL / name
    for name in ['manifest.json', '04-build-generate.log', '08-build-use.log', 'generate-training.json', 'use-training.json']:
        sources['selected-control/pgo/' + name] = manifest_path.parent / name
    for mode in ['pgo']:
        prior = Path('/tmp/oriole-arena-capacity-bound-native-study') / mode
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
    for mode in ['pgo']:
        protocol = read(Path('/tmp/oriole-' + NATIVE) / mode / 'native-screen/protocol.json')
        for engine, library in protocol['libraries'].items():
            sources['native-artifact-identities/' + mode + '/' + engine] = Path(library['path'])
        for item in protocol['conditions']:
            if item['name'].startswith('generated-'):
                sources['reproduce/generated/' + item['name'] + '.xml'] = Path(item['input'])
    for name in ['COPYING', 'expat/COPYING']:
        sources['notices/expat-2.8.4/' + name] = Path('/home/dev-user/.cache/oriole/upstream/expat-2.8.4') / name
    for name in ['LICENSE-APACHE', 'LICENSE-MIT', 'tests/c/UPSTREAM-NOTICES.txt']:
        sources['notices/oriole/' + name] = WORK / name
    for name in ['package.py', 'verify.py', 'replay_native.py', 'replay-adaptation.patch', 'verify-adaptation.patch', 'package-adaptation.patch']:
        sources['reproduce/' + name] = ROOT / name

    aliases = []
    alias_map = {}
    for mode in ['pgo']:
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
    assert len(aliases) == 672
    (ROOT / 'worker-aliases.json').write_bytes(encoded({'aliases': aliases,
        'scope': 'All original worker file bytes were compared before omission; exact JSON serialization reconstruction, not an assumed JSON-value alias.'}))
    sources[AUDIT + '/worker-aliases.json'] = ROOT / 'worker-aliases.json'
    sources[AUDIT + '/replay.py'] = ROOT / 'replay_native.py'
    data = (REVIEWS / 'pgo-conditions.csv').read_bytes()
    assert digest(data) == 'd6de78e45bad215a67f73e8563945a214bc95c91e093575e3e58aad70b6d6d16'
    (ROOT / 'conditions.csv').write_bytes(data)
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
    assert len(worker_aliases) == 672
    assert sum(map(len, objects.values())) <= 128 * 1024**2
    index = {'format': 'sha256-object-archive-v1', 'files': files, 'excluded_binaries_and_caches': excluded,
             'native_worker_aliases': worker_aliases, 'objects': len(objects), 'unique_bytes': sum(map(len, objects.values())),
             'alias_encoding': 'Remove observations/median_seconds; json.dumps(indent=2)+newline in original insertion order, UTF-8. Every reconstructed byte sequence was checked against its original worker file.',
             'cache_scope': 'Candidate reusable pgo/targets cache is not traversed; the frozen90-file build inventory supplies artifact identities. Older study archives/histories are not copied.'}
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
    report = {'status': 'assembled_rejected_experiment', 'decision': 'Experimental V2 rejected under the predeclared PGO-first gate: all 28 native conditions are slower than capacity-fixed PGO. No normal native or CPython timing collected.',
              'source_manifest_sha256': digest((BUILD / 'source.json').read_bytes()), 'source_files': 73,
              'runtime_delta_vs_v1': ['crates/oriole/src/encoding.rs'], 'guard_patch_sha256': digest((ROOT / 'guard.patch').read_bytes()),
              'candidate_commit': '421bc188622498fcdda60ac2b2a1461ee72b9245', 'v1_parent_commit': 'f766243bcc485ab305d339f5d37f095e0940eb49',
              'selected_control_commit': 'de859c89577b2982d59b6923d682032f6a7c7800',
              'local_tests': 444, 'local_doctests': 1, 'focused_tests': 8, 'actual_compiler_vectors': 9, 'generated_training_records': 864,
              'native': {'pgo': native['native']['pgo']['groups']},
              'native_totals': {'workers': 672, 'samples': 106176, 'measured_timed_samples': 105420, 'timed_warmups': 588, 'preflight_samples': 168},
              'correctness': {'pgo_exact_differential_cases': 3318},
              'archive_sha256': digest((ROOT / 'evidence.tar.gz').read_bytes()), 'archive_index_sha256': digest(index_bytes),
              'archive_bytes': (ROOT / 'evidence.tar.gz').stat().st_size, 'logical_files': len(files), 'unique_files': len(objects),
              'excluded_binaries_and_caches': len(excluded), 'native_worker_aliases': len(worker_aliases),
              'scope': 'One fixed PGO native campaign, rejected before normal native or CPython timing. Local 444 tests, 1 doc test and 8 focused tests passed after the final source edit. PGO 3,318 exact differential rows match the capacity-fixed parent. No V2 CI/Miri, full upstream API/PBS/fuzz or strict CPython claim. Retained source/guard diff, matched normal+PGO build provenance, first outcomes and source/build/native reviews.'}
    (ROOT / 'report.json').write_bytes(encoded(report))
    names = ['README.md', 'package.py', 'verify.py', 'replay_native.py', 'replay-adaptation.patch', 'verify-adaptation.patch', 'package-adaptation.patch',
             'parent-execution-receipt.json', 'guard.patch', 'replay-review.json', 'evidence.tar.gz', 'archive-index.json', 'report.json', 'conditions.csv', 'worker-aliases.json']
    seal = {name: {'sha256': digest((ROOT / name).read_bytes()), 'bytes': (ROOT / name).stat().st_size} for name in names}
    (ROOT / 'files.json').write_bytes(encoded({'format': 'sha256-file-manifest-v1', 'files': seal}))
    print(json.dumps({k: report[k] for k in ['status', 'archive_sha256', 'archive_bytes', 'logical_files', 'unique_files', 'native_worker_aliases']}))


if __name__ == '__main__':
    main()
