"""Archive completed Python campaigns and reviews; exclude compiled binaries."""
from pathlib import Path
import gzip,hashlib,io,json,tarfile

OUT=Path('/tmp/oriole-pgo-python-studies-handoff')
STUDIES={
 'five-engine':Path('/tmp/oriole-current-pgo-python-root-study'),
 'fat-versus-thin-pgo':Path('/tmp/oriole-fat-pgo-python-root-study'),
}
REVIEWS={
 'five-engine':Path('/tmp/oriole-current-pgo-python-review-handoff'),
 'fat-versus-thin-pgo':Path('/tmp/oriole-fat-pgo-python-review-handoff'),
 'fat-pgo-protocol':Path('/tmp/oriole-fat-pgo-python-protocol-independent-review'),
}
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def load(p):return json.loads(Path(p).read_bytes())
def write(n,v): (OUT/n).write_text(json.dumps(v,indent=2)+'\n')
if not __debug__:raise RuntimeError('assertions required')
assert not OUT.exists(),'Never overwrite a sealed handoff'
planned={};excluded=[]
def add(name,p):
 p=Path(p);assert p.is_file(),p
 assert name not in planned
 b=p.read_bytes();h=hashlib.sha256(b).hexdigest()
 row={'path':name,'origin':str(p),'size':len(b),'sha256':h}
 if p.is_symlink():row['symlink_target']=str(p.readlink())
 if b.startswith(b'\x7fELF') or b.startswith(b'!<arch>\n'):
  excluded.append(row);return
 assert not p.is_symlink(),p
 planned[name]=row
summary={'scope':'Two separate fixed Python timing campaigns. Cross-study timing samples are never paired. Full raw records, controllers, patches, source inputs and independent reviews retained; direct compiled libraries/extensions excluded with hashes.','studies':{},'limitations':['Fresh variant preflights are canonical benchmark controls, not a full strict CPython or API compatibility suite.','Automatic GC enabled; parse and explicit result destruction timed, imports/collection/canonicalization outside.','Shared-host limitations and historical Expat PGO provenance limitations remain in individual reviews.','Build/training/native studies are separate evidence packages. Shared runtime/tool source archives are included here for the compilation source identity.']}
for label,b in STUDIES.items():
 assert load(b/'time-controller.json')['status']=='passed'
 result=load(b/'screen/results.json');assert result['status']=='passed'
 assert load(b/'summary.json')['raw_result_sha256']==sha(b/'screen/results.json')
 for p in sorted(b.rglob('*')):
  if p.is_file():add(f'studies/{label}/{p.relative_to(b).as_posix()}',p)
 review=load(REVIEWS[label]/'review.json')
 assert 'passed' in review['status'] and review['results_sha256']==sha(b/'screen/results.json')
 summary['studies'][label]={'result_sha256':sha(b/'screen/results.json'),'summary_sha256':sha(b/'summary.json'),'review_sha256':sha(REVIEWS[label]/'review.json'),'counts':review['counts'],'overall':review['overall']}
 # Preserve exact external compile inputs; names are unique by content hash.
 for path,h in load(b/'consumers/build.json')['source_sha256_before'].items():
  assert sha(path)==h
  name=f'compile-inputs/{h}/{Path(path).name}'
  if name not in planned:add(name,path)
 for c in result['conditions']:
  p=Path(c['input']);h=sha(p);name=f'project-inputs/{h}/{p.name}'
  if name not in planned:add(name,p)
for label,p in REVIEWS.items():
 for f in sorted(p.rglob('*')):
  if f.is_file():add(f'independent-reviews/{label}/{f.relative_to(p).as_posix()}',f)
for name in ['source.json','source.tar.gz','tool-source.json','tool-source.tar.gz']:
 add('parser-source/'+name,Path('/tmp/oriole-native-byte-count-pgo-study')/name)
add('project-inputs/corpus-manifest.json','/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json')
for label,script in [('five-engine','/tmp/oriole-summarize-current-pgo-python.py'),('fat-versus-thin-pgo','/tmp/oriole-summarize-fat-pgo-python.py')]:
 assert load(STUDIES[label]/'summary.json')['script_sha256']==sha(script)
 add(f'studies/{label}/summary-generator.py',script)
add('packager.py',Path(__file__))
for name in ['oriole-package-pgo-python-studies-attempt-01.py','oriole-package-pgo-python-studies-attempt-01.log']:
 add('packaging-attempts/'+name,Path('/tmp')/name)
OUT.mkdir()
try:
 with tarfile.open(OUT/'evidence.tar.gz','w:gz') as t:
  for name,row in sorted(planned.items()):
   assert sha(row['origin'])==row['sha256']
   t.add(row['origin'],arcname=name,recursive=False)
 nested=[]
 def inspect_nested(name,data):
  with tarfile.open(fileobj=io.BytesIO(data),mode='r:*') as t:
   for m in t.getmembers():
    assert not m.issym() and not m.islnk() and not m.name.startswith('/') and '..' not in Path(m.name).parts
    if not m.isfile():continue
    raw=t.extractfile(m).read()
    assert len(raw)==m.size
    nested.append({'archive':name,'path':m.name,'size':len(raw),'sha256':hashlib.sha256(raw).hexdigest()})
    if m.name.endswith(('.tar.gz','.tgz','.tar')):inspect_nested(name+'!'+m.name,raw)
 with tarfile.open(OUT/'evidence.tar.gz','r:gz') as t:
  assert len(t.getmembers())==len(planned)
  for m in t.getmembers():
   row=planned[m.name];raw=t.extractfile(m).read()
   assert m.isfile() and len(raw)==row['size']==m.size and hashlib.sha256(raw).hexdigest()==row['sha256']==sha(row['origin'])
   if m.name.endswith(('.tar.gz','.tgz','.tar')):inspect_nested(m.name,raw)
 for row in excluded:assert sha(row['origin'])==row['sha256']
 for n,d in [('archive-members.json.gz',list(planned.values())),('nested-members.json.gz',nested)]:
  with gzip.open(OUT/n,'wt') as f:json.dump(d,f,indent=2);f.write('\n')
 write('excluded-binaries.json',excluded)
 summary['archive']={'sha256':sha(OUT/'evidence.tar.gz'),'bytes':(OUT/'evidence.tar.gz').stat().st_size,'members':len(planned),'uncompressed_bytes':sum(r['size'] for r in planned.values()),'nested_members_read':len(nested),'direct_compiled_exclusions':len(excluded)}
 write('summary.json',summary)
 (OUT/'README.md').write_text('# Matched PGO Python studies\n\nBoth campaigns passed independent saved-data reviews. [Summary](summary.json) lists every aggregate comparison and the exact counts. Each archived study retains all worker records, commands, source patches, controllers, summaries and independent details. No samples or adverse conditions were dropped.\n\n[Evidence](evidence.tar.gz), [member hashes and origins](archive-members.json.gz), [nested member hashes](nested-members.json.gz), and [excluded compiled binary hashes](excluded-binaries.json) define the package. Original source/training/native evidence is separate. See each review for timing boundaries, shared-host limits and the absence of fresh full-suite certification at measurement time.\n')
 (OUT/'package.py').write_bytes(Path(__file__).read_bytes())
 write('files.json',[{'path':p.name,'size':p.stat().st_size,'sha256':sha(p)} for p in sorted(OUT.iterdir()) if p.is_file() and p.name!='files.json'])
 print(json.dumps(summary['archive']))
except BaseException as exc:
 write('package-failure.json',{'exception':repr(exc),'scope':'Packaging failure only; no target execution or input edits.'})
 raise
