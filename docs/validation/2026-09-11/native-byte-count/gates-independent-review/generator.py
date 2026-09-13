"""Independent saved-data audit only. Never imports or executes project tools."""
from pathlib import Path
from collections import Counter
import ast
import difflib
import gzip
import hashlib
import io
import itertools
import json
import os
import re
import tarfile

R = Path('/tmp/oriole-native-byte-count-gates')
O = Path(__file__).resolve().parent
W = Path('/home/dev-user/code/oss/oriole-native-byte-count')
L = Path('/tmp/oriole-native-byte-count-thinlto-study')
B = Path('/tmp/oriole-end-frame-cells-thinlto-study')
assert sorted(os.sched_getaffinity(0)) == [4]
tracked = {}
checks = []
def read(path):
    p = Path(path)
    data = p.read_bytes()
    value = hashlib.sha256(data).hexdigest()
    assert tracked.setdefault(str(p), value) == value, str(p)
    return data
def h(path):
    read(path)
    return hashlib.sha256(read(path)).hexdigest()
def j(path):
    return json.loads(read(path))
def ck(name, condition):
    checks.append({'name': name, 'passed': bool(condition)})
    assert condition, name
def hashes(mapping):
    return all(h(path) == value for path, value in mapping.items())
def unchanged(report, before='before', after='after'):
    return report[before] == report[after] and hashes(report[before])
def commands(report, root, allow_api=False, cpu=1):
    for command in report['commands']:
        ck('command CPU ' + command['label'], command['command'][:3] == ['taskset','-c',str(cpu)])
        ck('command exit ' + command['label'], command['exit'] == (1 if allow_api and command['label'] == 'api' else 0))
        ck('command no exception ' + command['label'], 'exception' not in command)
        for stream in ['stdout','stderr']:
            ck('command stream hash ' + command['label'] + stream, h(root / (command['label'] + '.' + stream)) == command[stream + '_sha256'])

S=L;prior_root=Path('/tmp/oriole-end-frame-cells-thinlto-gates');initial_root=Path('/tmp/oriole-native-byte-count-gates-initial-cpu4')
source=j(R/'source.json');preparation=j(R/'preparation.json');initial_preparation=j(initial_root/'preparation.json');build=j(R/'build.json');summary=j(R/'summary.json')
source_review_path=Path('/tmp/oriole-native-byte-count-source-build-independent-review/review.json')
ck('source build review pinned',h(source_review_path)=='f1f801a46736344b8fae130ed3ca815c9d71c542c0dda15b098cd5e784c0d886')
ck('source70 identity',len(source['source_sha256'])==70 and h(R/'source.json')==h(S/'source.json')==build['source_manifest_sha256']=='735afb9491158c3a34fd376109edc2312727382b5fa61e5e8b084796d7294f1d')
normal_source=j(B/'source.json');changed=sorted(n for n,v in source['source_sha256'].items() if normal_source['source_sha256'][n]!=v)
ck('summary source delta exact',changed==summary['source']['changed_files_from_control']==['crates/oriole/src/encoding.rs','crates/oriole_expat/src/tests.rs'])
ck('all70 live source bytes',all(h(W/n)==v for n,v in source['source_sha256'].items()))
for p in [R/'source.tar.gz',S/'source.tar.gz']:
 with tarfile.open(fileobj=io.BytesIO(read(p)),mode='r:gz') as a:
  ms=a.getmembers();ck('source archive70 '+str(p),len(ms)==len({m.name for m in ms})==70 and all(m.isfile() for m in ms) and {m.name:hashlib.sha256(a.extractfile(m).read()).hexdigest() for m in ms}==source['source_sha256'])
