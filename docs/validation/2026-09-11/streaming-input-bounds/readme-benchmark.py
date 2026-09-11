from pathlib import Path
import hashlib
import json
import statistics

worktree = Path('/home/dev-user/code/oss/oriole-streaming-input-bounds')
destination = worktree / 'docs/validation/2026-09-11/streaming-input-bounds'
source = Path('/tmp/oriole-streaming-work-pgo-native-study/native-screen/results.json')
data = json.loads(source.read_text())
assert data['status'] == 'passed'
labels = [('vulkan', 'Vulkan registry'), ('wayland', 'Wayland protocol'), ('maven', 'Maven POM'), ('batik', 'Batik SVG'), ('gtk', 'GTK UI'), ('docbook', 'DocBook XSL')]
rows = []
table = ['| Project XML | Oriole (PGO) | Expat (PGO) | Oriole / Expat |', '| --- | ---: | ---: | ---: |']
for name, label in labels:
    groups = [r for r in data['rows'] if r['condition']['name'] == name and r['condition']['chunk'] == 4096 and not r['condition']['namespaces']]
    assert len(groups) == 7 and {r['pair'] for r in groups} == set(range(7))
    medians = {engine: [] for engine in ('candidate', 'expat')}
    paired = []
    for group in sorted(groups, key=lambda r: r['pair']):
        pair = {}
        for process in group['processes']:
            if process['engine'] not in medians:
                continue
            samples = json.loads(process['stdout'])['samples']
            median = statistics.median(sample['seconds'] for sample in samples if not sample['warmup'])
            assert median == process['median_seconds'] and process['returncode'] == 0
            medians[process['engine']].append(median)
            pair[process['engine']] = median
        paired.append(pair['candidate'] / pair['expat'])
    ratio = statistics.median(paired)
    row = {'name': name, 'label': label, 'chunk': 4096, 'namespaces': False, 'process_medians_seconds': medians, 'paired_ratios': paired, 'oriole_ms': statistics.median(medians['candidate']) * 1000, 'expat_ms': statistics.median(medians['expat']) * 1000, 'ratio': ratio}
    rows.append(row)
    table.append(f"| {label} | {row['oriole_ms']:.3f} ms | {row['expat_ms']:.3f} ms | {ratio:.2f}× |")
report = {'source': str(source), 'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'rows': rows, 'method': 'Seven process medians per engine; independently recomputed from raw non-warmup samples. Table ratios are medians of paired ratios. This is a six-condition display subset of the full24-condition real cohort.'}
(destination / 'readme-benchmark.json').write_text(json.dumps(report, indent=2) + '\n')
readme = worktree / 'README.md'
text = readme.read_text()
start = text.index('| Project XML |')
end = text.index('ThinLTO is the default')
replacement = '\n'.join(table) + """

These [native measurements](docs/validation/2026-09-11/streaming-input-bounds/) use original XML from six pinned projects, 4 KiB chunks and namespaces disabled on a shared Linux AMD EPYC-Milan host. Both parsers use PGO trained on generated XML; these projects are held out of training. Times are medians of seven process medians; ratios are medians of paired ratios. Expat is version 2.8.4.

Across all 24 real-project conditions, Oriole takes **1.38× Expat PGO's native time and 1.14× its CPython time**. The new streaming policy is effectively flat against the preceding parser: PGO native and CPython time each decrease by 0.3%. Normal native time decreases by 1.4%, while normal CPython time is flat. Generated native controls regress by 1.2% normally and 0.6% with PGO. The [full report](docs/validation/2026-09-11/streaming-input-bounds/) retains every condition, sample and adverse result.

"""
text = text[:start] + replacement + text[end:]
start = text.index('The [latest compatibility report]')
end = text.index('The [Windows test portability supplement]')
text = text[:start] + """The [latest compatibility report](docs/validation/2026-09-11/streaming-input-bounds/) records 420 workspace tests, one doc test, and **4,347 passing / 391 failing / two timed-out upstream API configurations**. Shared and static CPython each retain the same two text-grouping failures across 802 method outcomes. Actual incremental streams through 257 MiB and a separate 2,049 MiB text stream match Expat's checked output and positions with constant tracked allocation peaks. Remaining allocation, resource, diagnostic, and callback differences are explicit in the [compatibility guide](docs/compatibility.md).

Six [sustained ASan campaigns](docs/validation/2026-09-11/current-asan/) completed 10,496,706 executions on the preceding `be22a27` runtime without findings. The [`be22a27` PBS distribution](docs/validation/2026-09-11/current-pbs/) loads both XML accelerators on glibc 2.17 and passes 1,024 threaded parses; its XML suite retains the same two grouping failures.

""" + text[end:]
readme.write_text(text)
(destination / 'readme-benchmark.py').write_bytes(Path(__file__).read_bytes())
print('\n'.join(table))
