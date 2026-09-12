"""Independent saved archive, source, arithmetic and overlay documentation review."""
from pathlib import Path, PurePosixPath
import ast,csv,hashlib,json,math,os,re,statistics,subprocess,tarfile
assert os.sched_getaffinity(0)=={6}
D=Path('/tmp/oriole-native-ascii-text-plan-publication');P=D/'draft/docs/validation/2026-09-12/native-ascii-text-plan';W=Path('/home/dev-user/code/oss/oriole-text-plan-combined');B=Path('/tmp/oriole-text-plan-combined-study')
H=lambda b:hashlib.sha256(b).hexdigest()
J=lambda p:json.loads(Path(p).read_text())
S=J(D/'STABLE.json');I=J(P/'index.json');R=J(P/'report.json');raw=J('/tmp/oriole-text-plan-combined-independent-raw-review.json')
assert H((D/'STABLE.json').read_bytes())=='0e47f7882317c2ef056d729ce7e37d1eaa65aee43835d9432de0aa4e210fe310'
for name,h in S['files'].items():assert H((D/name).read_bytes())==h,name
for name,h in S['preparation'].items():assert H(Path(name).read_bytes())==h,name
assert H((P/I['archive']).read_bytes())==I['archive_sha256']==S['assembly']['archive_sha256']=='c32a50267365fe26ed182adcd0aa2bb8b03c08aa7f3c82cb9acdd8afef02c21c'
assert (P/I['archive']).stat().st_size==I['archive_bytes']==7701073
idx={r['member']:r for r in I['members']};assert len(idx)==len(I['members'])==3735
seen=set();saved={};total=0;original_count=0
with tarfile.open(P/I['archive'],'r:gz') as tar:
 for m in tar:
  assert m.isfile() and m.name in idx and m.name not in seen
  assert not PurePosixPath(m.name).is_absolute() and '..' not in PurePosixPath(m.name).parts
  b=tar.extractfile(m).read();r=idx[m.name];assert len(b)==m.size==r['size'] and H(b)==r['sha256']
  assert not b.startswith((b'\x7fELF',b'!<arch>\n')) and not m.name.endswith(('.profraw','.profdata','.pyc','.callgrind'))
  if m.name=='source/member-map.json':assert r['original_path']=='generated from four verified source manifests'
  else:assert Path(r['original_path']).read_bytes()==b,r['original_path'];original_count+=1
  if m.name.startswith(('source/','inputs/')):saved[m.name]=b
  total+=len(b);seen.add(m.name)
assert seen==set(idx)
excluded={r['path']:r for r in I['excluded']};assert len(excluded)==len(I['excluded'])==35
for r in excluded.values():assert Path(r['path']).stat().st_size==r['size'] and H(Path(r['path']).read_bytes())==r['sha256']
origins={r['original_path']:r['sha256'] for r in I['members']}
# All original pins used by the completed arithmetic/correctness readers have portable bytes or explicit excluded identities.
source_map=json.loads(saved['source/member-map.json']);assert set(source_map['member_maps'])=={'candidate','control','isolated','isolated-control'}
for label,study,count in [('candidate',B,74),('control',Path('/tmp/oriole-fused-normal-current-study'),73),('isolated',Path('/tmp/oriole-text-plan-study'),74),('isolated-control',Path('/tmp/oriole-expanded-start-capacity-study'),73)]:
 manifest=J(study/'source.json');assert len(manifest['sha256'])==count
 assert source_map['sha256'][label]==manifest['sha256'] and set(source_map['member_maps'][label])==set(manifest['sha256'])
 for name,h in manifest['sha256'].items():assert idx[source_map['member_maps'][label][name]]['sha256']==h
