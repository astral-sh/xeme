"""Deterministic current-source ASan evidence package; no target execution."""
import gzip
import hashlib
import io
import json
from pathlib import Path
import shutil
import sys
import tarfile

PREP = Path('/tmp/oriole-streaming-policy-asan-preparation')
STUDY = Path('/tmp/oriole-streaming-policy-asan-study')
OUT = Path('/tmp/oriole-streaming-policy-asan-package')
REPORT = Path('/home/dev-user/code/oss/oriole-streaming-input-bounds/docs/validation/2026-09-11/streaming-input-bounds')
sys.path.insert(0, str(PREP))
from common import check_preparation, check_source

def digest(data):
    return hashlib.sha256(data).hexdigest()

def encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':')) + '\n').encode()

def load(path):
    return json.loads(path.read_bytes())

check_preparation()
source = check_source()
assert load(STUDY / 'summary.json')['status'] == 'passed'
OUT.mkdir(exist_ok=False)
members = {}
member_origins = {}
omitted = []
blobs = {}
blob_origins = {}
layouts = {'current': {}, 'historical_seed_inputs': {}}

def add(name, data, origin):
    assert name not in members and not name.startswith('/') and '..' not in Path(name).parts
    members[name] = data
    member_origins[name] = origin

def add_file(name, path):
    assert path.is_file() and not path.is_symlink()
    add(name, path.read_bytes(), {'path': str(path)})

def omit(path, reason):
    data = path.read_bytes()
    omitted.append({'path': str(path), 'bytes': len(data), 'sha256': digest(data), 'reason': reason})

for label, root in [('preparation', PREP), ('study', STUDY)]:
    for path in sorted(root.rglob('*')):
        if not path.is_file():
            continue
        relative = path.relative_to(root)
        if '__pycache__' in relative.parts:
            continue
        if label == 'study' and relative.parts[0] == 'binaries':
            omit(path, 'Compiled fuzz executable; hash retained, bytes excluded.')
            continue
        if label == 'study' and relative.parts[0] == 'campaigns' and relative.parts[2] in ('initial-corpus', 'corpus'):
            continue
        if relative.as_posix() == 'source.tar.gz':
            omit(path, 'Current source archive unpacked once into source/; no nested archive.')
            continue
        assert path.suffix not in ('.gz', '.zip', '.a', '.so'), str(path)
        add_file(label + '/' + relative.as_posix(), path)

with tarfile.open(PREP / 'source.tar.gz', 'r:gz') as archive:
    seen = set()
    for member in archive:
        assert member.isfile() and member.name in source['source_sha256']
        assert member.name not in seen
        seen.add(member.name)
        data = archive.extractfile(member).read()
        assert digest(data) == source['source_sha256'][member.name]
        add('source/' + member.name, data, {'archive': str(PREP / 'source.tar.gz'), 'member': member.name})
    assert seen == set(source['source_sha256']) and len(seen) == 70
assert (STUDY / 'source.tar.gz').read_bytes() == (PREP / 'source.tar.gz').read_bytes()

def corpus(directory, expected):
    actual = {p.name for p in directory.iterdir() if p.is_file()}
    assert actual == set(expected), str(directory)
    mapping = {}
    for name, expected_hash in sorted(expected.items()):
        path = directory / name
        data = path.read_bytes()
        key = digest(data)
        assert key == expected_hash
        if key in blobs:
            assert blobs[key] == data
        else:
            blobs[key] = data
        blob_origins.setdefault(key, set()).add(str(path))
        mapping[name] = key
    return mapping

current_results = {}
for target in ['parse', 'streaming', 'ffi', 'ffi_family', 'multibyte', 'value_family', 'streaming_work']:
    directory = STUDY / 'campaigns' / target
    initial = load(directory / 'initial-manifest.json')
    result = load(directory / 'result.json')
    assert result['status'] == 'passed' and not result['artifacts']
    assert result['initial_inputs_unchanged'] and result['binary_unchanged'] and result['source_unchanged']
    current_results[target] = result
    layouts['current'][target] = {
        'initial-corpus': corpus(directory / 'initial-corpus', {h: h for h in initial['inputs']}),
        'corpus': corpus(directory / 'corpus', result['final_corpus']),
    }
    assert all(len(blobs[h]) == size for h, size in initial['inputs'].items())
    for key, origins in initial['origins'].items():
        for origin in origins:
            path = Path(origin['path'])
            assert digest(path.read_bytes()) == key
            blob_origins[key].add(str(path))

