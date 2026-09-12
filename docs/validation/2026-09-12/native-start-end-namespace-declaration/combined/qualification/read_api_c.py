"""Saved API/C readback of root-collected normal artifacts; strict not read."""
from pathlib import Path
from collections import Counter
import hashlib
import json
import os

assert __debug__ and os.sched_getaffinity(0) == {6}
P = Path(__file__).parent
B = Path('/tmp/oriole-native-start-end-namespace-declaration-study')
W = Path('/home/dev-user/code/oss/oriole-native-start-end-namespace-declaration')
BASE = Path('/tmp/oriole-reference-frame-api-gates/api-native/upstream-api')
inputs = {}


def read(path):
    path = Path(path)
    value = path.read_bytes()
    inputs[str(path)] = hashlib.sha256(value).hexdigest()
    return value


def sha(path):
    read(path)
    return inputs[str(Path(path))]


def load(path):
    return json.loads(read(path))


api = load(P / 'api-native/report.json')
pins = load(P / 'pins.json')
assert all(sha(p) == h for p, h in pins.items())
assert api['unchanged'] and api['before'] == api['after'] and len(api['commands']) == 13
assert api['status'] in {'passed','api_differences_native_passed'}
assert all(sha(p) == h for p, h in api['before'].items())
binding = load(B / 'build-binding.json')
assert binding['status'] == 'passed'
library = B / 'normal'
for name in ['liboriole_expat.so', 'liboriole_expat.a']:
    assert sha(library / name) == binding['candidate_libraries'][name]
rows = []
manifests = []
for root in [BASE, P / 'api-native/upstream-api']:
    report, manifest = load(root / 'results.json'), load(root / 'manifest.json')
    events = []
    for line in read(root / 'tests.log').decode().splitlines():
        if line.startswith('ORIOLE_RESULT\t'):
            _, context, test, outcome, code = line.split('\t')
            events.append({'context': context, 'test': test, 'outcome': outcome, 'code': int(code)})
    assert events == report['results'] and len(events) == 4740
    log_lines = read(root / 'tests.log').decode().splitlines()
    assert [line for line in log_lines if line.startswith('ORIOLE_LIBRARY\t')] == ['ORIOLE_LIBRARY\t' + str(root / 'liboriole_expat.so')]
    begins = [tuple(line.split('\t')[1:]) for line in log_lines if line.startswith('ORIOLE_BEGIN\t')]
    assert begins == [(row['context'], row['test']) for row in events]
    assert sum(line.startswith('ASSERTION: ') for line in log_lines) == sum(row['outcome']=='fail' for row in events)
    assert report['returncode'] == 1
    assert report['passed'] == sum(row['outcome']=='pass' for row in events)
    assert report['failed'] == sum(row['outcome']!='pass' for row in events)
    assert sha(root / 'runtests') == manifest['binary_sha256']

    assert report['selection_complete'] and report['library_origin_verified'] and not report['timed_out']
    assert sha(root / 'liboriole_expat.so') == manifest['library_sha256']
    assert manifest['chunks'] == [0, 1, 2, 3, 4, 5] and manifest['deferral'] == [0, 1]
    assert (manifest['per_test_seconds'], manifest['per_test_address_space_bytes'], manifest['per_test_rss_limit_bytes'], manifest['total_timeout_seconds']) == (3, 1024**3, 768 * 1024**2, 240)
    for name, digest in manifest['adapted_sources'].items():
        assert sha(root / 'adapted' / name) == digest
    rows.append(events)
    manifests.append(manifest)
assert dict(Counter(row['outcome'] for row in rows[0])) == {'pass': 4347, 'fail': 391, 'timeout': 2}
expected_rows = [dict(row) for row in rows[0]]
expected_changes = []
for index, context in [(110, 'chunksize=0 deferral=0'), (505, 'chunksize=0 deferral=1')]:
    before = {'context': context, 'test': 'test_misc_input_2gb', 'outcome': 'timeout', 'code': 14}
    after = {**before, 'outcome': 'pass', 'code': 0}
    assert rows[0][index] == before
    expected_rows[index] = after
    expected_changes.append({'baseline': before, 'candidate': after})
# Preserve both historical oracle and selected outcomes; allocation-test deltas
# require source/assertion review rather than equality to allocation ordinals.
selected_root = Path('/tmp/oriole-core-owned-text-raw-correctness/api-native/upstream-api')
selected = load(selected_root / 'results.json')
selected_events = []
for line in read(selected_root / 'tests.log').decode().splitlines():
    if line.startswith('ORIOLE_RESULT\t'):
        _, context, test, outcome, code = line.split('\t')
        selected_events.append({'context':context,'test':test,'outcome':outcome,'code':int(code)})
assert selected_events == selected['results'] == expected_rows
audit = load('/tmp/oriole-selected-allocation-semantic-readiness.json')
allocation_categories = {row['test']:row['category'] for row in audit['rows']}
assert len(allocation_categories) == 34
assert [(r['context'],r['test']) for r in rows[1]] == [(r['context'],r['test']) for r in selected_events]
selected_changes = []
for index,(before,after) in enumerate(zip(selected_events,rows[1])):
    if before != after:
        selected_changes.append({'index':index,'selected':before,'candidate':after,
            'classification':allocation_categories.get(after['test'],'nonallocation_semantic_change'),
            'disposition':'requires_assertion_and_semantic_review; no automatic conformance credit'})
historical_changes = [{'baseline':before,'candidate':after} for before,after in zip(rows[0],rows[1]) if before != after]
assert api['api_comparison']['changes'] == historical_changes
assert api['api_comparison']['all4740rows_exact'] == (not historical_changes)
for index in (110,505):
    assert rows[1][index] == expected_rows[index], 'Both original2GiB semantic successes must remain'
