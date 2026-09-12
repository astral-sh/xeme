from pathlib import Path
import collections, hashlib, json, re, subprocess

ROOT = Path('/tmp/oriole-native-composition-ci/pbs')
V = ROOT / 'validation'
CWD = '/home/dev-user/code/oss/oriole-native-composition-publication'
HEAD = '5d983f7e09a095fc6c9a94c6ce05ef5a8ec95c8d'
OLD = Path('/tmp/oriole-core-owned-text-raw-ci/pbs/validation')
STRICT = Path('/tmp/oriole-native-start-end-namespace-declaration-correctness')

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def read(path):
    return json.loads(path.read_text())

completed = read(ROOT / 'completed.json')
assert completed['head_sha'] == HEAD and completed['conclusion'] == 'failure'
run = read(ROOT / '019-run.json')
jobs = read(ROOT / '019-jobs.json')
assert run['id'] == 34722487535 and run['run_attempt'] == 1
job, = jobs['jobs']
assert job['head_sha'] == HEAD and job['conclusion'] == 'failure'
steps = {s['name']: s['conclusion'] for s in job['steps']}
assert {k for k,v in steps.items() if v == 'failure'} == {
    'Validate the complete distribution', 'Run the installed interpreter on glibc 2.17'}
for name in ['Build the distribution validator', 'Build the frozen Oriole bundle',
             'Build CPython 3.12.13 with Oriole', 'Preserve the experimental distribution',
             'Verify the installed parser identity', 'Retain build and validation evidence']:
    assert steps[name] == 'success'
assert steps['Prepare optional PGO tools'] == 'skipped'

manifest = read(V / 'oriole-bundle/manifest.json')
provenance = read(V / 'pbs-installed-provenance.json')
assert provenance['pgo'] is False and provenance['target_cpu'] is None
assert provenance['pbs_target'] == provenance['rust_target'] == 'x86_64-unknown-linux-gnu'
assert provenance['installed_manifest_sha256'] == sha(V / 'oriole-bundle/manifest.json')
assert provenance['static_archive_sha256'] == manifest['files']['libexpat.a']
assert manifest['target_cpu'] is None and 'pgo' not in manifest
assert len(manifest['compiler_vectors']['normal']) == 3
for vector in manifest['compiler_vectors']['normal']:
    assert 'opt-level=3' in vector and 'codegen-units=1' in vector
    assert not any('target-cpu' in x or 'profile-use' in x or 'profile-generate' in x for x in vector)
assert 'lto=thin' in manifest['compiler_vectors']['normal'][-1]

# Read only the 76 source blobs named by the installed build manifest.
sources = manifest['sources']
assert len(sources) == 76
requests = ''.join(f'{HEAD}:{name}\n' for name in sources).encode()
raw = subprocess.check_output(['git', 'cat-file', '--batch'], cwd=CWD, input=requests)
offset = 0
for name, expected in sources.items():
    end = raw.index(b'\n', offset)
    header = raw[offset:end].split()
    assert header[1] == b'blob', (name, header)
    size = int(header[2]); start = end + 1
    data = raw[start:start + size]
    assert hashlib.sha256(data).hexdigest() == expected, name
    assert raw[start + size:start + size + 1] == b'\n'
    offset = start + size + 1
assert offset == len(raw)

host = (V / 'pbs-xml-tests.log').read_text()
glibc = (V / 'pbs-glibc217-tests.log').read_text()
assert 'Total tests: run=806 failures=4 skipped=12' in host
assert 'Total tests: run=802 failures=2 skipped=13' in glibc
names = ['test.test_pyexpat.BufferTextTest.test1', 'test.test_sax.CDATAHandlerTest.test_handlers']
assert all(name in host for name in names)
assert '2 tests failed again:\n    test_pyexpat test_sax' in host
assert 'Result: FAILURE then FAILURE' in host
assert 'returned non-zero exit status 2' in glibc
assert '4 tests OK.' in host and '4 tests OK.' in glibc
assert 'Ran 22 tests' in (V / 'pbs-custom-tests.log').read_text()
assert 'OK (skipped=5)' in (V / 'pbs-custom-tests.log').read_text()
assert (V / 'pbs-validate.log').read_text().endswith(' OK\n\n')
threaded = json.loads(next(line for line in glibc.splitlines() if line.startswith('{"glibc"')))
assert threaded == {'glibc': ['glibc', '2.17'], 'expat': 'oriole_compat_2.8.4', 'threaded_parses': 1024}