provenance = load(PREP / 'corpus-provenance.json')
for target, origin in sorted(provenance['targets'].items()):
    initial_path, final_path = Path(origin['initial_manifest']), Path(origin['result'])
    assert digest(initial_path.read_bytes()) == origin['initial_manifest_sha256']
    assert digest(final_path.read_bytes()) == origin['result_sha256']
    add_file('historical-seed-metadata/' + target + '/initial-manifest.json', initial_path)
    add_file('historical-seed-metadata/' + target + '/result.json', final_path)
    initial, final = load(initial_path), load(final_path)
    layouts['historical_seed_inputs'][target] = {
        'initial-corpus': corpus(Path(origin['initial_directory']), {h: h for h in initial['inputs']}),
        'corpus': corpus(Path(origin['final_directory']), final['final_corpus']),
    }
    assert all(len(blobs[h]) == size for h, size in initial['inputs'].items())

for key, data in sorted(blobs.items()):
    add('corpus-by-sha256/' + key, data, {'origin_map': 'corpus-origin-map.json', 'key': key})
add('corpus-layouts.json', encoded({
    'format': 'For each logical filename, obtain bytes from corpus-by-sha256/<sha256>. No corpus filename is discarded.',
    'layouts': layouts,
}), {'generated_by': 'package-streaming-policy-asan.py'})
add('corpus-origin-map.json', encoded({key: sorted(paths) for key, paths in sorted(blob_origins.items())}),
    {'generated_by': 'package-streaming-policy-asan.py'})
add('omissions.json', encoded({'files': omitted, 'untraversed': ['Cargo target and shared intermediate caches'],
    'scope': 'Historical archives and binaries are not included; only exact seed metadata and reconstructable seed bytes are included.'}),
    {'generated_by': 'package-streaming-policy-asan.py'})
add_file('package-streaming-policy-asan.py', Path(__file__))
add_file('summarize-streaming-policy-asan.py', Path('/tmp/oriole-summarize-streaming-policy-asan.py'))
add_file('prepare-streaming-policy-asan.py', Path('/tmp/oriole-prepare-streaming-policy-asan.py'))
archive_path = OUT / 'asan-evidence.tar.gz'
with archive_path.open('xb') as raw:
    with gzip.GzipFile(fileobj=raw, filename='', mode='wb', mtime=0, compresslevel=9) as zipped:
        with tarfile.open(fileobj=zipped, mode='w', format=tarfile.USTAR_FORMAT) as archive:
            for name, data in sorted(members.items()):
                member = tarfile.TarInfo(name)
                member.size = len(data)
                member.mode = 0o444
                member.uid = member.gid = member.mtime = 0
                archive.addfile(member, io.BytesIO(data))
archive_hash = digest(archive_path.read_bytes())
index = {'archive': archive_path.name, 'sha256': archive_hash, 'members': [
    {'name': name, 'bytes': len(data), 'sha256': digest(data), 'origin': member_origins[name]}
    for name, data in sorted(members.items())]}
index_path = OUT / 'asan-evidence-members.json.gz'
with index_path.open('xb') as raw:
    with gzip.GzipFile(fileobj=raw, filename='', mode='wb', mtime=0, compresslevel=9) as zipped:
        zipped.write(encoded(index))

# Read every compressed index row and archive member; validate reconstruction using readback bytes.
assert json.loads(gzip.decompress(index_path.read_bytes())) == index
readback = {}
with tarfile.open(archive_path, 'r:gz') as archive:
    for member in archive:
        assert member.isfile() and member.name not in readback
        data = archive.extractfile(member).read()
        assert data == members[member.name]
        assert member.mode == 0o444 and member.uid == member.gid == member.mtime == 0
        readback[member.name] = data
assert set(readback) == set(members)
for group in layouts.values():
    for phases in group.values():
        for mapping in phases.values():
            for key in mapping.values():
                assert digest(readback['corpus-by-sha256/' + key]) == key
check_preparation()
check_source()

summary = load(STUDY / 'summary.json')
compact = {key: summary[key] for key in [
    'status', 'role', 'source_manifest_sha256', 'preparation_sha256', 'source_and_preparation_unchanged',
    'first_attempts_only', 'selected_policy_regressions_passed', 'original_harnesses', 'additional_harness',
    'campaign_seconds_per_target', 'campaign_seed', 'totals', 'targets', 'evidence', 'limitations']}
