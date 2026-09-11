"""Replay archived CPython records without importing parser binaries.

Original paths are identity labels. Actual artifact-byte verification belongs
to the archived direct source/build/preflight reviews.
"""
from pathlib import Path
from collections import Counter
import argparse
import gzip
import hashlib
import json
import math
import random
import statistics

cli=argparse.ArgumentParser(description=__doc__)
cli.add_argument('--root',type=Path,default=Path('/tmp/oriole-serialized-accounting-python-study'))
cli.add_argument('--mode',choices=('normal','pgo'),required=True)
cli.add_argument('--output',type=Path,default=Path(__file__).parent)
args=cli.parse_args()
if not __debug__: raise RuntimeError('Run without -O: this verifier uses assertions')
D=args.root/args.mode;S=D/'screen'
original=Path('/tmp/oriole-serialized-accounting-python-study')/args.mode
pins={};aliases=[]
def read(p):
    p=Path(p);b=p.read_bytes()
    try:key=str(p.relative_to(args.root))
    except ValueError:key=str(p)
    pins[key]=hashlib.sha256(b).hexdigest();return b
def sha(p):return hashlib.sha256(read(p)).hexdigest()
def load(p):return json.loads(read(p))
def canonical_sha(x):return hashlib.sha256(json.dumps(x,sort_keys=True,separators=(',',':')).encode()).hexdigest()
def key(c):return f"{c['name']}/{c['chunk']}/{c['mode']}"
def gm(values):return math.exp(statistics.fmean(math.log(x) for x in values))
python='/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3'
a=load(D/'adaptation.json');build=load(D/'consumers/build.json');pref=load(S/'preflight.json');data=load(S/'results.json');controller=load(D/'time-controller.json')
assert a['mode']==args.mode and build['status']=='passed' and build['adaptations'] is None
assert build['cpython_revision']=='3bb231a6a5dc02b95658877318bf61501a7209e9'
assert build['source_sha256_before']==build['source_sha256_after']
for f,h in a['files'].items():assert sha(D/f)==h
assert controller['status']=='passed' and controller['controller_sha256']==sha(D/'run.py') and len(controller['jobs'])==1
j=controller['jobs'][0]
assert j['exit']==0 and j['timeout_seconds']==1200 and 'exception' not in j and j['end']>=j['start']
assert j['command']==['taskset','-c','0',python,'-I','-S',str(original/'consumer_screen.py'),'--phase','time','--kind','python','--build',str(original/'consumers/build.json'),'--input','/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json','--output',str(original/'screen')]
assert sha(D/'time-controller.log')==j['log_sha256']
assert data['status']=='passed' and data['affinity']==[0] and data['kind']=='python'
assert pref['status']=='preflight_passed' and pref['affinity']==[3]
assert data['preflight_sha256']==sha(S/'preflight.json')
assert data['preflights']==pref['preflights'] and data['expected']==pref['expected']
assert data['hashes_before']==data['hashes_after']==pref['hashes_before']==pref['hashes_after'] and data['hash_errors']==pref['hash_errors']=={}
assert data['conditions']==pref['conditions']
conditions=data['conditions'];assert len(conditions)==24 and len({key(c) for c in conditions})==24
assert data['pairs']==pref['pairs']==7 and data['seed']==pref['seed']==202609104412
assert len(data['rows'])==len(data['processes'])==504 and len(pref['preflights'])==len(pref['processes'])==72
libs=a['libraries'];identities={}
for engine,c in build['consumers'].items():
    d=Path(c['directory']);assert d==original/'consumers'/engine
    identity={module:{'path':str(d/(module+'.cpython-312-x86_64-linux-gnu.so')),'sha256':c['files'][str(d/(module+'.cpython-312-x86_64-linux-gnu.so'))]} for module in ('pyexpat','_elementtree')}
    identity['parser_library']={'path':c['library'],'sha256':libs[engine]['sha256']}
    identity['expat_version']='expat_2.8.4' if engine=='expat' else 'oriole_compat_2.8.4'
    assert c['files'][c['library']]==libs[engine]['sha256']
    identities[engine]=identity
engines=['control','candidate','expat'];ratios=[('candidate','control'),('control','expat'),('candidate','expat')]
rng=random.Random(data['seed']);jobs=[(c,pair) for c in conditions for pair in range(7)];rng.shuffle(jobs)
sequence=[]
for c,pair in jobs:
    order=engines.copy();rng.shuffle(order)
    for position,engine in enumerate(order):sequence.append((c,pair,engine,position))
