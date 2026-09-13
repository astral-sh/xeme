from pathlib import Path, PurePosixPath
import collections, csv, hashlib, json, math, os, re, statistics, subprocess, tarfile
assert os.sched_getaffinity(0)=={6}
P=Path('/tmp/oriole-fused-normal-publication-draft/docs/validation/2026-09-12/fused-attribute-scan')
W=Path('/home/dev-user/code/oss/oriole-fused-normal-current')
B=Path('/tmp/oriole-fused-normal-current-study')
H=lambda b:hashlib.sha256(b).hexdigest()
J=lambda p:json.loads(Path(p).read_text())
R=J(P/'report.json'); I=J(P/'index.json'); S=J(P.parents[3]/'STABLE.json')
assert H((P/I['archive']).read_bytes())==I['archive_sha256']==S['archive_sha256']
assert (P/I['archive']).stat().st_size==I['archive_bytes']==S['archive_bytes']==6980176
assert H((P/'index.json').read_bytes())==S['index_sha256']
index={x['member']:x for x in I['members']}; assert len(index)==len(I['members'])==3380
seen=set(); total=0; stored={}; original_count=0
with tarfile.open(P/I['archive'],'r:gz') as tar:
 for m in tar:
  assert m.isfile() and m.name in index and m.name not in seen
  assert not PurePosixPath(m.name).is_absolute() and '..' not in PurePosixPath(m.name).parts
  row=index[m.name]; content=tar.extractfile(m).read(); assert len(content)==m.size==row['size'];assert H(content)==row['sha256']
  if m.name=='source/member-map.json':
   assert row['original_path']=='generated from both verified73-source manifests'
  else:
   original=Path(row['original_path']).read_bytes();assert content==original,row['original_path'];original_count+=1
  assert not content.startswith(b'\x7fELF') and not content.startswith(b'!<arch>\n'),m.name
  seen.add(m.name);total+=len(content)
  if m.name.startswith('source/') or m.name.endswith('manifest.json'):stored[m.name]=content
assert seen==set(index)
for row in I['excluded']:
 p=Path(row['path']);assert p.stat().st_size==row['size'];assert H(p.read_bytes())==row['sha256']
for row in R['inputs']:assert H(Path(row['path']).read_bytes())==row['sha256']
source=J(B/'source.json');commit=J(P/'committed-source.json');assert len(source['sha256'])==commit['files']==73
assert source['sha256']==commit['committed_and_live_sha256']
assert H((B/'source.json').read_bytes())==commit['source_manifest_sha256']
assert subprocess.check_output(['git','-C',str(W),'rev-parse','HEAD'],text=True).strip()==commit['commit']=='09da8b14f06720f23c526eb091e3f34efeb09630'
for name,sha in source['sha256'].items():
 assert H((W/name).read_bytes())==sha
 assert H(subprocess.check_output(['git','-C',str(W),'show',commit['commit']+':'+name]))==sha
 assert H(stored['source/candidate/'+name])==sha
assert subprocess.check_output(['git','-C',str(W),'diff',commit['parent'],commit['commit'],'--name-only'],text=True).splitlines()==['crates/oriole/src/tag.rs']
mapping=json.loads(stored['source/member-map.json']);assert {'candidate','control'} <= set(mapping)
for engine,sha_map in [('candidate',source['sha256']),('control',source['sha256'])]:
 assert set(mapping[engine])==set(sha_map)
 for name,member in mapping[engine].items():
  expected=sha_map[name] if engine=='candidate' else H(subprocess.check_output(['git','-C',str(W),'show',commit['parent']+':'+name]))
  assert index[member]['sha256']==expected
# Exact summaries stay tied to the completed saved readers.
native=J(B/'native/review.json');python=J(B/'python/normal-review.json')
assert R['native']['counts']==native['counts']
for name,g in native['groups'].items():
 assert {k:v for k,v in R['native']['groups'][name].items() if k!='candidate_within_1_10_of_expat'}==g
