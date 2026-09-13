from pathlib import Path
import hashlib
import json
import statistics

worktree = Path('/home/dev-user/code/oss/oriole-literal-attribute-check')
destination = worktree / 'docs/validation/2026-09-11/literal-attribute-check'
source = Path('/tmp/oriole-literal-attribute-pgo-native-study/native-screen/results.json')
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
end = text.index('A separate [C allocator study]')
replacement = '\n'.join(table) + '''

These [native measurements](docs/validation/2026-09-11/literal-attribute-check/) use original XML from six pinned projects, 4 KiB chunks and namespaces disabled on a shared Linux AMD EPYC-Milan host. Both parsers use PGO trained on generated XML; these projects are held out of training. Times are medians of seven process medians; ratios are medians of paired ratios. Expat is version 2.8.4.

Across all 24 real-project conditions, Oriole takes **1.38× Expat PGO's native time and 1.13× its CPython time**. The literal-attribute optimization reduces time by 2.1% and 1.2%, respectively, against the preceding parser. The normal build is flat overall, with a 2.1% regression on generated native controls. The [full report](docs/validation/2026-09-11/literal-attribute-check/) retains every condition, sample and adverse result.

ThinLTO is the default, and [PGO is opt-in](tools/pgo/). The [PGO/LTO study](benchmarks/results/2026-09-11/pgo-lto/) on the preceding `be22a27` runtime measured PGO reductions of 24.8% natively and 15.5% through CPython. Fat LTO alone added little; combining it with PGO regressed native time by 6.8%.

'''
readme.write_text(text[:start] + replacement + text[end:])
(destination / 'readme-benchmark.py').write_bytes(Path(__file__).read_bytes())
print('\n'.join(table))
