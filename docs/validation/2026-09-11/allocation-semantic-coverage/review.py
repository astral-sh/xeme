"""Read saved artifacts only; never load the parser or execute a test binary."""
from pathlib import Path
import argparse
import collections
import difflib
import hashlib
import json
import re
import tarfile

parser = argparse.ArgumentParser()
parser.add_argument('--local', type=Path, help='Also hash the preserved original run and libraries')
args = parser.parse_args()
root = Path(__file__).resolve().parent
sha = lambda data: hashlib.sha256(data).hexdigest()
index = json.loads((root / 'records.files.json').read_text())
with tarfile.open(root / 'records.tar.gz', 'r:gz') as archive:
    members = archive.getmembers()
    assert all(m.isfile() for m in members)
    assert len(members) == len(index) == len({m.name for m in members})
    files = {m.name: archive.extractfile(m).read() for m in members}
assert set(index) == set(files)
for name, data in files.items():
    assert len(data) == index[name]['bytes'] and sha(data) == index[name]['sha256'], name
    assert not name.endswith(('.so', '.a')) and not data.startswith(b'\x7fELF'), name
load = lambda name: json.loads(files[name])
text = lambda name: files[name].decode()
prep = load('preparation.json')
report = load('run01/report.json')
original = load('original-results.json')
manifest = load('original-manifest.json')
history = load('historical-selected-results.json')
expected = {(f'chunksize={chunk} deferral={mode}', test) for chunk in range(6) for mode in range(2) for test in prep['selected_tests']}
assert len(expected) == 72
records = lambda value: [dict(context=c, test=t, outcome=o, code=int(n)) for c,t,o,n in re.findall(r'^ORIOLE_RESULT\t([^\t\n]+)\t([^\t\n]+)\t([^\t\n]+)\t(\d+)$', value, re.M)]
rows = records(text('run01/tests.log'))
assert rows == report['results'] and len(rows) == 72
assert {(r['context'], r['test']) for r in rows} == expected
assert all(r['outcome'] == 'pass' and r['code'] == 0 for r in rows)
begins = re.findall(r'^ORIOLE_BEGIN\t([^\t\n]+)\t([^\t\n]+)$', text('run01/tests.log'), re.M)
assert begins == [(r['context'], r['test']) for r in rows]
assert report['status'] == 'complete' and report['compile_exit_code'] == report['run_exit_code'] == 0
for name in ['preflight_passed', 'selection_complete', 'library_origin_verified', 'library_unchanged', 'original_tree_unchanged', 'all_diagnostic_contexts_passed', 'original_tree_unchanged_on_exit']:
    assert report[name] is True, name
origins = re.findall(r'^ORIOLE_LIBRARY\t(.+)$', text('run01/tests.log'), re.M)
assert origins == report['library_origins'] == ['/tmp/oriole-reference-frame-semantic-diagnostic/liboriole_expat.so']
assert report['controller_sha256'] == sha(files['run.py'])
assert report['preparation_sha256'] == sha(files['preparation.json'])
for command, log in zip(report['commands'], ['run01/build.log', 'run01/tests.log'], strict=True):
    assert command['exit_code'] == 0 and command['log_sha256'] == sha(files[log])
    assert command['environment'] == {'PATH':'/usr/bin:/bin', 'LANG':'C', 'LC_ALL':'C', 'TMPDIR':'/tmp', **prep['environment']}
assert [c['timeout_seconds'] for c in report['commands']] == [120, 300]
assert report['commands'][0]['argv'] == prep['compile_command']
assert collections.Counter(r['outcome'] for r in original['results']) == {'pass':4347, 'fail':391, 'timeout':2}
assert len(original['results']) == 4740
original_rows = [r for r in original['results'] if r['test'] in prep['selected_tests']]
assert len(original_rows) == 72 and {(r['context'],r['test']) for r in original_rows} == expected
assert all(r['outcome'] == 'fail' and r['code'] == 100 for r in original_rows)
assert records(text('original-selected-failures.log')) == original_rows
assert len(history['results']) == 72 and all(r['outcome'] == 'pass' for r in history['results'])
assert {(r['context'],r['test']) for r in history['results']} == expected
assert prep['original_matrix_sha256'] == sha(files['original-results.json'])
before = text('original/alloc_tests.c')
after = text('adapted/alloc_tests.c')
modified = before
for change in prep['ceiling_changes']:
    test = change['test']
    start = before.index('START_TEST(' + test + ')')
    end = before.index('END_TEST', start) + len('END_TEST')
    body = before[start:end]
    assert before[:start].count('\n') + 1 == change['line']
    assert sha(body.encode()) == change['body_sha256_before']
    replaced = body.replace(f"const int max_alloc_count = {change['old_ceiling']};", 'const int max_alloc_count = 512;', 1)
    assert body != replaced and sha(replaced.encode()) == change['body_sha256_after']
    modified = modified.replace(body, replaced, 1)
