"""Independent saved-data audit; never compile, load or execute parser targets."""
from pathlib import Path
import hashlib
import json
import re
import shlex
import subprocess
import tarfile

OUT = Path(__file__).parent
STUDY = Path('/tmp/oriole-streaming-policy-asan-study')
PREP = Path('/tmp/oriole-streaming-policy-asan-preparation')
WORK = Path('/home/dev-user/code/oss/oriole-streaming-input-bounds')
TARGETS = ['parse', 'streaming', 'ffi', 'ffi_family', 'multibyte', 'value_family', 'streaming_work']
SOURCE_HASH = 'e52c4fa16e71a7dcd76864b29509b157a42b6fbe4531f87b68f98395418d5fc3'
SUMMARY_HASH = '5faee1d338e2dfe9bd7f25ae2b3132821d974f1c79b0fda3042ebbb6db47a82f'


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def vectors(text):
    return [{'raw': line.strip(), 'argv': shlex.split(line.split('Running `', 1)[1][:-1])}
            for line in text.splitlines() if 'Running `' in line and '/rustc ' in line]


def crate(row):
    args = row['argv']
    return args[args.index('--crate-name') + 1]


def hash_directory(path):
    files = list(path.iterdir())
    assert all(p.is_file() and not p.is_symlink() for p in files), path
    return {p.name: digest(p) for p in files}


assert digest(STUDY / 'summary.json') == SUMMARY_HASH
summary = read(STUDY / 'summary.json')
for name, expected in summary['evidence'].items():
    assert digest(STUDY / name) == expected, name
assert digest(STUDY / 'source.json') == SOURCE_HASH
source = read(STUDY / 'source.json')['source_sha256']
assert len(source) == 70
assert all(digest(WORK / name) == expected for name, expected in source.items())
with tarfile.open(STUDY / 'source.tar.gz', 'r:gz') as archive:
    files = {m.name: hashlib.sha256(archive.extractfile(m).read()).hexdigest()
             for m in archive if m.isfile()}
assert files == source
manifest = read(STUDY / 'preparation-MANIFEST.json')
assert manifest == read(PREP / 'MANIFEST.json')
assert all(digest(PREP / name) == expected for name, expected in manifest.items())
preparation = read(PREP / 'preparation.json')
assert digest(PREP / 'preparation.json') == summary['preparation_sha256']
for target, expected in preparation['original_harness_sha256'].items():
    for path in [WORK / f'fuzz/fuzz_targets/{target}.rs',
                 PREP / f'fuzz/fuzz_targets/{target}.rs',
                 STUDY / f'fuzz-source/fuzz_targets/{target}.rs']:
        assert digest(path) == expected, path
original = (STUDY / 'fuzz-source/fuzz_targets/streaming.rs').read_bytes()
variant = (STUDY / 'fuzz-source/fuzz_targets/streaming_work.rs').read_bytes()
old = b'max_entity_expansion_bytes: 65_536,'
new = b'max_entity_expansion_bytes: 64,\n            max_work_amplification: Some(100),'
assert original.count(old) == 1 and original.replace(old, new) == variant

build = read(STUDY / 'build.json')
assert build['status'] == 'passed' and build['build']['exit_code'] == 0
assert digest(STUDY / 'build.log') == build['build']['log_sha256']
actual = vectors((STUDY / 'build.log').read_text())
assert actual == read(STUDY / 'compiler-invocations.json')
required = {'oriole_storage', 'oriole', 'oriole_expat', *TARGETS}
selected = [row for row in actual if crate(row) in required]
assert len(selected) == 10 and {crate(row) for row in selected} == required
proof = read(STUDY / 'workspace-and-target-proof.json')
for row in selected:
    name, args = crate(row), row['argv']
    assert proof[name] == row
    for flag in ['-Zsanitizer=address', '-Zexternal-clangrt', '-Clinker=clang',
                 '-Clink-arg=-fsanitize=address', '-Cpasses=sancov-module',
                 '-Cllvm-args=-sanitizer-coverage-inline-8bit-counters',
                 '-Cllvm-args=-sanitizer-coverage-pc-table', '-Ccodegen-units=1']:
        assert flag in args, (name, flag)
    assert args[args.index('--target') + 1] == 'x86_64-unknown-linux-gnu'
    assert not any(s in ' '.join(args) for s in ['relink-only', 'public-api-hash', 'profile-use', 'profile-generate'])
    if name.startswith('oriole'):
        assert str(WORK / f'crates/{name}/src/lib.rs') in args
    else:
        assert f'fuzz_targets/{name}.rs' in args
