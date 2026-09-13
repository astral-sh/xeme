"""Independent saved archive/corpus readback; adapted from the preceding ASan reader."""
from pathlib import Path
import argparse,gzip,hashlib,json,lzma,os,tarfile
args=argparse.ArgumentParser()
args.add_argument('--released',action='store_true',required=True)
args.parse_args()
assert os.sched_getaffinity(0)=={6}
OUT=Path('/tmp/oriole-reference-frame-asan-final-review')
RESULT=Path(__file__).parent/'publication-audit.json'
ORIGINAL=Path('/tmp/oriole-reference-frame-asan-package')
PACKAGE=Path('/tmp/oriole-reference-frame-asan-publication')
STUDY=Path('/tmp/oriole-reference-frame-asan-study')
PREP=Path('/tmp/oriole-reference-frame-asan-preparation')
TARGETS={'parse','streaming','ffi','ffi_family','multibyte','value_family','streaming_work'}
def hash_bytes(data): return hashlib.sha256(data).hexdigest()
def digest(path): return hash_bytes(Path(path).read_bytes())
def read(path): return json.loads(Path(path).read_bytes())
raw=read(OUT/'raw-audit.json')
assert raw['status']=='passed'
assert digest(STUDY/'summary.json')==raw['summary_sha256']
compact=read(PACKAGE/'asan.json')
summary=read(STUDY/'summary.json')
assert {k:v for k,v in compact.items() if k not in ('package','independent_saved_record_audit')}==summary
assert compact['independent_saved_record_audit']['sha256']==digest(OUT/'raw-audit.json')
meta=compact['package']
assert digest(PACKAGE/meta['archive'])==meta['sha256']
assert digest(PACKAGE/meta['index'])==meta['index_sha256']
assert digest(PACKAGE/'package.py')==meta['packager_sha256']==digest(OUT/'package.py')
index=json.loads(gzip.decompress((PACKAGE/meta['index']).read_bytes()))
assert index['sha256']==meta['sha256']
original_meta=read(ORIGINAL/'asan.json')['package']
original_index=json.loads(gzip.decompress((ORIGINAL/original_meta['index']).read_bytes()))
assert index['members']==original_index['members'] and len(index['members'])==72087
assert index['archive']==meta['archive']=='asan-evidence.tar.xz'
conversion=read(PACKAGE/'recompression.json')
assert conversion['member_index_rows_unchanged']==72087
assert conversion['original_gzip_archive']['sha256']==original_meta['sha256']=='a9c5b323e1099b60dd47ba1837b719a3301e7269ae5e88913be45ca026d92efc'
assert conversion['xz_archive']['sha256']==meta['sha256']=='73579260ee1df28286ea2561deb10a3cc0edfd17a34d02e16207b28c6c4d02b7'
for name,row in conversion['original_files'].items():
    assert digest(ORIGINAL/name)==row['sha256'] and (ORIGINAL/name).stat().st_size==row['bytes']
for name,row in conversion['output_files'].items():
    assert digest(PACKAGE/name)==row['sha256'] and (PACKAGE/name).stat().st_size==row['bytes']
assert digest(PACKAGE/'recompress.py')==meta['recompression']['converter_sha256']
for name,value in original_meta.items():
    if name not in ('archive','sha256','bytes','index_sha256'):assert meta[name]==value
assert meta['recompression']['original_index_sha256']==original_meta['index_sha256']
for opener,path in [(gzip.open,ORIGINAL/original_meta['archive']),(lzma.open,PACKAGE/meta['archive'])]:
    h=hashlib.sha256();size=0
    with opener(path,'rb') as stream:
        while data:=stream.read(1024*1024):h.update(data);size+=len(data)
    assert (h.hexdigest(),size)==('da9f0f2528ccf5ba6282761240f3abccd661644dc0fc5e7695ca5bf18894ac18',272291840)
assert conversion['uncompressed_tar']=={'sha256':h.hexdigest(),'bytes':size,'gzip_and_xz_identical':True}
indexed={r['name']:r for r in index['members']}
assert len(indexed)==len(index['members'])==meta['members']
assert list(indexed)==sorted(indexed)
members={}
with tarfile.open(PACKAGE/meta['archive'],'r:xz') as archive:
    for item in archive:
        assert item.isfile() and item.name not in members
        assert not item.name.startswith('/') and '..' not in Path(item.name).parts
        assert item.mode==0o444 and item.uid==item.gid==item.mtime==0
        data=archive.extractfile(item).read()
        row=indexed[item.name]
        assert len(data)==row['bytes'] and hash_bytes(data)==row['sha256']
        members[item.name]=data
assert list(members)==list(indexed)
expected_names=set()
for label,root in [('preparation',PREP),('study',STUDY)]:
    for path in root.rglob('*'):
        if not path.is_file(): continue
        relative=path.relative_to(root)
        if '__pycache__' in relative.parts or relative.as_posix()=='source.tar.gz': continue
        if label=='study' and relative.parts[0]=='binaries': continue
        if label=='study' and relative.parts[0]=='campaigns' and len(relative.parts)>2 and relative.parts[2] in ('initial-corpus','corpus'): continue
        name=label+'/'+relative.as_posix()
        assert members[name]==path.read_bytes(),name
        expected_names.add(name)
