"""Held API and strict CPython comparison; execute only after root release."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import signal
import subprocess
import sys
import time

P = Path(__file__).resolve().parent
W = Path('/home/dev-user/code/oss/oriole-namespace-name-storage')
B = Path('/tmp/oriole-native-start-end-namespace-declaration-correctness')
OLD_W = Path('/home/dev-user/code/oss/oriole-native-start-end-namespace-declaration')
PY = '/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3.12'
SUPERVISOR = '/tmp/oriole-api-supervisor-preparation/run.py'
CLEAN = {'PATH': str(Path(PY).parent) + ':/usr/bin:/bin', 'LC_ALL': 'C.UTF-8',
         'PYTHONNOUSERSITE': '1', 'PYTHONSAFEPATH': '1', 'PYTHONDONTWRITEBYTECODE': '1'}


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def load(path):
    return json.loads(Path(path).read_text())


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


def differences(before, after):
    return [{'index': i, 'baseline': before[i] if i < len(before) else None,
             'candidate': after[i] if i < len(after) else None}
            for i in range(max(len(before), len(after)))
            if (before[i] if i < len(before) else None) != (after[i] if i < len(after) else None)]


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--build-json', required=True, type=Path)
parser.add_argument('--output', required=True, type=Path)
parser.add_argument('--inside-supervisor', action='store_true', help=argparse.SUPPRESS)
args = parser.parse_args()
assert __debug__ and os.sched_getaffinity(0) == {3}, 'Use pinned Python under taskset -c 3, without -O'
assert args.build_json.is_absolute() and args.output.is_absolute()
build_path, output = args.build_json.resolve(), args.output.resolve()
assert not output.exists() and str(output).startswith('/tmp/')
pins = load(P / 'pins.json')
assert all(sha(path) == digest for path, digest in pins.items()), 'Prepared tool/baseline changed'
build = load(build_path)
assert build['status'] == 'passed'
tracked = subprocess.check_output(['/usr/bin/git', '--no-optional-locks', '-C', str(W),
                                   'ls-files', '-z', '--', 'Cargo.toml', 'Cargo.lock', 'crates'], timeout=30)
assert set(build['source']) == set(tracked.decode().rstrip('\0').split('\0')), 'Incomplete tracked Cargo/crates source map'
assert build['commands'] and all(c['exit'] == 0 and c['reaped'] and not c['timed_out'] for c in build['commands'])
assert {c['label'] for c in build['commands']} >= {'rustc-version', 'fmt', 'tests', 'clippy', 'release'}
release = next(c['argv'] for c in build['commands'] if c['label'] == 'release')
assert release[:3] == ['cargo', '+ohm', '-Zohm-defaults=no'] and '--release' in release
assert release[release.index('--target') + 1] == 'x86_64-unknown-linux-gnu'
assert '--crate-type' in release and release[release.index('--crate-type') + 1] == 'cdylib,staticlib'
libraries = {Path(path).name: Path(path) for path in build['artifacts']}
assert set(libraries) == {'liboriole_expat.so', 'liboriole_expat.a'}
assert all(path.is_absolute() and path.parent == build_path.parent / 'normal' for path in libraries.values())
assert all(not Path(name).is_absolute() and '..' not in Path(name).parts for name in build['source'])
dynamic_pins = {str(W / name): digest for name, digest in build['source'].items()}
dynamic_pins.update(build['artifacts'])
dynamic_pins[str(build_path)] = sha(build_path)
assert all(sha(path) == digest for path, digest in dynamic_pins.items()), 'Build/source/library mismatch'

if not args.inside_supervisor:
    # Existing subreaper handles nested sessions even if an inner harness fails.
    argv = [PY, '-I', '-S', SUPERVISOR, '--cpu', '3', '--output',
            str(output) + '-supervision', '--timeout', '2400', '--', PY, '-I', '-S',
            str(P / 'run.py'), '--build-json', str(build_path), '--output', str(output),
            '--inside-supervisor']
    os.execve(PY, argv, CLEAN)

output.mkdir()
save(output / 'build.json', build)
report = {'status': 'running', 'build_json': str(build_path), 'build_sha256': sha(build_path),
          'source': build['source'], 'libraries': build['artifacts'], 'commands': [],
          'baseline': str(B), 'runner_sha256': sha(__file__), 'input_pins': pins,
          'scope': 'Original API assertions and strict cleanup-patched CPython; no fragmentation allowance. No normal Rust build or benchmark.'}


def run(label, argv, timeout):
    row = {'label': label, 'argv': argv, 'timeout': timeout, 'start': time.time(), 'reaped': False}
    report['commands'].append(row)
    save(output / 'report.json', report)
    process = None
    with (output / (label + '.stdout')).open('xb') as stdout, (output / (label + '.stderr')).open('xb') as stderr:
        try:
            process = subprocess.Popen(argv, cwd=W, env=CLEAN, stdout=stdout, stderr=stderr, start_new_session=True)
            row['pid'] = process.pid
            row['exit'] = process.wait(timeout=timeout)
        except BaseException as error:
            row['exception'] = repr(error)
            if process is not None:
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGKILL)
                row['exit'] = process.wait()
            raise
        finally:
            row['reaped'] = process is not None and process.poll() is not None
            row['end'] = time.time()
            stdout.flush(); stderr.flush()
            row['stdout_sha256'] = sha(output / (label + '.stdout'))
            row['stderr_sha256'] = sha(output / (label + '.stderr'))
            save(output / 'report.json', report)
    print(json.dumps({'completed': label, 'raw_exit': row['exit'], 'reaped': row['reaped']}), flush=True)
    return row['exit']


def api_lines(directory):
    prefixes = ('ORIOLE_BEGIN\t', 'ORIOLE_RESULT\t', 'ASSERTION: ', 'ERROR: ')
    return [line.replace(str(directory), '<OUTPUT>') for line in (directory / 'tests.log').read_text().splitlines()
            if line.startswith(prefixes)]


try:
    base_api, api = B / 'api-native/upstream-api', output / 'upstream-api'
    expected = load(base_api / 'results.json')
    assert expected['passed'] == 4349 and expected['failed'] == 391 and not expected['timed_out']
    code = run('api', [PY, '-I', '-S', str(B / 'api-native/run_clean.py'), str(W / 'tools/upstream-expat/run.py'),
                      '--source', '/home/dev-user/.cache/oriole/upstream/expat-2.8.4',
                      '--config', str(base_api / 'adapted/expat_config.h'), '--library', str(libraries['liboriole_expat.so']),
                      '--output', str(api), '--test-timeout', '3', '--memory-mib', '1024', '--rss-mib', '768', '--timeout', '240'], 300)
    actual = load(api / 'results.json')
    before, after = load(base_api / 'manifest.json'), load(api / 'manifest.json')
    def normalized_manifest(value, directory):
        return {key: [x.replace(str(directory), '<OUTPUT>') for x in item] if key == 'compile_command' else item
                for key, item in value.items() if key not in ['library_sha256', 'binary_sha256']}
    comparison = {'counts': {k: actual[k] for k in ['passed', 'failed', 'timed_out']},
                  'ordered_row_changes': differences(expected['results'], actual['results']),
                  'assertion_and_sequence_changes': differences(api_lines(base_api), api_lines(api)),
                  'manifest_matches': normalized_manifest(before, base_api) == normalized_manifest(after, api),
                  'baseline_results_sha256': sha(base_api / 'results.json'), 'candidate_results_sha256': sha(api / 'results.json')}
    save(output / 'api-comparison.json', comparison)
    report['api'] = comparison
    assert code == actual['returncode'] == 1
    assert actual['selection_complete'] and actual['library_origin_verified'] and not actual['timed_out']
    assert len(expected['results']) == len(actual['results']) == 4740
    assert comparison['counts'] == dict(passed=4349, failed=391, timed_out=False)
    assert after['library_sha256'] == sha(libraries['liboriole_expat.so'])
    assert comparison['manifest_matches'] and not comparison['ordered_row_changes'] and not comparison['assertion_and_sequence_changes'], 'API delta retained; review required'

    methods = runpy.run_path(str(P / 'methods.py'), init_globals={'read': lambda path: Path(path).read_bytes()})['methods']
    outcome_pattern = r'^(.+?) \.\.\. (ok|FAIL|ERROR|skipped.*|expected failure|unexpected success)$'
    for linkage, suffix in [('shared', 'so'), ('static', 'a')]:
        base, dest = B / 'strict-cpython' / linkage, output / 'strict-cpython' / linkage
        library = libraries['liboriole_expat.' + suffix]
        argv = [PY, '-I', '-S', str(W / 'tools/cpython/run.py'), '--source',
                '/home/dev-user/.cache/oriole/upstream/cpython-3.12.13', '--library', str(library), '--output', str(dest), '--consumer-fix']
        if linkage == 'static':
            for name in ['gcc_s', 'util', 'rt', 'pthread', 'm', 'dl', 'c']:
                argv += ['--native-library', name]
        code = run('strict-' + linkage, argv, 1000)
        old, new, origins = load(base / 'summary.json'), load(dest / 'summary.json'), load(dest / 'origin.log')
        old_text, text = (base / 'tests.log').read_text(), (dest / 'tests.log').read_text()
        old_methods, new_methods = methods(base / 'tests.log'), methods(dest / 'tests.log')
        rendered = lambda value: [m.group(0) for m in re.finditer(outcome_pattern, value, re.M)]
        extras = lambda value: [line for line in value.splitlines() if line.startswith(('FAIL:', 'ERROR:')) or ' ... skipped ' in line or (line.startswith('  ') and ' ... ' in line)]
        comparison = {'methods': len(new_methods), 'rendered_outcomes': len(rendered(text)),
                      'method_changes': differences(list(old_methods.items()), list(new_methods.items())),
                      'rendered_changes': differences(rendered(old_text), rendered(text)),
                      'supplementary_changes': differences(extras(old_text), extras(text)),
                      'failures': sorted(k for k, v in new_methods.items() if v in ['FAIL', 'ERROR']),
                      'baseline_log_sha256': sha(base / 'tests.log'), 'candidate_log_sha256': sha(dest / 'tests.log')}
        save(dest / 'comparison.json', comparison)
        report[linkage] = comparison
        assert code == new['tests_exit_code'] == new['gate_exit_code'] == old['tests_exit_code'] == old['gate_exit_code'] == 2
        assert new['probe_exit_code'] == new['origin_exit_code'] == 0 and new['text_fragmentation'] is None
        for key in ['test_command', 'cpython_revision', 'consumer_fix', 'consumer_adaptation', 'source_sha256', 'compiled_source_sha256', 'loader_sha256', 'import_check_sha256']:
            assert new[key] == old[key], key
        assert sha(dest / 'pyexpat.c') == sha(base / 'pyexpat.c') == new['compiled_source_sha256']
        assert new['library_sha256'] == sha(dest / library.name) == sha(library)
        assert new['loader_sha256'] == sha(dest / 'sitecustomize.py') == sha(W / 'tools/cpython/extension_loader.py')
        assert new['import_check_sha256'] == sha(W / 'tools/cpython/check_extension_imports.py')
        assert new['origin_command'] == [PY, '-s', str(W / 'tools/cpython/check_extension_imports.py'), str(dest)]
        expected_commands = [[x.replace(str(base), str(dest)).replace(str(OLD_W), str(W)) for x in cmd] for cmd in old['commands']]
        assert new['commands'] == expected_commands and new['linkage'] == linkage
        assert origins['status'] == 'passed' and origins['accelerator_imports'] == ['cET', 'cET_alias']
        assert origins['pure_python_and_blocked_imports'] == 'passed' and len(origins['origins']) == 4
        for name in ['pyexpat', '_elementtree']:
            entry = origins['origins']['initial:' + name]
            assert entry == origins['origins']['fresh:' + name]
            assert Path(entry['path']).parent == dest and sha(entry['path']) == entry['sha256']
        assert len(old_methods) == len(new_methods) == 802 and len(rendered(old_text)) == len(rendered(text)) == 809
        assert comparison['failures'] == ['test.test_pyexpat.BufferTextTest.test1', 'test.test_sax.CDATAHandlerTest.test_handlers']
        assert not any(comparison[k] for k in ['method_changes', 'rendered_changes', 'supplementary_changes']), 'Strict outcome delta retained; review required'
    report['status'] = 'passed_baseline_compatibility_comparison'
except BaseException as error:
    report.update(status='failed', exception=repr(error))
    raise
finally:
    report['inputs_unchanged'] = all(sha(path) == digest for path, digest in {**pins, **dynamic_pins}.items())
    if not report['inputs_unchanged']:
        report['status'] = 'failed_inputs_changed'
    save(output / 'report.json', report)
assert report['inputs_unchanged']
print(json.dumps({'status': report['status'], 'report': str(output / 'report.json')}))