assert set(build['binaries']) == set(TARGETS)
for target, expected in build['binaries'].items():
    assert digest(STUDY / 'binaries' / target) == expected

instrumentation = read(STUDY / 'instrumentation/report.json')
assert instrumentation['binaries'] == build['binaries']
assert instrumentation['build_sha256'] == digest(STUDY / 'build.json')
assert instrumentation['compiler_proof_sha256'] == digest(STUDY / 'workspace-and-target-proof.json')
machine_commands = []
for target in TARGETS:
    command = ['taskset', '-c', '6', 'nm', '-D', str(STUDY / 'binaries' / target)]
    data = subprocess.check_output(command, timeout=60)
    saved = STUDY / 'instrumentation' / (target + '-dynamic-symbols.txt')
    assert data == saved.read_bytes()
    assert b'__asan_init' in data and b'__asan_report_load' in data
    machine_commands.append({'command': command, 'sha256': digest(saved)})
for item, label in zip(instrumentation['functions'], ['XML_Parse', 'parse_text', 'text_bytes'], strict=True):
    saved = STUDY / 'instrumentation' / (label + '.asm')
    assert digest(saved) == item['sha256']
    command = ['taskset', '-c', '6', *item['command']]
    data = subprocess.check_output(command, timeout=60)
    assert data == saved.read_bytes()
    sites = [line.strip() for line in data.decode().splitlines()
             if '__asan_report_' in line and ('call' in line or 'jmp' in line)]
    assert sites == item['asan_report_sites'] and sites
    machine_commands.append({'command': command, 'sha256': digest(saved), 'report_sites': len(sites)})

regressions = read(STUDY / 'policy-regressions/report.json')
jobs = regressions['jobs']
assert len(jobs) == 13 and len({j['test'] for j in jobs}) == 13
assert regressions['source_manifest_sha256'] == SOURCE_HASH
assert regressions['asan_options'] == 'detect_leaks=0:abort_on_error=1'
regression_vectors = []
counts = {}
for job in jobs:
    name, test = job['crate'], job['test']
    index = counts.get(name, 0)
    counts[name] = index + 1
    path = STUDY / 'policy-regressions' / f'{name}-{index}.log'
    assert digest(path) == job['log_sha256']
    text = path.read_text()
    assert job['exit_code'] == 0 and not job.get('harness_timeout')
    expected = ['taskset', '-c', '2', 'cargo', '+ohm', '-Zohm-defaults=no', 'test',
                '--locked', '--offline', '--target', 'x86_64-unknown-linux-gnu',
                '-p', name, '--lib', '--verbose', test, '--', '--exact', '--test-threads=1']
    assert job['command'] == expected
    assert re.findall(r'^test (.+) \.\.\. ok$', text, re.M) == [test]
    assert re.findall(r'test result: ok\. (\d+) passed; (\d+) failed;', text) == [('1', '0')]
    assert job['outer_timeout_seconds'] == 1200 and job['elapsed_seconds'] < 1200
    regression_vectors.extend(vectors(text))
assert counts == {'oriole': 8, 'oriole_expat': 5}
saved_regression_vectors = read(STUDY / 'policy-regressions/compiler-lines.json')
assert sorted(row['raw'] for row in regression_vectors) == sorted(s.strip() for s in saved_regression_vectors)
for name in ['oriole_storage', 'oriole', 'oriole_expat']:
    rows = [r for r in regression_vectors if crate(r) == name]
    assert rows and all('-Zsanitizer=address' in r['argv'] for r in rows)

