"""Reconstruct completed normal native results; no parser or target imports."""
from pathlib import Path
import csv
import hashlib
import json
import math
import os
import random
import statistics

assert __debug__ and os.sched_getaffinity(0) == {6}
ROOT = Path('/tmp/oriole-native-start-end-namespace-declaration-study')
NATIVE = Path('/tmp/oriole-native-start-end-namespace-declaration-confirmation/native')
SCREEN = NATIVE / 'native-screen'
OUT = Path(__file__).parent
pins = {}

def sha(path):
    path = Path(path)
    value = hashlib.sha256(path.read_bytes()).hexdigest()
    assert str(path) not in pins or pins[str(path)] == value
    pins[str(path)] = value
    return value

def load(path):
    sha(path)
    return json.loads(Path(path).read_text())

protocol = load(SCREEN / 'protocol.json')
preflight = load(SCREEN / 'preflight.json')
results = load(SCREEN / 'results.json')
controller = load(NATIVE / 'controller.json')
adaptation = load(NATIVE / 'adaptation.json')
assert preflight['status'] == results['status'] == controller['status'] == adaptation['status'] == 'passed'
assert results['affinity'] == [0]
assert preflight['protocol_sha256'] == results['protocol_sha256'] == sha(SCREEN / 'protocol.json')
assert results['preflight_sha256'] == sha(SCREEN / 'preflight.json')
assert protocol['hashes'] == preflight['hashes_before'] == preflight['hashes_after'] == results['hashes_before'] == results['hashes_after']
for path, value in protocol['hashes'].items():
    assert sha(path) == value
libraries = protocol['libraries']
assert libraries['candidate']['sha256'] == 'c3e6533900cf0b1ec6b127fd173f7e25be210cb5afe23ad9963c84873f6ea025'
assert libraries['control']['sha256'] == 'e59d89d6e21b92042b8358bd8ceef45b0a60404c61c9aa06ccf2e85a00f1c7f6'
assert libraries['expat']['sha256'] == '7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478'
assert libraries == adaptation['libraries']
for entry in libraries.values():
    assert sha(entry['path']) == entry['sha256']
source, build, binding = (load(ROOT / name) for name in ['source.json', 'build.json', 'build-binding.json'])
assert source['base'] == '9c632547af2253b9672a92fac320c70af3263bb8' and len(source['sha256']) == 74
assert binding['source_count'] == 74 and set(binding['source_delta']) == {'crates/oriole/src/arena.rs', 'crates/oriole/src/encoding.rs', 'crates/oriole/src/lib.rs', 'crates/oriole/tests/adapter_frame.rs', 'crates/oriole/tests/allocation.rs', 'crates/oriole/tests/external_grammar.rs', 'crates/oriole/tests/external_limits.rs', 'crates/oriole/tests/multibyte.rs', 'crates/oriole/tests/raw_tokens.rs', 'crates/oriole_expat/src/lib.rs', 'crates/oriole_expat/src/tests.rs', 'crates/oriole_expat/tests/memory.rs'}
assert binding['tests'] == {'passed':483,'groups':35,'failures':0,'ignored':0}
fixture = 'crates/oriole_expat/src/context_text_tests.rs'
baseline_record = adaptation['baseline_source_record']
control_record = adaptation['compiled_control_source_record']
assert sha(baseline_record['path']) == baseline_record['sha256'] == binding['baseline_source_manifest_sha256']
assert sha(control_record['path']) == control_record['sha256'] == binding['control_source_manifest_sha256']
baseline = load(baseline_record['path'])
compiled_control = load(control_record['path'])
assert baseline['base'] == source['base'] and set(baseline['sha256']) == set(compiled_control['sha256']) == set(source['sha256'])
base_delta = {p: {'control': compiled_control['sha256'][p], 'baseline': h} for p, h in baseline['sha256'].items() if h != compiled_control['sha256'][p]}
assert set(base_delta) == {'crates/oriole/src/lib.rs', 'crates/oriole/tests/allocation.rs', 'crates/oriole_expat/src/tests.rs', fixture}
assert base_delta == binding['baseline_control_source_delta'] == adaptation['baseline_control_source_delta']
delta = {p: {'control': baseline['sha256'][p], 'candidate': h} for p, h in source['sha256'].items() if h != baseline['sha256'][p]}
control_delta = {p: {'control': compiled_control['sha256'][p], 'candidate': h} for p, h in source['sha256'].items() if h != compiled_control['sha256'][p]}
assert delta == binding['source_delta'] == adaptation['source_delta'] and len(delta) == 12
assert control_delta == binding['compiled_control_source_delta'] == adaptation['compiled_control_source_delta'] and len(control_delta) == 13
assert set(control_delta) == set(delta) | {fixture}
assert binding['control_fixture_correction'] == adaptation['control_fixture_correction']
assert source['sha256'][fixture] == baseline['sha256'][fixture] == binding['control_fixture_correction']['corrected_sha256']
assert binding['source_manifest_sha256'] == sha(ROOT / 'source.json')
assert source['sha256']['crates/oriole/src/lib.rs'] == '339ba2b24308e4672da689f661bc78e8d3cbda590c873acf8c8b0aa69a12980d'
for name, value in source['sha256'].items():
    assert sha(Path(source['worktree']) / name) == value
