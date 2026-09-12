"""Strict patched CPython shared/static consumers of the normal candidate."""
from pathlib import Path
import hashlib,json,os,re,signal,subprocess,time
ROOT=Path(__file__).parent/'strict-cpython'
WORK=Path('/home/dev-user/code/oss/oriole-matched-native-end-raw')
BUILD=Path('/tmp/oriole-matched-native-end-raw-study')
BASELINE=Path('/tmp/oriole-reference-frame-strict-cpython')
PYTHON='/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3.12'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert __debug__ and os.sched_getaffinity(0)=={3}
pins=json.loads((Path(__file__).parent/'strict-pins.json').read_text());assert all(sha(p)==h for p,h in pins.items())
bound=json.loads((Path(__file__).parent/'bound.json').read_text())
assert bound['status']=='bound_not_executed'
source=json.loads((BUILD/'source.json').read_text())
baseline_report=json.loads((BASELINE/'report.json').read_text())
assert baseline_report['status']=='completed_strict_results' and baseline_report['pins_unchanged']
baseline_source={'candidate_base_commit':baseline_report['source_commit']}
library_dir=BUILD/'normal'
provenance=json.loads((WORK/'integration/python-build-standalone/consumer-fix/provenance.json').read_text())
assert provenance['patched_source_sha256']=='1c3cf2ee6dfa568cf833742ad786880f4816042705689a116a4f308b7a9d95f2'
for linkage,extension in [('shared','so'),('static','a')]:
 p=library_dir/('liboriole_expat.'+extension);assert sha(p)==pins[str(p)]==bound['libraries'][extension]['sha256']
 b=json.loads((BASELINE/linkage/'summary.json').read_text());assert b['text_fragmentation'] is None and b['consumer_fix']==provenance and b['tests_exit_code']==b['gate_exit_code']==2
ROOT.mkdir(exist_ok=False)
report={'status':'incomplete','pins':pins,'commands':[],'source_commit':json.loads((Path(__file__).parent/'preparation.json').read_text())['candidate_source_commit'],'measured_source_base_commit':source['base'],'source_manifest_sha256':sha(BUILD/'source.json'),'source_identity':'Reviewed74-file candidate snapshot; base commit alone does not identify changed bytes','baseline_source_commit':baseline_source['candidate_base_commit'],'compiler_treatment':'Normal generic O3/ThinLTO/cgu1 parser, identical O2 CPython extension recipe','scope':'Original strict suites with the explicit pinned upstream pyexpat allocation-failure cleanup backport. Shared/static execution loop, raw exits, loader checks and comparisons unchanged. No text-fragmentation allowance or allocator override. These patched consumers are distinct from the unmodified benchmark consumers.'}
def save():(ROOT/'report.json').write_text(json.dumps(report,indent=2)+'\n')

try:
    for linkage, extension in [('shared', 'so'), ('static', 'a')]:
        output = ROOT / linkage
        command = [PYTHON, '-I', '-S', str(WORK / 'tools/cpython/run.py'),
                   '--source', '/home/dev-user/.cache/oriole/upstream/cpython-3.12.13',
                   '--library', str(library_dir / ('liboriole_expat.' + extension)),
                   '--output', str(output), '--consumer-fix']
        if linkage == 'static':
            for name in ['gcc_s', 'util', 'rt', 'pthread', 'm', 'dl', 'c']:
                command += ['--native-library', name]
        row = {'linkage': linkage, 'argv': command, 'started': time.time(), 'timeout': 1000}
        report['commands'].append(row)
        save()
        with (ROOT / (linkage + '.stdout')).open('xb') as out, (ROOT / (linkage + '.stderr')).open('xb') as err:
            process = subprocess.Popen(command, cwd=WORK, stdout=out, stderr=err, start_new_session=True)
            try:
                row['exit'] = process.wait(timeout=row['timeout'])
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                row['exit'] = process.wait()
                row['timeout_reached'] = True
            except BaseException:
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGKILL)
                row['exit'] = process.wait()
                row['reaped'] = True
                save()
                raise
            row['reaped'] = process.poll() is not None
        row['completed'] = time.time()
        for suffix in ['stdout', 'stderr']:
            row[suffix + '_sha256'] = sha(ROOT / (linkage + '.' + suffix))
        save()
        assert row['exit'] in [0, 2] and not row.get('timeout_reached'), row
        summary = json.loads((output / 'summary.json').read_text())
        assert summary['tests_exit_code'] == summary['gate_exit_code'] == row['exit']
        assert summary['probe_exit_code'] == summary['origin_exit_code'] == 0
        assert summary['text_fragmentation'] is None
        assert summary['library_sha256'] == pins[str(library_dir / ('liboriole_expat.' + extension))]
        row['summary_sha256'] = sha(output / 'summary.json')
        baseline_summary = json.loads((BASELINE / linkage / 'summary.json').read_text())
        assert summary['test_command'] == baseline_summary['test_command']
        assert summary['cpython_revision'] == baseline_summary['cpython_revision']
        assert summary['consumer_fix'] == baseline_summary['consumer_fix']
        assert sha(output / 'pyexpat.c') == sha(BASELINE / linkage / 'pyexpat.c')
        pattern = r'^(.+?) \.\.\. (ok|FAIL|ERROR|skipped.*|expected failure|unexpected success)$'
        baseline_outcomes = [match.group(0) for match in re.finditer(
            pattern, (BASELINE / linkage / 'tests.log').read_text(), re.MULTILINE)]
        candidate_outcomes = [match.group(0) for match in re.finditer(
            pattern, (output / 'tests.log').read_text(), re.MULTILINE)]
        assert baseline_outcomes and candidate_outcomes
        comparison = {
            'baseline': str(BASELINE / linkage),
            'baseline_source_commit': baseline_source['candidate_base_commit'],
            'baseline_tests_log_sha256': sha(BASELINE / linkage / 'tests.log'),
            'candidate_tests_log_sha256': sha(output / 'tests.log'),
            'baseline_tests_exit': baseline_summary['tests_exit_code'],
            'candidate_tests_exit': summary['tests_exit_code'],
            'baseline_rendered_outcome_lines': len(baseline_outcomes),
            'candidate_rendered_outcome_lines': len(candidate_outcomes),
            'outcome_lines_equal': candidate_outcomes == baseline_outcomes,
            'changes': [
                {'index': index,
                 'baseline': baseline_outcomes[index] if index < len(baseline_outcomes) else None,
                 'candidate': candidate_outcomes[index] if index < len(candidate_outcomes) else None}
                for index in range(max(len(baseline_outcomes), len(candidate_outcomes)))
                if (baseline_outcomes[index] if index < len(baseline_outcomes) else None)
                != (candidate_outcomes[index] if index < len(candidate_outcomes) else None)
            ],
        }
        (output / 'comparison.json').write_text(json.dumps(comparison, indent=2) + '\n')
        row['comparison'] = comparison
        print(linkage, row['exit'], flush=True)
    report['status'] = 'completed_strict_results'
finally:
    report['pins_unchanged'] = all(sha(p) == h for p, h in pins.items())
    save()
assert report['pins_unchanged']
