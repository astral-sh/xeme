"""Portable transport, source, alias and numerical readback; standard library only."""
from pathlib import Path
import gzip
import hashlib
import io
import json
import math
import statistics
import sys
import tarfile

def sha(data):return hashlib.sha256(data).hexdigest()
def main(root):
    inv=json.loads((root/'inventory.json').read_bytes());summary=json.loads((root/'summary.json').read_bytes())
    archive=(root/'evidence.tar.gz').read_bytes();assert sha(archive)==inv['archive_sha256'] and len(archive)==inv['archive_bytes']
    payload={}
    with tarfile.open(fileobj=io.BytesIO(archive),mode='r:gz') as t:
        for m in t.getmembers():
            assert m.isfile() and m.name not in payload and not m.name.startswith('/') and '..' not in Path(m.name).parts
            payload[m.name]=t.extractfile(m).read()
    assert len(payload)==inv['unique_members']
    assert set(payload)=={r['archive_member'] for r in inv['files'].values()}
    for logical,row in inv['files'].items():
        data=payload[row['archive_member']];assert len(data)==row['bytes'] and sha(data)==row['sha256'],logical
        assert not data.startswith((b'\x7fELF',b'!<arch>\n')),logical
    def read(logical):return payload[inv['files'][logical]['archive_member']]
    cache={}
    def load(logical):
        if logical not in cache:cache[logical]=json.loads(read(logical))
        return cache[logical]
    for name,h in inv['source_maps']['selected'].items():assert sha(read('shared/source-selected/'+name))==h
    for name,h in inv['source_maps']['expat'].items():assert sha(read('shared/expat-source/'+name))==h
    with tarfile.open(fileobj=io.BytesIO(read('text/build/source.tar.gz')),mode='r:gz') as t:
        source={m.name:t.extractfile(m).read() for m in t.getmembers() if m.isfile()}
    assert len(source)==70 and set(source)==set(inv['source_maps']['text'])
    for name,h in inv['source_maps']['text'].items():assert sha(source[name])==h
    for alias in inv['raw_worker_aliases']:
        row=load(alias['container'])
        for component in alias['json_pointer_components']:row=row[component]
        data=(json.dumps({k:v for k,v in row.items() if k not in ['observations','median_seconds']},indent=2)+'\n').encode()
        assert len(data)==alias['bytes'] and sha(data)==alias['sha256'] and alias['byte_equivalence']=='verified_exact'
    assert len(inv['raw_worker_aliases'])==3136
    # Recompute the actual per-worker and paired medians in the four native cohorts.
    cases={};workers=0;samples=0
    cohorts=[('additive','additive/native/native-screen',4),('bulk-native','bulk/native/native-screen',4),('text-normal','text/native-normal/native-screen',3),('text-pgo','text/native-pgo/native-screen',3)]
    for campaign,prefix,nengine in cohorts:
        pre=load(prefix+'/preflight.json');r=load(prefix+'/results.json');protocol=load(prefix+'/protocol.json')
        assert pre['status']=='passed' and r['status']=='passed'
        assert pre['hashes_before']==pre['hashes_after']==r['hashes_before']==r['hashes_after']
        assert r['preflight_sha256']==sha(read(prefix+'/preflight.json'))
        assert len(pre['rows'])==28*nengine and len(r['rows'])==196
        observation={};index={}
        for phase,rows in [('pre',[x for x in pre['rows']]),('time',[x for cohort in r['rows'] for x in cohort['processes']])]:
            for row in rows:
                assert row['returncode']==0 and not row['stderr']
                raw=json.loads(row['stdout']);c=row['condition'];key=(c['name'],c['chunk'],c['namespaces'])
                count=1 if phase=='pre' else c['iterations'];assert len(raw['samples'])==count+1
                for i,s in enumerate(raw['samples']):
                    assert s['iteration']==i and s['warmup']==(i==0) and s['seconds']>0 and math.isfinite(s['seconds'])
                    actual=(s['hash'],s['elements'],s['text_bytes']);assert observation.setdefault(key,actual)==actual
                median=statistics.median(s['seconds'] for s in raw['samples'][1:]);assert row['median_seconds']==median
                if phase=='time':index[key,row['pair'],row['engine']]=median
                workers+=1;samples+=len(raw['samples'])
        # Pair labels differ by experiment; the archived summary supplies names,
        # while every ratio value is independently regenerated from raw medians.
        for sr in summary['all_conditions']:
            if sr['campaign']!=campaign:continue
            c=sr['condition'];key=(c['name'],c['chunk'],c['namespaces'])
            for ratio,wanted in sr['median_ratios'].items():
                a,b=ratio.split('_over_');actual=statistics.median(index[key,p,a]/index[key,p,b] for p in range(7));assert actual==wanted,(campaign,key,ratio)
            cases[campaign,key]=sr
    # Python raw streams, module identities and explicit destruction samples.
    prefix='bulk/python/screen';pre=load(prefix+'/preflight.json');r=load(prefix+'/results.json');build=load('bulk/python/consumers/build.json')
    assert pre['status']=='preflight_passed' and r['status']=='passed'
    assert len(pre['preflights'])==96 and len(r['rows'])==672
    index={}
    for phase,record,rows in [('pre',pre,pre['preflights']),('time',r,r['rows'])]:
        processes={p['label']:p for p in record['processes']}
        for row in rows:
            p=processes[row['process']];assert p['returncode']==0 and p['status']=='passed' and p['timeout_seconds']==300
            assert sha(read(prefix+'/'+p['stdout']))==p['stdout_sha256'] and sha(read(prefix+'/'+p['stderr']))==p['stderr_sha256']
            assert not gzip.decompress(read(prefix+'/'+p['stderr']))
            raw=json.loads(gzip.decompress(read(prefix+'/'+p['stdout'])));assert raw['samples']==row['samples'] and raw['identity']==row['identity'] and raw['gc_enabled'] is True
            consumer=build['consumers'][row['engine']]
            for key in ('pyexpat','_elementtree','parser_library'):
                ident=raw['identity'][key];assert consumer['files'][ident['path']]==ident['sha256']
            spec=load(prefix+'/'+Path(p['command'][-1]).name);assert len(raw['samples'])==spec['iterations']+1
            for i,s in enumerate(raw['samples']):
                assert s['iteration']==i and s['warmup']==(i==0) and s['seconds']==(s['parse_ns']+s['destruction_ns'])/1e9
                assert s['seconds']>0 and [s['output_sha256'],s['output_rows']]==pre['expected'][row['key']]
            median=statistics.median(s['seconds'] for s in raw['samples'][1:]) if phase=='time' else None
            assert median==row['median_seconds']
            if phase=='time':index[row['key'],row['pair'],row['engine']]=median
            workers+=1;samples+=len(raw['samples'])
    for sr in summary['all_conditions']:
        if sr['campaign']!='bulk-python':continue
        c=sr['condition'];key=f"{c['name']}/{c['chunk']}/{c['mode']}"
        for ratio,wanted in sr['median_ratios'].items():
            a,b=ratio.split('_over_');assert statistics.median(index[key,p,a]/index[key,p,b] for p in range(7))==wanted
        cases['bulk-python',key]=sr
    assert len(cases)==summary['conditions']==136 and workers==summary['all_workers']==3904 and samples==summary['all_samples']==530640
    assert sum(r['adverse_primary'] for r in cases.values())==summary['adverse_primary_conditions']==54
    # Every included held-out fixture is accompanied by every pinned notice.
    corpus=load('shared/heldout/corpus-manifest.json');notice_count=0
    for project in corpus['projects']:
        for entry in project['files']:
            if entry['role'] in ('input','license','notice'):
                assert sha(read('shared/heldout/'+entry['path']))==entry['sha256']
                if entry['role']!='input':notice_count+=1
    assert notice_count==9
    real=load('shared/real-training/manifest.json');assert len(real['documents'])==6
    assert sha(read('shared/cpython/Modules/pyexpat.c'))=='fd679a16f36304444af11953ad276d4493f2044f904a39b8fdf028c6f7def8a0'
    assert sha(read('shared/cpython/Modules/_elementtree.c'))=='de4c3c8716f7e3b1d2fdf077656246220ef0493053c990ca7b724ad31498fad0'
    for logical in inv['redistribution_notices']:assert logical in inv['files']
    result={'status':'passed_portable_saved_data_readback','archive_sha256':inv['archive_sha256'],'logical_files':len(inv['files']),'unique_members':len(payload),'native_worker_aliases':3136,'selected_source_files':70,'text_source_files':70,'expat_source_files':len(inv['source_maps']['expat']),'conditions':len(cases),'workers':workers,'samples':samples,'adverse_primary_conditions':54,'heldout_notice_entries':9,'target_execution':False,'limits':'Portable readback verifies transport, source hashes, reconstructed native workers and raw medians. Original independent audits retain full seed/order/compiler/build validation; this does not re-execute those machine-specific scripts or claim new compatibility.'}
    print(json.dumps(result,indent=2))

if __name__=='__main__':
    assert __debug__
    main(Path(sys.argv[1] if len(sys.argv)>1 else '.').resolve())