expected_libraries={'liboriole_expat.so':'ccfb22956a11c1b241ec755f46245c61f088187ef93143b77a7afec748983821','liboriole_expat.a':'f58d5bc6ed28077933fcbcb84ad876cc7208e0102c9008347223da91a328b03c'}
normal_libraries={'liboriole_expat.so':'02fcab59f6e3818c128da6706d67987ae7450dd8b59e97cd28ce497a547f28f5','liboriole_expat.a':'69ebed414946cbaef1a940c052a636be277c809902943d676e824deab45d22d5'}
ck('build alias exact',read(R/'build.json')==read(S/'report.json') and build['status']=='passed' and build['source_unchanged'] and build['libraries']==expected_libraries)
for root,libmap in [(S,expected_libraries),(B,normal_libraries)]:ck('library identities '+str(root),all(h(root/n)==v for n,v in libmap.items()))
ck('all81 prepared inputs unchanged',len(preparation['before'])==81 and hashes(preparation['before']))
ck('initial preparation receipt',h(initial_root/'preparation.json')==preparation['initial_cpu4_preparation']['sha256']=='e292da0ce5fce28123c94f79ea79efb139d82540b7a25b1a532ac22230f4b72e' and initial_preparation['status']=='prepared_not_executed')
repls=[(str(prior_root),str(R)),(str(B),str(S)),('/tmp/oriole-ffi-family-cell-thinlto-study',str(B)),('/tmp/oriole-ffi-family-cell-thinlto-gates/api-native/upstream-api',str(prior_root/'api-native/upstream-api')),('/home/dev-user/code/oss/oriole-end-frame-cells',str(W)),(normal_libraries['liboriole_expat.so'],expected_libraries['liboriole_expat.so']),('0cfd610876f1bfd26a4d37a7f86adc9961896dfe24177c180ec7d15e0bae04e9',normal_libraries['liboriole_expat.so'])]
ck('seven original substitutions declared',preparation['replacements']==[list(x) for x in repls]==initial_preparation['replacements'])
def adapt(value):
 for i,(a,b) in enumerate(repls):value=value.replace(a,'__NATIVE_COUNT_'+str(i)+'__')
 for i,(a,b) in enumerate(repls):value=value.replace('__NATIVE_COUNT_'+str(i)+'__',b)
 return value
def cpu1(value):
 return value.replace('CPU4','CPU1').replace("'4'","'1'").replace("'api':4,'other':4","'api':1,'other':1").replace('=={4}','=={1}').replace('owned by structural agent','owned by the byte-count source author')
deltas=j(R/'controller-deltas.json')
ck('13 original controllers and retained deltas',len(preparation['controllers'])==13 and set(preparation['controllers'])==set(deltas) and read(R/'controller-deltas.json')==read(initial_root/'controller-deltas.json'))
for name,value in preparation['controllers'].items():
 original_path=prior_root/name;original=read(original_path).decode();initial=read(initial_root/name).decode();current=read(R/name).decode()
 ck('prior/current/initial controller pins '+name,h(original_path)==preparation['prior_controllers'][str(original_path)] and h(initial_root/name)==initial_preparation['controllers'][name] and h(R/name)==value)
 ck('initial exact seven substitutions '+name,adapt(original)==initial)
 ck('initial exact retained delta '+name,deltas[name]==''.join(difflib.unified_diff(original.splitlines(True),initial.splitlines(True),fromfile=str(original_path),tofile=str(R/name))))
 ck('current only CPU/prose substitutions '+name,cpu1(initial)==current)
ck('initial combined patch retained',read(R/'initial-controller.patch')==read(initial_root/'controller.patch')==''.join(deltas.values()).encode())
ck('CPU lane patch pin',h(R/'cpu-lane.patch')==preparation['cpu_lane_patch_sha256'])
for name in ['probe.c','original-allocations.c','expat.h']:ck('End OOM original assertions '+name,read(R/'end-oom-final'/name)==read(prior_root/'end-oom-final'/name))
for lane,status,names,codes in [('api','completed_zero_exits',['api_native.py'],[0]),('other','failed',['other_controls.py','run_publication.py','run_malformed.py','end-lifecycle/controller.py'],[0,0,0,1]),('end','completed_zero_exits',['end-lifecycle-final/controller.py','end-oom-final/run.py'],[0,0])]:
 report=j(R/(lane+'-controller.json'))
 ck('lane identity/status '+lane,report['status']==status and report['unchanged'] and report['before']==report['after']==preparation['before'] and not report['hash_errors'] and [r['script'] for r in report['jobs']]==names and [r['exit'] for r in report['jobs']]==codes and report['cpu']==1)
 for row in report['jobs']:
  ck('lane command bound '+row['script'],row['timeout']==660 and 'exception' not in row and row['command']==['taskset','-c','1','/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3.12','-I','-S',str(R/row['script'])] and h(R/row['script'])==row['sha256'] and 0<row['end']-row['start']<660)
  for stream in ['stdout','stderr']:ck('lane stream '+row['script']+stream,h(R/(row['script'].replace('/','-')+'.'+stream))==row[stream+'_sha256'])
  ck('lane child stderr empty '+row['script'],read(R/(row['script'].replace('/','-')+'.stderr'))==b'')
failed=j(R/'end-lifecycle/controller.json');fix=j(R/'end-affinity-fix.json')
ck('initial End guard failed before any parser runs',failed['status']=='failed' and failed['runs']==fix['initial_runs']==[] and 'assert sorted(os.sched_getaffinity(0)) == [4]' in failed['error'] and h(R/'end-lifecycle/controller.json')==fix['initial_controller_sha256'])
fixdiff=''
for name in ['controller.py','oracle.py']:
 old=read(R/'end-lifecycle'/name).decode();new=read(R/'end-lifecycle-final'/name).decode()
 expected=old.replace("'cpu': 4","'cpu': 1").replace('== [4]','== [1]')
 ck('End final only CPU guard/metadata '+name,expected==new)
 fixdiff+=''.join(difflib.unified_diff(old.splitlines(True),new.splitlines(True),fromfile='initial/'+name,tofile='final/'+name))
