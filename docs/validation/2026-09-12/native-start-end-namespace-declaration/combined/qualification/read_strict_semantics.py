"""Read completed strict and separate semantic results; no target execution."""
from pathlib import Path
import ast
import hashlib
import json
import os
import re

assert __debug__ and os.sched_getaffinity(0) == {6}
P = Path(__file__).parent
W = Path('/home/dev-user/code/oss/oriole-native-start-end-namespace-declaration')
BASE = Path('/tmp/oriole-reference-frame-strict-cpython')
OLD_WORK = Path('/home/dev-user/code/oss/oriole-reference-frame')
inputs = {}

def read(path):
    path = Path(path)
    value = path.read_bytes()
    digest = hashlib.sha256(value).hexdigest()
    assert str(path) not in inputs or inputs[str(path)] == digest
    inputs[str(path)] = digest
    return value

def sha(path):
    read(path)
    return inputs[str(Path(path))]

def load(path):
    return json.loads(read(path))

# Reuse the existing multiline/subtest-aware method reader unchanged.
tree = ast.parse(read(P / 'methods.py'))
node = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'methods')
exec(compile(ast.Module(body=[node], type_ignores=[]), str(P / 'methods.py'), 'exec'))
pins = load(P / 'strict-pins.json')
assert all(sha(path) == value for path, value in pins.items())
strict = load(P / 'strict-cpython/report.json')
assert strict['status'] == 'completed_strict_results' and strict['pins_unchanged']
assert strict['pins'] == pins and [r['linkage'] for r in strict['commands']] == ['shared', 'static']
assert strict['commands'][0]['completed'] <= strict['commands'][1]['started']
provenance = load(W / 'integration/python-build-standalone/consumer-fix/provenance.json')
bound = load(P / 'bound.json')
python_results = {}
for row in strict['commands']:
    linkage = row['linkage']
    directory = P / 'strict-cpython' / linkage
    baseline = BASE / linkage
    summary, base, origin = (load(path) for path in [directory / 'summary.json', baseline / 'summary.json', directory / 'origin.log'])
    assert row['reaped'] and row['exit'] == 2 and not row.get('timeout_reached') and row['timeout'] == 1000
    assert row['completed'] >= row['started'] and 'exception' not in row
    extension = 'so' if linkage == 'shared' else 'a'
    library = Path(bound['libraries'][extension]['path'])
    expected_argv = ['/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3.12', '-I', '-S', str(W / 'tools/cpython/run.py'), '--source', '/home/dev-user/.cache/oriole/upstream/cpython-3.12.13', '--library', str(library), '--output', str(directory), '--consumer-fix']
    if linkage == 'static':
        for name in ['gcc_s', 'util', 'rt', 'pthread', 'm', 'dl', 'c']:
            expected_argv += ['--native-library', name]
    assert row['argv'] == expected_argv
    for suffix in ['stdout', 'stderr']:
        assert sha(P / 'strict-cpython' / (linkage + '.' + suffix)) == row[suffix + '_sha256']
    assert sha(directory / 'summary.json') == row['summary_sha256']
    assert summary['linkage'] == linkage and summary['tests_exit_code'] == summary['gate_exit_code'] == 2
    assert summary['probe_exit_code'] == summary['origin_exit_code'] == 0
    assert summary['consumer_fix'] == base['consumer_fix'] == provenance and summary['text_fragmentation'] is None
    assert summary['consumer_adaptation'] == 'CPython upstream allocation-failure fix'
    assert summary['cpython_revision'] == provenance['cpython_revision'] == '3bb231a6a5dc02b95658877318bf61501a7209e9'
    assert summary['source_sha256'] == provenance['source_sha256']
    assert summary['compiled_source_sha256'] == sha(directory / 'pyexpat.c') == sha(baseline / 'pyexpat.c') == provenance['patched_source_sha256']
    assert summary['test_command'] == base['test_command']
    expected_compilers = [[value.replace(str(baseline), str(directory)).replace(str(OLD_WORK), str(W)) for value in vector] for vector in base['commands']]
    assert summary['commands'] == expected_compilers and len(expected_compilers) == 2
    assert summary['library_sha256'] == sha(directory / library.name) == pins[str(library)] == bound['libraries'][extension]['sha256']
    assert summary['loader_sha256'] == sha(directory / 'sitecustomize.py') == sha(W / 'tools/cpython/extension_loader.py')
    assert summary['import_check_sha256'] == sha(W / 'tools/cpython/check_extension_imports.py')
    assert summary['origin_command'] == [expected_argv[0], '-s', str(W / 'tools/cpython/check_extension_imports.py'), str(directory)]
    assert origin['status'] == 'passed' and origin['accelerator_imports'] == ['cET', 'cET_alias']
    assert origin['pure_python_and_blocked_imports'] == 'passed' and len(origin['origins']) == 4
    for name in ['pyexpat', '_elementtree']:
        entry = origin['origins']['initial:' + name]
        assert entry == origin['origins']['fresh:' + name]
        assert Path(entry['path']).parent == directory and sha(entry['path']) == entry['sha256']
        assert str(Path(entry['path'])) == expected_compilers[['pyexpat', '_elementtree'].index(name)][-1]
    before, after = methods(baseline / 'tests.log'), methods(directory / 'tests.log')
    assert len(before) == len(after) == 802 and before.keys() == after.keys()
    changes = [{'method': name, 'baseline': before[name], 'candidate': after[name]} for name in before if before[name] != after[name]]
    assert changes == []
    extra = []
    for path in [baseline, directory]:
        text = read(path / 'tests.log').decode()
        extra.append({'failure_headers': re.findall(r'^(FAIL|ERROR): (.+)$', text, re.M), 'skip_lines': [line for line in text.splitlines() if ' ... skipped ' in line], 'subtest_lines': [line for line in text.splitlines() if line.startswith('  ') and ' ... ' in line]})
    assert extra[0] == extra[1]
    failures = sorted(name for name, value in after.items() if value in ['FAIL', 'ERROR'])
    assert failures == ['test.test_pyexpat.BufferTextTest.test1', 'test.test_sax.CDATAHandlerTest.test_handlers']
    comparison = load(directory / 'comparison.json')
    assert comparison == row['comparison'] and comparison['changes'] == [] and comparison['outcome_lines_equal']
    # Retain the established rendered-line oracle in addition to the method map.
    outcome_pattern = r'^(.+?) \.\.\. (ok|FAIL|ERROR|skipped.*|expected failure|unexpected success)$'
    rendered = [[match.group(0) for match in re.finditer(outcome_pattern, read(path / 'tests.log').decode(), re.M)] for path in (baseline, directory)]
    assert rendered[0] == rendered[1] and len(rendered[0]) == len(rendered[1]) == 809
    assert comparison['baseline_rendered_outcome_lines'] == comparison['candidate_rendered_outcome_lines'] == 809
    assert comparison['baseline_tests_log_sha256'] == sha(baseline / 'tests.log')
    assert comparison['candidate_tests_log_sha256'] == sha(directory / 'tests.log')
    python_results[linkage] = {'methods': 802, 'rendered_outcome_lines': 809, 'raw_exit': 2, 'changes': changes, 'failures': failures, 'supplementary_outcomes': extra[1], 'full_method_map': after, 'origins': origin['origins'], 'compiler_commands': summary['commands'], 'consumer_fix': provenance}
    for name in ['consumer-fix.log', 'probe.log', 'pyexpat-build.log', '_elementtree-build.log']:
        read(directory / name)