assert not any(r['classification']=='nonallocation_semantic_change' for r in selected_changes), 'Substantive nonallocation outcomes require investigation'
assert manifests[1]['library_sha256'] == binding['candidate_libraries']['liboriole_expat.so']
assert api['api_comparison']['baseline_results_sha256'] == sha(BASE / 'results.json')
assert api['api_comparison']['candidate_results_sha256'] == sha(P / 'api-native/upstream-api/results.json')
for key in manifests[0]:
    if key not in ['compile_command', 'library_sha256', 'binary_sha256']:
        assert manifests[0][key] == manifests[1][key], key
assert [value.replace(str(BASE), str(P / 'api-native/upstream-api')) for value in manifests[0]['compile_command']] == manifests[1]['compile_command']
assert all(a['end'] <= b['start'] for a,b in zip(api['commands'], api['commands'][1:]))
assert api['commands'][0]['timeout'] == 300
expected_api = ['taskset','-c','3','/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3.12','-I','-S',str(P/'api-native/run_clean.py'),str(P/'api-native/tools/run.py'),'--source','/home/dev-user/.cache/oriole/upstream/expat-2.8.4','--config',str(P/'api-native/expat_config.h'),'--library',str(library/'liboriole_expat.so'),'--output',str(P/'api-native/upstream-api'),'--test-timeout','3','--memory-mib','1024','--rss-mib','768','--timeout','240']
assert api['commands'][0]['command'] == expected_api
controller_source = read(P / 'api_native.py').decode()
assert "'ASAN_OPTIONS':'detect_leaks=0:abort_on_error=1'" in controller_source
assert "'UBSAN_OPTIONS':'halt_on_error=1'" in controller_source
native = P / 'api-native/native'
expected_labels = ['api']
c_runs = []
for linkage in ['dynamic', 'static']:
    for name in ['integration', 'adversarial', 'allocations']:
        label = f'native-{name}-{linkage}'
        expected_labels.extend([label + '-build', label])
        source = native / (name + '.c')
        assert sha(source) == sha(W / 'tests/c' / (name + '.c'))
        binary = native / (name + '-' + linkage)
        links = ['-L', str(library), '-loriole_expat', '-Wl,-rpath,' + str(library)] if linkage == 'dynamic' else [str(library / 'liboriole_expat.a'), '-ldl', '-lpthread', '-lm']
        expected = ['taskset', '-c', '3', 'cc', '-std=c11', '-O1', '-g', '-Wall', '-Wextra', '-Werror', '-fsanitize=address,undefined', '-fno-omit-frame-pointer', '-I', str(native), str(source), *links, '-o', str(binary)]
        build = next(row for row in api['commands'] if row['label'] == label + '-build')
        run = next(row for row in api['commands'] if row['label'] == label)
        assert build['command'] == expected and run['command'] == ['taskset', '-c', '3', str(binary)]
        assert build['end'] <= run['start']
        c_runs.append({'label': label, 'exit': run['exit'], 'binary_sha256': sha(binary)})
assert [row['label'] for row in api['commands']] == expected_labels
assert all(row['timeout']==120 for row in api['commands'][1:])
assert sha(native/'expat.h')==sha(W/'include/expat.h')
for row in api['commands']:
    assert row['reaped'] and row['end'] >= row['start'] and 'exception' not in row
    assert row['exit'] == (1 if row['label'] == 'api' else 0)
    for suffix in ['stdout', 'stderr']:
        assert sha(P / 'api-native' / (row['label'] + '.' + suffix)) == row[suffix + '_sha256']
    if 'binary_sha256' in row:
        assert sha(row['command'][-1]) == row['binary_sha256']
counts = dict(Counter(row['outcome'] for row in rows[1]))
assert sum(counts.values()) == 4740
counts.setdefault('timeout', 0)
# Strict execution may be active. This reader opens only completed API/C files.
for path, digest in inputs.items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest
result = {'status': 'passed_saved_api_c_readback', 'role': 'Direct-Start preparer adapted the completed selected raw-view API/C saved reader; allocation deltas are classified without waiving semantic requirements; root owns target and reader execution. Original raw reconstruction and compiler-vector checks retained; preparation authorship is disclosed.', 'targets_executed': False, 'api': {'ordered_configurations': 4740, 'counts': counts, 'selected_counts': {'pass':4349,'fail':391,'timeout':0}, 'selected_changes':selected_changes, 'selected_all4740rows_exact':not selected_changes, 'qualification':'pending_allocation_delta_review' if selected_changes else 'selected_outcomes_preserved', 'baseline_counts': {'pass': 4347, 'fail': 391, 'timeout': 2}, 'changes': historical_changes, 'inherited_timeout_improvements': 2, 'other_4738_ordered_rows_exact': rows[1] == expected_rows, 'raw_exit': 1, 'original_assertions_and_limits': True}, 'c_consumers': c_runs, 'limitations': ['C harnesses use ASan/UBSan; linked Rust normal-release libraries are uninstrumented and leak detection is disabled by the unchanged controller.', 'Strict and supplemental semantic results are outside this API/C audit; no adoption or performance claim.'], 'inputs': inputs, 'reader_sha256': sha(__file__)}
dest = P / 'api-c-readback.json'
with dest.open('x') as stream:
    stream.write(json.dumps(result, indent=2) + '\n')
print(json.dumps({'status': result['status'], 'api': result['api'], 'c_consumers': len(c_runs), 'path': str(dest), 'sha256': sha(dest)}))