source=read(STUDY/'source.json')['source_sha256']
assert len(source)==326
for name,value in source.items():
    assert hash_bytes(members['source/'+name])==value
    expected_names.add('source/'+name)
assert len([n for n in members if n.startswith('source/')])==326
assert len(json.loads(members['study/instrumentation/report.json'])['binaries'])==7
assert members['review/final/raw-audit.json']==(OUT/'raw-audit.json').read_bytes()
layouts=json.loads(members['corpus-layouts.json'])['layouts']
origin_map=json.loads(members['corpus-origin-map.json'])
assert set(layouts)=={'current','historical_seed_inputs','be22_seed_inputs','detached_frame_seed_inputs'}
expected_origins={}
placements=0
for group in ['current','historical_seed_inputs','be22_seed_inputs']:
    targets=layouts[group]
    assert set(targets)==(TARGETS if group!='be22_seed_inputs' else TARGETS-{'streaming_work'})
    for target,phases in targets.items():
        assert set(phases)=={'initial-corpus','corpus'}
        if group=='current':
            root=STUDY/'campaigns'/target
            initial=read(root/'initial-manifest.json'); result=read(root/'result.json')
            roots={phase:root/phase for phase in phases}
        else:
            if group=='historical_seed_inputs':
                provenance=read(PREP/'corpus-provenance.json'); prefix='historical-seed-metadata'
            else:
                provenance=read(PREP/'prior/streaming-policy/corpus-provenance.json'); prefix='be22-seed-metadata'
            item=provenance['targets'][target]
            ip,rp=Path(item['initial_manifest']),Path(item['result'])
            assert digest(ip)==item['initial_manifest_sha256'] and digest(rp)==item['result_sha256']
            initial,result=read(ip),read(rp)
            roots={'initial-corpus':Path(item['initial_directory']),'corpus':Path(item['final_directory'])}
            for filename,path in [('initial-manifest.json',ip),('result.json',rp)]:
                name=f'{prefix}/{target}/{filename}'
                assert members[name]==path.read_bytes()
                expected_names.add(name)
            for run in result['runs']:
                filename=run['label']+'.log'; name=f'{prefix}/{target}/{filename}'
                assert members[name]==(rp.parent/filename).read_bytes()
                assert hash_bytes(members[name])==run['log_sha256']
                expected_names.add(name)
        expected={'initial-corpus':{k:k for k in initial['inputs']},'corpus':result['final_corpus']}
        assert phases==expected
        for phase,mapping in phases.items():
            assert {p.name for p in roots[phase].iterdir()}==set(mapping)
            for filename,key in mapping.items():
                data=members['corpus-by-sha256/'+key]
                assert hash_bytes(data)==key
                if phase=='initial-corpus': assert len(data)==initial['inputs'][key]
                expected_origins.setdefault(key,set()).add(str(roots[phase]/filename))
                placements+=1
assert origin_map=={key:sorted(paths) for key,paths in expected_origins.items()}
for key,paths in origin_map.items():
    for path in paths: assert digest(path)==key
ancestry=read(PREP/'prior/be22/corpus-provenance.json')
assert set(layouts['detached_frame_seed_inputs'])==TARGETS-{'streaming_work'}
for target,inputs in ancestry['inputs'].items():
    phases={'initial-corpus':{},'corpus':{}}
    for key,record in inputs.items():
        assert len(members['corpus-by-sha256/'+key])==record['size']
        for origin in record['origins']:
            phase={'initial':'initial-corpus','final':'corpus'}[origin['origin']]
            assert origin['name'] not in phases[phase]
            phases[phase][origin['name']]=key
            placements+=1
    assert phases==layouts['detached_frame_seed_inputs'][target]
    assert len(phases['initial-corpus'])==ancestry['counts'][target]['initial']
    assert len(phases['corpus'])==ancestry['counts'][target]['final']
# Original detached-frame archive metadata, logs, and complete filename maps remain
# scoped to historical seed ancestry. No historical execution is counted as current.
handoff=Path('/tmp/oriole-frame-fuzz-handoff')
for name in ['members.json','final-corpus-members.json','readback.json','source.json','summary.json','corpus-provenance.json']:
    key='detached-frame-metadata/'+name
    assert members[key]==(handoff/name).read_bytes()
    expected_names.add(key)
original_index=read(handoff/'members.json')
for row in original_index['members']:
    if row['path'].endswith('.tar.gz'): continue
    name='detached-frame-raw/'+row['path']
    assert len(members[name])==row['bytes'] and hash_bytes(members[name])==row['sha256']
    expected_names.add(name)
coverage=read(PREP/'committed-seed-coverage.json')
assert sum(len(row['committed_seed_files']) for row in coverage.values())==246
for target,row in coverage.items():
    for name,key in row['committed_seed_files'].items():
        assert key in layouts['current'][target]['initial-corpus']