ck('End final correction patch exact',fixdiff==read(R/'end-affinity-fix.patch').decode() and h(R/'end-affinity-fix.patch')==fix['patch_sha256']=='da24ac6eb69eacdaf58fbd0395059ca355513a74ac27b82262608a25f14ef899')
ck('End SOURCE_REVIEW unchanged',read(R/'end-lifecycle-final/SOURCE_REVIEW.md')==read(R/'end-lifecycle/SOURCE_REVIEW.md'))
lane_code=read(R/'run_lane.py').decode();remaining=read(R/'run_remaining.py').decode()
expected_remaining=lane_code.replace("cpu={'api':1,'other':1}","cpu={'end':1}").replace("names={'api':['api_native.py'],'other':['other_controls.py','run_publication.py','run_malformed.py','end-lifecycle/controller.py','end-oom-final/run.py']}[lane]","names={'end':['end-lifecycle-final/controller.py','end-oom-final/run.py']}[lane]")
ck('remaining controller only changed job selection',expected_remaining==remaining)
ck('malformed worker exact inherited preparation',adapt(read(prior_root/'malformed/malformed_probe.py').decode())==read(R/'malformed/malformed_probe.py').decode())
for p in [R/'prepare.py',R/'remaining-controller.patch',R/'controller.patch',R/'summary-adaptation.patch']:h(p)
A=R/'api-native'; U=A/'upstream-api'
api_report=j(A/'report.json'); comparison=j(A/'api-comparison.json')
ck('API/native saved report passed and references unchanged',api_report['status']=='passed' and api_report['unchanged'] and unchanged(api_report))
commands(api_report,A,allow_api=True,cpu=1)
ck('API and six build/run pairs count',len(api_report['commands'])==13)
manifest=j(U/'manifest.json'); results=j(U/'results.json'); baseline=Path(comparison['baseline'])
base_manifest=j(baseline/'manifest.json'); base_results=j(baseline/'results.json')
bounds={k:manifest[k] for k in ['per_test_seconds','per_test_address_space_bytes','per_test_rss_limit_bytes','total_timeout_seconds']}
ck('original bounds3s/1024MiB/768MiB/240s', list(bounds.values())==[3,1073741824,805306368,240])
ck('full original public selection',manifest['selected_tests'] is None and len(manifest['public_tests_in_source'])==395 and manifest['chunks']==[0,1,2,3,4,5] and manifest['deferral']==[0,1])
norm=lambda m,p:{k:([x.replace(str(p),'<OUTPUT>') for x in v] if k=='compile_command' else v) for k,v in m.items() if k not in ['library_sha256','binary_sha256']}
ck('baseline/candidate manifests otherwise exact',norm(manifest,U)==norm(base_manifest,baseline))
for name,value in manifest['adapted_sources'].items(): ck('adapted API source '+name,h(U/'adapted'/name)==value)
for name,value in manifest['upstream_include_sources'].items(): ck('upstream include source '+name,h(U/name)==value)
for name,value in manifest['adapter_sources'].items(): ck('API adapter source '+name,h(A/'tools'/name)==value)
ck('API compiled binary and copied library hashes', h(U/'runtests')==manifest['binary_sha256'] and h(U/'liboriole_expat.so')==manifest['library_sha256']==expected_libraries['liboriole_expat.so'])
ck('API raw result bytes match baseline and comparison hashes',read(U/'results.json')==read(baseline/'results.json') and h(U/'results.json')==comparison['candidate_results_sha256']==comparison['baseline_results_sha256'])
raw=[]; began=[]
for line in read(U/'tests.log').decode().splitlines():
    if line.startswith('ORIOLE_RESULT\t'):
        _,context,test,outcome,code=line.split('\t');raw.append({'context':context,'test':test,'outcome':outcome,'code':int(code)})
    if line.startswith('ORIOLE_BEGIN\t'):
        _,context,test=line.split('\t');began.append((test,context))
