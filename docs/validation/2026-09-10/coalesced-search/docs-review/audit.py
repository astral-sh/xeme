"""Read-only docs, package bytes and displayed arithmetic audit; no parser calls."""
from pathlib import Path
from urllib.parse import urlsplit,unquote
import hashlib,json,re,subprocess,tarfile,io,statistics,math
R=Path('/home/dev-user/code/oss/oriole-coalesced-search');O=Path(__file__).parent
B=R/'docs/validation/2026-09-10/coalesced-search';E=R/'benchmarks/results/2026-09-10/coalesced-search'
H={}
def read(p):
 p=Path(p);d=p.read_bytes();H[str(p)]=hashlib.sha256(d).hexdigest();return d
def sha(p):read(p);return H[str(Path(p))]
def load(p):return json.loads(read(p))
commit='193e1278ae059bad3a7890b4e34b9a91be5296e8'
assert subprocess.check_output(['git','-C',str(R),'rev-parse','HEAD']).decode().strip()==commit
paths=['README.md','benchmarks/README.md','docs/compatibility.md','docs/review.md',str(E.relative_to(R)/'README.md'),str(B.relative_to(R)/'README.md')]
texts={p:read(R/p).decode() for p in paths};readme=texts['README.md'];validation=texts[paths[-1]];benchmark=texts[paths[-2]]
prior=subprocess.check_output(['git','-C',str(R),'show',commit+':README.md']).decode()
assert re.findall(r'^#{1,6} .+$',readme,re.M)==re.findall(r'^#{1,6} .+$',prior,re.M)
assert readme.split('## Highlights')[0]==prior.split('## Highlights')[0]
assert readme[readme.index('## Installation'):]==prior[prior.index('## Installation'):]
assert readme[readme.index('## License'):]==prior[prior.index('## License'):]
assert set(subprocess.check_output(['git','-C',str(R),'diff','--name-only']).decode().splitlines())==set(paths[:4])
links=[]
for name,text in texts.items():
 for target in re.findall(r'\[[^\]]*\]\(([^\s)]+)\)',text):
  parts=urlsplit(target)
  if parts.scheme or parts.netloc:continue
  path=(R/name).parent/unquote(parts.path) if parts.path else R/name
  assert path.exists(),(name,target)
  if parts.fragment and path.is_file() and path.suffix=='.md':
   slug=lambda x:re.sub(r'[^\w\- ]','',x.lower()).replace(' ','-')
   assert parts.fragment in [slug(x) for x in re.findall(r'^#{1,6} (.+)$',path.read_text(),re.M)]
  links.append({'source':name,'target':target})
index=load(B/'files.json')['files'];assert len(index)==26
assert {x['path'] for x in index}=={str(p.relative_to(B)) for p in B.rglob('*') if p.is_file() and p.name!='files.json'}
for x in index:assert len(read(B/x['path']))==x['bytes'] and sha(B/x['path'])==x['sha256']
source=load(B/'source.json');assert len(source['source_sha256'])==69
for name,h in source['source_sha256'].items():
 assert sha(R/name)==h
 assert hashlib.sha256(subprocess.check_output(['git','-C',str(R),'show',commit+':'+name])).hexdigest()==h
assembly=load(B/'assembly.json');assert assembly['commit']==commit and assembly['source_manifest_sha256']==sha(B/'source.json') and assembly['source_matches_measured_candidate']
summary=load(B/'summary.json');assert summary['source_files']==69 and summary['workspace_tests']==394
expected_reviews={'source-review.json':'a99dfbbf19478f55eb5e0df3e38236c518417d7978b6f08ae180f4566f8f25dd','native-review.json':'a6b605890b88af5c82cde80efc80074d72e45b7ab49744783252669d4bb4d193','python-review.json':'4fe4f578267649a0d5772ce35bdc2c5dfdffd47c0ad836d4a0487d9c0aac7c5a','compatibility-review.json':'e47c954bf177d067514c3aecfc118209fa88326c594b73bf688bdb20119b4e1d'}
for p,h in expected_reviews.items():assert sha(B/p)==h
# Full stored archive and live-origin readback; never extract by path.
membermap=load(B/'archive-members.json');entries=membermap['members'];excluded=membermap['excluded_binaries'];nested=[]
assert len(entries)==2982 and len(excluded)==22
with tarfile.open(B/'evidence.tar.gz') as t:
 allmembers=t.getmembers();assert len(allmembers)==len(entries)
 for member,entry in zip(allmembers,entries,strict=True):
  assert member.isfile() and member.name==entry['path'] and not Path(member.name).is_absolute() and '..' not in Path(member.name).parts
  d=t.extractfile(member).read();assert len(d)==entry['bytes'] and hashlib.sha256(d).hexdigest()==entry['sha256']==sha(entry['source'])
  assert not d.startswith((b'\x7fELF',b'!<arch>\n'))
  if member.name.endswith(('.tar.gz','.tgz')):
   with tarfile.open(fileobj=io.BytesIO(d)) as nt:
    count=0
    for nm in nt:
     if nm.isfile():assert not nt.extractfile(nm).read().startswith((b'\x7fELF',b'!<arch>\n'));count+=1
    nested.append({'path':member.name,'regular_files':count})