assert build['status'] == binding['status'] == 'passed' and build['source_unchanged']
assert binding['build_sha256'] == sha(ROOT / 'build.json')
preparation = load(NATIVE / 'preparation.json')
assert preparation['status'] == 'prepared_no_targets'
for path, expected in preparation['pins'].items(): assert sha(path) == expected, path

old = Path(adaptation['origin'])
for name in ['native_screen.py', 'run.py']:
    assert sha(old / name) == adaptation['parent_files'][name]
    assert sha(NATIVE / name) == adaptation['files'][name]
# The reviewed confirmation wrapper changes only its shared-host scope caption.
old_caption = 'Sequential preflightCPU5 then exclusive native elapsedCPU0;'
new_caption = 'Sequential preflight CPU5 then native elapsed with CPU0 affinity;'
assert preparation['changes']['native/run.py'] == [[old_caption, new_caption]]
old_run, current_run = ((p / 'run.py').read_text() for p in [old, NATIVE])
assert old_run.count(old_caption) == current_run.count(new_caption) == 1
assert old_run.replace(old_caption, new_caption) == current_run
current_text, old_text = ((p / 'native_screen.py').read_text() for p in [NATIVE, old])
marker = 'def run(condition, engine, pair, cpu, count):'
assert current_text[current_text.index(marker):] == old_text[old_text.index(marker):]
prior_protocol = load(adaptation['selected_normal_protocol']['path'])
assert sha(adaptation['selected_normal_protocol']['path']) == adaptation['selected_normal_protocol']['sha256']
conditions = protocol['conditions']
assert conditions == prior_protocol['conditions'] and len(conditions) == 28
assert protocol['pairs'] == 7 and protocol['seed'] == 2026091003
assert protocol['preflights'] == 84 and protocol['timed_workers'] == 588
assert controller['controller_sha256'] == sha(NATIVE / 'native_screen.py')
assert controller['wrapper_sha256'] == sha(NATIVE / 'run.py')
assert len(controller['commands']) == 2
for row, phase, cpu, bound in zip(controller['commands'], ['preflight', 'time'], [5, 0], [600, 1200], strict=True):
    assert row['phase'] == phase and row['exit'] == 0 and 'exception' not in row
    assert row['command'] == ['taskset', '-c', str(cpu), 'python3', str(NATIVE / 'native_screen.py'), phase]
    assert row['timeout'] == bound and row['end'] >= row['start']
    assert sha(NATIVE / (phase + '-controller.log')) == row['log_sha256']
assert controller['commands'][0]['end'] <= controller['commands'][1]['start']

driver = '/tmp/oriole-grammar-bench-plain/native-driver'
workers = sorted(SCREEN.glob('worker-*.json'))
assert [p.name for p in workers] == [f'worker-{n:04d}.json' for n in range(672)]
ordinal = 0
observations = {}
counts = {'preflight_workers': 0, 'timed_workers': 0, 'preflight_samples': 0, 'timed_warmups': 0, 'measured_samples': 0}

def condition_key(condition):
    return json.dumps(condition, sort_keys=True)

def worker(condition, engine, pair, cpu, count):
    global ordinal
    raw = load(workers[ordinal])
    ordinal += 1
    argv = ['taskset', '-c', str(cpu), driver, libraries[engine]['path'], condition['input'], str(condition['chunk']), str(count)]
    if condition['namespaces']:
        argv.append('namespaces')
    assert raw['condition'] == condition and raw['engine'] == engine and raw['pair'] == pair
    assert raw['command'] == argv and raw['returncode'] == 0 and raw['stderr'] == ''
    data = json.loads(raw['stdout'])
    assert data['version'] == ('expat_2.8.4' if engine == 'expat' else 'oriole_compat_2.8.4')
    samples = data['samples']
    assert len(samples) == count + 1
    assert [s['iteration'] for s in samples] == list(range(count + 1))
    assert [s['warmup'] for s in samples] == [True] + [False] * count
    assert all(math.isfinite(s['seconds']) and s['seconds'] > 0 for s in samples)
    seen = sorted({(s['hash'], s['elements'], s['text_bytes']) for s in samples})
    assert len(seen) == 1
    observed = [list(x) for x in seen]
    key = condition_key(condition)
    observations.setdefault(key, observed)
    assert observations[key] == observed
    median = statistics.median(s['seconds'] for s in samples[1:])
    if pair == -1:
        counts['preflight_workers'] += 1
        counts['preflight_samples'] += len(samples)
    else:
        counts['timed_workers'] += 1
        counts['timed_warmups'] += 1
        counts['measured_samples'] += count
    return raw | {'observations': observed, 'median_seconds': median}

