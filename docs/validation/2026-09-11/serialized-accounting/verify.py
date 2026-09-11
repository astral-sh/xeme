"""Verify archived records and recompute every native benchmark condition.

This reads saved data only. It does not compile, execute parsers, or verify the
original machine's compiled libraries; the archive records their identities.
"""
from pathlib import Path
import hashlib
import json
import math
import random
import re
import statistics
import tarfile

if not __debug__:
    raise RuntimeError('Run without -O: this saved-data verifier uses assertions')

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
for variant in ('candidate',):
    source = read(variant + '/build/source.json')
    assert len(source['source_sha256']) == 72
    for name, digest in source['source_sha256'].items():
        assert sha(data[variant + '/source/' + name]) == digest
    changed = [name for name in source['source_sha256'] if data[variant + '/source/' + name] != data['selected/source/' + name]]
    prepared = read(variant + '/build/preparation.json')
    assert source['candidate_base_commit'] == prepared['head'] == 'bcf15427e861b832f2ec53a57a765c2c4ecb7165'
    assert prepared['check_attempt'] == '03' and len(prepared['expected_source_delta']) == 9
    assert sorted(changed) == prepared['expected_source_delta']
    checks = read(variant + '/build/source-checks.json')
    assert checks['status'] == 'passed' and checks['test_count'] == 440
    assert checks['failed'] == checks['ignored'] == 0
    for command in checks['commands']:
        assert command['exit'] == 0 and command['reaped'] and command['source_unchanged']
        assert sha(data[variant + '/build/local-checks/' + command['label'] + '/output.log']) == command['log_sha256']
        for name, digest in command['source_sha256'].items():
            assert sha(data[variant + '/source/' + name]) == digest
    assert [row['label'] for row in checks['commands']] == prepared['check_labels']
    test_log = data[variant + '/build/local-checks/' + prepared['check_labels'][0] + '/output.log'].decode()
    groups = [tuple(map(int, row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', test_log)]
    assert len(groups) == 34 and sum(row[0] for row in groups) == 440 and all(row[1:] == (0, 0) for row in groups)
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

# Reconstruct the allocation counts from the C diagnostic's original records.
# Compiled binaries and external tools are not in this portable archive.
def allocation_audit(label):
    base = 'allocation/' + label + '/'
    plan = read(base + 'plan.json')
    build = read(base + 'runs/build.json')
    assert build['status'] == 'reaped' and build['exit'] == 0 and build['pins_unchanged']
    assert build['plan_sha256'] == sha(data[base + 'plan.json'])
    assert build['log_sha256'] == sha(data[base + 'runs/build.log'])
    assert len(plan['cases']) == 12
    rows = []
    for case in plan['cases']:
        name = case['name']
        receipt = read(base + 'runs/' + name + '.json')
        log = data[base + 'runs/' + name + '.log']
        input_data = data[base + 'inputs/' + name + '.xml']
        assert sha(input_data) == case['input_sha256']
        assert receipt['status'] == 'reaped' and receipt['exit'] == 0 and receipt['pins_unchanged']
        assert receipt['plan_sha256'] == sha(data[base + 'plan.json'])
        assert receipt['log_sha256'] == sha(log)
        assert receipt['origin_verified'] and receipt['complete_result'] and receipt['successful_result']
        assert receipt['timeout_seconds'] == 4 and 'exception' not in receipt and 'outer_timeout' not in receipt
        assert receipt['argv'][1:] == [case['input'], case['namespace'], str(case['triplets']), str(case['expected_starts']), str(case['expected_text_bytes']), str(case['warmup_bytes'])]
        origins, calls, phases, histograms, results = {}, [], {}, {}, []
        for line in log.decode().splitlines():
            row = line.split('\t')
            if row[0] == 'ORIGIN_API':
                assert len(row) == 3 and row[1] not in origins
                origins[row[1]] = row[2]
            elif row[0] == 'CALL':
                assert len(row) == 12
                calls.append(list(map(int, row[1:])))
            elif row[0] == 'PHASE':
                assert len(row) == 8 and int(row[1]) not in phases
                phases[int(row[1])] = dict(zip(['mallocs', 'reallocs', 'frees', 'requested', 'peak_bytes', 'peak_blocks'], map(int, row[2:]), strict=True))
            elif row[0] == 'SIZE':
                assert len(row) == 5 and row[2] in ('M', 'R', 'F')
                key = (int(row[1]), row[2], int(row[3]))
                assert key not in histograms and int(row[4]) > 0
                histograms[key] = int(row[4])
            elif row[0] == 'RESULT':
                assert len(row) == 18
                results.append(row)
            else:
                raise AssertionError(row)
        assert set(origins) == {'XML_ParserCreate_MM', 'XML_SetUserData', 'XML_SetElementHandler', 'XML_SetCharacterDataHandler', 'XML_SetReturnNSTriplet', 'XML_SetHashSalt', 'XML_Parse', 'XML_GetErrorCode', 'XML_GetParsingStatus', 'XML_ParserFree'}
        assert set(origins.values()) == {receipt['xml_parse_origin']}
        assert receipt['xml_parse_origin'] == str(Path(receipt['argv'][0]).parent / 'liboriole_expat.so')
        assert set(phases) == {0, 1, 2, 3} and len(results) == 1
        assert all(phase in phases and size >= 0 for phase, _, size in histograms)
        assert all(value >= 0 for phase in phases.values() for value in phase.values())
        for phase, totals in phases.items():
            for operation, key in [('M', 'mallocs'), ('R', 'reallocs'), ('F', 'frees')]:
                assert sum(count for (p, op, _), count in histograms.items() if p == phase and op == operation) == totals[key]
            assert sum(size * count for (p, op, size), count in histograms.items() if p == phase and op in ('M', 'R')) == totals['requested']
        length, warmup, offset = len(input_data), case['warmup_bytes'], 0
        previous_starts = previous_ends = 0
        for index, call in enumerate(calls, 1):
            ordinal, phase, count, final, status, error, starts, ends, depth, live, blocks = call
            expected = min(4096, length - offset, warmup - offset if offset < warmup else length)
            assert ordinal == index and phase == (1 if offset < warmup else 2)
            assert count == expected > 0 and final == int(offset + count == length)
            assert status == 1 and error == 0 and starts >= previous_starts and ends >= previous_ends
            assert depth == starts - ends >= 0 and live >= 0 and blocks >= 0
            assert phases[phase]['peak_bytes'] >= live and phases[phase]['peak_blocks'] >= blocks
            previous_starts, previous_ends = starts, ends
            offset += count
        assert offset == length and calls
        result = results[0]
        assert result[1:4] == ['1', '0', '2']
        callbacks = dict(zip(['starts', 'ends', 'depth', 'text_callbacks', 'text_bytes', 'count'], map(int, result[4:10]), strict=True))
        callbacks['hash'] = result[10]
        allocation = dict(zip(['retained_bytes', 'retained_blocks', 'peak_bytes', 'peak_blocks', 'live_bytes_after_free', 'live_blocks_after_free', 'allocation_failed'], map(int, result[11:]), strict=True))
        assert callbacks == receipt['callbacks'] and allocation == receipt['allocation']
        assert callbacks['starts'] == callbacks['ends'] == case['expected_starts'] and callbacks['depth'] == 0
        assert callbacks['count'] == callbacks['starts'] + callbacks['ends'] + callbacks['text_callbacks']
        assert callbacks['text_bytes'] == case['expected_text_bytes']
        assert calls[-1][6:] == [callbacks['starts'], callbacks['ends'], 0, allocation['retained_bytes'], allocation['retained_blocks']]
        assert allocation['live_bytes_after_free'] == allocation['live_blocks_after_free'] == allocation['allocation_failed'] == 0
        assert allocation['peak_bytes'] == max(row['peak_bytes'] for row in phases.values())
        assert allocation['peak_blocks'] == max(row['peak_blocks'] for row in phases.values())
        assert phases[3]['mallocs'] == phases[3]['reallocs'] == phases[3]['requested'] == 0
        assert phases[3]['peak_bytes'] == allocation['retained_bytes'] and phases[3]['peak_blocks'] == allocation['retained_blocks']
        assert sum(size * count for (phase, kind, size), count in histograms.items() if phase == 3 and kind == 'F') == allocation['retained_bytes']
        rows.append({'case': name, 'create': phases[0], 'warmup': phases[1], 'parse': phases[2], 'free': phases[3], 'callbacks': callbacks, 'allocation': allocation,
                     'parse_histogram': [{'kind': op, 'size': size, 'count': count} for (phase, op, size), count in sorted(histograms.items()) if phase == 2],
                     'all_histograms': [{'phase': phase, 'kind': op, 'size': size, 'count': count} for (phase, op, size), count in sorted(histograms.items())],
                     'parse_calls': calls, 'xml_parse_origin': origins['XML_Parse']})
    return plan, rows

selected_plan, selected_rows = allocation_audit('selected')
suffix_plan, suffix_rows = allocation_audit('candidate')
assert selected_plan['bounds'] == suffix_plan['bounds']
assert suffix_plan['library']['sha256'] == read('candidate/build/pair-report.json')['pgo_libraries']['use/liboriole_expat.so']
for name in ('probe.c', 'run.py'):
    assert data['allocation/selected/' + name] == data['allocation/candidate/' + name]
compared = []
for selected_case, suffix_case, a, b in zip(selected_plan['cases'], suffix_plan['cases'], selected_rows, suffix_rows, strict=True):
    assert {k: v for k, v in selected_case.items() if k != 'input'} == {k: v for k, v in suffix_case.items() if k != 'input'}
    assert a['case'] == b['case'] and a['callbacks'] == b['callbacks']
    assert [call[:9] for call in a['parse_calls']] == [call[:9] for call in b['parse_calls']]
    delta = {phase: {key: b[phase][key] - a[phase][key] for key in a[phase]} for phase in ('create', 'warmup', 'parse', 'free', 'allocation')}
    memory = [{'ordinal': left[0], 'final': bool(left[3]), 'selected_live_bytes': left[9], 'candidate_live_bytes': right[9], 'delta_live_bytes': right[9] - left[9], 'selected_live_blocks': left[10], 'candidate_live_blocks': right[10], 'delta_live_blocks': right[10] - left[10]} for left, right in zip(a['parse_calls'], b['parse_calls'], strict=True)]
    compared.append({'case': a['case'], 'selected': a, 'candidate': b, 'candidate_minus_selected': delta, 'every_feed_memory': memory, 'max_intermediate_live_bytes_increase': max((row['delta_live_bytes'] for row in memory if not row['final']), default=None)})
assert compared == read('allocation/candidate/comparison.json')['rows']

# Original API assertions and ceilings, including every retained failure.
api_base = 'candidate/api/api-native/'
api = read(api_base + 'upstream-api/results.json')
assert api == read('selected/api/results.json')
assert api['selection_complete'] and api['library_origin_verified']
for log_name, original_root in [('candidate/api/api-native/upstream-api/tests.log', '/tmp/oriole-serialized-accounting-api-gates/api-native/upstream-api'), ('selected/api/tests.log', '/tmp/oriole-suffix-element-names-api-gates/api-native/upstream-api')]:
    raw_log = data[log_name].decode()
    origins = re.findall(r'^ORIOLE_LIBRARY\t(.+)$', raw_log, re.MULTILINE)
    assert origins and set(origins) == {original_root + '/liboriole_expat.so'}
    raw_rows = [{'context': context, 'test': test, 'outcome': outcome, 'code': int(code)} for context, test, outcome, code in re.findall(r'^ORIOLE_RESULT\t([^\t]+)\t([^\t]+)\t([^\t]+)\t(\d+)$', raw_log, re.MULTILINE)]
    assert raw_rows == api['results']
assert len(api['results']) == 4740 and api['passed'] == 4347 and api['failed'] == 393
outcomes = {}
for row in api['results']:
    outcomes[row['outcome']] = outcomes.get(row['outcome'], 0) + 1
assert outcomes == {'pass': 4347, 'fail': 391, 'timeout': 2}
manifest = read(api_base + 'upstream-api/manifest.json')
assert manifest['library_sha256'] == suffix_plan['library']['sha256']
for name, digest in manifest['adapted_sources'].items():
    assert sha(data[api_base + 'upstream-api/adapted/' + name]) == digest
assert manifest['per_test_seconds'] == 3 and manifest['total_timeout_seconds'] == 240
baseline_manifest = read('selected/api/manifest.json')
def normalize_manifest(value, output):
    return {key: [argument.replace(output, '<OUTPUT>') for argument in item] if key == 'compile_command' else item for key, item in value.items() if key not in ('library_sha256', 'binary_sha256')}
assert normalize_manifest(manifest, '/tmp/oriole-serialized-accounting-api-gates/api-native/upstream-api') == normalize_manifest(baseline_manifest, '/tmp/oriole-suffix-element-names-api-gates/api-native/upstream-api')
api_report = read(api_base + 'report.json')
assert api_report['status'] == 'passed' and api_report['unchanged'] and api_report['before'] == api_report['after']
for path, digest in api_report['native_sources'].items():
    assert sha(data[index['original_paths'][path]]) == digest
for command in api_report['commands']:
    assert command['exit'] == (1 if command['label'] == 'api' else 0)
    for stream in ('stdout', 'stderr'):
        assert sha(data[api_base + command['label'] + '.' + stream]) == command[stream + '_sha256']
consumers = [c for c in api_report['commands'] if c['label'].startswith('native-') and not c['label'].endswith('-build')]
assert len(consumers) == 6

# Keep published condition CSVs bound to the independent saved-data reports.
import csv
import io
csv_rows = []
for variant in ('candidate',):
    for mode in ('normal', 'pgo'):
        review = read(variant + '/review/' + mode + '-review.json')
        assert review['status'] == 'passed'
        csv_name = variant + '/review/' + mode + '-conditions.csv'
        assert sha(data[csv_name]) == review['condition_csv_sha256']
        for group in ('real', 'generated'):
            old = review['native'][mode]['groups'][group]
            new = report[variant][mode]['groups'][group]
            assert old['ratios'] == {key: value for key, value in new.items() if key != 'adverse_conditions'}
            assert old['conditions'] - old['candidate_faster_than_control'] == new['adverse_conditions']
        for row in csv.DictReader(io.StringIO(data[csv_name].decode())):
            matching = [entry for entry in report[variant][row['phase']]['conditions'] if entry['condition']['name'] == row['project'] and entry['condition']['chunk'] == int(row['chunk']) and str(entry['condition']['namespaces']) == row['namespaces']]
            assert len(matching) == 1 and int(row['iterations']) == matching[0]['condition']['iterations']
            assert row['group'] == ('generated' if row['project'].startswith('generated-') else 'real')
            for key, value in matching[0]['median_ratios'].items():
                assert float(row[key]) == value
            csv_rows.append({'variant': variant, **row})
assert list(csv.DictReader((root / 'conditions.csv').open())) == csv_rows
assert (root / 'allocations.csv').read_bytes() == data['allocation/candidate/comparison.csv']

# Inspect saved disassembly and size-probe records without loading either library.
codegen = read('codegen/comparison.json')
assert codegen['reader_sha256'] == sha(data['codegen/compare.py'])
for name, variant in codegen['variants'].items():
    base = 'codegen/' + name + '/'
    inspected = read(base + 'report.json')
    assert sha(data[base + 'report.json']) == variant['static_readback_sha256']
    assert inspected['library_sha256'] == variant['sha256']
    assert inspected['script_sha256'] == sha(data['codegen/inspect.py'])
    build = read(('candidate' if name.startswith('candidate') else 'selected') + '/build/pair-report.json')
    expected = build['pgo_libraries']['use/liboriole_expat.so'] if 'pgo' in name else build['normal_libraries']['liboriole_expat.so']
    assert variant['sha256'] == expected
    for tool in ('nm', 'objdump'):
        command = read(base + tool + '.json')
        assert command['exit'] == 0 and command['reaped']
        assert command['argv'][-1] == variant['library']
        for stream in ('stdout', 'stderr'):
            assert sha(data[base + tool + '.' + stream]) == command[stream + '_sha256']
    instructions = []
    for line in data[base + 'objdump.stdout'].decode().splitlines():
        match = re.match(r'^\s*([0-9a-fA-F]+):\s*((?:[0-9a-fA-F]{2}\s+)+)(\S.*)?$', line)
        if match:
            instructions.append((int(match[1], 16), line))
    assert len(instructions) == inspected['verified_instruction_rows']
    symbols = {match[4]: (int(match[1], 16), int(match[2], 16)) for line in data[base + 'nm.stdout'].decode().splitlines() if (match := re.fullmatch(r'([0-9a-fA-F]+) ([0-9a-fA-F]+) ([tT]) (.+)', line))}
    for method, record in variant['methods'].items():
        address, size = symbols[record['symbol']]
        assert (hex(address), size) == (record['address'], record['symbol_bytes'])
        rows = [line for pc, line in instructions if address <= pc < address + size]
        assert data[base + method + '.asm'].decode() == '\n'.join(rows) + '\n'
        assert len(rows) == record['static_instruction_rows']
        assert [line.strip() for line in rows if re.search(r'\block\b|\bcmpxchg\w*\b', line)] == record['locked_or_cas_rows']
        assert [line.strip() for line in rows if re.search(r'\bcall\s', line)] == record['calls']
        if 'reviewed_counter_store' in record:
            assert record['reviewed_counter_store'] in [line.strip() for line in rows]
    assert variant['remaining_storage_atomic_examples'] == inspected['storage_helpers_with_atomics']
    assert variant['remaining_storage_atomic_examples']
for mode in ('normal', 'pgo'):
    control = codegen['variants']['control-' + mode + '-02']
    candidate = codegen['variants']['candidate-' + mode]
    for method in ('account_source', 'charge_expansion'):
        assert len(control['methods'][method]['locked_or_cas_rows']) == 1
        assert candidate['methods'][method]['locked_or_cas_rows'] == []
    for key, change in codegen['changes'][mode].items():
        assert change == {'control': control[key], 'candidate': candidate[key], 'difference': candidate[key] - control[key], 'percent': 100 * (candidate[key] / control[key] - 1)}
for path, digest in codegen['storage_source_equal'].items():
    assert sha(data['candidate/source/' + path]) == sha(data['selected/source/' + path]) == digest
layout = read('layout/report.json')
assert layout['source_text'].encode() == data['layout/layout.rs']
assert layout['source_sha256'] == sha(data['layout/layout.rs'])
assert layout['script_sha256'] == sha(data['layout/layout-run.py'])
assert layout['commands'] == read('layout/commands.json')
for command in layout['commands']:
    assert command['exit'] == 0 and command['reaped']
    for stream in ('stdout', 'stderr'):
        assert sha(data['layout/' + command['label'] + '.' + stream]) == command[stream + '_sha256']
for label, variant in layout['variants'].items():
    base = 'layout/checks/' + label + '/'
    check = read(base + 'report.json')
    assert sha(data[base + 'report.json']) == variant['check_report_sha256']
    assert sha(data[base + 'output.log']) == variant['check_log_sha256'] == check['log_sha256']
    assert check['exit'] == 0 and check['reaped'] and variant['source_matches_passed_checks_before_after']
    for path, digest in check['source_sha256'].items():
        assert sha(data[('candidate' if label == 'candidate' else 'selected') + '/source/' + path]) == digest
    assert variant['output'].encode() == data['layout/' + label + '-sizes.stdout']
    expected = {'XML_ParserStruct': (3160, 8), 'Parser': (2408, 8), 'AdapterFrame': (256, 8)}
    if label == 'candidate':
        expected['CParser'] = (2408, 8)
    assert {name: (int(size), int(alignment)) for name, size, alignment in (line.split() for line in variant['output'].splitlines())} == expected
print(json.dumps({'status': 'passed', 'codegen_and_layout': {'pgo_text_change': codegen['changes']['pgo']['text_section_bytes'], 'handle_bytes': 3160, 'core_bytes': 2408}, 'archive_files': len(data), 'workers': 1344, 'samples': 212352, 'experiments': report, 'allocation_cases': len(compared), 'allocations': compared, 'api': {'configurations': 4740, 'outcomes': outcomes, 'all_rows_unchanged': True, 'c_sanitizer_consumers': len(consumers)}}, indent=2))