assert modified == after
assert text('retry-ceilings.patch') == ''.join(difflib.unified_diff(before.splitlines(True), after.splitlines(True), fromfile='alloc_tests.c', tofile='alloc_tests.c'))
assert len(prep['ceiling_changes']) == 6
assert sorted(c['old_ceiling'] for c in prep['ceiling_changes']) == [20,20,20,20,25,50]
for name, expected_hash in manifest['adapted_sources'].items():
    archived = 'original/alloc_tests.c' if name == 'alloc_tests.c' else 'adapted/' + name
    assert sha(files[archived]) == expected_hash, archived
for name, expected_hash in manifest['upstream_include_sources'].items():
    assert sha(files[name]) == expected_hash, name
source = load('selected-source.json')
build = load('selected-build.json')
assert source['candidate_base_commit'] == prep['selected_runtime']['measured_head'] == '0f66d54ac8418f0a6e628ad18570677a19c9ed45'
assert build['source_manifest_sha256'] == prep['selected_runtime']['source_manifest_sha256'] == sha(files['selected-source.json'])
assert prep['selected_runtime']['build_report_sha256'] == sha(files['selected-build.json'])
assert manifest['library_sha256'] == prep['selected_runtime']['library_sha256'] == build['pgo_libraries']['use/liboriole_expat.so'] == 'd3c43850247b83aa3089b7f08deb31a084ee2aecb47b52d27e9d05e97b0e1bd4'
assert prep['limits'] == {'test_seconds':15,'address_space_mib':4096,'rss_mib':3072,'total_test_seconds':300,'compile_seconds':120}
assert (manifest['per_test_seconds'],manifest['per_test_address_space_bytes'],manifest['per_test_rss_limit_bytes'],manifest['total_timeout_seconds']) == (3,1073741824,805306368,240)
local = None
if args.local:
    local_root = args.local.resolve()
    for name, expected_hash in prep['staged_files'].items():
        assert sha((local_root / name).read_bytes()) == expected_hash, name
    base = Path(prep['original_base'])
    assert prep['original_files'] == {str(p.relative_to(base)):sha(p.read_bytes()) for p in sorted(base.rglob('*')) if p.is_file()}
    assert sha((local_root / 'run01/runtests').read_bytes()) == report['binary_sha256']
    assert sha(Path(prep['selected_runtime']['library_path']).read_bytes()) == prep['selected_runtime']['library_sha256']
    assert sha(Path(prep['compile_command'][0]).read_bytes()) == prep['compiler_sha256']
    local = {'original_files_unchanged':len(prep['original_files']), 'staged_files_unchanged':len(prep['staged_files']), 'binary_library_and_compiler_hashes_verified':True}
summary = {'status':'passed','scope':'Saved artifact/source review only; no target execution. The reviewer prepared the controller but did not execute the diagnostic. Original and raised-ceiling outcomes reconstructed independently from raw logs.', 'archive_sha256':sha((root/'records.tar.gz').read_bytes()),'archive_members':len(files),'original_matrix':{'configurations':4740,'pass':4347,'assertion_fail':391,'timeout':2},'selected_original_contexts_failed':72,'diagnostic_contexts_passed':72,'changed_retry_constants':6,'other_adapted_sources_unchanged':len(manifest['adapted_sources'])-1,'library_sha256':manifest['library_sha256'],'test_log_sha256':sha(files['run01/tests.log']),'local_readback':local}
print(json.dumps(summary,indent=2))
