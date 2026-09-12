"""Normal-only saved preflight review, reusing the selected CPython readback checks."""
from pathlib import Path
import gzip
import hashlib
import json
import math
import os

D=Path('/tmp/oriole-native-start-end-namespace-declaration-study/python');C=Path('/tmp/oriole-native-start-end-namespace-declaration-confirmation/python');S=C/'screen';pins={}
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
a=load(D/'adaptation.json');binding=load(D.parent/'build-binding.json')
assert binding['status']=='passed' and a['status']=='prepared_only' and a['mode']=='normal'
consumer_binding=load(D/'consumer-binding.json')
assert consumer_binding['status']=='passed' and consumer_binding['adaptation_sha256']==sha(D/'adaptation.json')
check(consumer_binding['pins'])
for key in ('candidate_source_record','candidate_build_record'):
    v=a[key];assert sha(v['path'])==v['sha256']
for name,h in a['files'].items():assert sha(D/name)==h
parent=Path(a['origin'])
assert read(D/'python_worker.py')==read(parent/'python_worker.py')
assert read(D/'run.py')==read(parent/'run.py')
assert sha(a['header']['path'])==a['header']['sha256']
for v in a['consumer_source']['files'].values():assert sha(v['path'])==v['sha256']
check(a['consumer_source']['interpreter_and_public_headers'])
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

# Original D source/build provenance above remains historical. C is this fresh campaign.
confirmation=load(C/'preparation.json')
assert confirmation['status']=='prepared_no_targets' and confirmation['fresh_compiles']==0
check(confirmation['pins'])
fresh=load(C/'prepare-controller.json')
assert fresh['status']=='passed' and fresh['controller_sha256']==sha(C/'run.py') and len(fresh['jobs'])==1
job=fresh['jobs'][0]
assert job['label']=='preflight' and job['exit']==0 and job['timeout_seconds']==600 and 'exception' not in job and job['end']>=job['start']
assert job['command']==['taskset','-c','3',python,'-I','-S',str(C/'consumer_screen.py'),'--phase','prepare','--kind','python','--build',str(D/'consumers/build.json'),'--input','/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json','--output',str(S)]
assert sha(C/'preflight-controller.log')==job['log_sha256']

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
    assert process['command']==[python,'-I','-S',str(C/'python_worker.py'),'--worker',str(specpath)]
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
result={'status':'passed','mode':'normal','targets_executed':False,'scope':'Six historical O2 compile vectors and reused unchanged modules; zero confirmation compiles. Fresh worker-reported module paths plus in-process dladdr parser DSO origins; no module loading or new ELF execution here. These 24 benchmark canonical comparisons coalesce adjacent character callbacks and do not replace the strict CPython suite.',
        'exact_compile_vectors':compiles,'extension_compiles':{'fresh':0,'reused':6,'total':6},'preflight_workers':72,'conditions':p['conditions'],'canonical_outputs':expected_outputs,'origins':origins,'pins':pins,
        'reader_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
output=C/'preflight-review.json'
with output.open('x') as stream:stream.write(json.dumps(result,indent=2)+'\n')
print(json.dumps({'status':'passed','preflight_workers':72,'canonical_conditions':24,'input_pins':len(pins),'report':str(output),'sha256':sha(output)}))