ck('all4740 rows reconstructed exactly from raw API log',len(raw)==4740 and raw==results['results'] and raw==base_results['results'])
expected_selection={(test,f'chunksize={chunk} deferral={defer}') for test in manifest['public_tests_in_source'] for chunk in manifest['chunks'] for defer in manifest['deferral']}
ck('API each selected context began and completed once',len(set(began))==len(began)==4740 and set(began)==expected_selection=={(row['test'],row['context']) for row in raw})
api_counts=Counter(row['outcome'] for row in raw)
ck('original4347 pass393 fail exit1 retained',api_counts=={'pass':4347,'fail':393} and results['passed']==4347 and results['failed']==393 and results['returncode']==1 and results['selection_complete'] and results['library_origin_verified'] and not results['timed_out'])
ck('API library origin appears in original run log',f'ORIOLE_LIBRARY\t{U}/liboriole_expat.so' in read(U/'tests.log').decode())
native=[]
for linkage in ['dynamic','static']:
    for name in ['integration','adversarial','allocations']:
        label='native-'+name+'-'+linkage
        compile_row=next(row for row in api_report['commands'] if row['label']==label+'-build')
        run_row=next(row for row in api_report['commands'] if row['label']==label)
        cmd=compile_row['command']
        ck('C ASan/UBSan flags '+label,'-fsanitize=address,undefined' in cmd and '-fno-omit-frame-pointer' in cmd and '-Werror' in cmd and '-DNDEBUG' not in cmd)
        ck('C bounded compile and run '+label,compile_row['timeout']==run_row['timeout']==120)
        ck('C original source '+label,h(A/'native'/(name+'.c'))==h(W/'tests/c'/(name+'.c')))
        ck('C binary hash '+label,h(A/'native'/(name+'-'+linkage))==run_row['binary_sha256'])
        ck('C consumer/build stderr empty '+label,read(A/(label+'.stderr'))==b'' and read(A/(label+'-build.stderr'))==b'')
        ck('C expected linkage '+label,('-loriole_expat' in cmd and '-Wl,-rpath,'+str(L) in cmd) if linkage=='dynamic' else str(L/'liboriole_expat.a') in cmd)
        output=read(A/(label+'.stdout')).decode().strip()
        if name=='allocations':ck('327 allocation scenarios '+linkage,'327 scenarios' in output)
        native.append({'consumer':name,'linkage':linkage,'exit':run_row['exit'],'stdout':output,'binary_sha256':run_row['binary_sha256']})
ck('native source manifest hashes',hashes(api_report['native_sources']))
ck('native leak/UB options retained in controller',"'ASAN_OPTIONS':'detect_leaks=0:abort_on_error=1'" in read(R/'api_native.py').decode() and "'UBSAN_OPTIONS':'halt_on_error=1'" in read(R/'api_native.py').decode())

D=R/'other-gates'; other=j(D/'report.json')
ck('other gates report identity',other['status']=='passed' and other['unchanged'] and unchanged(other) and hashes(other['tool_hashes']) and hashes(other['alias_original_hashes']))
commands(other,D)
strict=j(D/'differential/summary.json'); ref=j(D/'differential/reference.json'); cand=j(D/'differential/oriole.json')
ck('strict3318 full paired records exact',len(ref['results'])==len(cand['results'])==3318 and ref['results']==cand['results'] and strict['exact_pass'] and not strict['differences'])
ck('strict3318 no callback exceptions',all(not row['result']['callback_errors'] for row in cand['results']))
ck('strict identities and child exits',all(value['returncode']==0 and value['sha256_before']==value['sha256_after']==h(value['path']) for value in strict['libraries'].values()))
aliases=[]
for row in other['aliases']:
    label='full-events-comparison' if row['name']=='full-events' else 'external-semantic-comparison'
    path=D/'aliases'/(label+'.json');data=j(path)
    ck('alias script/report hashes '+label,h(D/'aliases'/(row['name']+'.py'))==row['script_sha256'] and h(path)==row['report_sha256'])
    ck('alias compressed report readback '+label,gzip.decompress(read(Path(str(path)+'.gz')))==read(path))
    ck('alias retained zero-difference report '+label,data['cases']==row['cases'] and data['differences']==[] and data['sha256']==expected_libraries['liboriole_expat.so'])
    aliases.append({'name':row['name'],'cases':row['cases'],'raw_successful_pairs_retained':False})
def dict_size(path, variable):
    tree=ast.parse(read(path))
    for node in tree.body:
        if isinstance(node,ast.Assign) and any(isinstance(t,ast.Name) and t.id==variable for t in node.targets):
            assert isinstance(node.value,ast.Dict)
            return len(node.value.keys)
    raise AssertionError(variable)
full_count=(dict_size(D/'aliases/probe.py','CASES')+dict_size(D/'aliases/full-events.py','extra'))*3*98*2
external_count=dict_size(D/'aliases/external-semantic-compare.py','cases')*2*98*3
ck('alias source loop arithmetic',full_count==28812 and external_count==7644 and sum(row['cases'] for row in aliases)==36456)

