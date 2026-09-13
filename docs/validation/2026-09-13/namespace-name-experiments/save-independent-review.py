"""Summarize completed saved-file audits without running any parser or compiler."""
from pathlib import Path
import hashlib
import json
import re
import sys

CONFIG = {
    'storage': ('namespace-name-storage', 'c3780faeea8cfe61ad5256c33c2fcce7d941ea49', 484, False),
    'proof': ('namespace-name-proof', '8b849fec6b6a915df24e7241aff98db8994e036d', 485, True),
    'revision': ('namespace-binding-revision', '5cd5c6ecab6f363f5ef35353103c20fb7696573e', 487, True),
}
name, commit, tests, python_done = CONFIG[sys.argv[1]]
root = Path('/tmp/oriole-' + name + '-benchmark')
study = Path('/tmp/oriole-' + name + '-study')
dest = root / 'independent-review.json'
assert not dest.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
load = lambda p: json.loads(Path(p).read_text())

build = load(study / 'build.json')
binding = load(root / 'binding.json')
assert build['status'] == binding['status'] == 'passed'
assert binding['build_record'] == {'path': str(study / 'build.json'), 'sha256': sha(study / 'build.json')}
for path, value in build['artifacts'].items():
    assert sha(path) == value
for value in binding['libraries'].values():
    assert sha(value['path']) == value['sha256']
for command in build['commands']:
    assert command['exit'] == 0 and command['reaped'] and not command['timed_out']
    assert sha(study / (command['label'] + '.log')) == command['log_sha256']
