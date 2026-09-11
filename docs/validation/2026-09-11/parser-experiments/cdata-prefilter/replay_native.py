"""Portable arithmetic/semantic replay of the six retained native JSON records.

Does not load a parser or require binaries, XML input bytes, original absolute
filesystem paths, or individual worker files. The original local audit separately
verified those bytes and each worker file against these embedded records.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import statistics

if not __debug__:
    raise RuntimeError('Do not disable validation with Python -O')
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--evidence-root',type=Path,required=True,
                    help='Directory containing normal/native-screen and pgo/native-screen')
parser.add_argument('--review',type=Path,default=Path(__file__).with_name('review.json'))
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
read=lambda p:json.loads(p.read_text())
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
review=read(args.review)
assert review['status']=='passed'
logical_root=Path('/tmp/oriole-cdata-prefilter-native-study')
result={'status':'passed_records_only','modes':{},'input_sha256':{},
        'scope':'Portable replay from hash-bound full JSON records only. No current compiler, library, XML, source or original worker-file bytes are verified in this mode; those checks belong to the original local audit.',
        'review_sha256':sha(args.review),'replay_sha256':sha(Path(__file__))}

for mode in ('normal','pgo'):
    root=args.evidence_root/mode/'native-screen'
    for name in ('protocol.json','preflight.json','results.json'):
        digest=sha(root/name)
        assert digest==review['input_sha256'][str(logical_root/mode/'native-screen'/name)]
        result['input_sha256'][mode+'/'+name]=digest
    p,f,r=[read(root/name) for name in ('protocol.json','preflight.json','results.json')]
    assert f['status']==r['status']=='passed' and r['affinity']==[0]
    assert f['protocol_sha256']==r['protocol_sha256']==sha(root/'protocol.json')
    assert r['preflight_sha256']==sha(root/'preflight.json')
    assert f['hashes_before']==f['hashes_after']==r['hashes_before']==r['hashes_after']==p['hashes']
    conditions=[q['condition'] for q in review['native'][mode]['all_conditions']]
    assert p['conditions']==conditions and len(conditions)==28
    assert p['pairs']==7 and p['seed']==2026091003
    assert list(p['libraries'])==['control','candidate','expat']
    assert len(f['rows'])==84 and len(r['rows'])==196 and len(r['summary'])==28
    observations={}
    medians={}
    totals={'workers':0,'samples':0}

    def worker(row,c,pair,engine,cpu,count):
        assert row['condition']==c and row['pair']==pair and row['engine']==engine
        assert row['returncode']==0 and row['stderr']==''
        command=['taskset','-c',str(cpu),'/tmp/oriole-grammar-bench-plain/native-driver',
                 p['libraries'][engine]['path'],c['input'],str(c['chunk']),str(count)]
        if c['namespaces']:
            command.append('namespaces')
        assert row['command']==command
        samples=json.loads(row['stdout'])['samples']
        assert len(samples)==count+1
        assert [x['iteration'] for x in samples]==list(range(count+1))
        assert [x['warmup'] for x in samples]==[True]+[False]*count
        assert all(math.isfinite(x['seconds']) and x['seconds']>0 for x in samples)
        got=sorted({(x['hash'],x['elements'],x['text_bytes']) for x in samples})
        assert len(got)==1 and got[0][1]>=0 and got[0][2]>=0
        assert row['observations']==[list(x) for x in got]
        key=json.dumps(c,sort_keys=True)
        assert key not in observations or observations[key]==got
        observations[key]=got
        median=statistics.median(x['seconds'] for x in samples[1:])
        assert median==row['median_seconds']
        totals['workers']+=1
        totals['samples']+=len(samples)
        return median

    ordinal=0
    for condition in conditions:
        for engine in p['libraries']:
            worker(f['rows'][ordinal],condition,-1,engine,5,1)
            ordinal+=1
    rng=random.Random(p['seed'])
    jobs=[(condition,pair) for condition in conditions for pair in range(7)]
    rng.shuffle(jobs)
    for row,(condition,pair) in zip(r['rows'],jobs,strict=True):
        order=list(p['libraries'])
        rng.shuffle(order)
        assert row['condition']==condition and row['pair']==pair and row['order']==order
        for record,engine in zip(row['processes'],order,strict=True):
            medians[(json.dumps(condition,sort_keys=True),pair,engine)]=worker(
                record,condition,pair,engine,0,condition['iterations'])
    summary=[]
    for condition in conditions:
        pairs=[row['pair'] for row in r['rows'] if row['condition']==condition]
        assert sorted(pairs)==list(range(7))
        key=json.dumps(condition,sort_keys=True)
        ratios={a+'_over_'+b:[medians[(key,pair,a)]/medians[(key,pair,b)] for pair in pairs]
                for a,b in [('candidate','control'),('candidate','expat'),('control','expat')]}
        summary.append({'condition':condition,'paired_ratios':ratios,
                        'median_ratios':{k:statistics.median(v) for k,v in ratios.items()}})
    assert summary==r['summary']
    assert [{'condition':q['condition'],'median_ratios':q['median_ratios']} for q in summary]==review['native'][mode]['all_conditions']
    assert totals=={'workers':672,'samples':106176}
    groups={}
    for name in ('real','generated'):
        entries=[q for q in summary if q['condition']['name'].startswith('generated-')==(name=='generated')]
        groups[name]={'conditions':len(entries),
                      'ratios':{k:statistics.geometric_mean(q['median_ratios'][k] for q in entries)
                                for k in entries[0]['median_ratios']},
                      'candidate_faster_than_control':sum(q['median_ratios']['candidate_over_control']<1 for q in entries),
                      'candidate_faster_than_expat':sum(q['median_ratios']['candidate_over_expat']<1 for q in entries)}
    groups['adverse_conditions']=[{'name':q['condition']['name'],'chunk':q['condition']['chunk'],
                                  'namespaces':q['condition']['namespaces'],
                                  'ratio':q['median_ratios']['candidate_over_control']}
                                 for q in summary if q['median_ratios']['candidate_over_control']>1]
    assert groups==review['native'][mode]['groups']
    result['modes'][mode]={'totals':totals,'groups':groups}

result['totals']={'workers':sum(x['totals']['workers'] for x in result['modes'].values()),
                  'samples':sum(x['totals']['samples'] for x in result['modes'].values())}
assert result['totals']=={'workers':1344,'samples':212352}
with args.output.open('x') as f:
    json.dump(result,f,indent=2)
    f.write('\n')
print(json.dumps({'status':result['status'],'output':str(args.output),'sha256':sha(args.output),
                  'totals':result['totals']}))
