import hashlib,io,json,tarfile
from pathlib import Path
root=Path('/home/dev-user/code/oss/oriole-frame-evidence')
base=root/'docs/validation/2026-09-10/detached-frames'
out=Path(__file__).parent
H=lambda b:hashlib.sha256(b).hexdigest()
meta=json.loads((base/'archive-members.json').read_bytes())
summary=json.loads((base/'summary.json').read_bytes())
archive=base/'evidence.tar.gz'
assert H(archive.read_bytes())==summary['archive']['sha256']=='9da01c97f4d273e3f571687a03859707419e797bc2752b6727fb652e8d9e4f5e'
expected={m['path']:m for m in meta['members']};assert len(expected)==2357
seen=set();nested=[];origins=[]
def check_nested(data,path):
 with tarfile.open(fileobj=io.BytesIO(data),mode='r:gz') as tar:
  members=tar.getmembers();files=0;total=0
  for m in members:
   assert m.isfile() or m.isdir(),(path,m.name,m.type)
   if not m.isfile():continue
   b=tar.extractfile(m).read();files+=1;total+=len(b)
   assert not (b.startswith(b'\x7fELF') or b.startswith(b'!<arch>\n')),(path,m.name)
   if m.name.endswith('.tar.gz'):check_nested(b,path+'!'+m.name)
  nested.append({'path':path,'files':files,'bytes':total,'sha256':H(data)})
with tarfile.open(archive) as tar:
 for m in tar:
  assert m.isfile() and m.name not in seen;seen.add(m.name)
  b=tar.extractfile(m).read();e=expected[m.name]
  assert len(b)==m.size==e['bytes'] and H(b)==e['sha256']
  assert not (b.startswith(b'\x7fELF') or b.startswith(b'!<arch>\n'))
  source=Path(e['source']);assert H(source.read_bytes())==e['sha256'],source
  origins.append({'archive_path':m.name,'source':e['source'],'sha256':e['sha256']})
  if m.name.endswith('.tar.gz'):check_nested(b,m.name)
assert seen==set(expected)
assert sum(m['bytes'] for m in meta['members'])==summary['archive']['uncompressed_bytes']==88773781
assert archive.stat().st_size==summary['archive']['bytes']==10786733
assert len(meta['excluded_binaries'])==50==summary['archive']['excluded_binaries']
for e in meta['excluded_binaries']:
 b=Path(e['source']).read_bytes();assert len(b)==e['bytes'] and H(b)==e['sha256']
 assert b.startswith(b'\x7fELF') or b.startswith(b'!<arch>\n')
atomic=base/'unselected-accounting'
for name,e in json.loads((atomic/'manifest.json').read_bytes()).items():
 b=(atomic/name).read_bytes();assert len(b)==e['bytes'] and H(b)==e['sha256']
check_nested((atomic/'evidence.tar.gz').read_bytes(),'unselected-accounting/evidence.tar.gz')
a=json.loads((atomic/'members.json').read_bytes());print('atomic map format',list(a)[:5])
# Copy initial mutable outer documentation/index snapshots for review history.
for p in [root/'README.md',root/'benchmarks/README.md',root/'docs/review.md',root/'docs/compatibility.md',root/'benchmarks/results/2026-09-10/detached-frames/README.md',base/'README.md',base/'ci.json',base/'files.json']:
 target=out/'initial'/p.relative_to(root);target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(p.read_bytes())
result={'status':'passed','main_archive_sha256':H(archive.read_bytes()),'main_members':len(seen),'main_uncompressed_bytes':88773781,'main_compressed_bytes':10786733,'excluded_binary_hashes_verified':50,'all_main_live_origins_match':True,'nested_archives':nested,'atomic_manifest_sha256':H((atomic/'manifest.json').read_bytes()),'scope':'Read archived bytes and retained originals only; no extraction, execution, build or repository edit.'}
(out/'readback.json').write_text(json.dumps(result,indent=2)+'\n')
(out/'origins.json').write_text(json.dumps(origins,indent=2)+'\n')
print(json.dumps({'status':'passed','nested':len(nested),'readback_sha256':H((out/'readback.json').read_bytes())}))