rows = [tuple(map(int, row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', (study / 'tests.log').read_text())]
assert len(rows) == 35 and sum(row[0] for row in rows) == tests
assert sum(row[1] + row[2] for row in rows) == 0

artifacts = {}
def pin(path):
    path = Path(path)
    artifacts[str(path)] = sha(path)
    return {'path': str(path), 'sha256': artifacts[str(path)]}

def adverse_summary(rows):
    def ratio(row):
        return row['median_ratios']['candidate_over_control']
    bad = [row for row in rows if ratio(row) > 1]
    return {
        'count': len(bad),
        'above_1_percent': sum(ratio(row) > 1.01 for row in rows),
        'above_2_percent': sum(ratio(row) > 1.02 for row in rows),
        'largest_three': [
            {'condition': {key: value for key, value in row['condition'].items() if key in ('name', 'chunk', 'namespaces', 'mode')}, 'candidate_over_control': ratio(row)}
            for row in sorted(bad, key=ratio, reverse=True)[:3]
        ],
    }

native_path = root / 'native/review.json'
native = load(native_path)
assert native['status'] == 'passed'
assert load(root / 'execution-native.json')['status'] == 'passed'
for path, value in native['inputs'].items():
    assert sha(path) == value, path
native_summary = {
    'status': 'passed',
    'counts': native['counts'],
    'groups': {key: {k: v for k, v in group.items() if k != 'adverse'} for key, group in native['groups'].items()},
    'real_adverse': adverse_summary([row for row in native['all_conditions'] if not row['condition']['name'].startswith('generated-')]),
    'generated_adverse': adverse_summary([row for row in native['all_conditions'] if row['condition']['name'].startswith('generated-')]),
    'full_results': pin(native_path),
    'raw_input_pins_count': len(native['inputs']),
    'raw_input_pins_sha256': hashlib.sha256(json.dumps(native['inputs'], sort_keys=True).encode()).hexdigest(),
    'checks': ['All 672 saved workers and 106176 samples reconstructed earlier in this task.', 'Every condition median and paired ratio matched the saved results; native canonical callback hashes matched across engines.', 'All saved native input pins rechecked when writing this receipt.'],
}
python_summary = {'status': 'not_run_for_this_candidate'}
if python_done:
    assert load(root / 'execution-python.json')['status'] == 'passed'
    py = load(root / 'python/normal-review.json')
    assert py['status'] == 'passed'
    for path, value in py['evidence_sha256'].items():
        assert sha(path) == value, path
    python_summary = {
        'status': 'passed',
        'counts': py['counts'],
        'aggregates': {key: {k: v for k, v in group.items() if k != 'adverse_conditions'} for key, group in py['aggregates'].items()},
        'adverse': adverse_summary(py['summary']),
        'full_results': pin(root / 'python/normal-review.json'),
        'raw_input_pins_count': len(py['evidence_sha256']),
        'raw_input_pins_sha256': hashlib.sha256(json.dumps(py['evidence_sha256'], sort_keys=True).encode()).hexdigest(),
        'checks': ['All 576 saved workers and 26364 samples reconstructed in this task using the existing saved-file reader with output writes removed.', 'Canonical outputs, module hashes, in-process dladdr parser origins, seeded order, warmups, process medians, paired ratios and aggregate arithmetic matched.', 'All saved Python evidence pins rechecked when writing this receipt.'],
    }
    for relative in ['execution-python.json', 'python/elapsed_review.py', 'python/preflight-review.json', 'python/consumers/build.json', 'python/screen/preflight.json', 'python/screen/results.json']:
        pin(root / relative)

for relative in ['build.json', 'release.log', 'tests.log', 'rustc-version.log', 'candidate.patch']:
    pin(study / relative)
for relative in ['binding.json', 'preparation.json', 'execution-native.json', 'native/read_results.py', 'native/native-screen/protocol.json', 'native/native-screen/preflight.json', 'native/native-screen/results.json']:
    pin(root / relative)
for path in build['artifacts']:
    pin(path)
corrections = []
if sys.argv[1] == 'storage':
    corrections = [
        {'path': '/tmp/oriole-namespace-name-storage-failed-build-01', 'issue': 'Initial test build required a mutable parser fixture after the method signature changed.'},
        {'path': '/tmp/oriole-namespace-name-storage-failed-build-02', 'issue': 'A fixture expected two retained cache entries but observed one; preserved original failed assertion.'},
    ]
    for item in corrections:
        for name in ['build.json', 'tests.log']:
            pin(Path(item['path']) / name)
elif sys.argv[1] == 'proof':
    corrections = ['The initial read-only audit expected the preceding 484-test count. The new suite contains 485 passing tests; this was an audit expectation correction, not a parser/test failure.']

result = {
    'status': 'completed_saved_evidence_review',
    'reviewer_role': 'Benchmark preparer independently checked root-run saved evidence; this is not a separate third-party audit.',
    'scope': 'No parser, compiler, profiler, benchmark target, sanitizer, or qualification run was executed by this review. Only saved files, Git blobs and read-only arithmetic were examined. This receipt records checks completed earlier in the task and rechecks saved artifact pins.',
    'source': {'commit': commit, 'recorded_files_checked_against_commit': 69, 'additional_control_C_and_header_files_checked': 5, 'scope': 'Committed source blobs were matched to the build snapshot. The mutable working tree may have advanced; this does not claim the current working tree equals the tested commit.'},
    'build': {'status': 'passed', 'tests': tests, 'test_groups': 35, 'failed': 0, 'ignored': 0, 'compiler_comparison': 'Same rustc version and release Cargo command as qualified c3e65339 control. Fresh Rust compiler vectors match after excluding output paths and crate disambiguators. Existing dependencies retain normal Cargo reuse.', 'settings': 'Generic x86-64, O3, ThinLTO, one codegen unit; no PGO or Ohm experimental optimization flags.'},
    'libraries': binding['libraries'],
    'native': native_summary,
    'python': python_summary,
    'corrections_and_preserved_failures': corrections,
    'limitations': ['Shared-host observations and CPU affinity do not establish isolation or statistical significance.', 'Real project XML fixtures, not whole-project executions.', 'Canonical checks coalesce adjacent text callbacks and do not establish exact fragmentation compatibility.', 'This saved-result audit does not establish full upstream API, CPython, sanitizer, or fuzz qualification.'],
    'audited_artifacts_sha256': artifacts,
}
dest.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'path': str(dest), 'sha256': sha(dest), 'bytes': dest.stat().st_size, 'native_real': native_summary['groups']['real']['geomean_ratios'], 'python': python_summary.get('aggregates', {}).get('all', {})}, indent=2))