commit=J(P/'committed-source.json');assert commit['commit']==raw['commit']==R['runtime_commit']==S['source_commit']=='ea52004a6590a224bfc0ec96c8e80a3159920db5'
assert commit['sha256']==raw['source_sha256']==source_map['sha256']['candidate'] and commit['source_count']==74
assert commit['source_manifest_sha256']==H((B/'source.json').read_bytes())
for name,h in raw['pins'].items():assert H(Path(name).read_bytes())==h,name
N=J(B/'native/review.json');Y=J(B/'python/normal-review.json');A=J('/tmp/oriole-text-plan-combined-correctness/api-c-readback.json');T=J('/tmp/oriole-text-plan-combined-correctness/strict-semantic-readback.json')
assert R['combined']['native_counts']==N['counts']==raw['native_counts'] and N['groups']==raw['native_groups']
assert R['combined']['native_groups']=={k:{key:value for key,value in v.items() if key!='adverse'} for k,v in N['groups'].items()}
assert R['combined']['adverse_native']==[{'condition':r['condition'],**r['median_ratios']} for r in N['all_conditions'] if r['median_ratios']['candidate_over_control']>1]
assert R['combined']['python_counts']==Y['counts']==raw['python_counts'] and R['combined']['python_groups']==Y['aggregates']==raw['python_aggregates']
assert R['combined_native_all_conditions']==N['all_conditions'] and R['combined_python_all_conditions']==Y['summary']
assert R['compatibility']['api']==A['api'] and R['compatibility']['c_consumers']==len(A['c_consumers'])==6
assert all(r['exit']==0 for r in A['c_consumers'])
for mode,entry in R['compatibility']['strict'].items():
 for key,value in entry.items():assert T['strict'][mode][key]==value
assert R['compatibility']['semantic_tests']==T['semantics']
# Reuse isolated native raw assertion block only; no output writes/targets.
reader=Path('/tmp/oriole-text-plan-study/native/read_results.py');text=reader.read_text();assert H(reader.read_bytes())=='b4749d6a9b571c1268c02709889563ab5abcfd2bcb6d2d0130824bb8981206d3'
tree=ast.parse(text);cut=next(i for i,n in enumerate(tree.body) if text.splitlines()[n.lineno-1].startswith('with (OUT /'))
scope={'__file__':str(reader),'__name__':'independent_saved_review'};exec(compile(ast.Module(body=tree.body[:cut],type_ignores=[]),str(reader),'exec'),scope)
O=J(reader.parent/'review.json');assert O['counts']==scope['counts'] and O['groups']==scope['groups'] and O['all_conditions']==scope['summary']
assert R['isolated']['native_counts']==O['counts'] and R['isolated']['native_groups']==O['groups'] and R['isolated_native_all_conditions']==O['all_conditions']
for pinmap in [N['inputs'],O['inputs'],A['inputs'],T['inputs'],Y['evidence_sha256']]:
 for path,h in pinmap.items():assert origins.get(path,excluded.get(path,{}).get('sha256'))==h,path
for path,h in scope['pins'].items():assert H(Path(path).read_bytes())==h,path
rows=list(csv.DictReader((P/'conditions.csv').open()));assert len(rows)==80
expected=[]
for campaign,consumer,values in [('combined','native',N['all_conditions']),('combined','python',Y['summary']),('isolated','native',O['all_conditions'])]:
 for value in values:
  c=value['condition'];expected.append({'campaign':campaign,'consumer':consumer,'project':c['name'],'chunk_bytes':str(c['chunk']),'namespace_or_mode':c.get('mode',str(c.get('namespaces','')).lower()),**{k:str(v) for k,v in value['median_ratios'].items()},'adverse':str(value['median_ratios']['candidate_over_control']>1)})
assert rows==expected
adverse=list(csv.DictReader((P/'adverse-conditions.csv').open()));assert adverse==[r for r in rows if r['adverse']=='True'] and len(adverse)==9 and sum(r['campaign']=='combined' for r in adverse)==6
# Exact top-level table from current native raw process medians, preserving execution-pair order.
results=J(B/'native/native-screen/results.json');table=J(P/'README-table.json');main=(D/'draft/README.md').read_text()
for row in table['rows']:
 cohorts=[c for c in results['rows'] if c['condition']['name']==row['project'] and c['condition']['chunk']==4096 and c['condition']['namespaces'] is False];assert len(cohorts)==7
 vals={engine:[next(p['median_seconds'] for p in c['processes'] if p['engine']==engine) for c in cohorts] for engine in ['candidate','expat']}
 assert row['process_medians_seconds']==vals
 assert statistics.median(vals['candidate'])*1000==row['candidate_ms'] and statistics.median(vals['expat'])*1000==row['expat_ms']
 assert statistics.median(a/b for a,b in zip(vals['candidate'],vals['expat'],strict=True))==row['paired_ratio']
 line=f"| {row['label']} | {row['candidate_ms']:.3f} ms | {row['expat_ms']:.3f} ms | {row['paired_ratio']:.3f}× |"
 assert line in main and line in (P/'README.md').read_text()