M=R/'malformed'; ml=j(M/'launch.json'); mr=j(M/'malformed-probe.json')
ck('malformed launch complete identities',ml['status']=='passed' and ml['exit']==0 and ml['timeout']==180 and unchanged(ml))
for stream in ['stdout','stderr']:ck('malformed launch hash '+stream,h(M/stream)==ml[stream+'_sha256'])
ck('malformed stderr empty',read(M/'stderr')==b'')
paired=[json.loads(line) for line in gzip.decompress(read(M/'observations.jsonl.gz')).splitlines()]
ck('all2392 malformed full paired records exact',len(paired)==2392 and all(row['control']==row['candidate'] and not row['control']['callback_errors'] for row in paired))
reconstructed=[{'sha256':row['input_sha256'],'encoding':row['encoding'],'chunk':row['chunk'],'control_status':row['control']['status'],'error':row['control']['error'],'fields':[]} for row in paired]
ck('malformed summary reconstructed without normalization',mr['cases']==2392 and not mr['differences'] and mr['rows']==reconstructed)
fixture=[]
for prefix,tail in itertools.product(['','a','a\n','a\r','a\r\n','\n\n','\r\r','\ré\n'],['bad]]>','bad]]>\x01','\x01]]>',']\n]>','a\x01',']]','a\r\nb',']]><n/>']):
    for internal in [False,True]:
        xml=f"<!DOCTYPE r [<!ENTITY e '{prefix}{tail}'>]><r>&e;</r>" if internal else f'<r>{prefix}{tail}</r>'
        fixture.append((xml,[1,2,3,7,64,4096,len(xml.encode())]))
for size,newline,tail in itertools.product([65532,65535,65536,65537,70000],['\n','\r','\r\n'],[']]>','\x01]]>','bad]]>\x01','ok']):
    fixture.append(('<r>a\n'+('x'*(size-2))+newline+tail+'</r>',[65533,65535,65536,65537,200000]))
expected_keys=[(hashlib.sha256(xml.encode(encoding)).hexdigest(),encoding,chunk) for xml,chunks in fixture for encoding in ['utf-8','utf-16'] for chunk in sorted(set(chunks))]
actual_keys=[(row['input_sha256'],row['encoding'],row['chunk']) for row in paired]
ck('malformed every original generated input hash/encoding/chunk reconstructed',expected_keys==actual_keys and len(set(actual_keys))==2392)

P=R/'publication-probe'; pl=j(R/'publication-launch/report.json'); pr=j(P/'report.json')
ck('publication launch complete bounds',pl['status']=='passed' and pl['exit']==0 and pl['timeout']==240 and pl['generator_sha256']==h(R/'publication_probe.py'))
for stream in ['stdout','stderr']:ck('publication stream hash '+stream,h(R/'publication-launch'/stream)==pl[stream+'_sha256'])
ck('publication stderr empty',read(R/'publication-launch/stderr')==b'')
ck('publication source/helper/library hash identity',pr['status']=='passed' and pr['failure'] is None and not pr['hash_errors'] and unchanged(pr,'hashes_before','hashes_after'))
publication=json.loads(gzip.decompress(read(P/'observations.json.gz')))
ck('publication1304 cases3912 parses exact counts',len(publication)==pr['cases']==1304 and pr['library_parses']==3912)
ck('publication observation hash',h(P/'observations.json.gz')==pr['observations_sha256'])
ck('publication1304 unique full case descriptors',len({json.dumps(row['case'],sort_keys=True) for row in publication})==1304)
ck('publication matrix1152 external144 dynamic8 compaction',Counter(row['case']['label']=='compaction-large-fallback' and 'compact' or row['case']['label']=='dynamic' and 'dynamic' or 'external' for row in publication)=={'external':1152,'dynamic':144,'compact':8})
positions=Counter();reference_different=[]
for index,row in enumerate(publication):
    values=row['values'];a=values['candidate'];b=values['baseline'];ref=values['reference']
    ck('publication complete baseline equivalence '+str(index),a==b and set(values)=={'candidate','baseline','reference'} and all(not value['callback_errors'] for value in values.values()))
    ck('publication reference all non-event fields '+str(index),{k:v for k,v in a.items() if k!='events'}=={k:v for k,v in ref.items() if k!='events'})
    ck('publication reference event lengths '+str(index),len(a['events'])==len(ref['events']))
    if a!=ref:reference_different.append(index)
    for left,right in zip(a['events'],ref['events']):
        if left!=right:
            ck('reference only default/external position fields',left[:-1]==right[:-1] and left[1] in ['default','external'] and all(type(v) is int for v in left[-1]+right[-1]))
            positions[left[1]]+=1