assert len(sequence)==504
all_counts=Counter();medians={};destruction={}
def worker(c,pair,engine,position,row,process,index,phase):
    label=key(c).replace('/','-')+f'-{engine}-{pair}-0';specpath=S/(label+'.json')
    assert process['label']==row['process']==label and process['status']=='passed' and process['returncode']==0 and process['timeout_seconds']==300
    assert process['command']==[python,'-I','-S',str(original/'python_worker.py'),'--worker',str(original/'screen'/(label+'.json'))]
    count=0 if phase=='preflight' else c['iterations']
    spec=load(specpath)
    assert spec=={'input':c['input'],'input_sha256':data['hashes_before'][c['input']],'chunk':c['chunk'],'mode':c['mode'],'consumer':str(original/'consumers'/engine),'library_sha256':libs[engine]['sha256'],'iterations':count}
    for channel in ('stdout','stderr'):assert sha(S/process[channel])==process[channel+'_sha256']
    assert gzip.decompress(read(S/process['stderr']))==b''
    raw_bytes=gzip.decompress(read(S/process['stdout']));raw=json.loads(raw_bytes)
    assert raw['identity']==identities[engine] and raw['gc_enabled'] is True and raw['python']==build['python']==data['python_version']
    samples=raw['samples'];assert len(samples)==count+1
    values=[];destroy=[]
    for i,s in enumerate(samples):
        assert s['iteration']==i and s['warmup']==(i==0) and s['parse_ns']>0 and s['destruction_ns']>=0
        assert math.isfinite(s['seconds']) and s['seconds']==(s['parse_ns']+s['destruction_ns'])/1e9
        assert [s['output_sha256'],s['output_rows']]==data['expected'][key(c)]
        all_counts[phase+'_warmups' if i==0 else phase+'_measured']+=1
        if i:values.append(s['seconds']);destroy.append(s['destruction_ns']/1e9)
    median=statistics.median(values) if values else None
    reconstructed={'key':key(c),'engine':engine,'pair':pair,'process':label,'identity':raw['identity'],'median_seconds':median,'samples':samples}
    if phase=='time':reconstructed['order']=position
    assert row==reconstructed
    aliases.append({'report':f'{args.mode}/screen/'+('preflight.json' if phase=='preflight' else 'results.json'),'array':'preflights' if phase=='preflight' else 'rows','index':index,'raw_stdout':f'{args.mode}/screen/'+process['stdout'],'raw_payload_sha256':hashlib.sha256(raw_bytes).hexdigest(),'identity_sha256':canonical_sha(raw['identity']),'samples_sha256':canonical_sha(samples),'row_sha256':canonical_sha(reconstructed)})
    if phase=='time':medians[key(c),pair,engine]=median;destruction[key(c),pair,engine]=statistics.median(destroy)
for i,(row,process) in enumerate(zip(pref['preflights'],pref['processes'],strict=True)):
    worker(conditions[i//3],-1,engines[i%3],None,row,process,i,'preflight')
for i,((c,pair,engine,position),row,process) in enumerate(zip(sequence,data['rows'],data['processes'],strict=True)):
    worker(c,pair,engine,position,row,process,i,'time')
assert all_counts=={'preflight_warmups':72,'time_warmups':504,'time_measured':25788}
summaries=[]
for c in conditions:
    pm={engine:[medians[key(c),pair,engine] for pair in range(7)] for engine in engines}
    prs={f'{a}_over_{b}':[pm[a][pair]/pm[b][pair] for pair in range(7)] for a,b in ratios}
    summaries.append({'condition':c,'process_pair_medians_seconds':pm,'paired_ratios':prs,'median_ratios':{k:statistics.median(v) for k,v in prs.items()}})
assert summaries==data['summary']
aggregates={}
for group in ('all','elementtree','pyexpat-events'):
    selected=[r for r in summaries if group=='all' or r['condition']['mode']==group]
    aggregates[group]={'conditions':len(selected),'geomean_ratios':{f'{a}_over_{b}':gm([r['median_ratios'][f'{a}_over_{b}'] for r in selected]) for a,b in ratios},'candidate_lower_than_control':sum(r['median_ratios']['candidate_over_control']<1 for r in selected),'adverse_conditions':[r['condition'] for r in selected if r['median_ratios']['candidate_over_control']>1]}
read(__file__)
out={'status':'passed_independent_raw_elapsed_reconstruction','mode':args.mode,'scope':'Saved records only; no binaries imported or executed, no elapsed rerun. Archived records bind source, compiler vectors and identities; actual compiler/library bytes were checked in the original preflight review.',
'counts':{'conditions':24,'cohorts':168,'engines':3,'preflight_workers':72,'timed_workers':504,'all_workers':576,'samples':dict(all_counts),'all_samples':sum(all_counts.values())},
'aggregates':aggregates,'summary':summaries,'libraries':libs,'seed':data['seed'],'pair_order_verified':True,'raw_rows_exactly_reconstructed':576,'all_callback_canonical_outputs_and_module_dladdr_records_verified':True,
'limitations':['Creation/feed/finalization/callbacks plus explicit destruction are timed; canonical validation, input read, imports and gc.collect are outside. Automatic GC remains enabled.','These are original XML files parsed through unmodified CPython, not whole projects or the cleanup-patched strict consumer. Adjacent text callbacks are coalesced for canonical checking.','All adverse conditions are retained. Host noise remains; no selection decision is inferred.','The enclosing portable verifier binds source/build records and saved extension vectors. Raw preflight and timed identities are reconstructed; actual binary bytes are excluded.'],
'source_results_sha256':sha(S/'results.json'),'source_preflight_sha256':sha(S/'preflight.json'),'evidence_sha256':dict(sorted(pins.items()))}
for path,digest in pins.items():
    actual=Path(path) if Path(path).is_absolute() else args.root/path
    assert hashlib.sha256(actual.read_bytes()).hexdigest()==digest,path
args.output.mkdir(parents=True,exist_ok=True)
aliaspath=args.output/(args.mode+'-row-aliases.json')
with aliaspath.open('x') as stream:stream.write(json.dumps(aliases,indent=2)+'\n')
out['row_aliases_sha256']=hashlib.sha256(aliaspath.read_bytes()).hexdigest()
dest=args.output/(args.mode+'-review.json')
with dest.open('x') as stream:stream.write(json.dumps(out,indent=2)+'\n')
print(json.dumps({'mode':args.mode,'status':out['status'],'report_sha256':hashlib.sha256(dest.read_bytes()).hexdigest(),'counts':out['counts'],'aggregates':{k:{n:v for n,v in d.items() if n!='adverse_conditions'} for k,d in aggregates.items()}},indent=2))