assert R['python']['counts']==python['counts'] and R['python']['aggregates']==python['aggregates']
assert R['native_all_conditions']==native['all_conditions'] and R['python_all_conditions']==python['summary']
assert R['native']['counts']['all_workers']+R['python']['counts']['all_workers']==1248
assert R['native']['counts']['all_samples']+R['python']['counts']['all_samples']==132540
# Recompute every timed process median and every pair ratio directly from samples.
N=J(B/'native/native-screen/results.json');native_medians={};native_samples=0;native_pair_order={}
for cohort in N['rows']:
 c=cohort['condition'];key=(c['name'],c['chunk'],c['namespaces']);pair=cohort['pair'];observations=[];native_pair_order.setdefault(key,[]).append(pair)
 for p in cohort['processes']:
  assert p['returncode']==0 and p['stderr']==''
  samples=json.loads(p['stdout'])['samples'];assert len(samples)==c['iterations']+1
  assert samples[0]['warmup'] and all(not s['warmup'] for s in samples[1:])
  median=statistics.median(s['seconds'] for s in samples[1:]);assert median==p['median_seconds'];native_samples+=len(samples)
  native_medians[key,pair,p['engine']]=median
  observations.append({(s['hash'],s['elements'],s['text_bytes']) for s in samples})
 assert len(observations[0])==1 and observations[0]==observations[1]==observations[2]
assert len(native_medians)==588 and native_samples==106008
Y=J(B/'python/screen/results.json');python_medians={};python_samples=0
for p in Y['rows']:
 samples=p['samples'];assert samples[0]['warmup'] and all(not s['warmup'] for s in samples[1:])
 for s in samples:assert math.isclose(s['seconds'],(s['parse_ns']+s['destruction_ns'])/1e9,rel_tol=1e-14)
 median=statistics.median(s['seconds'] for s in samples[1:]);assert median==p['median_seconds'];python_samples+=len(samples)
 python_medians[p['key'],p['pair'],p['engine']]=median
assert len(python_medians)==504 and python_samples==26292
rows=list(csv.DictReader((P/'conditions.csv').open()));assert len(rows)==52
ratios={};medians={'native':native_medians,'python':python_medians}
for consumer,conditions in [('native',R['native_all_conditions']),('python',R['python_all_conditions'])]:
 for c in conditions:
  cond=c['condition'];key=(cond['name'],cond['chunk'],cond['namespaces']) if consumer=='native' else f"{cond['name']}/{cond['chunk']}/{cond['mode']}"
  rkey=(consumer,cond['name'],str(cond['chunk']),str(cond['namespaces']).lower() if consumer=='native' else cond['mode']);ratios[rkey]={}
  for label in ['candidate_over_control','candidate_over_expat','control_over_expat']:
   a,b=label.split('_over_');pairs=[medians[consumer][key,p,a]/medians[consumer][key,p,b] for p in (native_pair_order[key] if consumer=='native' else range(7))]
   assert pairs==c['paired_ratios'][label];assert statistics.median(pairs)==c['median_ratios'][label]
   ratios[rkey][label]=statistics.median(pairs)
for row in rows:
 key=(row['consumer'],row['project'],row['chunk_bytes'],row['namespace_or_mode'])
 for label,value in ratios[key].items():assert float(row[label])==value
 assert (row['adverse']=='True')==(ratios[key]['candidate_over_control']>1)
adverse=list(csv.DictReader((P/'adverse-conditions.csv').open()));assert adverse==[r for r in rows if r['adverse']=='True'] and len(adverse)==2
for consumer,groups in [('native',R['native']['groups']),('python',R['python']['aggregates'])]:
 for name,g in groups.items():
  selected=[r for r in rows if r['consumer']==consumer]
  if consumer=='native':
   selected=[r for r in selected if r['project'].startswith('generated-')==(name=='generated')]
   if name.startswith('real_namespaces_'):selected=[r for r in selected if r['namespace_or_mode']==str(name.endswith('_on')).lower()]
  elif name!='all':selected=[r for r in selected if r['namespace_or_mode']==name]
  assert len(selected)==g['conditions']
  for label,value in g['geomean_ratios'].items():assert math.isclose(math.exp(statistics.mean(math.log(float(r[label])) for r in selected)),value,rel_tol=1e-14)
