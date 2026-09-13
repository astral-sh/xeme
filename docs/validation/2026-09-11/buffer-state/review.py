"""Read saved records only; adapted from allocation-semantic-coverage/review.py."""
from pathlib import Path
import argparse
import collections
import csv
import difflib
import hashlib
import io
import json
import re
import tarfile

parser = argparse.ArgumentParser()
parser.add_argument('--local', type=Path, help='Also hash preserved local inputs, compiler and binaries')
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
name = 'test_nsalloc_parse_buffer'
expected_order = [(f'chunksize={chunk} deferral={mode}', name) for chunk in range(6) for mode in range(2)]
expected = set(expected_order)
assert prep['selected_tests'] == [name] and prep['expected_configurations'] == 12
records = lambda value: [dict(context=c, test=t, outcome=o, code=int(n)) for c, t, o, n in re.findall(r'^ORIOLE_RESULT\t([^\t\n]+)\t([^\t\n]+)\t([^\t\n]+)\t(\d+)$', value, re.M)]
raw = text('run01/tests.log')
rows = records(raw)
assert rows == report['results'] and len(rows) == 12
assert [(r['context'], r['test']) for r in rows] == expected_order
assert all(r['outcome'] == 'pass' and r['code'] == 0 for r in rows)
begins = re.findall(r'^ORIOLE_BEGIN\t([^\t\n]+)\t([^\t\n]+)$', raw, re.M)
assert begins == expected_order
blocks = list(re.finditer(r'^ORIOLE_BEGIN\t([^\t\n]+)\t([^\t\n]+)\n(.*?)^ORIOLE_RESULT\t\1\t\2\t([^\n]+)\n', raw, re.M | re.S))
assert len(blocks) == 12
observations = []
for block in blocks:
    found = re.findall(r'^ORIOLE_EMPTY_BUFFER_OBSERVATION\tstatus=(-?\d+)\terror=(-?\d+)\tallocation_count=(-?\d+)$', block[3], re.M)
    assert len(found) == 1 and found[0] == ('1', '0', '0')
    status, error, count = map(int, found[0])
    observations.append({'context': block[1], 'test': block[2], 'status': status, 'error': error, 'allocation_count': count})
assert observations == report['empty_buffer_observations']
assert raw.count('ORIOLE_EMPTY_BUFFER_OBSERVATION') == 12
assert report['status'] == 'complete' and report['compile_exit_code'] == report['run_exit_code'] == 0
for key in ['preflight_passed', 'selection_complete', 'library_origin_verified', 'library_unchanged', 'original_tree_unchanged', 'all_diagnostic_contexts_passed', 'original_tree_unchanged_on_exit', 'observation_selection_complete']:
    assert report[key] is True, key
