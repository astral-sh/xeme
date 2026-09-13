"""Fixed three-engine matched native End raw-view screen."""

import hashlib
import json
import os
from pathlib import Path
import random
import statistics
import subprocess
import sys

root = Path('/tmp/oriole-matched-native-end-raw-confirmation/native')
output = root / 'native-screen'
manifest = Path('/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json')
driver = Path('/tmp/oriole-grammar-bench-plain/native-driver')
libraries = {'control': Path('/tmp/oriole-core-owned-text-raw-study/normal/liboriole_expat.so'),
             'candidate': Path('/tmp/oriole-matched-native-end-raw-study/normal/liboriole_expat.so'),
             'expat': Path('/tmp/oriole-pgo-study/expat-control-liboriole_expat.so')}
ratios = [('candidate', 'control'), ('candidate', 'expat'), ('control', 'expat')]
iterations = {'vulkan': 7, 'wayland': 64, 'maven': 128, 'batik': 512, 'gtk': 256,
              'docbook': 256, 'generated-rare-declarations': 32, 'generated-entities': 32}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(name, value):
    (output / name).write_text(json.dumps(value, indent=2) + '\n')


fixtures = {}
for project in json.loads(manifest.read_text())['projects']:
    entry = next(f for f in project['files'] if f['role'] == 'input')
    path = (manifest.parent / entry['path']).resolve()
    assert digest(path) == entry['sha256']
    fixtures[project['name']] = path
fixtures['generated-rare-declarations'] = Path('/tmp/oriole-text-frame-prototype/text/screen/rare-declarations.xml')
fixtures['generated-entities'] = Path('/tmp/oriole-grammar-bench-plain/entities.xml')
conditions = [{'name': name, 'input': str(path), 'chunk': chunk, 'namespaces': ns,
               'iterations': iterations[name]}
              for chunk in (4096, 65536)
              for name, path in fixtures.items()
              for ns in ((False,) if name.startswith('generated-') else (False, True))]
assert len(conditions) == 28
hashes = {str(p): digest(p) for p in [manifest, driver, Path(__file__), *libraries.values(), *fixtures.values()]}
protocol = {'scope': 'Matched normal native C lifecycle screen: selected raw-view e59d baseline and matched native End raw-range candidate. Both Oriole C-only ThinLTO builds have no PGO; Expat2.8.4 normal GCC13.3 O3/shared/no-LTO. Same explicit host and verified within-engine compiler settings, original native callbacks. All six held-out projects and two generated controls,28conditions retained. No application/full-correctness claim; no predicted benefit, root schedules elapsed after other local targets are reaped.',
            'libraries': {name: {'path': str(path), 'sha256': digest(path)} for name, path in libraries.items()},
            'conditions': conditions, 'pairs': 7, 'seed': 2026091003,
            'preflights': 84, 'timed_workers': 588, 'hashes': hashes}


def run(condition, engine, pair, cpu, count):
    command = ['taskset', '-c', str(cpu), str(driver), str(libraries[engine]),
               condition['input'], str(condition['chunk']), str(count)]
    if condition['namespaces']:
        command.append('namespaces')
    result = subprocess.run(command, capture_output=True, text=True, timeout=90)
    record = {'condition': condition, 'engine': engine, 'pair': pair, 'command': command,
              'returncode': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}
    ordinal = len(list(output.glob('worker-*.json')))
    save(f'worker-{ordinal:04d}.json', record)
    assert result.returncode == 0, record
    samples = json.loads(result.stdout)['samples']
    assert len(samples) == count + 1
    assert [sample['warmup'] for sample in samples] == [True] + [False] * count
    assert all(sample['seconds'] > 0 for sample in samples)
    observed = sorted({(sample['hash'], sample['elements'], sample['text_bytes']) for sample in samples})
    assert len(observed) == 1
    record['observations'] = [list(row) for row in observed]
    record['median_seconds'] = statistics.median(sample['seconds'] for sample in samples[1:])
    return record


phase = sys.argv[1]
if phase == 'preflight':
    assert os.sched_getaffinity(0) == {5}
    output.mkdir(exist_ok=False)
    save('protocol.json', protocol)
    report = {'status': 'incomplete', 'rows': [], 'protocol_sha256': digest(output / 'protocol.json'), 'hashes_before': hashes}
    try:
        for index, condition in enumerate(conditions):
            pair = [run(condition, engine, -1, 5, 1) for engine in libraries]
            assert all(p['observations'] == pair[0]['observations'] for p in pair)
            report['rows'].extend(pair)
        assert len(report['rows']) == 84
        report['hashes_after'] = {path: digest(path) for path in hashes}
        assert report['hashes_after'] == hashes
        report['status'] = 'passed'
    finally:
        save('preflight.json', report)
    print(json.dumps({'status': report['status'], 'preflights': len(report['rows']), 'protocol_sha256': report['protocol_sha256']}))
else:
    assert phase == 'time' and os.sched_getaffinity(0) == {0}
    assert json.loads((output / 'protocol.json').read_text()) == protocol
    preflight = json.loads((output / 'preflight.json').read_text())
    assert preflight['status'] == 'passed' and preflight['hashes_after'] == hashes
    expected = {json.dumps(row['condition'], sort_keys=True): row['observations'] for row in preflight['rows']}
    report = {'status': 'incomplete', 'protocol_sha256': digest(output / 'protocol.json'),
              'preflight_sha256': digest(output / 'preflight.json'), 'hashes_before': hashes,
              'affinity': sorted(os.sched_getaffinity(0)), 'rows': [], 'summary': []}
    try:
        rng = random.Random(protocol['seed'])
        jobs = [(condition, pair) for condition in conditions for pair in range(7)]
        rng.shuffle(jobs)
        for condition, pair in jobs:
            order = list(libraries)
            rng.shuffle(order)
            rows = [run(condition, engine, pair, 0, condition['iterations']) for engine in order]
            assert all(row['observations'] == expected[json.dumps(condition, sort_keys=True)] for row in rows)
            report['rows'].append({'condition': condition, 'pair': pair, 'order': order, 'processes': rows})
            save('results.json', report)
        for condition in conditions:
            pairs = [row for row in report['rows'] if row['condition'] == condition]
            values = {f"{a}_over_{b}": [] for a, b in ratios}
            for pair in pairs:
                seconds = {process['engine']: process['median_seconds'] for process in pair['processes']}
                for a, b in ratios:
                    values[f'{a}_over_{b}'].append(seconds[a] / seconds[b])
            report['summary'].append({'condition': condition, 'paired_ratios': values,
                                      'median_ratios': {key: statistics.median(data) for key, data in values.items()}})
        assert len(report['rows']) == 196 and len(report['summary']) == 28
        report['hashes_after'] = {path: digest(path) for path in hashes}
        assert report['hashes_after'] == hashes
        report['status'] = 'passed'
    finally:
        save('results.json', report)
    print(json.dumps({'status': report['status'], 'timed_workers': 3 * len(report['rows']), 'summary': report['summary']}))