campaign_summary = read(STUDY / 'campaign-summary.json')
assert campaign_summary['status'] == 'passed' and not campaign_summary['unstarted_targets']
assert campaign_summary['environment'] == {'ASAN_OPTIONS': 'detect_leaks=0:abort_on_error=1'}
assert campaign_summary['runner_sha256'] == digest(PREP / 'run_all.py')
assert campaign_summary['aggregate_timeout_seconds'] == 1920
assert campaign_summary['elapsed_seconds'] < 1920
lanes = {target: cpu for cpu, names in campaign_summary['lanes'] for target in names}
assert lanes == {'parse': 1, 'multibyte': 1, 'streaming': 2, 'value_family': 2, 'streaming_work': 2, 'ffi': 4, 'ffi_family': 4}
provenance = read(STUDY / 'corpus-provenance.json')
historical = {}
origin_count = 0
for target, item in provenance['targets'].items():
    assert digest(item['initial_manifest']) == item['initial_manifest_sha256']
    assert digest(item['result']) == item['result_sha256']
    old_initial = read(item['initial_manifest'])['inputs']
    old_final = read(item['result'])['final_corpus']
    assert len(old_initial) == item['initial_count'] and len(old_final) == item['final_count']
    initial_hashes = hash_directory(Path(item['initial_directory']))
    assert initial_hashes == {name: name for name in old_initial}
    assert hash_directory(Path(item['final_directory'])) == old_final
    expected_inputs, expected_origins = {}, {}
    for phase, entries in [('initial', initial_hashes), ('final', old_final)]:
        for name, value in entries.items():
            path = Path(item[phase + '_directory']) / name
            size = path.stat().st_size
            if phase == 'initial':
                assert size == old_initial[name]
            expected_inputs[value] = size
            expected_origins.setdefault(value, []).append({'phase': phase, 'path': str(path)})
            origin_count += 1
    historical[target] = (expected_inputs, expected_origins)

results = []
for target in TARGETS:
    root = STUDY / 'campaigns' / target
    result = read(root / 'result.json')
    initial = read(root / 'initial-manifest.json')
    source_target = 'streaming' if target == 'streaming_work' else target
    assert (initial['inputs'], initial['origins']) == historical[source_target]
    assert hash_directory(root / 'initial-corpus') == {name: name for name in initial['inputs']}
    assert all((root / 'initial-corpus' / name).stat().st_size == size for name, size in initial['inputs'].items())
    assert hash_directory(root / 'corpus') == result['final_corpus']
    assert not list((root / 'artifacts').iterdir()) and result['artifacts'] == []
    assert result['initial_manifest_sha256'] == digest(root / 'initial-manifest.json')
    assert result['source_manifest_sha256'] == SOURCE_HASH
    assert result['binary_sha256'] == build['binaries'][target]
    assert result['status'] == 'passed' and result['passed']
    assert result['initial_inputs'] == len(initial['inputs'])
    assert [row['label'] for row in result['runs']] == ['replay', 'campaign']
    records = {}
    for row in result['runs']:
        label = row['label']
        log = root / (label + '.log')
        assert digest(log) == row['log_sha256']
        text = log.read_text()
        assert row['exit_code'] == 0 and not row.get('harness_timeout') and not row.get('controller_aborted')
        expected = ['taskset', '-c', str(lanes[target]), str(STUDY / 'binaries' / target),
                    '-seed=20260911', '-max_len=65536', '-timeout=10', '-rss_limit_mb=1536',
                    f'-artifact_prefix={root}/artifacts/']
        expected += (['-runs=0', str(root / 'initial-corpus')] if label == 'replay' else
                     ['-max_total_time=120', '-print_final_stats=1', str(root / 'corpus'), str(root / 'initial-corpus')])
        assert row['command'] == expected
        assert row['outer_timeout_seconds'] == (180 if label == 'replay' else 690)
        assert 0 < row['elapsed_seconds'] < row['outer_timeout_seconds']
        assert 'INFO: Seed: 20260911' in text
        assert not re.search(r'ERROR: (?:AddressSanitizer|libFuzzer)|SUMMARY: AddressSanitizer|ABORTING|thread .+ panicked', text)
        done = re.findall(r'^#(\d+)\s+DONE\b', text, re.M)
        assert len(done) == 1
        done = int(done[0])
        run_end = re.findall(r'^Done (\d+) runs in (\d+) second\(s\)', text, re.M)
        assert len(run_end) == 1 and int(run_end[0][0]) == done
        if label == 'replay':
            assert done == sum(size != 0 for size in initial['inputs'].values()) + 1
            assert done == result['replay_executions'] == result['replay_expected_executions']
        else:
            stats = {name: int(value) for name, value in re.findall(r'^stat::(\w+):\s+(\d+)', text, re.M)}
            assert stats == result['stats'] and done == stats['number_of_executed_units']
            assert int(run_end[0][1]) >= 120 and stats['peak_rss_mb'] < 1536
        records[label] = done
    top = next(row for row in summary['targets'] if row['target'] == target)
    assert top['result_sha256'] == digest(root / 'result.json')
    assert top['replay_executions'] == records['replay']
    assert top['campaign_executed_units'] == records['campaign']
    results.append({'target': target, 'initial_seed_files': len(initial['inputs']),
                    'replay_executions': records['replay'], 'campaign_executed_units': records['campaign'],
                    'final_disk_corpus_files': len(result['final_corpus']), 'failure_artifacts': 0})
    print('verified', target, flush=True)

