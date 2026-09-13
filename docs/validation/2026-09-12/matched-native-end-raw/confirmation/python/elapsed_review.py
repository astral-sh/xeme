"""Replay all saved normal CPython workers with the original independent arithmetic."""
from pathlib import Path
from collections import Counter
from types import SimpleNamespace
import csv
import gzip
import hashlib
import json
import math
import random
import statistics
import os

assert os.sched_getaffinity(0)=={6}
D=Path('/tmp/oriole-matched-native-end-raw-study/python');C=Path('/tmp/oriole-matched-native-end-raw-confirmation/python');S=C/'screen';original=C
args=SimpleNamespace(mode='normal');pins={}
def read(p):
    p=Path(p);data=p.read_bytes();pins[str(p)]=hashlib.sha256(data).hexdigest();return data
def sha(p):return hashlib.sha256(read(p)).hexdigest()
def load(p):return json.loads(read(p))
def key(c):return f"{c['name']}/{c['chunk']}/{c['mode']}"
def gm(values):return math.exp(statistics.fmean(math.log(x) for x in values))
python='/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3'
binding=load(C/'preflight-review.json')
assert binding['status']=='passed' and binding['mode']=='normal'
for path,expected in binding['pins'].items():assert sha(path)==expected,path
source=load(D.parent/'source.json')
for name,expected in source['sha256'].items():assert sha(Path(source['worktree'])/name)==expected,name
buildbinding=load(D.parent/'build-binding.json')
assert buildbinding['status']=='passed'
assert buildbinding['build_sha256']==sha(D.parent/'build.json')
assert buildbinding['source_manifest_sha256']==sha(D.parent/'source.json')
a=load(D/'adaptation.json');build=load(D/'consumers/build.json');pref=load(S/'preflight.json');data=load(S/'results.json');controller=load(C/'time-controller.json')
assert a['mode']==args.mode and build['status']=='passed' and build['adaptations'] is None
assert build['cpython_revision']=='3bb231a6a5dc02b95658877318bf61501a7209e9'
assert build['source_sha256_before']==build['source_sha256_after']
for f,h in a['files'].items():assert sha(D/f)==h
assert controller['status']=='passed' and controller['controller_sha256']==sha(C/'run.py') and len(controller['jobs'])==1
j=controller['jobs'][0]
assert j['exit']==0 and j['timeout_seconds']==1200 and 'exception' not in j and j['end']>=j['start']
assert j['command']==['taskset','-c','0',python,'-I','-S',str(original/'consumer_screen.py'),'--phase','time','--kind','python','--build',str(D/'consumers/build.json'),'--input','/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json','--output',str(original/'screen')]
assert sha(C/'time-controller.log')==j['log_sha256']
assert data['status']=='passed' and data['affinity']==[0] and data['kind']=='python'
assert pref['status']=='preflight_passed' and pref['affinity']==[3]
assert data['preflight_sha256']==sha(S/'preflight.json')
assert data['preflights']==pref['preflights'] and data['expected']==pref['expected']
assert data['hashes_before']==data['hashes_after']==pref['hashes_before']==pref['hashes_after'] and data['hash_errors']==pref['hash_errors']=={}
assert data['conditions']==pref['conditions']==binding['conditions']
conditions=data['conditions'];assert len(conditions)==24 and len({key(c) for c in conditions})==24
assert data['pairs']==pref['pairs']==7 and data['seed']==pref['seed']==202609104412
assert len(data['rows'])==len(data['processes'])==504 and len(pref['preflights'])==len(pref['processes'])==72
libs=a['libraries'];identities={}
for engine,c in build['consumers'].items():
    d=Path(c['directory']);assert str(d)==a['consumer_directories'][engine]
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
    assert spec=={'input':c['input'],'input_sha256':data['hashes_before'][c['input']],'chunk':c['chunk'],'mode':c['mode'],'consumer':a['consumer_directories'][engine],'library_sha256':libs[engine]['sha256'],'iterations':count}
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

rows=[]
for r in summaries:
    c=r['condition'];ratios=r['median_ratios']
    rows.append({'project':c['name'],'chunk':c['chunk'],'consumer':c['mode'],'iterations':c['iterations'],**ratios})
for group,entry in aggregates.items():
    selected=[r for r in summaries if group=='all' or r['condition']['mode']==group]
    entry['candidate_within_1_10_of_expat']=sum(r['median_ratios']['candidate_over_expat']<=1.10 for r in selected)
    entry['adverse_conditions']=[{'condition':r['condition'],**r['median_ratios']} for r in selected if r['median_ratios']['candidate_over_control']>1]
report={'status':'passed','mode':'normal','scope':'Saved-only final reconstruction using the selected independent raw-worker arithmetic. Six historical compiler vector/source bindings and unchanged reused modules, zero confirmation compiles, all module and dladdr parser origins, canonical outputs, seeded worker order, sample arithmetic and summaries verified. No target execution or timing rerun.',
        'counts':{'conditions':24,'cohorts':168,'preflight_workers':72,'timed_workers':504,'all_workers':576,'samples':dict(all_counts),'all_samples':sum(all_counts.values())},
        'aggregates':aggregates,'summary':summaries,'libraries':libs,'pair_order_verified':True,
        'limitations':['Original project XML inputs through unmodified CPython consumers, not whole project runs.','Adjacent text callbacks are coalesced by canonical checking; strict-suite callback assertions remain separate.','Parse construction/feed/finalization/callbacks and explicit result destruction timed; input reads/imports/canonical checks/gc.collect outside. GC remains enabled.','All adverse conditions retained; shared-host frequency/cache/bandwidth remain uncontrolled.'],
        'source_results_sha256':sha(S/'results.json'),'source_preflight_sha256':sha(S/'preflight.json'),'reader_sha256':sha(__file__),'evidence_sha256':dict(sorted(pins.items()))}
for path,expected in pins.items():assert hashlib.sha256(Path(path).read_bytes()).hexdigest()==expected,path
csvpath=C/'normal-conditions.csv'
with csvpath.open('x',newline='') as stream:
    writer=csv.DictWriter(stream,fieldnames=list(rows[0]));writer.writeheader();writer.writerows(rows)
report['condition_csv_sha256']=hashlib.sha256(csvpath.read_bytes()).hexdigest()
out=C/'normal-review.json'
with out.open('x') as stream:stream.write(json.dumps(report,indent=2)+'\n')
print(json.dumps({'status':report['status'],'counts':report['counts'],'aggregates':report['aggregates'],'csv':str(csvpath),'report':str(out),'report_sha256':hashlib.sha256(out.read_bytes()).hexdigest()},indent=2))
