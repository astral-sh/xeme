"""Publish the reviewed Finder summary and recompute its README display subset."""
from pathlib import Path
import hashlib
import json
import shutil
import statistics

worktree = Path('/home/dev-user/code/oss/oriole-cdata-finder')
destination = worktree / 'docs/validation/2026-09-11/cdata-finder'
destination.mkdir(parents=True, exist_ok=True)
source = Path('/tmp/oriole-cdata-finder-pgo-native-study/native-screen/results.json')
data = json.loads(source.read_bytes())
assert data['status'] == 'passed'
labels = [('vulkan', 'Vulkan registry'), ('wayland', 'Wayland protocol'), ('maven', 'Maven POM'), ('batik', 'Batik SVG'), ('gtk', 'GTK UI'), ('docbook', 'DocBook XSL')]
rows = []
table = ['| Project XML | Oriole (PGO) | Expat (PGO) | Oriole / Expat |', '| --- | ---: | ---: | ---: |']
for name, label in labels:
    groups = [row for row in data['rows'] if row['condition']['name'] == name and row['condition']['chunk'] == 4096 and not row['condition']['namespaces']]
    assert len(groups) == 7 and {row['pair'] for row in groups} == set(range(7))
    medians = {engine: [] for engine in ('candidate', 'expat')}
    paired = []
    for group in sorted(groups, key=lambda row: row['pair']):
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
report = {'source': str(source), 'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'rows': rows, 'method': 'Seven process medians per engine, recomputed from all raw non-warmup samples. Ratios are medians of paired ratios. Six display conditions from the full24-condition real cohort.'}
(destination / 'readme-benchmark.json').write_text(json.dumps(report, indent=2) + '\n')
(destination / 'publication.py').write_bytes(Path(__file__).read_bytes())
shutil.copyfile('/tmp/oriole-cdata-finder-native-decision/decision.json', destination/'decision.json')

readme = worktree/'README.md'
text = readme.read_text()
old = text
start = text.index('| Project XML |')
end = text.index('ThinLTO is the default')
replacement = '\n'.join(table) + '''

These [native measurements](docs/validation/2026-09-11/cdata-finder/) use original XML from six pinned projects, 4 KiB chunks and namespaces disabled on a shared Linux AMD EPYC-Milan host. Both parsers use PGO trained on generated XML; these projects are held out of training. Times are medians of seven process medians; ratios are medians of paired ratios. Expat is version 2.8.4.

Across all 24 real-project conditions, Oriole takes **1.37× Expat PGO's native time and 1.13× its CPython time**. Reusing the CDATA searcher reduces PGO native and CPython time by 0.5% each. Normal native time decreases by 1.0%, while normal CPython time increases by 0.4%. Generated native controls improve by 3.3% normally and 2.3% with PGO. The [full report](docs/validation/2026-09-11/cdata-finder/) retains every condition, sample and adverse result.

'''
text = text[:start] + replacement + text[end:]
text = text.replace('Fat LTO alone added little; combining it with PGO regressed native time by 6.8%.', 'Fat LTO alone added little; combining it with PGO regressed native time by 6.8%. A later [O2 experiment](docs/validation/2026-09-11/cdata-finder/rejected-o2/) regressed PGO time by 3.1%, so O3 remains selected.')
text = text.replace('The [latest compatibility report](docs/validation/2026-09-11/streaming-input-bounds/)', 'The [latest compatibility report](docs/validation/2026-09-11/cdata-finder/)')
text = text.replace('Actual incremental streams through 257 MiB and a separate 2,049 MiB text stream match Expat\'s checked output and positions with constant tracked allocation peaks.', 'The preceding [streaming-policy validation](docs/validation/2026-09-11/streaming-input-bounds/) matched Expat on incremental streams through 257 MiB and a separate 2,049 MiB text stream with constant tracked allocation peaks.')
text = text.replace('Seven [current-source ASan campaigns]', 'Seven [streaming-runtime ASan campaigns]')
text = text.replace('Each campaign ran for 120 seconds;', 'These tested `708ca42`, before the CDATA search optimization, for 120 seconds each;')
text = text.replace('uses the selected streaming runtime, runs on glibc 2.17', 'uses that preceding streaming runtime, runs on glibc 2.17')
assert [line for line in old.splitlines() if line.startswith('#')] == [line for line in text.splitlines() if line.startswith('#')]
assert old[old.index('## License'):] == text[text.index('## License'):]
assert [line for line in old.splitlines() if line.startswith('>')] == [line for line in text.splitlines() if line.startswith('>')]
readme.write_text(text)
print('\n'.join(table))