totals = {'targets': len(results), 'initial_seed_files_per_target_sum': sum(r['initial_seed_files'] for r in results),
          'replay_executions': sum(r['replay_executions'] for r in results),
          'campaign_executed_units': sum(r['campaign_executed_units'] for r in results),
          'final_disk_corpus_files_per_target_sum': sum(r['final_disk_corpus_files'] for r in results),
          'failure_artifacts': 0}
assert totals == summary['totals']
report = {
    'status': 'passed', 'role': 'Independent saved-data audit by next_hotspot; collector scripts not imported or executed.',
    'summary_sha256': SUMMARY_HASH, 'source_manifest_sha256': SOURCE_HASH,
    'source_files_verified': len(source), 'original_harnesses_unchanged': 6,
    'supplemental_harness_only_two_config_changes': True,
    'build_workspace_vectors': 3, 'build_target_vectors': 7,
    'raw_build_vectors_total': len(actual), 'policy_tests_exact_passes': 13,
    'policy_test_workspace_vectors': {name: sum(crate(r) == name for r in regression_vectors)
                                      for name in ['oriole_storage', 'oriole', 'oriole_expat']},
    'replay_and_campaign_commands_verified': 14, 'results': results, 'recomputed_totals': totals,
    'historical_origin_files_rehashed': origin_count,
    'all_initial_final_corpus_hashes_and_origins_verified': True,
    'instrumentation_readback_commands': machine_commands,
    'limitations': [
        'Current source is streaming-policy e52c, not the direct-writer prototype.',
        'Seven 120-second follow-up campaigns do not transfer historical six-by-600-second outcomes; historical bytes and provenance are seeds only.',
        'Seed and corpus counts are per-target sums; streaming_work deliberately reuses streaming seeds. Executed-unit stats include campaign initialization and mutation, separate from replay.',
        'ASAN_OPTIONS disables leak detection. No LSan, UBSan, whole std/libc instrumentation, PGO, performance or exhaustive-safety claim.',
        'Raw instrumentation scope sentence says six fuzz targets; actual raw vectors, seven binary hashes, symbol readbacks and this receipt establish seven. Raw caption is preserved.',
        'Saved process records and controllers show first attempts and cleanup; no new parser target was executed by this audit.',
        'Preparation-only names were corrected before execution; original preparation correction is retained. Packaging audit is separate.',
    ],
}
(OUT / 'raw-audit.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'status': 'passed', 'raw_audit_sha256': digest(OUT / 'raw-audit.json'), 'totals': totals}, indent=2))
