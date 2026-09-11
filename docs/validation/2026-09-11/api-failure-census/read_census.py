"""Summarize completed original API results and source assertions; run no targets."""
from collections import Counter, defaultdict
import csv
import hashlib
import json
import os
from pathlib import Path
import re
import shutil

assert os.sched_getaffinity(0) == {6}
A = Path('/tmp/oriole-reference-frame-api-gates/api-native/upstream-api')
W = Path('/home/dev-user/code/oss/oriole-reference-frame')
O = Path('/tmp/oriole-reference-frame-api-census')
assert not O.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
data = json.loads((A / 'results.json').read_text())
manifest = json.loads((A / 'manifest.json').read_text())
assert manifest['library_sha256'] == 'd3c43850247b83aa3089b7f08deb31a084ee2aecb47b52d27e9d05e97b0e1bd4'
assert sha(A / 'liboriole_expat.so') == manifest['library_sha256']
rows = data['results']
assert len(rows) == 4740
assert Counter(r['outcome'] for r in rows) == {'pass': 4347, 'fail': 391, 'timeout': 2}
events = []
diagnostics = {}
for line in (A / 'tests.log').read_text().splitlines():
    if line.startswith('ORIOLE_BEGIN\t'):
        _, context, test = line.split('\t')
        messages = []
    elif line.startswith('ORIOLE_RESULT\t'):
        _, result_context, result_test, outcome, code = line.split('\t')
        assert (context, test) == (result_context, result_test)
        events.append({'context': context, 'test': test, 'outcome': outcome, 'code': int(code)})
        if outcome != 'pass':
            diagnostics[(context, test)] = messages
    else:
        messages.append(line) if 'messages' in globals() else None
assert events == rows
schedule = {'test_nsalloc_realloc_binding_uri', 'test_nsalloc_realloc_long_prefix',
            'test_nsalloc_realloc_longer_prefix', 'test_alloc_realloc_buffer',
            'test_alloc_ext_entity_realloc_buffer', 'test_alloc_realloc_param_entity_newline',
            'test_alloc_realloc_ce_extends_pe'}


def category(test):
    if test in schedule:
        return 'reallocation_schedule'
    if test == 'test_nsalloc_parse_buffer':
        return 'empty_buffer_allocation_expectation'
    if test == 'test_alloc_nested_entities':
        return 'fixed_budget_child_creation'
    if test.startswith(('test_alloc_', 'test_nsalloc_')):
        return 'allocation_retry_ceiling'
    return {'test_misc_version': 'literal_version_identity',
            'test_buffer_can_grow_to_max': 'single_buffer_policy',
            'test_bypass_heuristic_when_close_to_bufsize': 'deferral_allocation_growth',
            'test_misc_input_2gb': 'bounded_timeout'}[test]


grouped = defaultdict(list)
for row in rows:
    if row['outcome'] != 'pass':
        grouped[row['test']].append(row)
counts = Counter()
named = []
source_files = set()
for test, cases in grouped.items():
    classification = category(test)
    counts[classification] += len(cases)
    evidence = {tuple(diagnostics[(row['context'], test)]) for row in cases}
    assert len(evidence) == 1
    lines = list(next(iter(evidence)))
    assertion = next((line for line in lines if line.startswith('ASSERTION: ')), None)
    if assertion:
        match = re.fullmatch(r'ASSERTION: (\S+) at (.+):(\d+)', assertion)
        assert match.group(1) == test
        file = Path(match.group(2))
        line_number = int(match.group(3))
        assert file.parent == A / 'adapted'
        assert sha(file) == manifest['adapted_sources'][file.name]
        source_line = file.read_text().splitlines()[line_number - 1].strip()
        source_files.add(file)
    else:
        assert classification == 'bounded_timeout'
        file = A / 'adapted/misc_tests.c'
        line_number = 932
        source_line = file.read_text().splitlines()[line_number - 1].strip()
        assert source_line == 'START_TEST(test_misc_input_2gb) {'
        source_files.add(file)
    named.append({'test': test, 'configurations': len(cases), 'category': classification,
                  'outcome': cases[0]['outcome'], 'contexts': [row['context'] for row in cases],
                  'assertion_or_timeout_body': f'adapted/{file.name}:{line_number}',
                  'source_line': source_line, 'raw_diagnostic': lines,
                  'demonstrated': 'Observed first assertion or bounded timeout only; later test behavior is not established by this original failing record.'})
assert dict(counts) == {'empty_buffer_allocation_expectation': 12, 'allocation_retry_ceiling': 298,
                       'reallocation_schedule': 44, 'fixed_budget_child_creation': 12,
                       'literal_version_identity': 12, 'single_buffer_policy': 12,
                       'deferral_allocation_growth': 1, 'bounded_timeout': 2}
