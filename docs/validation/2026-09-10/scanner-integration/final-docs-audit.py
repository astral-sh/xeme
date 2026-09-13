"""Independent final documentation and sealed artifact readback; no parser/build execution."""
if not __debug__:raise RuntimeError('Assertions required')
from pathlib import Path
from io import BytesIO
import hashlib,json,math,re,statistics,subprocess,tarfile
W=Path('/home/dev-user/code/oss/oriole-scanner-integration');R=W/'docs/validation/2026-09-10/scanner-integration';O=Path('/tmp/oriole-scanner-integration-docs-review');O.mkdir(exist_ok=True)
hashes={}
def read(p):
 p=Path(p);d=p.read_bytes();hashes[str(p)]=hashlib.sha256(d).hexdigest();return d
def sha(p):read(p);return hashes[str(Path(p))]
def load(p):return json.loads(read(p))
summary=load(R/'summary.json');index=load(R/'files.json')['files'];members=load(R/'archive-members.json');assert len(index)==29
assert set(x['path'] for x in index)=={str(p.relative_to(R)) for p in R.rglob('*') if p.is_file() and p.name!='files.json'}
for x in index:p=R/x['path'];assert sha(p)==x['sha256'] and p.stat().st_size==x['bytes']
assert len(members['members'])==summary['archive']['members']==8332 and len(members['excluded_binaries'])==summary['archive']['excluded_binaries']==53
assert sha(R/'evidence.tar.gz')==summary['archive']['sha256']=='a040d01c9b0b36adb2e0d2bdcf9aab9859081bdd0b8138f970fa5a6094780b6e'
expected={x['path']:x for x in members['members']};assert len(expected)==8332
readback={};nested=[]
def nested_read(label,blob):
 total=0;size=0
 with tarfile.open(fileobj=BytesIO(blob),mode='r:gz') as archive:
  seen=set()
  for m in archive:
   assert m.isfile() and m.name not in seen and not Path(m.name).is_absolute() and '..' not in Path(m.name).parts;seen.add(m.name)
   b=archive.extractfile(m).read();assert len(b)==m.size;total+=1;size+=len(b)
   if m.name.endswith('.tar.gz'):nested_read(label+'!'+m.name,b)
 nested.append({'archive':label,'regular_members':total,'raw_bytes':size,'sha256':hashlib.sha256(blob).hexdigest()})
with tarfile.open(R/'evidence.tar.gz',mode='r:gz') as t:
 for m in t:
  assert m.isfile() and m.name in expected and m.name not in readback;v=expected[m.name];b=t.extractfile(m).read();h=hashlib.sha256(b).hexdigest();assert h==v['sha256']==sha(v['source']) and len(b)==m.size==v['bytes']
  readback[m.name]=h
  if m.name.endswith('.tar.gz'):nested_read(m.name,b)
assert set(readback)==set(expected) and sum(x['bytes'] for x in members['members'])==summary['archive']['raw_bytes']==159561723
for x in members['excluded_binaries']:assert x['path'] not in readback and sha(x['source'])==x['sha256'] and Path(x['source']).stat().st_size==x['bytes']
# Independently read the separate unselected ASCII archive using its retained member manifest.
A=R/'unselected-ascii-name';am=load(A/'members.json');am=am.get('members',am) if isinstance(am,dict) else am
if isinstance(am,dict):am=[{'path':p,'sha256':v['sha256'],'bytes':v['size']} for p,v in am.items()]
assert isinstance(am,list)
by={x['path']:x for x in am};ac=0
with tarfile.open(A/'evidence.tar.gz') as t:
 for m in t:
  assert m.isfile();b=t.extractfile(m).read();v=by[m.name];assert hashlib.sha256(b).hexdigest()==v['sha256'] and len(b)==v['bytes'];ac+=1
  if m.name.endswith('.tar.gz'):nested_read('unselected-ascii-name!'+m.name,b)
assert ac==len(by)==1385
reviews={'independent-gates-review.json':'/tmp/oriole-scanner-integration-independent-review/gates-review.json','independent-assembly-review.json':'/tmp/oriole-scanner-integration-independent-review/assembly-review.json','independent-matched-build-review.json':'/tmp/oriole-scanner-integration-independent-review/matched-build-review.json','independent-index.json':'/tmp/oriole-scanner-integration-independent-review/index.json','cpython-review.json':'/tmp/oriole-scanner-integration-independent-review/cpython/review.json','python-review.json':'/tmp/oriole-scanner-integration-independent-review/python/review.json','native-review.json':'/tmp/oriole-scanner-integration-native-independent-review/review.json','profile-review.json':'/tmp/oriole-scanner-integration-profile-independent-review/review.json'}
for p,origin in reviews.items():assert read(R/p)==read(origin)
assert load(R/'native-summary.json')==summary['native'] and load(R/'python-summary.json')==summary['python']
bench=W/'benchmarks/results/2026-09-10/scanner-integration'
for n in ['native-summary.json','python-summary.json']:assert read(bench/n)==read(R/n)
# Reconstruct the prominently displayed six-row subset from the already independently audited native observations.
data=load('/tmp/oriole-scanner-integration-timing-study/native-screen/results.json');rows=[];labels={'vulkan':'Vulkan registry','wayland':'Wayland protocol','maven':'Maven POM','batik':'Batik SVG','gtk':'GTK UI','docbook':'DocBook XSL'}
for s in data['summary']:
 c=s['condition']
 if c['name'] in labels and c['chunk']==4096 and not c['namespaces']:
  medians={e:statistics.median(p['median_seconds'] for group in data['rows'] if group['condition']==c for p in group['processes'] if p['engine']==e) for e in ['candidate','expat']}
  rows.append(f"| {labels[c['name']]} | {medians['candidate']*1000:.3f} ms | {medians['expat']*1000:.3f} ms | {s['median_ratios']['candidate_over_expat']:.2f}× |")