semantic = load(P / 'semantic-commands.json')
assert semantic['status'] == 'prepared_not_executed' and semantic['targets_executed'] is False
assert all(sha(path) == value for path, value in semantic['pins_sha256'].items())
assert semantic['script_sha256'] == sha(semantic['script']) == '97b76ae05228951f85fab5887ed9727cb2d218016cd049a898080d2e0d6fc4b4'
assert [r['mode'] for r in semantic['commands']] == ['shared', 'static']
template = load(P / 'semantic-commands-template.json')
receipts = []
for command, prepared in zip(semantic['commands'], template['commands'], strict=True):
    assert command['argv'] == prepared['argv'] and command['cwd'] == prepared['cwd']
    assert (command['timeout_seconds'], command['termination_grace_seconds'], command['outer_timeout_seconds']) == (120, 5, 130)
    assert command['expected_semantic_tests'] == 2 and command['strict_raw_exit'] == 2
    mode = command['mode']
    assert command['module_origins'] == python_results[mode]['origins']
    row = load(P / ('semantic-' + mode + '-receipt.json'))
    assert row['command_manifest_sha256'] == sha(P / 'semantic-commands.json')
    assert row['status'] == 'passed' and row['exit'] == 0 and row['reaped'] and row['pins_unchanged']
    assert 'exception' not in row and row['ended'] >= row['started'] >= strict['commands'][-1]['completed']
    assert all(row[key] == value for key, value in command.items() if key != 'status')
    for suffix in ['stdout', 'stderr']:
        assert sha(row[suffix]) == row[suffix + '_sha256']
    text = read(row['stderr']).decode()
    tests = re.findall(r'^(test_\w+) \(__main__\.TextFragmentationTests\.(test_\w+)\) \.\.\. ok$', text, re.M)
    assert tests == [('test_callback_controlled_buffering', 'test_callback_controlled_buffering'), ('test_sax_text_and_cdata_order', 'test_sax_text_and_cdata_order')]
    assert re.search(r'^Ran 2 tests in .+s$', text, re.M) and text.rstrip().endswith('OK')
    assert read(row['stdout']) == b''
    receipts.append(row)
assert receipts[0]['ended'] <= receipts[1]['started']
read(P / 'run_semantic.py')
for path, value in inputs.items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == value
result = {'status': 'saved_strict_semantic_verified', 'targets_executed': False, 'role': 'Direct-Start preparer adapted the completed raw-view strict/semantic saved reader; root owns target and reader execution. Original raw reconstruction retained; preparation authorship is disclosed.', 'strict': python_results, 'semantics': [{'mode': row['mode'], 'tests': 2, 'exit': row['exit'], 'reaped': row['reaped'], 'receipt_sha256': sha(P / ('semantic-' + row['mode'] + '-receipt.json'))} for row in receipts], 'inputs': inputs, 'reader_sha256': sha(__file__), 'limitations': ['Strict consumers contain the explicit pinned upstream pyexpat cleanup backport and differ from unmodified benchmark consumers.', 'The original two strict callback-fragmentation failures per linkage remain raw exit2. Passing semantic tests are separate evidence and do not turn the strict suites green.', 'The static and shared extension origins, fresh import behavior and compile/link vectors are verified from saved records; no new target or dynamic-origin probe was run.']}
with (P / 'strict-semantic-readback.json').open('x') as stream:
    stream.write(json.dumps(result, indent=2) + '\n')
print(json.dumps({'path': str(P / 'strict-semantic-readback.json'), 'sha256': sha(P / 'strict-semantic-readback.json'), 'status': result['status'], 'strict_methods_per_linkage': 802, 'strict_raw_exit_per_linkage': 2, 'strict_changes': 0, 'semantic_tests_per_linkage': 2, 'semantic_exit_per_linkage': 0}))