ck('publication exact reference differences reconstructed',reference_different==pr['candidate_reference_differences'] and pr['candidate_baseline_differences']==[] and positions=={'default':10416,'external':864})
origins=pr['origins_before_parses']
ck('publication9 recorded same-process origin checks',len(origins)==3 and all({row['symbol'] for row in rows}=={'XML_Parse','XML_ParserCreate','XML_ExternalEntityParserCreate'} and all(Path(row['path']).resolve()==Path(path).resolve() and h(row['path'])==row['sha256']==pr['hashes_before'][path] for row in rows) for path,rows in origins.items()))


OLD=prior_root
gate={'source':{'direct_control_shared_sha256':normal_libraries['liboriole_expat.so']},'libraries':expected_libraries}
LC=R/'end-lifecycle-final';OM=R/'end-oom-final';control=j(LC/'controller.json')
engines={name:j(LC/(name+'.json')) for name in ['baseline','candidate','expat']}
expected={'baseline':gate['source']['direct_control_shared_sha256'],'candidate':gate['libraries']['liboriole_expat.so'],'expat':'7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478'}
ck('38 lifecycle logical cases and114 parsers',control['status']=='passed' and control['logical_cases']==38 and control['parser_executions']==114 and control['outer_timeout_seconds_per_engine']==90)
for name,data in engines.items():
    ck('lifecycle runtime '+name,data['status']=='passed' and data['affinity']==[1] and len(data['cases'])==38 and data['library_unchanged'] and data['expected_sha256']==data['before_sha256']==data['after_sha256']==expected[name]==h(data['library']))
    ck('lifecycle21 same process bindings '+name,len(data['dladdr'])==21 and all(Path(v['path']).resolve()==Path(data['library']).resolve() and v['sha256']==expected[name]==h(v['path']) for v in data['dladdr'].values()))
    ck('all lifecycle case fields equal historical '+name,data['cases']==j(OLD/'end-lifecycle'/(name+'.json'))['cases'])
    ck('lifecycle case IDs unique '+name,len({c['id'] for c in data['cases']})==38)
    for case in data['cases']:
        ck('lifecycle successful invariants and cleanup',case['freed'] and case['retained_callback_count_through_free']==7 and not case['callback_errors'] and not case.get('harness_error') and not case.get('invariant_error') and all(case['invariants'].values()))
        ck('lifecycle exact input bytes',hashlib.sha256(bytes.fromhex(case['input_hex'])).hexdigest()==case['input_sha256'])
    row=next(r for r in control['runs'] if r['engine']==name)
    ck('lifecycle worker success',row['status']=='passed' and row['exit_code']==0 and row['command'][:3]==['taskset','-c','1'] and h(LC/(name+'.json'))==control['result_sha256'][name])
    for stream in ['stdout','stderr']:ck('lifecycle worker output',h(LC/(name+'.'+stream))==row[stream+'_sha256'])
    ck('38 immediate case receipts',[json.loads(line)['case'] for line in read(LC/(name+'.stdout')).splitlines()]==[case['id'] for case in data['cases']] and read(LC/(name+'.stderr'))==b'')
ck('lifecycle full candidate control equality',engines['candidate']['cases']==engines['baseline']['cases'])
comparison=j(LC/'comparison.json');oldcomparison=j(OLD/'end-lifecycle/comparison.json')
ck('exact retained full reference differences',comparison==oldcomparison and comparison['candidate_vs_baseline']==[] and len(comparison['expat_vs_baseline'])==38 and sum(any(not d['path'].startswith('/final/') for d in row['differences']) for row in comparison['expat_vs_baseline'])==14)
for name,field in [('oracle.py','oracle_sha256'),('controller.py','controller_sha256'),('SOURCE_REVIEW.md','source_review_sha256'),('comparison.json','comparison_sha256')]:ck('lifecycle source and comparison hash',h(LC/name)==control[field])
oom=j(OM/'report.json')
ck('OOM recorded success',oom['status']=='passed' and oom['unchanged'] and not oom['hash_errors'] and oom['before']==oom['after'] and len(oom['commands'])==4)
for p,value in oom['before'].items():ck('OOM before after identity',h(p)==value)
for row in oom['commands']:
    ck('OOM command bounds success',row['exit']==0 and row['timeout']==120 and row['command'][:3]==['taskset','-c','1'])
    for stream in ['stdout','stderr']:ck('OOM output hash',h(OM/(row['label']+'.'+stream))==row[stream+'_sha256'])
    if row['label'].endswith('-build'):ck('C-only sanitizer flags',all(x in row['command'] for x in ['-fsanitize=address,undefined','-fno-omit-frame-pointer','-Werror']) and read(OM/(row['label']+'.stderr'))==b'')
    else:ck('OOM binary and narrow XML_Parse origin',h(OM/row['label'])==oom['binaries'][row['label']] and read(OM/(row['label']+'.stderr')).decode()=='XML_Parse origin: '+engines[row['label']]['library']+'\n')