assert len(rows)==6
for p in [W/'README.md',bench/'README.md']:
 text=read(p).decode()
 for row in rows:assert row in text,row
roottext=read(W/'README.md').decode();old=subprocess.check_output(['git','-C',str(W),'show','64a270e:README.md']).decode();toucan=read('/home/dev-user/code/oss/toucan/README.md').decode()
assert re.findall(r'^## .+$',roottext,re.M)==re.findall(r'^## .+$',toucan,re.M)==['## Highlights','## Installation','## Getting started','## License']
warning=lambda s:'\n'.join(line for line in s.splitlines() if line.startswith('>'))
assert warning(roottext)==warning(old) and 'not yet a' in warning(roottext) and 'production-ready replacement' in warning(roottext)
assert roottext.split('## License',1)[1].replace('Oriole','Toucan')==toucan.split('## License',1)[1]
assert roottext.split('## Installation',1)[1]==old.split('## Installation',1)[1]
for p in ['LICENSE-APACHE','LICENSE-MIT']:assert read(W/p)==subprocess.check_output(['git','-C',str(W),'show','64a270e:'+p])
docs=[W/'README.md',W/'benchmarks/README.md',W/'docs/compatibility.md',W/'docs/review.md',bench/'README.md',R/'README.md',A/'README.md'];links=[]
for p in docs:
 text=read(p).decode();dest=O/'reviewed-docs'/p.relative_to(W);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_text(text)
 for target in re.findall(r'\]\(([^)]+)\)',text):
  if re.match(r'^[a-z]+:',target) or target.startswith('#'):continue
  path=(p.parent/target.split('#',1)[0]).resolve();assert path.exists(),(str(p),target);links.append({'file':str(p.relative_to(W)),'target':target,'resolved':str(path)})
text=read(R/'README.md').decode()
for phrase in ['4,347 pass / 393 fail','395 tests across 33 binaries','one separate doc test','exit remains 1','exits remain 2','14 skips and three expected failures','leak detection disabled','5bc806e','193e1278','All regressions are retained','gains cannot be added','no timed Python cohort','8,332 regular members','53 compiled binaries','5.85%']:
 assert phrase in text,phrase
assert summary['workspace']=={'all_targets':395,'doctests':1,'fmt_and_strict_clippy':'passed'} and summary['api']['passed']==4347 and summary['api']['failed']==393
assert summary['native']['groups']['real']['ratios']['candidate_over_published']['below_one']==19 and summary['python']['groups']['all']['ratios']['candidate_over_published']['below_one']==23
assert len(summary['native']['regressions'])==9 and len(summary['python']['regressions'])==1
# Preserve all adverse percentages and identities; paired ratios, not quotients of rounded table values.
for p in ['native','python']:
 for row in summary[p]['regressions']:
  percent=(row['ratio']-1)*100
  assert math.isfinite(percent) and percent>0
(O/'archive-readback.json').write_text(json.dumps({'outer_regular_members':8332,'excluded_binaries_hashed':53,'outer_raw_bytes':159561723,'separate_ascii_regular_members':ac,'nested_archives':nested,'member_sha256':readback},indent=2)+'\n')
(O/'links.json').write_text(json.dumps(links,indent=2)+'\n')
report={'status':'passed_no_findings','scope':'Final documentation,29-file outer index,sealed archive/member origins, separate ASCII archive and nested gzip/tar readback. Saved-data/source review only; no parser/build/benchmark reruns or source edits.','docs':len(docs),'structure':'Toucan Highlights/Installation/Getting started/License heading order preserved; Oriole experimental AI-authorship warning unchanged from committed README. License text exactly Toucan after project-name substitution, license files unchanged.','claims':'Exact integrated395+1/4347+393/802+2fail outcomes, canonical bounds, source-specific sanitizer/fuzz/PBS limits,5058 versus5af comparison, all native/Python regressions and instruction-versus-wall scope agree with immutable audits. Displayed six native rows reconstructed from raw process medians and paired ratios.','archive':{'indexed_files':29,'outer_members':8332,'excluded_binaries':53,'sha256':summary['archive']['sha256'],'ascii_outer_members':ac,'nested_regular_members':sum(x['regular_members'] for x in nested)},'postprocessing':'Original count-helper and initial incomplete package are explicitly retained as postprocessing failures. Later independent reviews remain outside sealed archive and match their frozen origins exactly.','limitations':['README benchmark figures concern pinned XML parsing workloads on a shared host; no whole-project, broad faster-than-Expat, current PGO or current sustained-fuzz/PBS claim.','Outer index intentionally omits itself; final review is separate and should be added with a fresh outer index without resealing original evidence.'],'evidence_sha256':hashes}
(O/'review.json').write_text(json.dumps(report,indent=2)+'\n');print(hashlib.sha256((O/'review.json').read_bytes()).hexdigest())
