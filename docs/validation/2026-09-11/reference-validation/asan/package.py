"""Deterministic current-source ASan evidence package; no target execution."""
import argparse,os
_args=argparse.ArgumentParser(description='Package released, audited saved records only.')
_args.add_argument('--released',action='store_true',required=True)
_args.parse_args()
assert os.sched_getaffinity(0)=={6}
import gzip
import hashlib
import io
import json
from pathlib import Path
import shutil
import sys
import tarfile

PREP = Path('/tmp/oriole-reference-frame-asan-preparation')
STUDY = Path('/tmp/oriole-reference-frame-asan-study')
OUT = Path('/tmp/oriole-reference-frame-asan-package')
READERS=Path('/tmp/oriole-reference-frame-asan-final-review')
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
raw_audit=load(READERS/'raw-audit.json')
assert raw_audit['status']=='passed'
assert raw_audit['summary_sha256']==digest((STUDY/'summary.json').read_bytes())
OUT.mkdir(exist_ok=False)
members = {}
member_origins = {}
omitted = []
blobs = {}
blob_origins = {}
layouts = {'current': {}, 'historical_seed_inputs': {}, 'be22_seed_inputs': {}, 'detached_frame_seed_inputs': {}}

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
        if label == 'study' and relative.parts[0] == 'campaigns' and len(relative.parts)>2 and relative.parts[2] in ('initial-corpus', 'corpus'):
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
    assert seen == set(source['source_sha256']) and len(seen) == 326
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

# The immediate seven-target campaign and the preceding six-target campaign use
# the same preserved manifest/result protocol. Retain their raw logs as ancestry.
for group, prefix, provenance_path in [
    ('historical_seed_inputs','historical-seed-metadata',PREP/'corpus-provenance.json'),
    ('be22_seed_inputs','be22-seed-metadata',PREP/'prior/streaming-policy/corpus-provenance.json'),
]:
    provenance=load(provenance_path)
    for target,origin in sorted(provenance['targets'].items()):
        initial_path,final_path=Path(origin['initial_manifest']),Path(origin['result'])
        assert digest(initial_path.read_bytes())==origin['initial_manifest_sha256']
        assert digest(final_path.read_bytes())==origin['result_sha256']
        add_file(f'{prefix}/{target}/initial-manifest.json',initial_path)
        add_file(f'{prefix}/{target}/result.json',final_path)
        initial,final=load(initial_path),load(final_path)
        layouts[group][target]={
            'initial-corpus':corpus(Path(origin['initial_directory']),{h:h for h in initial['inputs']}),
            'corpus':corpus(Path(origin['final_directory']),final['final_corpus']),
        }
        assert all(len(blobs[h])==size for h,size in initial['inputs'].items())
        for run in final['runs']:
            log=final_path.parent/(run['label']+'.log')
            assert digest(log.read_bytes())==run['log_sha256']
            add_file(f'{prefix}/{target}/'+log.name,log)

# Earlier detached-frame archives supplied the be22 initial union. Preserve their
# original filename mappings using the reviewed SHA/size/origin metadata; the same
# bytes are already present in the verified current and be22 initial corpora.
ancestry=load(PREP/'prior/be22/corpus-provenance.json')
for target,inputs in sorted(ancestry['inputs'].items()):
    phases={'initial-corpus':{},'corpus':{}}
    for key,record in inputs.items():
        assert key in blobs and len(blobs[key])==record['size']
        for origin in record['origins']:
            phase={'initial':'initial-corpus','final':'corpus'}[origin['origin']]
            assert origin['name'] not in phases[phase]
            phases[phase][origin['name']]=key
    layouts['detached_frame_seed_inputs'][target]=phases
    expected=ancestry['counts'][target]
    assert len(phases['initial-corpus'])==expected['initial']
    assert len(phases['corpus'])==expected['final']
# Retain the earlier raw logs/records, unpacking its evidence once and excluding
# nested archives. Corpus bytes themselves are represented by the layouts above.
handoff=Path('/tmp/oriole-frame-fuzz-handoff')
for name in ['members.json','final-corpus-members.json','readback.json','source.json','summary.json','corpus-provenance.json']:
    add_file('detached-frame-metadata/'+name,handoff/name)
