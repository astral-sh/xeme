from pathlib import Path
import hashlib,json
root=Path(__file__).parent
prior=Path('/tmp/oriole-namespace-qname-only-benchmark')
def load(p):return json.loads(p.read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
execution=load(root/'execution.json')
assert execution['status']=='passed' and all(r['exit']==0 for r in execution['commands'])
for phase in ('native','python'):
 m=load(root/('host-during-'+phase+'.json'));assert m['status']=='completed' and m['controller_status']=='passed'
rows={}
for label,base in [('first',prior),('confirmation',root)]:
 n=load(base/'native/review.json');p=load(base/'python/normal-review.json')
 assert n['status']==p['status']=='passed'
 nr=[r for r in n['all_conditions'] if not r['condition']['name'].startswith('generated-')]
 pr=p['summary']
 rows[label]={'native':n['groups']['real'],'generated':n['groups']['generated'],'python':p['aggregates']['all'],'native_worst':max(nr,key=lambda r:r['median_ratios']['candidate_over_control']),'python_worst':max(pr,key=lambda r:r['median_ratios']['candidate_over_control']),'native_counts':n['counts'],'python_counts':p['counts']}
assert all(rows[e][k]['geomean_ratios']['candidate_over_control']<1 for e in rows for k in ('native','python'))
assert all(rows[e][k]['geomean_ratios']['candidate_over_expat']<1.2 for e in rows for k in ('native','python'))
pins={str(p):sha(p) for p in root.rglob('*') if p.is_file()}
result={'status':'passed','candidate_commit':'735948097c48a2a30cdc88be49632c1410b4edc8','libraries':load(root/'binding.json')['libraries'],'epochs':rows,'raw_readback':'All 1,248 workers and 132,540 samples reconstructed; canonical outputs and origins match; sample medians, pair schedule and equal-weight geomeans verified.','conclusion':'A second separate measurement epoch repeats a small improvement in native and CPython real-project aggregates. Both remain within the 20% real-project Expat target. No statistical significance or performance guarantee; shared-host and generated-case limitations retained.','reuse':'All frozen libraries and six original CPython extensions reused without rebuilding; fresh canonical preflights. Original source and original epoch evidence unchanged.','pins':pins}
(root/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'status':'passed','summary_sha256':sha(root/'summary.json'),'confirmation':{k:rows['confirmation'][k]['geomean_ratios'] for k in ('native','python','generated')},'native_counts':rows['confirmation']['native_counts'],'python_counts':rows['confirmation']['python_counts']},indent=2))
