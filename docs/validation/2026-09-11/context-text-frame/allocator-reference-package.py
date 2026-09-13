"""Assemble saved allocator evidence, excluding binaries and duplicate workers.

Run only after the parent releases saved-data assembly. This does not execute a
compiler, parser or timing controller. Original producer evidence is read-only.
"""
from pathlib import Path
import gzip
import hashlib
import io
import json
import tarfile

ROOT = Path(__file__).resolve().parent
WORK = Path('/home/dev-user/code/oss/oriole-allocator-provenance')
NATIVE = 'allocator-provenance-native-study'
AUDIT = 'allocator-provenance-native-independent-review'
STUDIES = {name: Path('/tmp/oriole-' + name) for name in (
    'allocator-provenance-study', 'allocator-provenance-pgo-study',
    'allocator-provenance-build-independent-review', AUDIT,
    'allocator-provenance-compatibility-independent-review',
    'allocator-provenance-thin-pgo-gates',
    'allocator-provenance-thin-pgo-cpython-gates',
    'allocator-provenance-thin-pgo-cpython-preparation',
    'allocator-provenance-strict-outcome-review', NATIVE,
)}


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
    checks = read(STUDIES['allocator-provenance-study'] / 'checks-fix01/results.json')
    ci = read(STUDIES['allocator-provenance-study'] / 'fixed-ci.json')
    assert checks['status'] == 'passed'
    assert ci['conclusion'] == 'success'
    assert ci['headSha'] == 'e1d263a711e862e4f0f0018b15de9ac83a87a342'
    assert len(ci['jobs']) == 14 and all(x['conclusion'] == 'success' for x in ci['jobs'])
    assert read(STUDIES['allocator-provenance-pgo-study'] / 'pair-report.json')['status'] == 'passed'
    gates = read(STUDIES['allocator-provenance-thin-pgo-gates'] / 'summary.json')
    assert gates['api']['all4740rows_exact']
    assert (gates['api']['passed'], gates['api']['failed']) == (4347, 393)
    strict = read(STUDIES['allocator-provenance-strict-outcome-review'] / 'review.json')
    for row in strict['cpython'].values():
        assert row['methods'] == 802 and row['raw_exit'] == 2
        assert row['all_method_outcomes_equal_selected_patched_consumer_baseline']
    native = read(STUDIES[AUDIT] / 'review.json')
    assert native['status'] == 'passed'
    for mode in ('normal', 'pgo'):
        assert read(STUDIES[NATIVE] / mode / 'controller.json')['status'] == 'passed'

    sources = {}
    for prefix, folder in STUDIES.items():
        assert folder.is_dir(), folder
        for path in sorted(folder.rglob('*')):
            if path.is_file():
                sources[prefix + '/' + path.relative_to(folder).as_posix()] = path
    for name in ('oriole-allocation-prefix-independent-review.json',
                 'oriole-allocator-provenance-fix-independent-review.json',
                 'oriole-allocator-provenance-cgate-outcome-review.json'):
        sources['reviews/' + name] = Path('/tmp') / name
    for name in ('COPYING', 'expat/COPYING'):
        sources['notices/expat-2.8.4/' + name] = Path('/home/dev-user/.cache/oriole/upstream/expat-2.8.4') / name
    sources['notices/cpython-3.12.13/LICENSE'] = Path('/home/dev-user/.cache/oriole/upstream/cpython-3.12.13/LICENSE')
    for name in ('LICENSE-APACHE', 'LICENSE-MIT', 'tests/c/UPSTREAM-NOTICES.txt'):
        sources['notices/oriole/' + name] = WORK / name
    # These source helpers are outside the measured 70-file runtime map.
    for path in sorted((WORK / 'tools/cpython').glob('*')):
        if path.is_file():
            sources['reproduce/oriole/tools/cpython/' + path.name] = path
    for name in ('provenance.json', 'cpython-3.12.13-external-parser.patch'):
        sources['reproduce/oriole/consumer-fix/' + name] = WORK / 'integration/python-build-standalone/consumer-fix' / name
    for name in ('root-draft.py', 'package.py', 'verify.py', 'replay_native.py'):
        sources['reproduce/' + name] = ROOT / name

    alias_rows = read(STUDIES[AUDIT] / 'worker-aliases.json')['aliases']
    assert len(alias_rows) == 1344
    alias_map = {}
    for mode, ordinal, container, row, process, size, sha in alias_rows:
        logical = f'{NATIVE}/{mode}/native-screen/worker-{ordinal:04d}.json'
        assert logical not in alias_map
        alias_map[logical] = {'container': f'{NATIVE}/{mode}/native-screen/{container}.json',
                              'row_index': row, 'process_index': process,
                              'sha256': sha, 'bytes': size}

    files, excluded, aliases, objects = [], [], [], {}
    total_read = 0
    for logical, path in sorted(sources.items()):
        data = path.read_bytes()
        total_read += len(data)
        assert total_read <= 2 * 1024**3
        item = {'path': logical, 'origin': str(path), 'sha256': digest(data), 'bytes': len(data)}
        if logical in alias_map:
            target = alias_map[logical]
            assert (item['sha256'], item['bytes']) == (target['sha256'], target['bytes'])
            aliases.append(item | {'reconstruct': target})
            continue
        if ('__pycache__' in path.parts or 'targets' in path.parts
                or data.startswith((b'\x7fELF', b'!<arch>\n'))
                or path.suffix in {'.pyc', '.o', '.rlib', '.rmeta'}):
            excluded.append(item | {'reason': 'Compiled artifact or reusable build cache; identity only.'})
            continue
        objects.setdefault(item['sha256'], data)
        files.append(item)
    assert len(aliases) == 1344
    assert sum(len(x) for x in objects.values()) <= 256 * 1024**2
    index = {'format': 'sha256-object-archive-v1', 'files': files,
             'excluded_binaries_and_caches': excluded, 'native_worker_aliases': aliases,
             'alias_encoding': {'remove_fields': ['observations', 'median_seconds'],
                                'serialization': 'json.dumps(record, indent=2) + newline, UTF-8; insertion order and default ensure_ascii=True'},
             'objects': len(objects), 'unique_bytes': sum(map(len, objects.values()))}
    index_bytes = encoded(index)
    with (ROOT / 'evidence.tar.gz').open('xb') as raw:
        with gzip.GzipFile(fileobj=raw, mode='wb', mtime=0, filename='') as gz:
            with tarfile.open(fileobj=gz, mode='w') as bundle:
                for name, data in [('index.json', index_bytes), *[(f'objects/{h}', d) for h, d in sorted(objects.items())]]:
                    member = tarfile.TarInfo(name)
                    member.size = len(data)
                    member.mode = 0o644
                    bundle.addfile(member, io.BytesIO(data))
    assert (ROOT / 'evidence.tar.gz').stat().st_size <= 128 * 1024**2
    (ROOT / 'archive-index.json').write_bytes(index_bytes)
    (ROOT / 'conditions.csv').write_bytes((STUDIES[AUDIT] / 'conditions.csv').read_bytes())
    report = {
        'status': 'assembled_saved_evidence', 'runtime_commit': ci['headSha'],
        'source_manifest_sha256': digest((STUDIES['allocator-provenance-pgo-study'] / 'source.json').read_bytes()),
        'source_files': 70, 'runtime_delta': ['crates/oriole_storage/src/allocator.rs'],
        'local_tests': 143, 'miri_storage_tests_per_model': 34,
        'api': {'rows': 4740, 'passed': 4347, 'assertion_failures': 391, 'timeouts': 2,
                'raw_exit': 1, 'all_outcomes_match_finder': True},
        'cpython': strict['cpython'],
        'native': {mode: native['native'][mode]['groups'] for mode in ('normal', 'pgo')},
        'native_totals': {'workers': 1344, 'samples': 212352,
                          'measured_timed_samples': 210840, 'timed_warmups': 1176, 'preflight_samples': 336},
        'archive_sha256': digest((ROOT / 'evidence.tar.gz').read_bytes()),
        'archive_index_sha256': digest(index_bytes),
        'archive_bytes': (ROOT / 'evidence.tar.gz').stat().st_size,
        'logical_files': len(files), 'unique_files': len(objects),
        'excluded_binaries_and_caches': len(excluded), 'native_worker_aliases': len(aliases),
        'scope': 'Required allocator correctness repair. Native elapsed only; no fresh Python elapsed or PBS claim. All baseline failures, raw warnings and producer/reader first outcomes retained. Compatibility parity is not an all-green upstream suite. Exposed-provenance Miri is incomplete.',
    }
    (ROOT / 'report.json').write_bytes(encoded(report))
    seal = {name: {'sha256': digest((ROOT / name).read_bytes()), 'bytes': (ROOT / name).stat().st_size}
            for name in ('README.md', 'package.py', 'root-draft.py', 'verify.py', 'replay_native.py',
                         'evidence.tar.gz', 'archive-index.json', 'report.json', 'conditions.csv')}
    (ROOT / 'files.json').write_bytes(encoded({'format': 'sha256-file-manifest-v1', 'files': seal}))
    print(json.dumps({k: report[k] for k in ('status', 'archive_sha256', 'archive_bytes', 'logical_files',
                                           'unique_files', 'excluded_binaries_and_caches', 'native_worker_aliases')}))


if __name__ == '__main__':
    main()
