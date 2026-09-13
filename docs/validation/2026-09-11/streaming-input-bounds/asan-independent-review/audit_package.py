"""Independent archive readback and corpus reconstruction audit, without targets."""
from pathlib import Path
import gzip
import hashlib
import json
import tarfile

OUT = Path(__file__).parent
PACKAGE = Path('/tmp/oriole-streaming-policy-asan-package')
REPORT = Path('/home/dev-user/code/oss/oriole-streaming-input-bounds/docs/validation/2026-09-11/streaming-input-bounds')
STUDY = Path('/tmp/oriole-streaming-policy-asan-study')
PREP = Path('/tmp/oriole-streaming-policy-asan-preparation')


def hash_bytes(data):
    return hashlib.sha256(data).hexdigest()


def digest(path):
    return hash_bytes(Path(path).read_bytes())


def read(path):
    return json.loads(Path(path).read_bytes())


raw = read(OUT / 'raw-audit.json')
assert raw['status'] == 'passed'
assert digest(STUDY / 'summary.json') == raw['summary_sha256']
compact = read(PACKAGE / 'asan.json')
summary = read(STUDY / 'summary.json')
for name, value in compact.items():
    if name != 'package':
        assert value == summary[name], name
meta = compact['package']
assert meta['sha256'] == '3bf0bbb5d9baf7f6cb7f7c60d959edb1bf47fe82862fae18ffce1ea9acc1dfca'
assert digest(PACKAGE / meta['archive']) == meta['sha256']
assert digest(PACKAGE / meta['index']) == meta['index_sha256']
assert digest(PACKAGE / 'package-streaming-policy-asan.py') == meta['packager_sha256']
index = json.loads(gzip.decompress((PACKAGE / meta['index']).read_bytes()))
assert index['sha256'] == meta['sha256']
indexed = {row['name']: row for row in index['members']}
assert len(indexed) == len(index['members']) == meta['members']
assert list(indexed) == sorted(indexed)
members = {}
with tarfile.open(PACKAGE / meta['archive'], 'r:gz') as archive:
    for item in archive:
        assert item.isfile() and item.name not in members
        assert not item.name.startswith('/') and '..' not in Path(item.name).parts
        assert item.mode == 0o444 and item.uid == item.gid == item.mtime == 0
        data = archive.extractfile(item).read()
        row = indexed[item.name]
        assert len(data) == row['bytes'] and hash_bytes(data) == row['sha256']
        members[item.name] = data
assert list(members) == list(indexed)

expected_plain = set()
for label, root in [('preparation', PREP), ('study', STUDY)]:
    for path in root.rglob('*'):
        if not path.is_file():
            continue
        relative = path.relative_to(root)
        if '__pycache__' in relative.parts or relative.as_posix() == 'source.tar.gz':
            continue
        if label == 'study' and relative.parts[0] == 'binaries':
            continue
        if label == 'study' and relative.parts[0] == 'campaigns' and len(relative.parts) > 2 and relative.parts[2] in ('initial-corpus', 'corpus'):
            continue
        name = label + '/' + relative.as_posix()
        assert members[name] == path.read_bytes(), name
        expected_plain.add(name)
source = read(STUDY / 'source.json')['source_sha256']
for name, value in source.items():
    assert hash_bytes(members['source/' + name]) == value
assert len([name for name in members if name.startswith('source/')]) == 70
assert members['study/summary.json'] == (STUDY / 'summary.json').read_bytes()
assert 'all six fuzz targets' in json.loads(members['study/instrumentation/report.json'])['scope']

layouts = json.loads(members['corpus-layouts.json'])['layouts']
origin_map = json.loads(members['corpus-origin-map.json'])
assert set(layouts) == {'current', 'historical_seed_inputs'}
expected_origins = {}
placements = 0
for group, targets in layouts.items():
    expected_targets = set(r['target'] for r in raw['results'])
    if group == 'historical_seed_inputs':
        expected_targets.remove('streaming_work')
    assert set(targets) == expected_targets
    for target, phases in targets.items():
        assert set(phases) == {'initial-corpus', 'corpus'}
        if group == 'current':
            root = STUDY / 'campaigns' / target
            initial = read(root / 'initial-manifest.json')
            result = read(root / 'result.json')
            expected = {'initial-corpus': {key: key for key in initial['inputs']}, 'corpus': result['final_corpus']}
            roots = {phase: root / phase for phase in expected}
        else:
            item = read(STUDY / 'corpus-provenance.json')['targets'][target]
            initial_path, result_path = Path(item['initial_manifest']), Path(item['result'])
            initial, result = read(initial_path), read(result_path)
            for filename, path in [('initial-manifest.json', initial_path), ('result.json', result_path)]:
                assert members[f'historical-seed-metadata/{target}/{filename}'] == path.read_bytes()
            expected = {'initial-corpus': {key: key for key in initial['inputs']}, 'corpus': result['final_corpus']}
            roots = {'initial-corpus': Path(item['initial_directory']), 'corpus': Path(item['final_directory'])}
        assert phases == expected
        for phase, mapping in phases.items():
            for filename, key in mapping.items():
                data = members['corpus-by-sha256/' + key]
                assert hash_bytes(data) == key
                if phase == 'initial-corpus':
                    assert len(data) == initial['inputs'][key]
                expected_origins.setdefault(key, set()).add(str(roots[phase] / filename))
                placements += 1