def assertions(text):
    return collections.Counter(line for line in text.splitlines() if line.startswith('AssertionError:'))

host_assertions, glibc_assertions = assertions(host), assertions(glibc)
assert len(host_assertions) == len(glibc_assertions) == 2
assert sorted(host_assertions.values()) == [2, 2]
assert sorted(glibc_assertions.values()) == [1, 1]
assert host_assertions == assertions((OLD / 'pbs-xml-tests.log').read_text())
assert glibc_assertions == assertions((OLD / 'pbs-glibc217-tests.log').read_text())
strict_review = read(STRICT / 'strict-semantic-readback.json')
for mode in ('shared', 'static'):
    assert strict_review['strict'][mode]['failures'] == names
    assert assertions((STRICT / f'strict-cpython/{mode}/tests.log').read_text()) == glibc_assertions

paths = [ROOT / name for name in ['completed.json', '019-run.json', '019-jobs.json',
         'checks.json', 'artifacts.json', 'job-103630784165.log', 'validation-download.json', 'read_saved.py']]
paths += sorted(p for p in V.rglob('*') if p.is_file())
paths += [OLD / 'pbs-xml-tests.log', OLD / 'pbs-glibc217-tests.log', STRICT / 'strict-semantic-readback.json']
paths += [STRICT / f'strict-cpython/{mode}/tests.log' for mode in ('shared', 'static')]
summary = {
    'status': 'completed_failed_pbs_validation_saved_readback',
    'run_id': run['id'], 'run_attempt': 1, 'event': run['event'], 'tested_head': HEAD,
    'run_url': run['html_url'], 'run_conclusion': 'failure', 'job_conclusion': 'failure',
    'steps': steps,
    'build': {'pbs_revision': manifest['pbs_revision'], 'cpython_version': '3.12.13',
              'target': manifest['target'], 'target_cpu': None, 'pgo': False,
              'optimization': 'O3, ThinLTO, one codegen unit; generic x86_64',
              'rustc': manifest['rustc'], 'source_files_matching_tested_git_head': len(sources),
              'static_archive_sha256': provenance['static_archive_sha256'],
              'installed_manifest_sha256': provenance['installed_manifest_sha256'],
              'consumer_adaptation': manifest['consumer_adaptation']},
    'outcomes': {'structural_distribution_validation': 'passed',
                 'installed_identity': 'passed',
                 'custom_tests': {'run': 22, 'skipped': 5, 'status': 'passed'},
                 'host_xml_suite': {'status': 'failed', 'executions_including_repeats': 806,
                                    'failures_including_repeats': 4, 'skipped': 12,
                                    'failure_methods': names, 'both_failures_repeated': True},
                 'glibc217': {'status': 'failed_xml_suite_after_successful_identity_and_threaded_smoke',
                              'identity_and_threaded_smoke': threaded, 'executions': 802,
                              'failures': 2, 'skipped': 13, 'inner_exit': 2}},
    'assertion_comparison': {'exact_prior_installed_host_and_glibc_assertion_counters': True,
                            'exact_current_local_shared_static_assertions': True,
                            'host': dict(host_assertions), 'glibc217': dict(glibc_assertions)},
    'limits': ['Both known grouping failures remain failures; complete distribution qualification failed.',
               'The installed recipe backports only the disclosed CPython external-parser cleanup fix.',
               'No installed performance comparison was performed.',
               'Only validation logs/manifests were downloaded; the distribution was not downloaded or executed locally.'],
    'download': read(ROOT / 'validation-download.json'),
    'pins_sha256': {str(p): sha(p) for p in paths},
    'local_sessions': {'final_monitor_1399': 'exit0_reaped', 'artifact_download_5898': 'exit0_reaped'},
    'read_only': True, 'workflow_mutations_reruns_or_local_parser_targets': False}
output = ROOT / 'readback.json'
output.write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps({'path': str(output), 'sha256': sha(output), 'status': summary['status'],
                  'source_files': len(sources), 'known_failures_only': True}))
