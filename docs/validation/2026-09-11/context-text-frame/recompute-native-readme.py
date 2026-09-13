"""Rebuild main README's six 4-KiB, namespace-off rows from saved PGO samples."""
from pathlib import Path
import hashlib,json,statistics
root=Path(__file__).resolve().parent
source=Path('/tmp/oriole-context-text-frame-fixed-native-study/pgo/native-screen/results.json')
review_path=Path('/tmp/oriole-context-text-frame-fixed-independent-review/native/review.json')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
review=json.loads(review_path.read_text());assert review['status']=='passed'
assert sha(source)==review['input_sha256'][str(source)]
raw=json.loads(source.read_text());assert raw['status']=='passed'
labels={'vulkan':'Vulkan registry','wayland':'Wayland protocol','maven':'Maven POM','batik':'Batik SVG','gtk':'GTK UI','docbook':'DocBook XSL'}
rows=[]
for name,label in labels.items():
 groups=[row for row in raw['rows'] if row['condition']['name']==name and row['condition']['chunk']==4096 and row['condition']['namespaces'] is False]
 assert len(groups)==7 and {r['pair'] for r in groups}==set(range(7))
 workers=[]
 for cohort in groups:
  medians={}
  for worker in cohort['processes']:
   assert worker['returncode']==0 and worker['stderr']==''
   samples=json.loads(worker['stdout'])['samples'];assert [s['warmup'] for s in samples]==[True]+[False]*cohort['condition']['iterations']
   median=statistics.median(s['seconds'] for s in samples[1:]);assert median==worker['median_seconds']
   medians[worker['engine']]=median
  workers.append({'pair':cohort['pair'],'order':cohort['order'],'medians_seconds':medians})
 ratios={a+'_over_'+b:[w['medians_seconds'][a]/w['medians_seconds'][b] for w in workers] for a,b in [('candidate','control'),('candidate','expat'),('control','expat')]}
 med_ratios={key:statistics.median(values) for key,values in ratios.items()}
 retained=next(r for r in raw['summary'] if r['condition']==groups[0]['condition'])
 assert retained['paired_ratios']==ratios and retained['median_ratios']==med_ratios
 times={engine:statistics.median(w['medians_seconds'][engine] for w in workers) for engine in ('candidate','control','expat')}
 rows.append({'project':name,'label':label,'condition':groups[0]['condition'],'process_medians':workers,'median_of_process_medians_seconds':times,'median_of_process_medians_ms':{k:v*1000 for k,v in times.items()},'paired_ratios':ratios,'median_paired_ratios':med_ratios})
output={'status':'recomputed_from_raw_samples','caption':'Original held-out XML; 4 KiB feed, namespaces disabled. Durations are medians of seven process medians; ratios are medians of seven paired ratios and need not equal ratios of rounded durations. Same original-G PGO libraries and native driver as full24-condition review.','source_sha256':sha(source),'review_sha256':sha(review_path),'script_sha256':sha(Path(__file__)),'rows':rows,'all24_real_ratios':review['native']['pgo']['groups']['real']['ratios']}
(root/'native-readme-table.json').write_text(json.dumps(output,indent=2)+'\n')
lines=['| Project XML | Oriole (PGO) | Expat (PGO) | Oriole / Expat |','| --- | ---: | ---: | ---: |']
for row in rows:
 t=row['median_of_process_medians_ms'];ratio=row['median_paired_ratios']['candidate_over_expat']
 lines.append(f"| {row['label']} | {t['candidate']:.3f} ms | {t['expat']:.3f} ms | {ratio:.2f}× |")
(root/'native-readme-table.md').write_text('\n'.join(lines)+'\n')
print(json.dumps({'status':'passed','json_sha256':sha(root/'native-readme-table.json'),'markdown_sha256':sha(root/'native-readme-table.md'),'rows':len(rows)}))
print('\n'.join(lines))
