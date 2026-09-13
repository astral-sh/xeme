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
import argparse
args = argparse.ArgumentParser()
args.add_argument('--summary-sha256', required=True)
args.add_argument('--receipt-sha256', required=True)
args = args.parse_args()
bound = J(D/'bound.json')
assert all(H(path) == digest for path,digest in bound['pins'].items())
command_path = D/'w3c-command.json'
command = J(command_path)
assert H(W/'summary.json') == args.summary_sha256
assert H(D/'w3c-supervision/receipt.json') == args.receipt_sha256
s = J(W / 'summary.json')
supervision = J(D / 'w3c-supervision/receipt.json')
assert supervision['status'] == 'completed_raw_capture'
assert supervision['raw_exit'] == supervision['supervisor_exit'] and supervision['raw_exit'] in (0,1)
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
assert inconclusive_pairs == s['inconclusive_comparisons']
assert acceptance_differences == s['acceptance_differences']
field_differences = {field: [i for i, (a, b) in enumerate(pairs) if a['result'][field] != b['result'][field]] for field in ['status', 'error', 'index', 'children', 'loaded', 'resolver_errors']}
assert bool(any(x['mandatory_fail'] or x['inconclusive'] for x in classification.values())) == bool(supervision['raw_exit'])
source_pins = bound['pins']
for path, digest in source_pins.items():
    assert H(path) == digest
evidence = {str(p.relative_to(D)): H(p) for root in [W, D / 'w3c-supervision'] for p in sorted(root.iterdir()) if p.is_file()}
result = dict(status='saved_w3c_raw_classification_verified_pending_delta_review', targets_executed_by_reader=False, runtime_commit=bound['source']['candidate_source_commit'], base_commit='9c632547af2253b9672a92fac320c70af3263bb8', source_pins=source_pins, command_manifest_sha256=H(command_path), candidate_library_sha256=command['candidate_library_sha256'], reference_library_sha256=command['reference_library_sha256'], catalog_descriptors=len(catalog), selected_descriptors=len(selected), skipped_by_reason=dict(collections.Counter(c['skip'] for c in catalog if c.get('skip'))), rows_per_engine=len(expected), corpus_files=len(s['catalog_source_sha256']), engines=classification, acceptance_differences=len(acceptance_differences), inconclusive_comparisons=len(inconclusive_pairs), result_field_difference_counts={k: len(v) for k, v in field_differences.items()}, result_field_difference_row_indices=field_differences, evidence_sha256=evidence, supervisor_raw_exit=supervision['raw_exit'], supervisor_elapsed_seconds=supervision['elapsed_seconds'], all_processes_reaped=True, reader_sha256=H(__file__), limitations=['Acceptance-only comparison; no canonical callback output oracle.', 'Catalog expectation mismatches remain failures even when both engines agree.', 'Optional error cases and accepted invalid cases do not establish validating conformance.', 'Error codes, byte indices and some child error outcomes differ; acceptance equality is not exact API outcome equality.', 'Only this combined candidate and pinned normal Expat are executed by the held command; selected raw-view comparison reuses its pinned saved rows. All deltas require source/fixture review; no failure waiver.'])

# Compare every saved result field against selected e59d without waiving any delta.
selected_comparison = {}
for label in ['oriole', 'reference']:
    prior = J('/tmp/oriole-core-owned-text-raw-correctness/w3c/' + label + '.json')['rows']
    current = raw[label]
    assert [(r['id'], r['path'], r['type'], r['chunk']) for r in prior] == expected
    deltas = [{'row_index': i, 'selected': a, 'candidate': b} for i, (a, b) in enumerate(zip(prior, current, strict=True)) if a != b]
    selected_comparison[label] = {'rows': len(current), 'changed_rows': len(deltas), 'differences': deltas}
result['selected_raw_view_comparison'] = selected_comparison
result['bound_sha256'] = H(D/'bound.json')
result['acceptance_difference_rows'] = acceptance_differences
result['inconclusive_difference_rows'] = inconclusive_pairs
result['qualification'] = 'conformance_failed' if any(x['mandatory_fail'] or x['inconclusive'] for x in classification.values()) else 'acceptance_classification_passed_only'
result['selected_catalog_equal'] = catalog == J('/tmp/oriole-core-owned-text-raw-correctness/w3c/catalog.json')
assert result['selected_catalog_equal']

# Exact declaration-qualified oracle; preserve every row before enforcing it.
qualified_comparison={}
for label in ['oriole','reference']:
    path=Path('/tmp/oriole-declaration-bootstrap-grammar-diagnostics/w3c')/(label+'.json')
    qualified=J(path)['rows']
    assert [(r['id'],r['path'],r['type'],r['chunk']) for r in qualified]==expected
    deltas=[{'row_index':i,'qualified':a,'candidate':b} for i,(a,b) in enumerate(zip(qualified,raw[label],strict=True)) if a!=b]
    qualified_comparison[label]={'source_sha256':H(path),'changed_rows':len(deltas),'differences':deltas}
result['qualified_declaration_comparison']=qualified_comparison
result['qualified_declaration_catalog_equal']=catalog==J('/tmp/oriole-declaration-bootstrap-grammar-diagnostics/w3c/catalog.json')
result['status']='passed_saved_combined_w3c_matches_qualified_declaration' if result['qualified_declaration_catalog_equal'] and not any(x['changed_rows'] for x in qualified_comparison.values()) else 'failed_combined_w3c_qualified_declaration_comparison'
out = D / 'w3c-saved-readback.json'
assert not out.exists()
out.write_text(json.dumps(result, indent=2) + '\n')
assert result['qualified_declaration_catalog_equal'] and not any(x['changed_rows'] for x in qualified_comparison.values()), 'New W3C outcome versus qualified declaration source; raw and comparison retained'
assert selected_comparison['oriole']['changed_rows']==48 and selected_comparison['reference']['changed_rows']==0
print(json.dumps({'path': str(out), 'sha256': H(out), 'status': result['status'], 'engines': classification, 'result_field_difference_counts': result['result_field_difference_counts']}))