old_index=load(handoff/'members.json')
old_members={r['path']:r for r in old_index['members']}
archive_path=Path(ancestry['archives']['evidence']['path'])
assert digest(archive_path.read_bytes())==ancestry['archives']['evidence']['sha256']
with tarfile.open(archive_path,'r:gz') as archive:
    for member in archive:
        assert member.isfile()
        data=archive.extractfile(member).read()
        record=old_members[member.name]
        assert digest(data)==record['sha256'] and len(data)==record['bytes']
        if member.name.endswith('.tar.gz'):
            continue
        add('detached-frame-raw/'+member.name,data,{'archive':str(archive_path),'member':member.name})

# Check all committed seeds against the same target's current initial union.
coverage=load(PREP/'committed-seed-coverage.json')
committed_count=0
for target,record in coverage.items():
    assert record['all_present_in_retained_union']
    for name,key in record['committed_seed_files'].items():
        assert key in layouts['current'][target]['initial-corpus']
        committed_count+=1
assert committed_count==246

for name in ['LICENSE-MIT','LICENSE-APACHE','THIRD_PARTY_LICENSES.md','licenses/expat.txt','licenses/libxml2.txt','licenses/cpython.txt','tests/c/UPSTREAM-NOTICES.txt']:
    add_file('licenses/'+name,Path('/home/dev-user/code/oss/oriole-reference-frame')/name)

for key, data in sorted(blobs.items()):
    add('corpus-by-sha256/' + key, data, {'origin_map': 'corpus-origin-map.json', 'key': key})
add('corpus-layouts.json', encoded({
    'format': 'For each logical filename, obtain bytes from corpus-by-sha256/<sha256>. No corpus filename is discarded.',
    'layouts': layouts,
}), {'generated_by': 'package.py'})
add('corpus-origin-map.json', encoded({key: sorted(paths) for key, paths in sorted(blob_origins.items())}),
    {'generated_by': 'package.py'})
add('omissions.json', encoded({'files': omitted, 'untraversed': ['Cargo target and shared intermediate caches'],
    'scope': 'Historical archives and binaries are not included; only exact seed metadata and reconstructable seed bytes are included.'}),
    {'generated_by': 'package.py'})
add_file('package.py', Path(__file__))
for name in ['summarize.py','audit_raw.py','audit_package.py','raw-audit.json','prepare_readers.py','prepare_package.py','reader-adaptation.patch','package-adaptation.patch','PLAN.md']:
    add_file('review/final/'+name,READERS/name)
for label,path in [
    ('preparation-readback.json','/tmp/oriole-reference-frame-asan-preparation-readback.json'),
    ('prerequisite-review.json','/tmp/oriole-reference-frame-asan-independent-review/review.json'),
    ('build-review.py','/tmp/oriole-reference-frame-asan-build-review.py'),
    ('build-review.json','/tmp/oriole-reference-frame-asan-build-review.json'),
    ('build-review.stdout','/tmp/oriole-reference-frame-asan-build-review.stdout'),
    ('build-review.stderr','/tmp/oriole-reference-frame-asan-build-review.stderr'),
]:
    add_file('review/'+label,Path(path))
for path in sorted(READERS.glob('*')):
    if path.is_file() and path.name.startswith(('summarize.','audit_raw.')) and path.suffix in ('.stdout','.stderr'):
        add_file('review/final/'+path.name,path)
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

