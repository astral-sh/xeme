"""Seal saved strict CPython evidence, excluding compiled binary bytes."""
from pathlib import Path
import gzip,hashlib,json,tarfile
P=Path(__file__).parent;G=Path('/tmp/oriole-current-thin-pgo-cpython-gates');R=Path('/tmp/oriole-current-thin-pgo-cpython-independent-review')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def load(p):return json.loads(Path(p).read_bytes())
def write(n,v):(P/n).write_text(json.dumps(v,indent=2)+'\n')
assert sha(R/'review.json')=='7b8b055505931c115a5d315268c8b5571518fc794601281052ee865261e736c2'
members={};excluded=[]
def add(n,p):
 p=Path(p);raw=p.read_bytes();row={'path':n,'origin':str(p),'size':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}
 if p.is_symlink():row['symlink_target']=str(p.readlink())
 if raw.startswith((b'\x7fELF',b'!<arch>\n')):excluded.append(row);return
 assert not p.is_symlink() and n not in members
 members[n]=row
for label,b in [('current',G),('independent-review',R)]:
 for p in sorted(b.rglob('*')):
  if p.is_file():add(label+'/'+p.relative_to(b).as_posix(),p)
for p,h in load(R/'source-inputs.json').items():
 assert sha(p)==h
 name='source-inputs/'+h+'/'+Path(p).name
 if name not in members:add(name,p)
for linkage in ['shared','static']:
 add('prior-baseline/'+linkage+'-tests.log',Path('/tmp/oriole-frame-integrated-gates')/('cpython-'+linkage)/'tests.log')
for n in ['test_pyexpat','test_xml_etree','test_xml_etree_c','test_minidom','test_sax','test_pulldom']:
 add('unchanged-upstream-tests/'+n+'.py',Path('/home/dev-user/.cache/oriole/upstream/cpython-3.12.13/Lib/test')/(n+'.py'))
for name in ['source.json','source.tar.gz']:
 add('parser-source/'+name,Path('/tmp/oriole-native-byte-count-pgo-study')/name)
add('prior-reader/audit.py','/tmp/oriole-native-byte-count-thinlto-cpython-root-review/audit.py')
add('package.py',Path(__file__))
for name in ['oriole-current-thin-pgo-cpython-package-attempt-01.py','oriole-current-thin-pgo-cpython-package-attempt-01.log']:
 add('packaging-attempts/'+name,Path('/tmp')/name)
with tarfile.open(P/'evidence.tar.gz','w:gz') as t:
 for n,row in sorted(members.items()):
  assert sha(row['origin'])==row['sha256'];t.add(row['origin'],arcname=n,recursive=False)
with tarfile.open(P/'evidence.tar.gz','r:gz') as t:
 assert len(t.getmembers())==len(members)
 for m in t.getmembers():
  row=members[m.name];raw=t.extractfile(m).read()
  assert m.isfile() and m.size==row['size']==len(raw) and hashlib.sha256(raw).hexdigest()==row['sha256']==sha(row['origin'])
  if m.name=='parser-source/source.tar.gz':
   import io
   source=load('/tmp/oriole-native-byte-count-pgo-study/source.json')['source_sha256']
   with tarfile.open(fileobj=io.BytesIO(raw),mode='r:gz') as s:
    actual={p.name:hashlib.sha256(s.extractfile(p).read()).hexdigest() for p in s.getmembers() if p.isfile()}
    assert actual==source and len(actual)==70
for row in excluded:assert sha(row['origin'])==row['sha256']
with gzip.open(P/'archive-members.json.gz','wt') as f:json.dump(list(members.values()),f,indent=2);f.write('\n')
write('excluded-binaries.json',excluded)
(P/'review.json').write_bytes((R/'review.json').read_bytes())
write('receipt.json',{'status':'sealed_and_readback_passed','archive_sha256':sha(P/'evidence.tar.gz'),'archive_members':len(members),'nested_source_members':70,'direct_binary_exclusions':len(excluded),'review_sha256':sha(P/'review.json'),'audit_first_attempt_passed':True,'scope':'Source and saved evidence only. Both strict suites retain two failures; no target reruns.'})
(P/'README.md').write_text('# ThinLTO PGO strict CPython validation\n\n[Independent review](review.json) verifies exact outcome parity for both shared and static builds: 802 method outcomes each, two retained failures, 14 reported skips and three expected failures. This is not a green-suite result.\n\n[Archive](evidence.tar.gz) retains both original test logs, source/commands/loader origins, baseline logs, upstream test modules and the complete independent reconstruction. [Members and origins](archive-members.json.gz), [excluded compiled binary hashes](excluded-binaries.json), and [readback receipt](receipt.json) define its exact scope.\n')
write('files.json',[{'path':p.name,'size':p.stat().st_size,'sha256':sha(p)} for p in sorted(P.iterdir()) if p.is_file() and p.name!='files.json'])
print(json.dumps(load(P/'receipt.json')))