for x in excluded:assert sha(x['source'])==x['sha256'] and read(x['source']).startswith((b'\x7fELF',b'!<arch>\n'))
assert sum(x['bytes'] for x in entries)==60427221
assert sha(B/'evidence.tar.gz')=='35fc323c0bf7c6a72b1f3b808c02faa56950f519f90319a34c42bec72db6b932'
assert summary['archive']['sha256']==sha(B/'evidence.tar.gz') and summary['archive']['bytes']==5899692
# Selector model: verify complete preserved family counts and explicit model scope.
design=load(B/'selector-design/manifest.json');assert len(design['files'])==design['count']==12
for name,x in design['files'].items():assert sha(B/'selector-design'/name)==x['sha256'] and len(read(B/'selector-design'/name))==x['bytes']
oracle=load(B/'selector-design/oracle.json');assert oracle['status']=='passed' and oracle['cases']==sum(oracle['families'].values())==sum(oracle['candidate_paths'].values())==5449334
assert 'source transcription' in oracle['scope'] and 'No Rust parser' in oracle['scope']
# Reconstruct all displayed table values from the already independently audited raw reports.
n=load('/tmp/oriole-coalesced-search-timing-study/native-screen/results.json');p=load('/tmp/oriole-coalesced-search-python-study/screen/results.json')
ng=load(B/'native-summary.json');pg=load(B/'python-summary.json')
assert ng==load(E/'native-summary.json')==summary['native'] and pg==load(E/'python-summary.json')==summary['python']
labels={'vulkan':'Vulkan registry','wayland':'Wayland protocol','maven':'Maven POM','batik':'Batik SVG','gtk':'GTK UI','docbook':'DocBook XSL'};display=[]
for s in n['summary']:
 c=s['condition']
 if c['name'] not in labels or c['chunk']!=4096 or c['namespaces']:continue
 med={e:statistics.median(q['median_seconds'] for row in n['rows'] if row['condition']==c for q in row['processes'] if q['engine']==e) for e in ['candidate','expat']}
 ratio=statistics.median(s['paired_ratios']['candidate_over_expat']);assert ratio==s['median_ratios']['candidate_over_expat']
 line=f"| {labels[c['name']]} | {med['candidate']*1000:.3f} ms | {med['expat']*1000:.3f} ms | {ratio:.2f}× |"
 assert line in readme and line in benchmark,line
 display.append({'project':c['name'],'candidate_ms':med['candidate']*1000,'expat_ms':med['expat']*1000,'ratio':ratio})
assert len(display)==6
for label,g in [('Native real-project XML',ng['groups']['real']),('Actual CPython consumers',pg['groups']['all']),('ElementTree',pg['groups']['elementtree']),('pyexpat callbacks',pg['groups']['pyexpat-events']),('Native generated adverse fixtures',ng['groups']['generated'])]:
 cp=g['ratios']['candidate_over_published'];ce=g['ratios']['candidate_over_expat']
 line=f"| {label} | {g['conditions']} | {cp['geomean']:.3f}× | {ce['geomean']:.3f}× | {cp['below_one']} | {ce['below_one']} |"
 assert line in validation,line
assert f"{100*(1-ng['groups']['real']['ratios']['candidate_over_published']['geomean']):.1f}%"=='3.3%'
assert f"{100*(1-pg['groups']['all']['ratios']['candidate_over_published']['geomean']):.1f}%"=='2.4%'
assert len(ng['regressions'])==6 and len(pg['regressions'])==2
wins=[s for s in p['summary'] if s['median_ratios']['candidate_over_expat']<1];assert len(wins)==2 and all(s['condition']['name']=='wayland' and s['condition']['mode']=='pyexpat-events' for s in wins)
assert '3 seconds' in validation and '768 MiB RSS and 240 seconds' in validation and '15 seconds' in validation
assert 'are not new campaigns or distribution builds of this' in validation
assert 'source adaptation or text-fragmentation waiver' in validation
assert '14.50 million' in readme and '`4b11ace` runtime' in readme
for name in paths:
 target=O/'reviewed-docs'/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((R/name).read_bytes())
(O/'links.json').write_text(json.dumps(links,indent=2)+'\n')
report={'status':'passed_no_findings','scope':'Independent read-only human-doc/package audit with prior full raw-source/profile/native/Python/compat reviews rehashed. No parser execution, compilation, tests, new measurements, GitHub query or repository edits.',
'commit':commit,'source_files_exact':69,'readme':{'warning_intro_exact':True,'headings_exact':True,'installation_through_license_exact':True},'local_links_checked':len(links),'outer_index_entries':26,
'archive':{'sha256':sha(B/'evidence.tar.gz'),'members':2982,'bytes':5899692,'raw_bytes':60427221,'all_member_and_live_origin_hashes_verified':True,'excluded_binaries_verified':22,'nested_archives':nested},
'selector_model':{'cases':5449334,'preserved_model_scope':'Python source-transcription model, separately retained; not5.45million Rust parser executions'},'native_display_table':display,
'interpretation':['Paired medians and rounded tables match stored data. All24 real conditions and4 generated native controls retained, including6 native and2 Python regressions. No broad Expat-speed claim.','Matched Oriole builds and separate C reference context retained. Source193e1278 exactly matches measured5058; no runtime bytes changed in doc-only rebase/assembly.','Original relaxed API manifest stays retained; separate canonical3s/1GiB/768MiB/240s run has unchanged4347/393 with exit1. Strict CPython has802 method outcomes,2 failures,14 reported skips and3expected per linkage, with no waiver.','Existing earlier4b PGO/fuzz/PBS scope remains explicit and is not attributed to5058. New5bc fuzz package is a separate pending audit/publication.','Instruction counts and elapsed benchmarks have different recorded collection scopes. CPython automaticGC remains enabled and worker canonicalization coalesces adjacent text, as underlying audited methods state.'],
'limits':['This report reuses the already completed raw-data audits and verifies packaged copies, displayed arithmetic and original artifact bytes; it does not rerun workloads.','files.json covers26 outer files at review time. Parent may add this review/generator and refresh outer index without changing the main archive.'],
'evidence_sha256':H}
(O/'review.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'status':report['status'],'sha256':sha(O/'review.json'),'links':len(links),'members':len(entries)}))