origins = re.findall(r'^ORIOLE_LIBRARY\t(.+)$', raw, re.M)
assert origins == report['library_origins'] == ['/tmp/oriole-nsalloc-buffer-tail-diagnostic/liboriole_expat.so']
assert report['controller_sha256'] == sha(files['run.py'])
assert report['preparation_sha256'] == sha(files['preparation.json'])
for command, log in zip(report['commands'], ['run01/build.log', 'run01/tests.log'], strict=True):
    assert command['exit_code'] == 0 and command['log_sha256'] == sha(files[log])
    assert command['environment'] == {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C', 'TMPDIR': '/tmp', **prep['environment']}
assert [c['timeout_seconds'] for c in report['commands']] == [120, 240]
assert report['commands'][0]['argv'] == prep['compile_command']
assert files['run01/build.log'] == b''
assert report['commands'][1]['argv'] == ['/tmp/oriole-nsalloc-buffer-tail-diagnostic/run01/runtests', '--verbose']

# Reconstruct the entire unchanged original matrix, including its raw ordering.
original_raw = records(text('original/tests.log'))
assert original_raw == original['results'] and len(original_raw) == 4740
assert collections.Counter(r['outcome'] for r in original_raw) == {'pass': 4347, 'fail': 391, 'timeout': 2}
original_rows = [r for r in original_raw if r['test'] == name]
assert len(original_rows) == 12 and {(r['context'], r['test']) for r in original_rows} == expected
assert all(r['outcome'] == 'fail' and r['code'] == 100 for r in original_rows)
assert records(text('original-selected-failures.log')) == original_rows
assert len(re.findall(r'^ASSERTION: test_nsalloc_parse_buffer at .*nsalloc_tests.c:129$', text('original-selected-failures.log'), re.M)) == 12
assert prep['original_matrix_sha256'] == sha(files['original-results.json'])
assert prep['original_files']['tests.log'] == sha(files['original/tests.log'])

# Reconstruct the complete source delta independently of the preparer's code.
before = text('original/nsalloc_tests.c')
after = text('adapted/nsalloc_tests.c')
old = '''  g_allocation_count = 0;
  if (XML_ParseBuffer(g_parser, 0, XML_FALSE) != XML_STATUS_ERROR)
    fail("Pre-init XML_ParseBuffer not faulted");
  if (XML_GetErrorCode(g_parser) != XML_ERROR_NO_MEMORY)
    fail("Pre-init XML_ParseBuffer faulted for wrong reason");'''
new = '''  g_allocation_count = 0;
  {
    const enum XML_Status status = XML_ParseBuffer(g_parser, 0, XML_FALSE);
    const enum XML_Error error = XML_GetErrorCode(g_parser);
    fprintf(stderr,
            "ORIOLE_EMPTY_BUFFER_OBSERVATION\\tstatus=%d\\terror=%d"
            "\\tallocation_count=%d\\n",
            (int)status, (int)error, g_allocation_count);
  }'''
assert before.count(old) == 1 and before.count('#include <string.h>') == 1
assert before.replace(old, new).replace('#include <string.h>', '#include <string.h>\n#include <stdio.h>', 1) == after
assert text('diagnostic.patch') == ''.join(difflib.unified_diff(before.splitlines(True), after.splitlines(True), fromfile='original/nsalloc_tests.c', tofile='diagnostic/nsalloc_tests.c'))
body = lambda s: s[s.index('START_TEST(' + name + ')'):s.index('END_TEST', s.index('START_TEST(' + name + ')')) + len('END_TEST')]
old_body, new_body = body(before), body(after)
tail = '  /* Now with actual memory allocation */'
assert old_body[old_body.index(tail):] == new_body[new_body.index(tail):]
assert re.findall(r'\b(XML_\w+)\s*\(', old_body) == re.findall(r'\b(XML_\w+)\s*\(', new_body) == prep['source_change']['parser_api_call_sequence_preserved']
assert sha(old_body.encode()) == prep['source_change']['old_body_sha256']
assert sha(new_body.encode()) == prep['source_change']['new_body_sha256']
for source_name, expected_hash in manifest['adapted_sources'].items():
    archived = 'original/nsalloc_tests.c' if source_name == 'nsalloc_tests.c' else 'adapted/' + source_name
    assert sha(files[archived]) == expected_hash, archived
for source_name, expected_hash in manifest['upstream_include_sources'].items():
    assert sha(files[source_name]) == expected_hash, source_name

# Preserve and describe the preparation-only header correction accurately.
first = load('preparation-attempt01.json')
assert prep['preparation_correction']['first_preparation_sha256'] == sha(files['preparation-attempt01.json'])
assert prep['preparation_correction']['first_source_patch_sha256'] == sha(files['diagnostic-attempt01.patch'])
assert first['staged_files']['adapted/nsalloc_tests.c'] == sha(before.replace(old, new).encode())
assert prep['preparation_correction']['target_executions'] == 0
assert first['status'] == 'prepared_not_executed'
assert files['preparation-attempt01.stderr'] == b''
assert json.loads(text('preparation-attempt01.stdout'))['preparation_sha256'] == sha(files['preparation-attempt01.json'])

source = load('selected-source.json')
build = load('selected-build.json')
assert source['candidate_base_commit'] == prep['selected_runtime']['measured_head'] == '0f66d54ac8418f0a6e628ad18570677a19c9ed45'
assert build['source_manifest_sha256'] == prep['selected_runtime']['source_manifest_sha256'] == sha(files['selected-source.json'])
assert prep['selected_runtime']['build_report_sha256'] == sha(files['selected-build.json'])
assert manifest['library_sha256'] == prep['selected_runtime']['library_sha256'] == build['pgo_libraries']['use/liboriole_expat.so'] == 'd3c43850247b83aa3089b7f08deb31a084ee2aecb47b52d27e9d05e97b0e1bd4'
assert prep['limits'] == {'test_seconds': 3, 'address_space_mib': 1024, 'rss_mib': 768, 'total_test_seconds': 240, 'compile_seconds': 120}
assert (manifest['per_test_seconds'], manifest['per_test_address_space_bytes'], manifest['per_test_rss_limit_bytes'], manifest['total_timeout_seconds']) == (3, 1073741824, 805306368, 240)

# Inventory coverage and prior six-case evidence; manual reachability claims remain a source review.
inventory = list(csv.DictReader(io.StringIO(text('reachability/inventory.csv'))))
census = load('census/report.json')
reach = load('reachability/report.json')
assert len(inventory) == 34 and sum(int(r['configurations']) for r in inventory) == 366
assert {r['test'] for r in inventory} == {r['test'] for r in census['names'] if r['test'].startswith(('test_alloc_', 'test_nsalloc_'))}
assert [r['test'] for r in inventory] == [r['test'] for r in reach['allocation_names']]
assert reach['additional_post_loop_text_or_flag_bodies'] == 0
six = {'test_alloc_dtd_default_handling', 'test_alloc_public_entity_value', 'test_alloc_notation', 'test_alloc_public_notation', 'test_alloc_system_notation', 'test_alloc_nested_groups'}
six_rows = records(text('prior-six/run01/tests.log'))
assert six_rows == load('prior-six/run01/report.json')['results'] and len(six_rows) == 72
assert {r['test'] for r in six_rows} == six and all(r['outcome'] == 'pass' and r['code'] == 0 for r in six_rows)
assert {(r['context'], r['test']) for r in six_rows} == {(context, test) for context, _ in expected for test in six}
assert load('prior-six/preparation.json')['selected_runtime']['library_sha256'] == manifest['library_sha256']
assert {r['test'] for r in inventory if r['current_replay'] == 'True'} == six
for suffix, archived in [('alloc_tests.c', 'adapted/alloc_tests.c'), ('nsalloc_tests.c', 'original/nsalloc_tests.c'), ('handlers.c', 'adapted/handlers.c'), ('basic_tests.c', 'adapted/basic_tests.c')]:
    assert reach['input_sha256']['/tmp/oriole-reference-frame-api-census/original/adapted/' + suffix] == sha(files[archived])
for relative in ['tests/c/integration.c', 'crates/oriole_expat/src/tests.rs']:
    assert reach['input_sha256']['/home/dev-user/code/oss/oriole-reference-frame/' + relative] == sha(files['selected-source/' + relative])

local = None
if args.local:
    local_root = args.local.resolve()
    for staged, expected_hash in prep['staged_files'].items():
        assert sha((local_root / staged).read_bytes()) == expected_hash, staged
    base = Path(prep['original_base'])
    assert prep['original_files'] == {str(p.relative_to(base)): sha(p.read_bytes()) for p in sorted(base.rglob('*')) if p.is_file()}
    assert sha((local_root / 'run01/runtests').read_bytes()) == report['binary_sha256']
    assert sha((local_root / 'liboriole_expat.so').read_bytes()) == prep['selected_runtime']['library_sha256']
    assert sha(Path(prep['selected_runtime']['library_path']).read_bytes()) == prep['selected_runtime']['library_sha256']
    assert sha(Path(prep['compile_command'][0]).read_bytes()) == prep['compiler_sha256']
    assert sha((local_root / 'run01/tests.log').read_bytes()) == sha(files['run01/tests.log'])
    assert sha((local_root / 'preparation.json').read_bytes()) == report['preparation_sha256']
    local = {'original_files_unchanged': len(prep['original_files']), 'staged_files_unchanged': len(prep['staged_files']), 'binary_both_library_copies_and_compiler_hashes_verified': True}

summary = {'status': 'passed', 'scope': 'Saved artifact/source review only; no target execution. Reviewer prepared the diagnostic but did not execute it. Full original matrix, twelve diagnostic outcomes and observations reconstructed from raw logs; source delta and immutable local pins checked separately from controller conclusions.', 'archive_sha256': sha((root / 'records.tar.gz').read_bytes()), 'archive_members': len(files), 'original_matrix': {'configurations': 4740, 'pass': 4347, 'assertion_fail': 391, 'timeout': 2}, 'selected_original_contexts_failed': 12, 'diagnostic_contexts_passed': 12, 'empty_buffer_observations': {'count': 12, 'status': 1, 'error': 0, 'allocation_count': 0}, 'tail_byte_identical': True, 'other_adapted_sources_unchanged': len(manifest['adapted_sources']) - 1, 'retry_constants_changed': 0, 'header_correction_preserved': True, 'allocation_names_reviewed': 34, 'prior_six_diagnostic_passes': 72, 'library_sha256': manifest['library_sha256'], 'test_log_sha256': sha(files['run01/tests.log']), 'local_readback': local}
print(json.dumps(summary, indent=2))