summary=load(STUDY/'summary.json')
compact=dict(summary)
compact['independent_saved_record_audit']={
    'path':'review/final/raw-audit.json','sha256':digest((READERS/'raw-audit.json').read_bytes()),
    'scope':raw_audit['role'],
}
compact['package']={
    'archive':archive_path.name,'sha256':archive_hash,'bytes':archive_path.stat().st_size,
    'members':len(members),'index':index_path.name,'index_sha256':digest(index_path.read_bytes()),
    'unique_corpus_blobs':len(blobs),'unique_corpus_bytes':sum(map(len,blobs.values())),
    'reconstructable_corpus_files':sum(len(mapping) for group in layouts.values() for phases in group.values() for mapping in phases.values()),
    'excluded_compiled_binaries':len([f for f in omitted if 'Compiled' in f['reason']]),
    'unpacked_source_files':len(seen),'all_members_and_layouts_read_back':True,
    'source_sha256_map_matches_live':True,'packager_sha256':digest(Path(__file__).read_bytes()),
}
(OUT/'asan.json').write_text(json.dumps(compact,indent=2)+'\n')
rows='\n'.join(f"| `{r['target']}` | {r['replay_executions']:,} | {r['campaign_executed_units']:,} | {r['final_disk_corpus_files']:,} |" for r in compact['targets'])
totals=compact['totals']
text=f'''# Selected reference-frame AddressSanitizer validation

Selected reference-frame source passed a fresh Rust AddressSanitizer build, 15 exact regression tests, retained-corpus replay and seven new 600-second fuzz campaigns. The saved records report zero failure artifacts, timeouts or target retries. An independent saved-data reader verified source, actual compiler vectors, representative machine code, exact outcomes, all fourteen replay/campaign commands and raw logs, and current and immediate historical corpus hashes. This is bounded sanitizer evidence; the project remains experimental.

Source commit `4064b0653534aff690c0e0f395a6b3b48da878fc` has the same 72 selected source hashes as measured runtime `0f66d54ac8418f0a6e628ad18570677a19c9ed45`. The broader 326-file source snapshot includes harnesses and all 246 committed seeds. Manifest SHA-256: `{compact['source_manifest_sha256']}`.

## Outcomes

| Target | Replay executions | Campaign executed units | Final disk corpus files |
| --- | ---: | ---: | ---: |
{rows}
| **Total** | **{totals['replay_executions']:,}** | **{totals['campaign_executed_units']:,}** | **{totals['final_disk_corpus_files_per_target_sum']:,}** |

The {totals['initial_seed_files_per_target_sum']:,} initial seed files are summed per target; identical bytes can appear in multiple targets. `streaming_work` retains its own previous initial and final corpus. Replay counts include libFuzzer initialization; campaign executed-unit counts separately include campaign initialization and mutation.

The six original harnesses are unchanged. `streaming_work` copies the streaming oracle with only `max_entity_expansion_bytes` reduced from 65,536 to 64 and `max_work_amplification: Some(100)` added. Thirteen policy regressions cover source/request bounds, positions, consumed-root credit, overflow, reset/old-child separation and namespace/default work. Two adapter tests cover decoded-reference ownership, accounting and callback/position/error boundaries.

## Instrumentation and bounds

Actual compiler vectors record ASan and coverage instrumentation for three workspace crates and seven fuzz targets. All seven binaries were rehashed and their ASan symbols read back. Retained representative disassembly shows ASan report calls in `XML_Parse`, core `parse_text` and `AdapterFrame::text_bytes`; this is a representative machine-code check.

Each campaign used `-max_total_time=600`, seed 20260911, maximum input 65,536 bytes, a 10-second per-input timeout and a 1,536 MiB libFuzzer RSS limit. Three lanes used CPUs 1, 2 and 4. Outer bounds were 180 seconds per replay, 690 seconds per campaign and 2,880 seconds for all campaigns; each exact regression had a 1,200-second bound. Controllers killed owned process groups on failure and waited for direct children. Saved first-attempt records and the completed root release support the outcome; the reviewer launched no parser/compiler/fuzzer targets.

`ASAN_OPTIONS=detect_leaks=0:abort_on_error=1`: leak detection was disabled. This does not validate LSan, UBSan, the entire standard library/libc, PGO builds, Expat differential equivalence or performance, and cannot prove memory safety or production readiness. Historical results are preserved as corpus ancestry, not transferred to the current source.

## Evidence and reconstruction

[Results](asan.json), [saved-record archive](asan-evidence.tar.gz), [member/hash/origin index](asan-evidence-members.json.gz) and [packager](package.py) accompany this report. The archive includes raw compiler/test/fuzzer records, source and exact harnesses, prerequisites, the intermediate build/regression review, the final saved-data reader and receipt, and corpus ancestry. Review stages state their own scopes.

Archive SHA-256: `{archive_hash}`. All {len(members):,} members were read back after compression, including {len(blobs):,} unique corpus blobs and 326 source files. Seven current compiled fuzz binaries are excluded with hashes; Cargo target/cache trees and nested historical archives are excluded.

`corpus-layouts.json` maps every filename for current, streaming-policy, be22 and detached-frame initial/final corpora to its bytes at `corpus-by-sha256/<sha256>`. `corpus-origin-map.json` retains checked physical source paths; older detached-frame archive paths/names are preserved in ancestry metadata. Deduplication removes no logical corpus placements. The scripts retain historical absolute paths for this machine; archive members, source and corpus reconstruction are available without those paths. The archive separately retains the project licenses and upstream notices.
'''
(OUT/'ASAN.md').write_text(text)
shutil.copyfile(Path(__file__),OUT/'package.py')
print(json.dumps({'output':str(OUT),'package':compact['package'],'asan_json_sha256':digest((OUT/'asan.json').read_bytes()),'ASAN_md_sha256':digest((OUT/'ASAN.md').read_bytes())},indent=2),flush=True)
