"""Package reviewed immutable evidence without compiler outputs or duplicate trees."""
from pathlib import Path
import gzip,hashlib,json,shutil,subprocess,tarfile
OUT=Path('/tmp/oriole-final-validation-package');OUT.mkdir(exist_ok=True)
artifacts={}
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def add(source,destination):
 source=Path(source);destination=Path(destination)
 if not source.is_file():raise FileNotFoundError(source)
 data=source.read_bytes()
 if data.startswith(b'\x7fELF') or source.suffix in ('.so','.a','.rlib','.rmeta'):raise ValueError('compiler output '+str(source))
 compress=source.suffix=='.log' or (source.suffix=='.json' and len(data)>65536)
 if compress:destination=Path(str(destination)+'.gz')
 target=OUT/destination;target.parent.mkdir(parents=True,exist_ok=True)
 if compress:
  with target.open('wb')as raw,gzip.GzipFile(filename='',fileobj=raw,mode='wb',mtime=0)as encoded:encoded.write(data)
 else:target.write_bytes(data)
 artifacts[str(destination)]={'source':str(source),'source_sha256':hashlib.sha256(data).hexdigest(),'stored_sha256':digest(target),'source_bytes':len(data),'stored_bytes':target.stat().st_size,'gzip_added':compress}

def metadata_tree(source,destination):
 source=Path(source)
 for p in source.rglob('*'):
  if p.is_file() and p.suffix in ('.json','.log','.md','.py','.patch','.txt'):
   add(p,Path(destination)/p.relative_to(source))

add('/tmp/oriole-values-final-source/source.json','runtime/source.json')
add('/tmp/oriole-package-final-validation.py','package.py')
for p in Path('/tmp').glob('oriole-values*tests.log'):
 add(p,'runtime/'+p.name)
for p in [Path('/tmp/oriole-values-final-clippy.log'),Path('/tmp/oriole-values-final-tests.log')]:
 if p.exists():add(p,'runtime/'+p.name)
metadata_tree('/tmp/oriole-values-consumers','consumers')
metadata_tree('/tmp/oriole-values-native','native')
for name in ('integration','adversarial','allocations'):
 add('/home/dev-user/code/oss/oriole/tests/c/'+name+'.c','native/source/'+name+'.c')

upstreams=['oriole-upstream-final-values','oriole-upstream-final-values-raised','oriole-upstream-final-reference','oriole-upstream-final-reference-large','oriole-upstream-final-reference-raised']
for name in upstreams:
 p=Path('/tmp')/name;label=name.removeprefix('oriole-upstream-final-')
 for f in p.iterdir():
  if f.is_file() and f.suffix in ('.json','.log','.py','.md'):add(f,Path('upstream')/label/f.name)
 if name=='oriole-upstream-final-values':
  for sub in ('allocation-raised512','encoding-classification'):
   for f in (p/sub).rglob('*'):
    if f.is_file():add(f,Path('upstream')/label/sub/f.relative_to(p/sub))
for p in Path('/home/dev-user/code/oss/oriole/tools/upstream-expat').iterdir():
 if p.is_file() and p.suffix in ('.py','.c','.h','.md','.patch'):add(p,'upstream/adapter/'+p.name)
baseline=Path('/home/dev-user/code/oss/oriole/docs/validation/2026-09-10/multibyte')
for p in baseline.glob('complete-api-*'):
 if p.is_file():add(p,'upstream/baseline/'+p.name)

review=Path('/tmp/oriole-external-value-review');manifest=json.loads((review/'combined-final-independent-review.json').read_text())
add(review/'combined-final-independent-review.json','combined-review/review.json')
for name in manifest['artifact_sha256']:
 add(review/name,'combined-review/'+name)
for name in ['independent_matrix.py','independent-combined-final.log','merge-final.log','default-switch-final.log','default-layer-combined-final.log','value-final-independent-review.json']:
 add(review/name,'combined-review/'+name)

campaign=Path('/tmp/oriole-value-campaign-handoff')
for p in campaign.rglob('*'):
 if p.is_file():add(p,Path('campaigns/final')/p.relative_to(campaign))
for p in Path('/tmp/oriole-value-fuzz-handoff').iterdir():
 if p.is_file():add(p,'campaigns/harness/'+p.name)
development=Path('/tmp/oriole-value-fuzz/development-evidence')
for p in development.iterdir():
 if p.is_file() and p.name!='value_family.gz':add(p,'campaigns/development/'+p.name)
with tarfile.open(OUT/'campaigns/development/original-seeds.tar.gz','w:gz')as archive:archive.add(development/'original-seeds',arcname='original-seeds')
artifacts['campaigns/development/original-seeds.tar.gz']={'source':str(development/'original-seeds'),'stored_sha256':digest(OUT/'campaigns/development/original-seeds.tar.gz'),'stored_bytes':(OUT/'campaigns/development/original-seeds.tar.gz').stat().st_size,'purpose':'Original bounded fuzz inputs; not an executable binary'}

metadata_tree('/tmp/oriole-pbs-glibc-success','pbs')
add('/tmp/oriole-pbs-independent-review.json','pbs/independent-tls-review.json')
for name in ['oriole-version-independent-review.json','oriole-version-independent.json','oriole-version-independent.py','oriole-xml-versions-version-review.json','oriole-xml-versions-namespace-review.json','oriole-xml-versions-review.py']:
 add('/tmp/'+name,'followups/version/'+name)
metadata_tree('/tmp/oriole-foreign-dtd/handoff','followups/foreign-dtd/handoff')
for name in ['matrix.py','matrix-final.json','matrix-final.cases.json.gz','matrix-comparison.json','nested_probe.py','nested-probe.json','upstream_focused.py']:
 add('/tmp/oriole-foreign-dtd/'+name,'followups/foreign-dtd/'+name)
metadata_tree('/tmp/oriole-foreign-dtd/upstream-focused','followups/foreign-dtd/upstream-focused')
for name in ['oriole-w3c-run.py','oriole-w3c-cli-independent-review.json','oriole-w3c-cli-review-before-fix.json','oriole-w3c-independent-review.json','oriole-w3c-reporting-tests.py','oriole-w3c-reporting-tests.json']:
 add('/tmp/'+name,'harness-review/w3c/'+name)
metadata_tree('/tmp/oriole-w3c-cli-fixed-output','harness-review/w3c/fixture-output')
for p in Path('/tmp/oriole-w3c-cli-review-fixture').iterdir():add(p,'harness-review/w3c/fixture/'+p.name)

manifest={'format':1,'scope':'Immutable evidence package; source separation described in README.md. Original failures/skips retained, no public test assertions waived. No executable libraries or compiler outputs included.','base_commit':'b68bdca6f61e932c45ce076d6ec0bdb7aadd95b4','root_source_manifest_sha256':'cc0f008866bc6b76c1a7cba72cb7b245d2a4eb26275a8928ead7dbd723afdd3b','artifacts':artifacts}
(OUT/'artifact-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print('artifacts',len(artifacts),'bytes',sum(v['stored_bytes']for v in artifacts.values()))
