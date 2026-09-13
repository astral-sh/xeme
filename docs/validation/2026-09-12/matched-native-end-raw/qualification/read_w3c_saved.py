"""Reconstruct completed W3C acceptance classifications from saved JSON only."""
from pathlib import Path
import collections
import hashlib
import json
import os

D = Path(__file__).resolve().parent
W = D / 'w3c'
J = lambda p: json.loads(Path(p).read_text())
H = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert __debug__ and os.sched_getaffinity(0) == {6}
command_path = Path('/tmp/oriole-matched-native-end-raw-supplemental-preparation/w3c-command.json')
command = J(command_path)
assert H(command_path) == '82e1b9ebf5ff79093b5bcc5507e3aaace858fdc8e78217194a5caa3025b484da'
assert H(W / 'summary.json') == 'a3ea909395ac4f636faa3f2e0cea19626c4e07c3be05c36b6c46078a87e7455d'
assert H(D / 'w3c-supervision/receipt.json') == '5c0d7d883ea85131d3d068172fe38d6125756d8b023a33c52de71cb554d50f6f'
s = J(W / 'summary.json')
supervision = J(D / 'w3c-supervision/receipt.json')
assert supervision['status'] == 'completed_raw_capture'
assert supervision['raw_exit'] == supervision['supervisor_exit'] == 1
assert supervision['wrapper_reaped'] and supervision['subreaper_enabled']
assert supervision['descendant_cleanup'] == {'status': 'completed', 'signalled': [], 'reaped': []}
assert not supervision['timed_out'] and not supervision['cleanup_errors']
assert supervision['exception'] is None and supervision['cancelled_signal'] is None
assert supervision['cpu'] == 3 and supervision['timeout_seconds'] == 300
assert supervision['argv'] == command['argv'][command['argv'].index('--') + 1:]
for stream in ['stdout', 'stderr']:
    assert H(D / 'w3c-supervision' / stream) == supervision[stream + '_sha256']
assert H(command['runner']) == command['runner_sha256'] == s['harness_sha256']
assert s['catalog_source_sha256']['xmlconf.xml'] == command['catalog_sha256']
assert s['worker_failures'] == {}
assert s['chunks'] == command['limits']['chunks'] == [1, 7, 4096]
assert s['worker_limits'] == {'address_space_bytes': 1073741824, 'wall_seconds': 120, 'core_bytes': 0}
assert s['resolver_limits'] == {'file_bytes': 2097152, 'depth': 32, 'requests': 1024}

catalog = J(W / 'catalog.json')
for case in catalog:
    expected_skip = None
    if (case.get('RECOMMENDATION', 'XML1.0').startswith(('XML1.1', 'NS1.1'))
            or '1.0' not in case.get('VERSION', '1.0').split()):
        expected_skip = 'XML 1.1'
    elif '5' not in case.get('EDITION', '5').split():
        expected_skip = 'Earlier XML edition only'
    assert case.get('skip') == expected_skip
selected = [case for case in catalog if not case.get('skip')]
expected = [(c['ID'], c['path'], c['TYPE'], chunk) for c in selected for chunk in s['chunks']]
assert len(catalog) == s['catalog_descriptors'] == 2585
assert len(selected) == s['selected_descriptors'] == 2001
assert len(expected) == 6003
raw = {}
classification = {}
for index, label in enumerate(['reference', 'oriole']):
    obj = J(W / (label + '.json'))
    library = command['reference_library' if label == 'reference' else 'candidate_library']
    assert obj['library'] == str(Path(library).resolve())
    assert obj['sha256'] == H(library) == command['reference_library_sha256' if label == 'reference' else 'candidate_library_sha256']
    worker = s['commands'][index]
    assert worker['returncode'] == 0 and worker['timed_out'] is False
    assert worker['command'] == [command['argv'][3], '-I', '-S', command['runner'], '--suite', command['suite'], '--output', str(W), '--library', str(Path(library).resolve()), '--worker', label]
    rows = raw[label] = obj['rows']
    assert [(r['id'], r['path'], r['type'], r['chunk']) for r in rows] == expected
    counts = collections.Counter(mandatory_pass=0, mandatory_fail=0, optional_observations=0, inconclusive=0, resolver_errors=0)
    failed, inconclusive = [], []
    for row in rows:
        result = row['result']
        assert result['status'] in (0, 1)
        assert row['type'] in ('valid', 'invalid', 'not-wf', 'error')
        for loaded in result['loaded']:
            assert loaded['sha256'] == s['catalog_source_sha256'][loaded['path']]
        for child in result['children']:
            assert child['path'] in s['catalog_source_sha256']
            assert child['status'] in (0, 1)
        if result['resolver_errors']:
            counts['inconclusive'] += 1
            counts['resolver_errors'] += 1
            inconclusive.append(row)
        elif row['type'] == 'error':
            counts['optional_observations'] += 1
        elif (result['status'] == 1) == (row['type'] in ('valid', 'invalid')):
            counts['mandatory_pass'] += 1
        else:
            counts['mandatory_fail'] += 1
            failed.append(row)
    assert failed == s['mismatches'][label] and inconclusive == s['inconclusive'][label]
    assert dict(counts) == {k: s['engines'][label][k] for k in counts}
    classification[label] = dict(counts)
    classification[label]['mismatch_types'] = dict(collections.Counter(r['type'] for r in failed))
    classification[label]['child_outcomes'] = len([c for r in rows for c in r['result']['children']])