base=commit['parent'];old=subprocess.check_output(['git','-C',str(W),'show',base+':README.md'],text=True)
assert main[main.index('> [!WARNING]'):main.index('\n## Highlights')]==old[old.index('> [!WARNING]'):old.index('\n## Highlights')]
assert main[main.index('## License'):]==old[old.index('## License'):]
for name in ['LICENSE-MIT','LICENSE-APACHE']:assert (W/name).read_bytes()==subprocess.check_output(['git','-C',str(W),'show',base+':'+name])
# Resolve relative links against the planned overlay and existing repository.
links=0
for doc in (D/'draft').rglob('*.md'):
 for link in re.findall(r'\[[^\]]*\]\(([^)]+)\)',doc.read_text()):
  target=link.split('#')[0].strip('<>');
  if not target or re.match(r'^[A-Za-z][A-Za-z0-9+.-]*:',target):continue
  resolved=(doc.parent/target).resolve();assert resolved.is_relative_to(D/'draft'),(doc,target)
  relative=resolved.relative_to(D/'draft');assert resolved.exists() or (W/relative).exists(),(doc,target)
  links+=1
assert not subprocess.check_output(['git','-C',str(W),'ls-tree','-r','--name-only',base,'--','docs/validation/2026-09-12/native-ascii-text-plan'],text=True)
assert all(p.parts[0]=='native-ascii-text-plan' for p in [(q.relative_to(D/'draft/docs/validation/2026-09-12')) for q in (D/'draft/docs/validation/2026-09-12').iterdir()])
assert not subprocess.check_output(['git','-C',str(W),'diff',base,'--name-only','--','docs/validation'],text=True)
for name,h in S['files'].items():assert H((D/name).read_bytes())==h,name
out={'status':'passed_independent_saved_packet_source_arithmetic_and_docs_review','role':'Independent Text runtime and final packet reviewer; no Text source/test or packet authorship. Existing raw readers replayed only through assertions and arithmetic. No targets, repository mutations or publication actions.','archive_sha256':I['archive_sha256'],'archive_bytes':I['archive_bytes'],'archive_members':len(seen),'original_byte_comparisons':original_count,'uncompressed_bytes':total,'excluded_identities':len(excluded),'source_commit':commit['commit'],'source_counts':{k:len(v) for k,v in source_map['sha256'].items()},'combined_workers':1248,'combined_samples':132540,'isolated_workers':672,'isolated_samples':106176,'conditions':80,'combined_adverse':6,'isolated_adverse':3,'readme_table_rows':6,'warning_and_license_exact':True,'relative_links':links,'historical_packet_not_overwritten':True,'runtime_source_review':'/tmp/oriole-text-plan-combined-source-final-review.json','combined_raw_review':'/tmp/oriole-text-plan-combined-independent-raw-review.json','findings':[],'reader_preparation_history':'Two initial reader attempts preserved: compact report omits repeated per-group adverse lists and stores a C-consumer count; corrected comparisons check the separate complete adverse rows and all six C exit records. No packet/input changes.', 'pins':{str(D/'STABLE.json'):H((D/'STABLE.json').read_bytes()),**{str(D/k):v for k,v in S['files'].items()}}}
dest=Path('/tmp/oriole-text-plan-final-packet-review.json');dest.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:v for k,v in out.items() if k!='pins'}|{'receipt':str(dest),'sha256':H(dest.read_bytes())},indent=2))