# Main README native table: medians of seven process medians; median paired ratios.
table=J(P/'README-table.json');main=(W/'README.md').read_text();labels=['Vulkan registry','Wayland protocol','Maven POM','Batik SVG','GTK UI','DocBook XSL']
for label,row in zip(labels,table['rows'],strict=True):
 key=(row['project'],4096,False)
 for engine,values in row['process_medians_seconds'].items():assert values==[native_medians[key,p,engine] for p in native_pair_order[key]]
 assert statistics.median(row['process_medians_seconds']['candidate'])==row['candidate_seconds']
 assert statistics.median(row['process_medians_seconds']['expat'])==row['expat_seconds']
 assert statistics.median(row['paired_ratios'])==row['candidate_over_expat']
 assert f"| {label} | {row['candidate_seconds']*1000:.3f} ms | {row['expat_seconds']*1000:.3f} ms | {row['candidate_over_expat']:.3f}× |" in main
# Keep the exact warning and licensing sections/files from the selected baseline.
old=subprocess.check_output(['git','-C',str(W),'show',commit['parent']+':README.md'],text=True)
assert main[main.index('> [!WARNING]'):main.index('\n## Highlights')]==old[old.index('> [!WARNING]'):old.index('\n## Highlights')]
assert main[main.index('## License'):]==old[old.index('## License'):]
for name in ['LICENSE-APACHE','LICENSE-MIT']:assert (W/name).read_bytes()==subprocess.check_output(['git','-C',str(W),'show',commit['parent']+':'+name])
api=J('/tmp/oriole-fused-normal-current-correctness/api-c-readback.json');strict=J('/tmp/oriole-fused-normal-current-correctness/strict-semantic-readback.json')
assert R['candidate_compatibility']['api']==api['api'] and R['candidate_compatibility']['c_consumers']==api['c_consumers']
for mode,s in R['candidate_compatibility']['strict'].items():
 for k in ['methods','rendered_outcome_lines','raw_exit','changes','failures']:assert s[k]==strict['strict'][mode][k]
assert [(x['tests'],x['exit'],x['reaped']) for x in R['candidate_compatibility']['supplemental_semantics']]==[(2,0,True),(2,0,True)]
out={'status':'passed_saved_archive_source_arithmetic_and_readme_review','role':'Independent final publication reviewer; earlier source/API-controller review authorship disclosed. No targets or repository edits.','archive_sha256':I['archive_sha256'],'archive_bytes':I['archive_bytes'],'members':len(seen),'uncompressed_member_bytes':total,'original_byte_comparisons':original_count,'excluded_identities_checked':len(I['excluded']),'source_commit':commit['commit'],'source_files':73,'all_condition_rows':52,'adverse_rows':2,'workers':1248,'samples':132540,'native_timed_processes_recomputed':588,'python_timed_processes_recomputed':504,'readme_table_rows':6,'warning_and_license_exact':True,'scope_review':'Normal candidate versus current4815/a240 and normal Expat; all API391 failures/2timeouts and two strict failures per linkage preserved. C-only sanitizers/uninstrumented Rust and strict-only consumer fix explicit. Prior control diagnostics/profiles and4064 sanitizer/PBS scope kept separate. 10% target remains unmet.','findings':[],'publication_path_followup':'Root moving new packet to fused-attribute-normal to preserve historical fused-attribute-scan; final outer metadata/relative-link review follows without rereading unchanged archive.','pins':{str(P/f):H((P/f).read_bytes()) for f in ['index.json','report.json','README.md','conditions.csv','adverse-conditions.csv','README-table.json','committed-source.json']}}
Path('/tmp/oriole-fused-normal-final-review.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