pairs = list(zip(raw['reference'], raw['oriole'], strict=True))
inconclusive_pairs = [{'reference': a, 'oriole': b} for a, b in pairs if a['result']['resolver_errors'] or b['result']['resolver_errors']]
acceptance_differences = [{'reference': a, 'oriole': b} for a, b in pairs if not a['result']['resolver_errors'] and not b['result']['resolver_errors'] and (a['result']['status'] == 1) != (b['result']['status'] == 1)]
assert inconclusive_pairs == s['inconclusive_comparisons'] == []
assert acceptance_differences == s['acceptance_differences'] == []
field_differences = {field: [i for i, (a, b) in enumerate(pairs) if a['result'][field] != b['result'][field]] for field in ['status', 'error', 'index', 'children', 'loaded', 'resolver_errors']}
assert bool(any(x['mandatory_fail'] or x['inconclusive'] for x in classification.values())) == bool(supervision['raw_exit'])
source_pins = {'/tmp/oriole-matched-native-end-raw-study/source.json': 'c428d62aa77d92493e7d4654bd0a1b3be128ca1228976929e0d93985c31bf6df', '/tmp/oriole-matched-native-end-raw-study/build.json': '1f709c889c0190386482fe257976985a65833fd35ce1f70b7cb2476976561cbb', '/tmp/oriole-matched-native-end-raw-study/build-binding.json': '7d78bc923c9da5b21c51b0887d40be9a41032e40f05a1775f800b81cc095aab8', '/tmp/oriole-matched-native-end-raw-study/candidate.patch': '1df8e708c774f958c32ded7b484849f41a86ed8c299476bc36a91f54fc124455', '/tmp/oriole-matched-native-end-raw-correctness/api-c-readback.json': '4a24ed1135c231879163b7b53b03c7ee916cd67581e69915c367531fb48dd55c', '/tmp/oriole-api-supervisor-preparation/run.py': '6938e87321413baf02abf70451efc87ca45d8627d1f51b105e2060068ffb19aa', '/tmp/oriole-core-owned-text-raw-correctness-preparation/w3c-command.json': 'dc044b23e8ac67d4abcfc2f84646768c31c8c2bfb8b71d5edbf62f63f849e7b8', '/tmp/oriole-core-owned-text-raw-correctness/w3c/oriole.json': '00495b40cef63a60b548d923abc562980206378cfbe6166d73a6ff13ca211f99', '/tmp/oriole-core-owned-text-raw-correctness/w3c/reference.json': 'b4abe0bd35a3bf469e512215f846fd605cedefe4f6666b7fd646c5c8f99b5a21', '/tmp/oriole-core-owned-text-raw-correctness/w3c/catalog.json': '7804e0e29a1da95217f32309c9d74f4b59baeec9989725fc289cf1b6eda6c4ed', '/tmp/oriole-core-owned-text-raw-correctness/read_w3c_saved.py': '6db7e8fdec6d86b124ee68ac852db7a0bcc3ba085815738b8b6f10590d3ef0cc'}
for path, digest in source_pins.items():
    assert H(path) == digest
evidence = {str(p.relative_to(D)): H(p) for root in [W, D / 'w3c-supervision'] for p in sorted(root.iterdir()) if p.is_file()}
result = dict(status='saved_w3c_raw_classification_verified_conformance_failed', targets_executed_by_reader=False, runtime_commit=None, base_commit='008d818237fe0d6f8c04a000a98ad69ab5780ceb', source_pins=source_pins, command_manifest_sha256=H(command_path), candidate_library_sha256=command['candidate_library_sha256'], reference_library_sha256=command['reference_library_sha256'], catalog_descriptors=len(catalog), selected_descriptors=len(selected), skipped_by_reason=dict(collections.Counter(c['skip'] for c in catalog if c.get('skip'))), rows_per_engine=len(expected), corpus_files=len(s['catalog_source_sha256']), engines=classification, acceptance_differences=0, inconclusive_comparisons=0, result_field_difference_counts={k: len(v) for k, v in field_differences.items()}, result_field_difference_row_indices=field_differences, evidence_sha256=evidence, supervisor_raw_exit=supervision['raw_exit'], supervisor_elapsed_seconds=supervision['elapsed_seconds'], all_processes_reaped=True, reader_sha256=H(__file__), limitations=['Acceptance-only comparison; no canonical callback output oracle.', 'Catalog expectation mismatches remain failures even when both engines agree.', 'Optional error cases and accepted invalid cases do not establish validating conformance.', 'Error codes, byte indices and some child error outcomes differ; acceptance equality is not exact API outcome equality.', 'Only matched-End candidate and pinned normal Expat were executed by this command; selected raw-view comparison reuses its pinned saved rows.'])

# Compare every saved result field against selected e59d without waiving any delta.
selected_comparison = {}
for label in ['oriole', 'reference']:
    prior = J('/tmp/oriole-core-owned-text-raw-correctness/w3c/' + label + '.json')['rows']
    current = raw[label]
    assert [(r['id'], r['path'], r['type'], r['chunk']) for r in prior] == expected
    deltas = [{'row_index': i, 'selected': a, 'candidate': b} for i, (a, b) in enumerate(zip(prior, current, strict=True)) if a != b]
    selected_comparison[label] = {'rows': len(current), 'changed_rows': len(deltas), 'differences': deltas}
result['selected_raw_view_comparison'] = selected_comparison
result['selected_catalog_equal'] = catalog == J('/tmp/oriole-core-owned-text-raw-correctness/w3c/catalog.json')
assert result['selected_catalog_equal']

out = D / 'w3c-saved-readback.json'
assert not out.exists()
out.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'path': str(out), 'sha256': H(out), 'status': result['status'], 'engines': classification, 'result_field_difference_counts': result['result_field_difference_counts']}))