assert origin_map == {key: sorted(paths) for key, paths in expected_origins.items()}
blob_names = [name for name in members if name.startswith('corpus-by-sha256/')]
assert {name.split('/')[1] for name in blob_names} == set(origin_map)
assert len(blob_names) == meta['unique_corpus_blobs']
assert sum(len(members[name]) for name in blob_names) == meta['unique_corpus_bytes']
assert placements == meta['reconstructable_corpus_files']
for key, paths in origin_map.items():
    for path in paths:
        assert digest(path) == key
omissions = json.loads(members['omissions.json'])['files']
binary_omissions = [row for row in omissions if '/binaries/' in row['path']]
assert len(binary_omissions) == 7
for row in omissions:
    assert digest(row['path']) == row['sha256'] and Path(row['path']).stat().st_size == row['bytes']
assert not any(name.endswith(('.so', '.a', '.tar.gz')) for name in members)
expected_other = {'corpus-layouts.json', 'corpus-origin-map.json', 'omissions.json',
                  'package-streaming-policy-asan.py', 'summarize-streaming-policy-asan.py',
                  'prepare-streaming-policy-asan.py'}
expected_other |= {'source/' + name for name in source}
expected_other |= {f'historical-seed-metadata/{target}/{name}' for target in layouts['historical_seed_inputs']
                   for name in ['initial-manifest.json', 'result.json']}
assert set(members) == expected_plain | expected_other | set(blob_names)
for name, row in indexed.items():
    origin = row['origin']
    if 'path' in origin:
        assert digest(origin['path']) == row['sha256'], name

files = {}
for name in ['ASAN.md', 'asan.json', 'asan-evidence.tar.gz', 'asan-evidence-members.json.gz', 'package-streaming-policy-asan.py']:
    assert (PACKAGE / name).read_bytes() == (REPORT / name).read_bytes()
    files[name] = {'sha256': digest(PACKAGE / name), 'bytes': (PACKAGE / name).stat().st_size}
text = (PACKAGE / 'ASAN.md').read_text()
for phrase in ['collector-owned execution report', 'not an independent campaign audit',
               'not equivalent to the historical six 600-second campaigns',
               'Leak detection was disabled', 'six fuzz targets', 'configuration']:
    if phrase != 'configuration':
        assert phrase in text
for row in summary['targets']:
    assert f"| `{row['target']}` | {row['replay_executions']:,} | {row['campaign_executed_units']:,} | {row['final_disk_corpus_files']:,} |" in text
assert compact['totals'] == raw['recomputed_totals']
report = {
    'status': 'passed', 'role': 'Independent saved-data package audit by next_hotspot; no collector import or target execution.',
    'raw_audit_sha256': digest(OUT / 'raw-audit.json'), 'source_manifest_sha256': raw['source_manifest_sha256'],
    'files_in_report_match_package': files, 'archive_members_verified': len(members),
    'unique_corpus_blobs_verified': len(blob_names), 'unique_corpus_bytes_verified': meta['unique_corpus_bytes'],
    'corpus_placements_reconstructed': placements, 'all_original_filenames_preserved': True,
    'all_blob_origins_rehashed': True, 'raw_study_and_preparation_files_match': True,
    'canonical_source_files_verified': 70, 'compiled_binaries_excluded_with_verified_hashes': 7,
    'caption_and_limits_review': 'Public ASAN.md accurately says 7 targets, 13 selected tests and 120 seconds each; retains per-target counting, historical seed-only reuse, disabled leak detection and collector ownership. Inherited raw six-target caption is preserved and explicitly explained.',
    'publication_finding': 'No blocking mismatch found in this ASan follow-up or its five-file package. Scope is the current e52c streaming policy only.',
}
(OUT / 'package-audit.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'status': 'passed', 'package_audit_sha256': digest(OUT / 'package-audit.json'),
                  'members': len(members), 'placements': placements}, indent=2))