assert len(named) == 38
O.mkdir()
(O / 'original/adapted').mkdir(parents=True)
for name in ['results.json', 'tests.log', 'manifest.json']:
    shutil.copyfile(A / name, O / 'original' / name)
for path in source_files:
    shutil.copyfile(path, O / 'original/adapted' / path.name)
shutil.copyfile(__file__, O / 'read_census.py')
shutil.copyfile('/home/dev-user/.cache/oriole/upstream/expat-2.8.4/COPYING', O / 'original/COPYING')
with (O / 'per-name.csv').open('w', newline='') as stream:
    writer = csv.DictWriter(stream, fieldnames=['test', 'configurations', 'category', 'outcome', 'assertion_or_timeout_body', 'source_line'])
    writer.writeheader()
    writer.writerows({key: row[key] for key in writer.fieldnames} for row in named)
report = {
    'status': 'saved_original_matrix_census', 'targets_executed': False,
    'measured_source_commit': '0f66d54ac8418f0a6e628ad18570677a19c9ed45',
    'source_equivalent_restacked_commit': '8b00d4cbb3f540647e1d7d795d0a04df00ed92f4',
    'library_sha256': manifest['library_sha256'], 'upstream_revision': manifest['revision'],
    'totals': {'configurations': 4740, 'passed': 4347, 'assertion_failures': 391, 'timeouts': 2,
               'nonpassing_test_names': 38, 'allocation_related_names': 34, 'allocation_related_configurations': 366},
    'categories': dict(counts), 'names': named,
    'unchanged_original_limits': {key: manifest[key] for key in ['chunks', 'deferral', 'per_test_seconds', 'per_test_address_space_bytes', 'per_test_rss_limit_bytes', 'total_timeout_seconds', 'excluded_internal_tests']},
    'source_explanations': {
        'literal_version_identity': {'path': 'crates/oriole_expat/src/lib.rs:2673', 'observed': 'Returns oriole_compat_2.8.4; original assertion requires literal expat_2.8.4. Prior numeric/version parsing checks were reached and passed.'},
        'single_buffer_policy': {'path': 'crates/oriole_expat/src/lib.rs:1737', 'observed': 'XML_GetBuffer rejects requests above min(source room, 256MiB); upstream asks for approximately 1 GiB. The original 1 GiB process address-space bound is separate. The fixed 512 MiB family allocation ceiling also remains independent.'},
        'allocation_related': {'observed': 'First assertions demonstrate original retry budgets exhausted, requested allocation/reallocation schedules differ, or child construction failed under injection. They do not prove an unsupported XML construct. No exact allocator implementation cause for every larger schedule is inferred.'},
        'deferral_allocation_growth': {'observed': 'Only chunksize=0 / deferral=1 is active and fails g_totalAlloc - alloc_before < 4096 after the large token has been fully supplied. This is a growth assertion before later element-count assertions; reservation-pressure candidates were already investigated and rejected.'},
        'bounded_timeout': {'observed': 'Only chunksize=0 contexts are active; both end with SIGALRM timeout code 14 under original 3-second wall bound. No final parser status or semantic success is inferred.'},
    },
    'conclusion': 'No new substantive XML/callback defect is established by the remaining original failures. Preserve them as nonpassing rather than inventing a parser fix or changing their assertions/bounds.',
    'coverage_limits': [
        'Classification identifies the reached assertion; an early allocation assertion can hide later content/error/cleanup assertions.',
        'Separate current 72 raised-ceiling diagnostic cases cover six unchanged later flag/text oracles; their altered budgets do not change this original matrix.',
        'Long inherited default value, nested successful declarations and phase-aware nested OOM already have permanent regressions; no repeat experiment proposed.',
        'The original narrow-character API selection excludes 12 private-only tests; no full Expat equivalence or all allocation-failure ordinals claim.',
    ],
    'readiness_actions': [
        'Carry exact original failures and separate successful semantic diagnostics into compatibility documentation.',
        'Before default CPython/PBS substitution, validate selected source on supported distribution/platform/architecture combinations and current sustained adversarial/sanitizer/fuzz campaigns; historical source-specific evidence is not automatically current.',
        'Document or resolve the two strict CPython callback-grouping differences for the intended opt-in integration; they remain failing suites.',
        'Continue real-corpus performance work: matched PGO native 1.326116x and combined CPython 1.117937x Expat still miss roughly 1.10x. Avoid repeating rejected reservation-pressure experiments.',
        'If a substantive defect appears in independent malformed-input/differential review, add the smallest reproducer and fix that established cause; this census supplies no such cause.',
    ],
    'input_sha256': {str(path): sha(path) for path in [A / 'results.json', A / 'tests.log', A / 'manifest.json', *sorted(source_files)]},
}
(O / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
table = '\n'.join(f"| `{row['test']}` | {row['configurations']} | {row['category'].replace('_', ' ')} | `{row['assertion_or_timeout_body']}` |" for row in named)
(O / 'README.md').write_text('''# Remaining original API failures

Saved-record census of selected reference-frame PGO library `d3c43850247b83aa3089b7f08deb31a084ee2aecb47b52d27e9d05e97b0e1bd4`, measured source `0f66d54ac8418f0a6e628ad18570677a19c9ed45` (source-equivalent restack `8b00d4cbb3f540647e1d7d795d0a04df00ed92f4`). No parser, compiler or test target was executed for this census.

**No new substantive XML/callback defect is established by these remaining failures.** The original matrix remains **4,347 passes, 391 assertion failures and two timeouts** across 4,740 configurations / 395 test bodies. The 393 nonpassing configurations span 38 test names. This classification does not turn any failing result into a pass.

| Reached failure category | Configurations |
| --- | ---: |
| Allocation retry ceilings | 298 |
| Expected realloc schedule | 44 |
| Allocation after empty ParseBuffer | 12 |
| Fixed-budget child creation | 12 |
| **Allocation-related subtotal** | **366** |
| Literal implementation version identity | 12 |
| Fixed single-buffer resource policy | 12 |
| Deferral allocation-growth assertion | 1 |
| Three-second 2 GiB timeout | 2 |

Original bounds remain six chunks (0–5), two deferral settings, three seconds per case, 1 GiB address space, 768 MiB RSS, 240 seconds overall, and the same twelve private-test exclusions. Timeout is not successful completion.

## What the records establish

The raw log and result JSON agree on all 4,740 ordered outcomes. The assertion locations below and their original source bytes match the producer manifest. The allocator-related records identify exhausted budgets or a different allocation schedule; they do not establish that the associated XML construct is unsupported. They also do not establish later assertions that execution did not reach. Exact allocator causes and all later failure ordinals are not inferred from the test name.

`test_misc_version` reaches its final literal comparison with `expat_2.8.4`; numeric and parsed versions already agree. `XML_ExpatVersion` deliberately returns `oriole_compat_2.8.4` (`crates/oriole_expat/src/lib.rs:2673`). `test_buffer_can_grow_to_max` asks for roughly 1 GiB while `XML_GetBuffer` caps one request at 256 MiB (`:1737`, constant at `:39`); the 512 MiB live family limit is independent. Removing identity or resource policies solely to improve the score is not a semantic fix.

The sole deferral failure is `g_totalAlloc - alloc_before < 4096`, not an XML acceptance or callback-value assertion. Its buffer-pressure mechanism and three performance candidates were already investigated in `docs/validation/2026-09-11/c-reservation-experiments/`; they were rejected. The two active `test_misc_input_2gb` contexts end under the unchanged three-second alarm, without a final parser outcome.

The separate current 72-configuration raised-ceiling replay reaches original flag/text oracles in six selected tests. It is supplementary semantic evidence with changed diagnostic budgets; this original matrix remains unchanged. Long inherited default values, successful nested entities and phase-aware nested OOM already have permanent C regressions. Those resolved coverage gaps are not new fix candidates.

## Remaining readiness work

- Preserve this exact census and the separate diagnostic scope in compatibility documentation; avoid implying 391 independent semantic defects or full compatibility.
- Keep the two strict CPython callback-grouping failures explicit for the intended opt-in deployment.
- Complete selected-source distribution/platform validation and sustained adversarial, fuzz and sanitizer coverage before a default PBS substitution; older source-specific reports do not automatically establish the current build.
- Continue measured performance work: PGO native 1.326116× and combined CPython 1.117937× Expat remain outside roughly 1.10×. No new narrow runtime compatibility fix is justified by this census alone.

## Exact original test assertions

Paths below are relative to `original/`. For timeouts, the location is the test body, since no source assertion fired. `per-name.csv` additionally records the exact source line; `report.json` retains raw diagnostics and every nonpassing context.

| Original test | Configurations | Category | Assertion / body |
| --- | ---: | --- | --- |
''' + table + '''

`original/` retains the matrix, log, manifest, five referenced adapted sources and their Expat license. `read_census.py` is the saved data-only summarizer; its absolute paths identify the original machine. Original producer receipts and source hashes remain authoritative. No repository source or upstream assertion was edited.
''')
print(json.dumps({'status': 'completed_saved_census', 'output': str(O), 'counts': dict(counts), 'names': len(named), 'files': {str(path.relative_to(O)): sha(path) for path in sorted(O.rglob('*')) if path.is_file()}}, indent=2))
