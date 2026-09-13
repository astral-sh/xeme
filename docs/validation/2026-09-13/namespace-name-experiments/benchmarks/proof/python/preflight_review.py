"""Normal-only saved preflight review, reusing the selected CPython readback checks."""
from pathlib import Path
import gzip
import hashlib
import json
import math
import os

D=Path('/tmp/oriole-namespace-name-proof-benchmark/python');S=D/'screen';pins={}
def read(p):
    p=Path(p);data=p.read_bytes();pins[str(p)]=hashlib.sha256(data).hexdigest();return data
def sha(p):read(p);return pins[str(Path(p))]
def load(p):return json.loads(read(p))
def check(mapping):
    for p,h in mapping.items():assert sha(p)==h,p

assert os.sched_getaffinity(0)=={6}
python='/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3'
upstream=Path('/home/dev-user/.cache/oriole/upstream/cpython-3.12.13').resolve()
header='/home/dev-user/code/oss/oriole-reference-frame/include'
inc='/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/include/python3.12'
suffix='.cpython-312-x86_64-linux-gnu.so'
a=load(D.parent/'binding.json');prep=load(D.parent/'preparation.json')
assert a['status']=='passed' and a['mode']=='normal' and a['preparation_sha256']==sha(D.parent/'preparation.json')
check(prep['pins'])
assert sha(a['build_record']['path'])==a['build_record']['sha256']
reuse=load(D/'reuse.json');check(reuse['pins'])
oldbuild=load(reuse['build_record'])
libs=a['libraries']
for v in libs.values():assert sha(v['path'])==v['sha256']
c=load(D/'prepare-controller.json');b=load(D/'consumers/build.json');p=load(S/'preflight.json')
assert c['status']=='passed' and c['controller_sha256']==sha(D/'run.py') and len(c['jobs'])==2
assert [job['label'] for job in c['jobs']]==['build','preflight']
for job in c['jobs']:
    assert job['exit']==0 and job['timeout_seconds']==600 and job['command'][:6]==['taskset','-c','3',python,'-I','-S'] and 'exception' not in job
    assert sha(D/(job['label']+'-controller.log'))==job['log_sha256']
assert b['status']=='passed' and b['adaptations'] is None and b['cpython_revision']=='3bb231a6a5dc02b95658877318bf61501a7209e9'
assert b['source_sha256_before']==b['source_sha256_after'];check(b['source_sha256_before'])
assert b['source_sha256_before'][str(upstream/'Modules/pyexpat.c')]=='fd679a16f36304444af11953ad276d4493f2044f904a39b8fdf028c6f7def8a0'
assert b['source_sha256_before'][str(upstream/'Modules/_elementtree.c')]=='de4c3c8716f7e3b1d2fdf077656246220ef0493053c990ca7b724ad31498fad0'
assert set(b['consumers'])=={'control','candidate','expat'}
assert b['reuse']=={'path':str(D/'reuse.json'),'sha256':sha(D/'reuse.json')}
assert b['compilation_origin']=={'control':'reused','expat':'reused','candidate':'fresh'}
for engine,entry in reuse['engines'].items():assert b['consumers'][engine]==oldbuild['consumers'][entry['prior_engine']]
origins={};compiles=[]
for engine,consumer in b['consumers'].items():
    directory=Path(a['consumer_directories'][engine]);library=directory/Path(libs[engine]['path']).name
    assert consumer['directory']==str(directory) and consumer['library']==str(library)
    assert sha(library)==libs[engine]['sha256'];check(consumer['files'])
    assert set(consumer['files'])=={str(library),*(str(directory/(m+suffix)) for m in ('pyexpat','_elementtree'))}
    for module,command in zip(('pyexpat','_elementtree'),consumer['commands'],strict=True):
        expected=['cc','-shared','-fPIC','-O2',*['-I'+x for x in (header,inc,inc+'/internal',str(upstream/'Modules/expat'),str(upstream/'Modules'))],str(upstream/('Modules/'+module+'.c')),str(library),'-Wl,-rpath,'+str(directory),'-o',str(directory/(module+suffix))]
        assert command['command']==expected and command['returncode']==0 and command['log_sha256']==sha(directory/(module+'-build.log'))
        compiles.append(expected)
    origins[engine]={m:{'path':str(directory/(m+suffix)),'sha256':sha(directory/(m+suffix))} for m in ('pyexpat','_elementtree')}
    origins[engine]['parser_library']={'path':str(library.resolve()),'sha256':libs[engine]['sha256']}
    origins[engine]['expat_version']='expat_2.8.4' if engine=='expat' else 'oriole_compat_2.8.4'
