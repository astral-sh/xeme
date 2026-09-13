"""Verify archived records and recompute every native benchmark condition.

This reads saved data only. It does not compile, execute parsers, or verify the
original machine's compiled libraries; the archive records their identities.
"""
from pathlib import Path
import hashlib
import json
import math
import random
import statistics
import tarfile

root = Path(__file__).resolve().parent
sha = lambda data: hashlib.sha256(data).hexdigest()
index = json.loads((root / 'files.json').read_text())
assert sha((root / 'evidence.tar.gz').read_bytes()) == index['archive_sha256']
with tarfile.open(root / 'evidence.tar.gz', 'r:gz') as archive:
    entries = archive.getmembers()
    assert len(entries) == len(index['files'])
    assert len({entry.name for entry in entries}) == len(entries)
    assert all(entry.isfile() and not entry.name.startswith('/') and '..' not in Path(entry.name).parts for entry in entries)
    data = {entry.name: archive.extractfile(entry).read() for entry in entries}
assert set(data) == set(index['files'])
for name, content in data.items():
    assert index['files'][name] == {'sha256': sha(content), 'bytes': len(content)}, name
read = lambda name: json.loads(data[name])
report = {}
shared = {}
for variant in ('pressure', 'cold', 'feed'):
    source = read(variant + '/build/source.json')
    assert len(source['source_sha256']) == 72
    for name, digest in source['source_sha256'].items():
        assert sha(data[variant + '/source/' + name]) == digest
    pair_build = read(variant + '/build/pair-report.json')
    assert pair_build['status'] == 'passed'
    report[variant] = {}
    for mode in ('normal', 'pgo'):
        base = variant + '/native/' + mode + '/'
        screen = base + 'native-screen/'
        p, f, r = [read(screen + name + '.json') for name in ('protocol', 'preflight', 'results')]
        assert f['status'] == r['status'] == 'passed' and r['affinity'] == [0]
        assert f['protocol_sha256'] == r['protocol_sha256'] == sha(data[screen + 'protocol.json'])
        assert r['preflight_sha256'] == sha(data[screen + 'preflight.json'])
        assert f['hashes_before'] == f['hashes_after'] == r['hashes_before'] == r['hashes_after'] == p['hashes']
        assert len(p['conditions']) == 28 and p['pairs'] == 7 and p['seed'] == 2026091003
        assert sum(c['name'].startswith('generated-') for c in p['conditions']) == 4
        selected = {key: p[key] for key in ('conditions', 'pairs', 'seed')}
        selected['libraries'] = {key: p['libraries'][key] for key in ('control', 'expat')}
        if mode in shared:
            assert shared[mode] == selected
        else:
            shared[mode] = selected
        expected_library = pair_build['normal_libraries']['liboriole_expat.so'] if mode == 'normal' else pair_build['pgo_libraries']['use/liboriole_expat.so']
        assert p['libraries']['candidate']['sha256'] == expected_library
        controller = read(base + 'controller.json')
        assert controller['status'] == 'passed'
        assert controller['controller_sha256'] == sha(data[base + 'native_screen.py'])
        assert controller['wrapper_sha256'] == sha(data[base + 'run.py'])
        for command, phase in zip(controller['commands'], ('preflight', 'time'), strict=True):
            assert command['phase'] == phase and command['exit'] == 0
            assert command['log_sha256'] == sha(data[base + phase + '-controller.log'])
        ordinal = samples = 0
        observations = {}
        medians = {}

        def worker(row, condition, pair, engine, cpu, count):
            global ordinal, samples
            raw = read(screen + f'worker-{ordinal:04d}.json')
            ordinal += 1
            assert raw == {k: v for k, v in row.items() if k not in ('observations', 'median_seconds')}
            assert row['condition'] == condition and row['pair'] == pair and row['engine'] == engine
            assert row['returncode'] == 0 and row['stderr'] == ''
            assert row['command'] == ['taskset', '-c', str(cpu), '/tmp/oriole-grammar-bench-plain/native-driver', p['libraries'][engine]['path'], condition['input'], str(condition['chunk']), str(count)] + (['namespaces'] if condition['namespaces'] else [])
            values = json.loads(row['stdout'])['samples']
            samples += len(values)
            assert len(values) == count + 1
            assert [v['iteration'] for v in values] == list(range(count + 1))
            assert [v['warmup'] for v in values] == [True] + [False] * count
            assert all(math.isfinite(v['seconds']) and v['seconds'] > 0 for v in values)
            seen = sorted({(v['hash'], v['elements'], v['text_bytes']) for v in values})
            assert len(seen) == 1 and row['observations'] == [list(v) for v in seen]
            key = json.dumps(condition, sort_keys=True)
            if key in observations:
                assert observations[key] == seen
            else:
                observations[key] = seen
            median = statistics.median(v['seconds'] for v in values[1:])
            assert median == row['median_seconds']
            return median

        assert len(f['rows']) == 84 and len(r['rows']) == 196
        for condition in p['conditions']:
            for engine in p['libraries']:
                worker(f['rows'][ordinal], condition, -1, engine, 5, 1)
        rng = random.Random(p['seed'])
        jobs = [(c, i) for c in p['conditions'] for i in range(7)]
        rng.shuffle(jobs)
        for row, (condition, pair) in zip(r['rows'], jobs, strict=True):
            order = list(p['libraries'])
            rng.shuffle(order)
            assert row['condition'] == condition and row['pair'] == pair and row['order'] == order
            for process, engine in zip(row['processes'], order, strict=True):
                medians[(json.dumps(condition, sort_keys=True), pair, engine)] = worker(process, condition, pair, engine, 0, condition['iterations'])
        summary = []
        for condition in p['conditions']:
            key = json.dumps(condition, sort_keys=True)
            pairs = [row['pair'] for row in r['rows'] if row['condition'] == condition]
            ratios = {f'{a}_over_{b}': [medians[(key, i, a)] / medians[(key, i, b)] for i in pairs] for a, b in [('candidate', 'control'), ('candidate', 'expat'), ('control', 'expat')]}
            summary.append({'condition': condition, 'paired_ratios': ratios, 'median_ratios': {k: statistics.median(v) for k, v in ratios.items()}})
        assert summary == r['summary'] and ordinal == 672 and samples == 106176
        assert len([name for name in data if name.startswith(screen + 'worker-')]) == 672
        groups = {}
        for group in ('real', 'generated'):
            rows = [row for row in summary if row['condition']['name'].startswith('generated-') == (group == 'generated')]
            groups[group] = {key: statistics.geometric_mean(row['median_ratios'][key] for row in rows) for key in rows[0]['median_ratios']}
            groups[group]['adverse_conditions'] = sum(row['median_ratios']['candidate_over_control'] > 1 for row in rows)
        report[variant][mode] = {'groups': groups, 'conditions': summary, 'workers': ordinal, 'samples': samples}
print(json.dumps({'status': 'passed', 'archive_files': len(data), 'workers': 4032, 'samples': 637056, 'experiments': report}, indent=2))