expected_preflights = [worker(c, e, -1, 5, 1) for c in conditions for e in ['control', 'candidate', 'expat']]
assert expected_preflights == preflight['rows']
rng = random.Random(protocol['seed'])
jobs = [(c, pair) for c in conditions for pair in range(7)]
rng.shuffle(jobs)
expected_rows = []
for condition, pair in jobs:
    order = ['control', 'candidate', 'expat']
    rng.shuffle(order)
    processes = [worker(condition, e, pair, 0, condition['iterations']) for e in order]
    expected_rows.append({'condition': condition, 'pair': pair, 'order': order, 'processes': processes})
assert expected_rows == results['rows'] and ordinal == 672
assert counts == {'preflight_workers': 84, 'timed_workers': 588, 'preflight_samples': 168, 'timed_warmups': 588, 'measured_samples': 105420}
counts.update(all_workers=672, all_samples=106176, cohorts=196, conditions=28)
summary = []
for condition in conditions:
    relevant = [r for r in expected_rows if r['condition'] == condition]
    assert sorted(r['pair'] for r in relevant) == list(range(7))
    values = {key: [] for key in ['candidate_over_control', 'candidate_over_expat', 'control_over_expat']}
    for row in relevant:
        medians = {p['engine']: p['median_seconds'] for p in row['processes']}
        for key in values:
            a, b = key.split('_over_')
            values[key].append(medians[a] / medians[b])
    summary.append({'condition': condition, 'paired_ratios': values, 'median_ratios': {k: statistics.median(v) for k, v in values.items()}})
assert summary == results['summary']
groups = {}
for label, predicate in [('real', lambda c: not c['name'].startswith('generated-')), ('generated', lambda c: c['name'].startswith('generated-')), ('real_namespaces_off', lambda c: not c['name'].startswith('generated-') and not c['namespaces']), ('real_namespaces_on', lambda c: not c['name'].startswith('generated-') and c['namespaces'])]:
    selected = [s for s in summary if predicate(s['condition'])]
    groups[label] = {'conditions': len(selected), 'geomean_ratios': {k: math.exp(sum(math.log(r['median_ratios'][k]) for r in selected) / len(selected)) for k in values}, 'candidate_faster_than_control': sum(r['median_ratios']['candidate_over_control'] < 1 for r in selected), 'adverse': [r for r in selected if r['median_ratios']['candidate_over_control'] > 1]}
with (OUT / 'conditions.csv').open('x', newline='') as stream:
    fieldnames = ['name', 'chunk', 'namespaces', *values]
    writer = csv.DictWriter(stream, fieldnames=fieldnames)
    writer.writeheader()
    for row in summary:
        writer.writerow({k: row['condition'][k] for k in ['name', 'chunk', 'namespaces']} | row['median_ratios'])
for path, value in pins.items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == value
result = {'status': 'passed_independent_saved_normal_native_reconstruction',
          'role': 'Controller preparer adapted this existing saved-file reader; root owns target and reader execution. No parser/compiler/profiler/benchmark execution by this reader.',
          'targets_executed': False, 'counts': counts, 'groups': groups, 'all_conditions': summary,
          'reader_adaptation': 'Reuse the completed raw-view normal reader with source, library and output bindings for Start + End + namespace + declaration composition. Full raw reconstruction unchanged. Protocol preparer adapted this reader; root owns execution and independent review. Original raw reconstruction, worker counts, all ratios and adverse conditions unchanged.',
          'libraries': libraries, 'source_manifest_sha256': sha(ROOT / 'source.json'),
          'raw_schedule_process_medians_ratios_callback_equality_exact': True,
          'seed': protocol['seed'], 'reader_sha256': sha(__file__), 'conditions_csv_sha256': sha(OUT / 'conditions.csv'),
          'inputs': {str(p): sha(p) for p in [SCREEN / 'protocol.json', SCREEN / 'preflight.json', SCREEN / 'results.json', NATIVE / 'controller.json', NATIVE / 'adaptation.json', ROOT / 'build-binding.json', ROOT / 'build.json']},
          'limitations': ['All28 conditions and every adverse result retained. Ratios use medians of seven paired process-median ratios, then equally weighted geometric means.', 'Separate normal confirmation campaign; original shared-host attempt retained. CPU affinity and counters describe a shared-host window, not OS isolation; no statistical-significance or whole-application claim.', 'The original native driver explicitly dlopens the recorded library path; hashes and actual argv are checked, with no new dladdr origin probe.', 'Callback hashes merge text fragments; equality is not exact callback fragmentation compatibility.', 'The earlier preparation receipt remains historically labeled unexecuted; completed controller and rawworker files are execution evidence.', 'Only this native-start-end-namespace-declaration normal native campaign is reconstructed here. Python preparation and timing are conditional on a native gain and are outside this evidence.']}
dest = OUT / 'review.json'
with dest.open('x') as stream:
    stream.write(json.dumps(result, indent=2) + '\n')
print(json.dumps({'status': result['status'], 'path': str(dest), 'sha256': sha(dest), 'counts': counts, 'groups': {k: {key: value for key, value in v.items() if key != 'adverse'} for k, v in groups.items()}, 'all_adverse': [{**r['condition'], 'candidate_over_control': r['median_ratios']['candidate_over_control']} for r in summary if r['median_ratios']['candidate_over_control'] > 1]}, indent=2))