raw=read(OM/'baseline.stdout')
ck('OOM current control candidate historical bytes exact',raw==read(OM/'candidate.stdout')==read(OLD/'end-oom-final/baseline.stdout'))
rows=[json.loads(line) for line in raw.splitlines()];summaries=[row for row in rows if row.get('summary')];retries=[]
ck('exact six OOM scenarios',len(rows)==37 and rows[-1]=={'complete':True,'scenarios':6} and [(r['scenario'],r['mode'],r['fail_index']) for r in summaries]==[(0,0,0),(1,0,1),(2,0,2),(3,1,0),(4,1,1),(5,1,2)])
for row in summaries:
    events=[r for r in rows if r.get('scenario')==row['scenario'] and 'event' in r];retry=[r for r in rows if r.get('scenario')==row['scenario'] and 'retry_error' in r]
    ck('OOM zero live and expected prefix allocations',row['live']==0 and row['calls_before']==33)
    if row['failed']:
        ck('OOM failed slot and stable retries',row['status']==0 and row['error']==1 and len(retry)==2 and retry[0]==retry[1] and retry[0]['retry_calls']==row['calls_before']+row['end_calls'] and retry[0]['retry_events']==len(events));retries.extend(retry)
    else:ck('OOM successful end namespace restoration',not retry and row['status']==1 and row['error']==0 and row['end_calls']==row['end_count']==2 and row['resumes']==row['mode'] and sum(e['event']=='namespace-end' for e in events)==2)
ck('exact measured retry33 retained',[(r['scenario'],r['retry_error']) for r in retries]==[(1,1),(1,1),(2,1),(2,1),(4,33),(4,33),(5,1),(5,1)])
ck('summary lifecycle raw completion',summary['end_lifecycle']=={k:control[k] for k in ['logical_cases','parser_executions','candidate_vs_baseline_different_cases','expat_vs_baseline_different_cases','invariant_failures']})
ck('summary OOM raw completion',summary['end_oom']=={k:oom[k] for k in ['row_counts','scenarios','failed_indices','success_controls','observations_exact']})