assert len(compiles)==6
prior=load('/tmp/oriole-reference-frame-python-study/normal/screen/preflight.json')
assert b['compiler']==load('/tmp/oriole-reference-frame-python-study/normal/consumers/build.json')['compiler']
assert p['status']=='preflight_passed' and p['kind']=='python' and p['affinity']==[3] and p['seed']==202609104412 and p['pairs']==7
assert p['hashes_before']==p['hashes_after'] and p['hash_errors']=={};check(p['hashes_before'])
assert p['conditions']==prior['conditions'] and len(p['conditions'])==24 and len(p['preflights'])==len(p['processes'])==72 and p['rows']==[]
expected_outputs={}
for index,(row,process) in enumerate(zip(p['preflights'],p['processes'],strict=True)):
    condition=p['conditions'][index//3];engine=['control','candidate','expat'][index%3]
    key=f"{condition['name']}/{condition['chunk']}/{condition['mode']}";label=key.replace('/','-')+'-'+engine+'--1-0'
    assert row['key']==key and row['engine']==engine and row['pair']==-1 and row['median_seconds'] is None and row['process']==label
    assert process['label']==label and process['status']=='passed' and process['returncode']==0 and process['timeout_seconds']==300
    specpath=S/(label+'.json')
    assert process['command']==[python,'-I','-S',str(D/'python_worker.py'),'--worker',str(specpath)]
    assert load(specpath)=={'input':condition['input'],'input_sha256':sha(condition['input']),'chunk':condition['chunk'],'mode':condition['mode'],'consumer':a['consumer_directories'][engine],'library_sha256':libs[engine]['sha256'],'iterations':0}
    for stream in ('stdout','stderr'):assert sha(S/process[stream])==process[stream+'_sha256']
    assert gzip.decompress(read(S/process['stderr']))==b''
    raw=json.loads(gzip.decompress(read(S/process['stdout'])))
    assert raw['identity']==row['identity']==origins[engine] and raw['samples']==row['samples']
    assert raw['gc_enabled'] is True and raw['python']==b['python']==p['python_version'] and len(raw['samples'])==1
    sample=raw['samples'][0]
    assert sample['iteration']==0 and sample['warmup'] is True and sample['parse_ns']>0 and sample['destruction_ns']>=0
    assert math.isfinite(sample['seconds']) and sample['seconds']==(sample['parse_ns']+sample['destruction_ns'])/1e9
    canonical=[sample['output_sha256'],sample['output_rows']];assert canonical[1]>0
    if key in expected_outputs:assert expected_outputs[key]==canonical
    else:expected_outputs[key]=canonical
assert expected_outputs==p['expected']==prior['expected']
result={'status':'passed','mode':'normal','targets_executed':False,'scope':'Four original reused and two fresh compile vectors and fresh worker-reported module paths plus in-process dladdr parser DSO origins; no module loading or new ELF execution here. These 24 benchmark canonical comparisons coalesce adjacent character callbacks and do not replace the strict CPython suite.',
        'exact_compile_vectors':compiles,'extension_compiles':{'fresh':2,'reused':4,'total':6},'preflight_workers':72,'conditions':p['conditions'],'canonical_outputs':expected_outputs,'origins':origins,'pins':pins,
        'reader_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
output=D/'preflight-review.json';output.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'status':'passed','preflight_workers':72,'canonical_conditions':24,'input_pins':len(pins),'report':str(output),'sha256':sha(output)}))