blob_names={n for n in members if n.startswith('corpus-by-sha256/')}
assert {n.split('/')[1] for n in blob_names}==set(origin_map)
assert len(blob_names)==meta['unique_corpus_blobs']
assert sum(len(members[n]) for n in blob_names)==meta['unique_corpus_bytes']
assert placements==meta['reconstructable_corpus_files']
expected_names|=blob_names
omissions=json.loads(members['omissions.json'])['files']
assert len([row for row in omissions if '/binaries/' in row['path']])==7
for row in omissions:
    assert digest(row['path'])==row['sha256'] and Path(row['path']).stat().st_size==row['bytes']
assert not any(n.endswith(('.so','.a','.tar.gz')) for n in members)
for name in ['summarize.py','audit_raw.py','audit_package.py','raw-audit.json','prepare_readers.py','prepare_package.py','reader-adaptation.patch','package-adaptation.patch','PLAN.md']:
    assert members['review/final/'+name]==(OUT/name).read_bytes()
    expected_names.add('review/final/'+name)
for path in OUT.glob('*'):
    if path.is_file() and path.name.startswith(('summarize.','audit_raw.')) and path.suffix in ('.stdout','.stderr'):
        assert members['review/final/'+path.name]==path.read_bytes()
        expected_names.add('review/final/'+path.name)
for name in ['preparation-readback.json','prerequisite-review.json','build-review.py','build-review.json','build-review.stdout','build-review.stderr']:
    expected_names.add('review/'+name)
for name in ['LICENSE-MIT','LICENSE-APACHE','THIRD_PARTY_LICENSES.md','licenses/expat.txt','licenses/libxml2.txt','licenses/cpython.txt','tests/c/UPSTREAM-NOTICES.txt']:
    key='licenses/'+name
    assert members[key]==(Path('/home/dev-user/code/oss/oriole-reference-frame')/name).read_bytes()
    expected_names.add(key)
expected_names|={'package.py','corpus-layouts.json','corpus-origin-map.json','omissions.json'}
assert set(members)==expected_names,sorted(set(members)^expected_names)
archive_origins={}
for name,row in indexed.items():
    origin=row['origin']
    if 'path' in origin:
        assert digest(origin['path'])==row['sha256'],name
    elif 'archive' in origin:
        archive_origins.setdefault(origin['archive'],{})[origin['member']]=name
for path,wanted in archive_origins.items():
    seen=set()
    with tarfile.open(path,'r:gz') as archive:
        for item in archive:
            if item.name not in wanted: continue
            assert archive.extractfile(item).read()==members[wanted[item.name]]
            seen.add(item.name)
    assert seen==set(wanted)
text=(PACKAGE/'ASAN.md').read_text()
for phrase in ['seven new 600-second fuzz campaigns','15 exact regression tests','326-file source snapshot','leak detection was disabled','project remains experimental','streaming-policy, be22 and detached-frame','2,880 seconds']:
    assert phrase in text
for row in summary['targets']:
    assert f"| `{row['target']}` | {row['replay_executions']:,} | {row['campaign_executed_units']:,} | {row['final_disk_corpus_files']:,} |" in text
assert compact['totals']==raw['recomputed_totals']
assert 'original packager' in text and 'converter' in text and 'References to gzip inside' in text
report={
    'status':'passed','role':'Independent saved XZ archive readback plus exact original gzip tar identity; no parser/compiler/fuzzer target execution.',
    'raw_audit_sha256':digest(OUT/'raw-audit.json'),'source_manifest_sha256':raw['source_manifest_sha256'],
    'archive_members_verified':len(members),'unique_corpus_blobs_verified':len(blob_names),
    'unique_corpus_bytes_verified':meta['unique_corpus_bytes'],'corpus_placements_reconstructed':placements,
    'corpus_generations':list(layouts),'original_tar_sha256':h.hexdigest(),'original_tar_bytes':size,'original_gzip_package_unchanged':True,'all_original_filenames_preserved':True,
    'all_physical_blob_origins_rehashed':True,'ancestral_archive_member_bytes_rechecked':True,
    'raw_study_and_preparation_files_match':True,'canonical_source_files_verified':326,
    'committed_seed_files_in_current_union':246,'compiled_current_binaries_excluded_with_hashes':7,
    'files':{name:{'sha256':digest(PACKAGE/name),'bytes':(PACKAGE/name).stat().st_size} for name in ['ASAN.md','asan.json','asan-evidence.tar.xz','asan-evidence-members.json.gz','package.py','recompress.py','recompression.json']},
    'scope':'Current selected reference-frame, 13 policy plus two reference tests, seven600-second campaigns. ASan only with leaks disabled; historical raw evidence is ancestry. No production, full compatibility or speed claim.',
}
output=RESULT
assert not output.exists()
output.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'status':'passed','package_audit_sha256':digest(output),'members':len(members),'placements':placements},indent=2))