initial_summary=j(R/'summary-initial-attempt/receipt.json');initial_code=read(R/'summary-initial-attempt/summarize-final.py');initial_log=read(R/'summary-initial-attempt/log').decode()
ck('initial missing classifier failure retained',initial_summary['status']=='failed_postprocessing_only' and initial_summary['parser_or_compiler_reruns']==0 and 'FileNotFoundError' in initial_log and 'malformed/readback.json' in initial_log)
ck('final summarizer unchanged after classifier generation',initial_code==read(R/'summarize-final.py'))
classification=j(R/'publication-probe/reference-classification.json');malformed_readback=j(R/'malformed/readback.json')
ck('saved malformed readback independently reproduced',malformed_readback=={'status':'passed','paired_rows':2392,'all_observation_fields_exact':True,'sha256':h(R/'malformed/observations.jsonl.gz')})
ck('publication saved classification independently reproduced',classification['cases']==1304 and classification['baseline_candidate_exact'] and classification['reference_outcome_differences']==classification['reference_callback_payload_order_differences']==classification['reference_final_position_differences']==0 and classification['retained_reference_position_events']==dict(positions) and classification['observations_sha256']==h(R/'publication-probe/observations.json.gz'))
ck('author summary source/library identities',summary['status']=='correctness_parity_gates_passed' and summary['source']['files']==70 and summary['source']['manifest_sha256']==h(R/'source.json') and summary['source']['git_commit']=='be22a27' and summary['libraries']=={'baseline':normal_libraries,'candidate':expected_libraries})
ck('author API/native summary',summary['api']['raw_exit']==1 and summary['api']['passed']==4347 and summary['api']['failed']==393 and summary['api']['all4740rows_exact'] and summary['api']['bounds']==bounds and summary['api']['raw_results_sha256']==summary['api']['baseline_sha256']==h(U/'results.json') and summary['native']['consumers']==6 and summary['native']['allocation_scenarios_per_linkage']==327)
ck('author paired scope summary',summary['strict_baseline_traces']==3318 and summary['custom_alias_baseline_comparisons']==36456 and summary['malformed_baseline_comparisons']==2392 and summary['publication']=={'logical_cases':1304,'parser_executions':3912,'exact_baseline':True,'same_process_origin_checks':9,'retained_reference_position_events':dict(positions)} and summary['rehash_recorded_subprocess_streams']==40)
ck('author retained failure summary',summary['retained_harness_failure']['initial_other_controller_sha256']==h(R/'other-controller.json') and summary['retained_harness_failure']['initial_end_controller_sha256']==h(R/'end-lifecycle/controller.json') and summary['retained_harness_failure']['initial_end_parser_executions']==0 and summary['retained_harness_failure']['patch_sha256']==h(R/'end-affinity-fix.patch'))
ck('no workspace results transferred','outside this C-gate artifact' in summary['workspace'] and not (R/'workspace-checks').exists())
audit_prep=j(O/'audit-preparation.json');ck('audit generator lineage',audit_prep['generator_sha256']==h(Path(__file__)) and audit_prep['prior_core_sha256']==h(audit_prep['prior_core']) and audit_prep['prior_end_supplement_sha256']==h(audit_prep['prior_end_supplement']))
for name in ['prepare-audit.py','audit-setup.py','audit-tail.py','adaptation.patch']:h(O/name)
for name in ['classify_observations.py','summary-controller.log','summary-controller-final.log','api-outer.log','other-outer.log','end-outer.log']:h(R/name)
ck('all inspected bytes unchanged at end',all(hashlib.sha256(Path(p).read_bytes()).hexdigest()==v for p,v in tracked.items()))
for name,value in [('checked-files.json',tracked),('checks.json',checks),('native-consumers.json',native),('api-outcomes.json',dict(api_counts))]: (O/name).write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
report={'status':'independent_native_byte_count_saved_C_gates_audit_passed','blocking_findings':[],'scope':'Saved source/hash/controller/raw-result reconstruction only. No target, parser, build, compiler or timing execution. Workspace and CPython gates are separate.','audit_cpu':4,'source_manifest_sha256':h(R/'source.json'),'source_build_review':{'path':str(source_review_path),'sha256':h(source_review_path)},'libraries':summary['libraries'],'api':{'rows':4740,'pass':4347,'fail':393,'exit':1,'all_rows_from_raw_log_exact':True,'all_rows_byte_equal_to_baseline':True,'selected_public_tests':395,'configurations_each':12,'bounds':bounds},'native':{'consumers':6,'allocation_scenarios_each_linkage':327,'all_C_builds_and_runs_exit0':True,'C_ASan_UBSan_only':True},'strict':{'full_paired_traces':3318,'all_fields_exact':True},'aliases':{'comparisons':36456,'groups':aliases,'verification':'Exact inherited source substitutions, fixture-loop counts, pinned libraries, command exits, script/report hashes, compressed readback and empty difference lists. Successful raw pairs are not retained and cannot be independently reread.'},'malformed':{'paired_rows':2392,'all_observations_exact':True,'all_input_hash_encoding_chunk_keys_reconstructed':True},'publication':{'cases':1304,'parser_executions':3912,'all_baseline_fields_exact':True,'same_process_origins':9,'reference_non_event_and_payload_order_exact':True,'retained_reference_callback_position_events':dict(positions)},'end':{'logical_cases':38,'parser_executions':114,'full_cases_equal_baseline_and_historical':True,'same_process_symbol_origins_each_engine':21,'retained_reference_different_cases':38,'retained_reference_in_handler_different_cases':14,'OOM_scenarios_each_engine':6,'OOM_rows_each_engine':37,'OOM_failed_indices_each_engine':4,'OOM_success_controls_each_engine':2,'OOM_raw_bytes_match_baseline_and_historical':True,'OOM_retry33_preserved':True},'retained_failures':{'initial_CPU4_preparation_separate':True,'other_lane_exit_pattern':[0,0,0,1],'initial_End_guard_parser_runs':0,'only_remaining_End38_and_End6_completed_after_CPU_guard_fix':True,'initial_missing_classifier_summary_failure_retained':True,'classifier_then_identical_final_summary':True},'limits':['Original393 canonical API failures and exit1 remain; this is parity with baseline.','Successful custom-alias raw pair records are absent in inherited tools. Source/count/hash/difference-summary audit is the available evidence.','C ASan/UBSan only; Rust release libraries are uninstrumented and leak checking is disabled. Origin records cover the recorded symbols and processes.','End SOURCE_REVIEW.md is historical fixture provenance. End6 namespace restoration does not establish every detached-End fast-path case.','Initial preparation contains CPU4 scripts; the CPU1 controller delta is separate. Initial End guard failed before any parser child, then only remaining End38/End6 ran; original successful probes remain.','Separate source735afb/commitbe22 integration yields the measured ccfb/f58 libraries. Historical sources retain their original identities.','No workspace, CPython, elapsed timing, adoption or blanket compatibility claim. Hash readback establishes stability during this audit, not historical immutability before the first receipt.'],'passed_checks':len(checks),'checked_file_count':len(tracked),'all_inspected_files_unchanged':True,'generator_sha256':h(Path(__file__))}
(O/'review.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'status':report['status'],'review_sha256':hashlib.sha256((O/'review.json').read_bytes()).hexdigest(),'checks':len(checks),'files':len(tracked)},indent=2))