compact['package'] = {
    'archive': archive_path.name, 'sha256': archive_hash, 'bytes': archive_path.stat().st_size,
    'members': len(members), 'index': index_path.name, 'index_sha256': digest(index_path.read_bytes()),
    'unique_corpus_blobs': len(blobs), 'unique_corpus_bytes': sum(map(len, blobs.values())),
    'reconstructable_corpus_files': sum(len(mapping) for group in layouts.values() for phases in group.values() for mapping in phases.values()),
    'excluded_compiled_binaries': len([f for f in omitted if 'Compiled' in f['reason']]),
    'unpacked_source_files': len(seen), 'all_members_and_layouts_read_back': True,
    'source_sha256_map_matches_live': True,
    'packager_sha256': digest(Path(__file__).read_bytes()),
}
(OUT / 'asan.json').write_text(json.dumps(compact, indent=2) + '\n')
rows = '\n'.join(f"| `{r['target']}` | {r['replay_executions']:,} | {r['campaign_executed_units']:,} | {r['final_disk_corpus_files']:,} |" for r in compact['targets'])
text = f'''# Current streaming-policy ASan follow-up

The current source passed a fresh Rust AddressSanitizer build, 13 selected policy regressions, retained-corpus replay on seven targets, and seven new 120-second fuzz campaigns. There were no failure artifacts or target retries. This is a **collector-owned execution report**, not an independent campaign audit.

The canonical 70-file source manifest is `{compact['source_manifest_sha256']}` and remained unchanged. Actual compiler records cover the three workspace crates and all seven fuzz binaries. Symbol checks cover all seven binaries; representative parser and adapter disassembly is retained. No historical sanitizer build is treated as current-source evidence.

## Outcomes

| Target | Replay executions | Campaign executed units | Final disk corpus files |
| --- | ---: | ---: | ---: |
{rows}
| **Total** | **68,897** | **2,170,731** | **2,931** |

The 68,893 retained seed files are counted per target. `streaming_work` deliberately reuses the streaming seed set. Replay counts include libFuzzer initialization; campaign executed-unit counts separately include campaign initialization and mutations. All raw logs and exact commands are retained.

The six original harnesses are byte-for-byte unchanged. The seventh is a copy of `streaming` with only `max_entity_expansion_bytes` changed from 65,536 to 64 and `max_work_amplification: Some(100)` added. Its whole-input versus incremental oracle is unchanged. The 13 exact regression tests cover consumed-root credit, overflow, per-source/request bounds, converted positions, family reset and old-child separation, and namespace/default work.

## Bounds and limits

Each new campaign used `-max_total_time=120`, seed 20260911, maximum input 65,536 bytes, per-input timeout 10 seconds, RSS limit 1,536 MiB, and `ASAN_OPTIONS=detect_leaks=0:abort_on_error=1`. Retained outer bounds were 180 seconds per replay, 690 seconds per campaign, and 1,920 seconds for the aggregate. The regression commands had 1,200-second outer bounds. The controller owned process groups and reaped all workers; no deadline was reached.

These seven new 120-second campaigns are a scoped incremental follow-up, **not equivalent to the historical six 600-second campaigns**. Historical seed metadata and bytes are retained only for seed provenance. Leak detection was disabled; this does not claim LSan, UBSan, whole-standard-library instrumentation, PGO validation, performance, or exhaustive safety.

The preserved raw instrumentation report has an inherited sentence saying “six fuzz targets”; its seven-binary map and actual compiler proof include all six original targets plus `streaming_work`. A regression-filter spelling correction was caught during preparation before any target ran; the earlier preparation and correction receipt are included. No build, regression, replay, or campaign failure was retried.

## Evidence and reconstruction

[Compact results](asan.json), [deterministic evidence archive](asan-evidence.tar.gz), [compressed member/hash/origin index](asan-evidence-members.json.gz), and [packager](package-streaming-policy-asan.py) accompany this report.

Archive SHA-256: `{archive_hash}`. It contains {len(members):,} members, including {len(blobs):,} unique corpus blobs and 70 unpacked canonical source files. All members and corpus layouts were read back after compression. Seven compiled fuzz binaries are excluded with exact hashes; Cargo target caches and historical archives are not included.

`corpus-layouts.json` preserves every original filename for current initial/final corpora and historical initial/final seed inputs. Each value names its bytes at `corpus-by-sha256/<sha256>`. `corpus-origin-map.json` retains every checked source path for those bytes. This deduplicates bytes without losing corpus membership or provenance. Historical result metadata is explicitly scoped to seed provenance.
'''
(OUT / 'ASAN.md').write_text(text)
shutil.copyfile(Path(__file__), OUT / 'package-streaming-policy-asan.py')
for name in ['ASAN.md', 'asan.json', 'asan-evidence.tar.gz', 'asan-evidence-members.json.gz', 'package-streaming-policy-asan.py']:
    destination = REPORT / name
    assert not destination.exists(), str(destination)
    shutil.copyfile(OUT / name, destination)
    assert destination.read_bytes() == (OUT / name).read_bytes()
print(json.dumps({'output': str(OUT), 'report': str(REPORT), 'package': compact['package'],
                  'asan_json_sha256': digest((OUT / 'asan.json').read_bytes()),
                  'ASAN_md_sha256': digest((OUT / 'ASAN.md').read_bytes())}, indent=2), flush=True)
