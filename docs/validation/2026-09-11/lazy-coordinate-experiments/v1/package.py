"""Package the rejected lazy-coordinate V1 prototype and saved results; no targets."""
import csv
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile

ROOT = Path(__file__).resolve().parent
BUILD = Path('/tmp/oriole-lazy-coordinates-pgo-study')
NATIVE = 'lazy-coordinates-native-study'
AUDIT = 'lazy-coordinates-native-review'
REVIEWS = Path('/tmp/oriole-lazy-coordinates-native-review')
WORK = Path('/home/dev-user/code/oss/oriole-lazy-coordinates')
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
    combined = read(REVIEWS / 'review.json')
    native = {'status': 'passed', 'native': {}, 'input_sha256': combined['input_sha256'],
              'scope': 'Derived replay view; original combined and phase audits are retained byte for byte.',
              'combined_review_sha256': digest((REVIEWS / 'review.json').read_bytes()), 'phase_review_sha256': {}}
    for mode in ['normal', 'pgo']:
        report = read(REVIEWS / (mode + '-review.json'))
        assert report['status'] == 'passed' and set(report['native']) == {mode}
        native['native'][mode] = report['native'][mode]
        native['phase_review_sha256'][mode] = digest((REVIEWS / (mode + '-review.json')).read_bytes())
    (ROOT / 'replay-review.json').write_bytes(encoded(native))
    build_review = read(Path('/tmp/oriole-lazy-coordinates-build-review/review.json'))
    assert combined['status'] == build_review['status'] == 'passed'
    assert native['combined_review_sha256'] == '940ce21940de41bad01d4f6ab2962ac54c9a866e754f53ce3c833d541bdb55e5'
    assert digest(Path('/tmp/oriole-lazy-coordinates-build-review/review.json').read_bytes()) == '7e7cc21c85c100f598448eaf055346fa27c51e76a28640ff2751f3572d8d3923'
    freeze = read(BUILD / 'pair-freeze.json')
    assert freeze['count'] == 112 and freeze['status'] == 'frozen'
    assert digest((BUILD / 'pair-freeze.json').read_bytes()) == '99e5efdf07a93c6effcb4a9024371dac29b0e07d4aff68df063bbda56ee2d552'
    sources = {}
    for name, identity in freeze['files'].items():
        path = BUILD / name
        data = path.read_bytes()
        assert (digest(data), len(data)) == (identity['sha256'], identity['bytes'])
        sources['lazy-coordinates-pgo-study/' + name] = path
    sources['lazy-coordinates-pgo-study/pair-freeze.json'] = BUILD / 'pair-freeze.json'
    for prefix, folder in [(NATIVE, Path('/tmp/oriole-' + NATIVE)),
                           (AUDIT, REVIEWS),
                           ('lazy-coordinates-build-review', Path('/tmp/oriole-lazy-coordinates-build-review')),
                           ('lazy-coordinates-correctness', Path('/tmp/oriole-lazy-coordinates-correctness')),
                           ('lazy-coordinates-ci', Path('/tmp/oriole-lazy-coordinates-ci'))]:
        for path in sorted(folder.rglob('*')):
            if path.is_file():
                sources[prefix + '/' + path.relative_to(folder).as_posix()] = path
    for name in ['correctness-plan.json', 'correctness-plan.md', 'correctness-run.py',
                 'correctness-review.json', 'correctness-audit.py', 'source-review.json', 'source-review.md']:
        sources['lazy-coordinates-' + name] = Path('/tmp/oriole-lazy-coordinates-' + name)
    assert digest(sources['lazy-coordinates-correctness-review.json'].read_bytes()) == '969ded35fe5d47116f9c03e52da746c70b028e60621ce963d6ea6efc1a00fa16'
    assert digest(Path('/tmp/oriole-lazy-coordinates-ci/readback.json').read_bytes()) == '116446678a086c63c8dd974d2ed102cc81bf1b6a2a2cfb2404331e764cb6d686'
    sources['lazy-coordinates-ci/ci.yml'] = WORK / '.github/workflows/ci.yml'
    assert digest(sources['lazy-coordinates-ci/ci.yml'].read_bytes()) == '49f2b4c7b9f171fbf7b1e1204f61ca41a9123c06293e1df2a084d307ffbba539'
    sources[AUDIT + '/replay-review.json'] = ROOT / 'replay-review.json'
    sources['parent-execution-receipt.json'] = ROOT / 'parent-execution-receipt.json'
    # Retain prerequisite harness/consumer-fix bytes, not a temporary CPython source tree.
    plan = read(Path('/tmp/oriole-lazy-coordinates-correctness-plan.json'))
    for name, expected in plan['prerequisite_file_sha256'].items():
        path = Path(name)
        assert digest(path.read_bytes()) == expected
        logical = ('reproduce/worktree/' + path.relative_to(WORK).as_posix()) if path.is_relative_to(WORK) else ('reproduce/prerequisites/' + path.name)
        assert logical not in sources or sources[logical] == path
        sources[logical] = path
    for linkage in ['shared', 'static']:
        for name in ['tests.log', 'summary.json']:
            sources['historical-strict/' + linkage + '/' + name] = Path('/tmp/oriole-context-text-frame-fixed-thin-pgo-cpython-gates') / linkage / name
    sources['historical-strict/method-maps.json'] = Path('/tmp/oriole-context-text-frame-fixed-strict-outcome-review/method-maps.json')
    sources['historical-strict/method-reader.py'] = Path('/tmp/oriole-native-byte-count-thinlto-cpython-root-review/audit.py')
    sources['notices/cpython-3.12.13/LICENSE'] = Path('/home/dev-user/.cache/oriole/upstream/cpython-3.12.13/LICENSE')
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
    for mode in ['normal', 'pgo']:
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
    data = (REVIEWS / 'conditions.csv').read_bytes()
    assert digest(data) == '4780ddec401af565af6edc3cfd07a8ef3954e358b2c0ce05290f0ea4459cf07d'
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
    assert len(worker_aliases) == 1344
    assert sum(map(len, objects.values())) <= 128 * 1024**2
    index = {'format': 'sha256-object-archive-v1', 'files': files, 'excluded_binaries_and_caches': excluded,
             'native_worker_aliases': worker_aliases, 'objects': len(objects), 'unique_bytes': sum(map(len, objects.values())),
             'alias_encoding': 'Remove observations/median_seconds; json.dumps(indent=2)+newline in original insertion order, UTF-8. Every reconstructed byte sequence was checked against its original worker file.',
             'cache_scope': 'Candidate reusable pgo/targets cache is not traversed; the frozen112-file build inventory supplies artifact identities. Older study archives/histories are not copied.'}
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
    report = {'status': 'assembled_rejected_experiment', 'decision': 'Experimental V1 rejected: every one of 56 native conditions is slower than the capacity-fixed control.',
              'source_manifest_sha256': digest((BUILD / 'source.json').read_bytes()), 'source_files': 73,
              'runtime_delta': ['crates/oriole/src/' + n for n in ['arena.rs', 'dtd.rs', 'encoding.rs', 'lib.rs', 'recycling.rs']] + ['crates/oriole_expat/src/lib.rs'],
              'candidate_commit': 'f766243bcc485ab305d339f5d37f095e0940eb49',
              'selected_control_commit': 'de859c89577b2982d59b6923d682032f6a7c7800',
              'local_tests': 443, 'local_doctests': 1, 'actual_compiler_vectors': 9, 'generated_training_records': 864,
              'native': {mode: native['native'][mode]['groups'] for mode in ['normal', 'pgo']},
              'native_totals': {'workers': 1344, 'samples': 212352, 'measured_timed_samples': 210840, 'timed_warmups': 1176, 'preflight_samples': 336},
              'correctness': {'commands': 30, 'differential_cases_per_mode': 3318, 'c_consumers': 12,
                              'strict_runs': 4, 'strict_methods_per_run': 802, 'known_strict_failures_per_run': 2},
              'ci': {'run': 34621512017, 'checks_passed': 16, 'miri_tests_total': 96},
              'archive_sha256': digest((ROOT / 'evidence.tar.gz').read_bytes()), 'archive_index_sha256': digest(index_bytes),
              'archive_bytes': (ROOT / 'evidence.tar.gz').stat().st_size, 'logical_files': len(files), 'unique_files': len(objects),
              'excluded_binaries_and_caches': len(excluded), 'native_worker_aliases': len(worker_aliases),
              'scope': 'One fixed native campaign per mode; no elapsed benefit. Local443 tests and1doc test preceded a test-only strengthened allocation witness, followed by focused1 test/Clippy/fmt. Strict CPython retains2 failures per run and compares historical Context method outcomes. C ASan/UBSan consumers only, uninstrumented Rust libraries, leak checks disabled. CI96 Miri tests; no full upstream API/PBS/fuzz or Python elapsed rerun. All candidate build/check/audit/native first outcomes retained, including the first audit caption correction.'}
    (ROOT / 'report.json').write_bytes(encoded(report))
    names = ['README.md', 'package.py', 'verify.py', 'replay_native.py', 'replay-adaptation.patch', 'verify-adaptation.patch', 'package-adaptation.patch',
             'parent-execution-receipt.json', 'replay-review.json', 'evidence.tar.gz', 'archive-index.json', 'report.json', 'conditions.csv', 'worker-aliases.json']
    seal = {name: {'sha256': digest((ROOT / name).read_bytes()), 'bytes': (ROOT / name).stat().st_size} for name in names}
    (ROOT / 'files.json').write_bytes(encoded({'format': 'sha256-file-manifest-v1', 'files': seal}))
    print(json.dumps({k: report[k] for k in ['status', 'archive_sha256', 'archive_bytes', 'logical_files', 'unique_files', 'native_worker_aliases']}))


if __name__ == '__main__':
    main()
