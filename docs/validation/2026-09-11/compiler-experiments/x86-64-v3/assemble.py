#!/usr/bin/env python3
"""Deterministic saved-evidence assembly only; never builds or executes parsers."""
from pathlib import Path,PurePosixPath
import argparse,gzip,hashlib,io,json,os,tarfile
R=Path(__file__).resolve().parent;S=Path('/tmp/oriole-x86-64-v3-pgo-study');G=Path('/tmp/oriole-allocator-provenance-pgo-study');E=Path('/tmp/oriole-pgo-study');N=Path('/tmp/oriole-x86-64-v3-native-independent-review');B=Path('/tmp/oriole-x86-64-v3-build-independent-review');W=Path('/home/dev-user/code/oss/oriole-allocator-x86-64-v3')
load=lambda p:json.loads(Path(p).read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()

def main():
 a=argparse.ArgumentParser();a.add_argument('--released',action='store_true');args=a.parse_args()
 assert args.released and os.sched_getaffinity(0)=={7} and not (R/'evidence.tar.gz').exists()
 known={};records={};omitted={};payloads={}
 for root in (S,G):
  known.update({str(root/n):v['sha256'] for n,v in load(root/'pair-freeze.json')['files'].items()})
 for p,k in [(B/'rust.json','verified_file_pins'),(B/'expat-native.json','verified_pins'),(N/'normal.json','raw_files_sha256'),(N/'pgo.json','raw_files_sha256')]:known.update(load(p)[k])
 def put(key,data,reason,expected=None,origin=None):
  h=sha(data);assert expected is None or h==expected,(key,expected,h)
  assert not data.startswith((b'\x7fELF',b'!<arch>\n')),key
  if key in records:assert records[key]['sha256']==h;return
  assert key not in omitted
  records[key]={'path':key,'origin':origin or key,'sha256':h,'bytes':len(data),'member':'payload/'+h,'reason':reason}
  assert h not in payloads or payloads[h]==data;payloads[h]=data
 def add(p,reason,expected=None):
  p=Path(p);key=str(p);expected=expected or known.get(key)
  if key in records or key in omitted:
   assert expected is None or (records.get(key) or omitted[key])['sha256']==expected;return
  assert p.is_file(),key
  with p.open('rb') as f:prefix=f.read(8)
  capsule=p.name.endswith(('.tar.gz','.tar.xz','.tar.zst'))
  binary=prefix.startswith((b'\x7fELF',b'!<arch>\n'))
  if capsule or binary:
   assert expected is not None,(key,'unbound excluded payload')
   omitted[key]={'path':key,'sha256':expected,'bytes':p.stat().st_size,'kind':'source_capsule' if capsule else 'compiled_binary','reason':'Source capsule expanded into exact source members' if capsule else 'Compiled library/tool payload excluded; recorded hash identity only','verification':'Portable verifier does not read omitted payload bytes'};return
  put(key,p.read_bytes(),reason,expected)
 def tree(root,reason):
  for p in sorted(Path(root).rglob('*')):
   if p.is_file() and '__pycache__' not in p.parts and p.suffix!='.pyc':add(p,reason)
 for root in (S,G):
  add(root/'pair-freeze.json','Original frozen build inventory, including omitted binaries')
  for n,v in load(root/'pair-freeze.json')['files'].items():add(root/n,'Frozen v3 build or generic control provenance',v['sha256'])
  source=load(root/'source.json')['source_sha256']
  with tarfile.open(root/'source.tar.gz') as t:
   files=[m for m in t if m.isfile()];assert len(files)==len(source)==70
   for m in files:
    assert not PurePosixPath(m.name).is_absolute() and '..' not in PurePosixPath(m.name).parts
    put(str(W/m.name),t.extractfile(m).read(),'Exact selected e1 source from frozen capsule',source[m.name],str(root/'source.tar.gz')+'#'+m.name)
 for mode in ('normal','pgo'):tree(S/('native-'+mode),'Original completed native campaign, every raw worker and first outcome')
 tree(N,'Independent native arithmetic and post-hoc review; original first reader outputs')
 tree(B,'Independent compiler/source/build/profile review')
 for q in sorted(Path('/tmp').iterdir()):
  if q.is_file() and (q.name.startswith(('oriole-x86-64-v3-','oriole-v3-cpu5-')) or q.name=='prepare-v3-native-independent-reader.py'):add(q,'Original preparation/review source and retained reader attempts')
 for path,h in sorted(known.items()):add(path,'Exact source/log/profile/worker dependency of independent audits',h)
 # Include the original generic Expat control profiles/training provenance. Its
 # older input-manifest captions remain unchanged and explicitly explained.
 for n in ['expat-source.json','expat-profile.json','training-inputs.json','generate_training.py','train.py','training-launch-provenance.json','expat-profile-counts.txt']:
  add(E/n,'Original generic Expat generated-only provenance')
 for phase in ('control','generate','use'):
  for suffix in ('build.json','training.json','configure.log','compile.log'):add(E/('expat-'+phase+'-'+suffix),'Original generic Expat build and training')
 for n,h in load(E/'expat-profile.json')['files'].items():add(E/n,'Original generic Expat gcda profile',h)
 for row in load(E/'training-inputs.json')['rows']:add(row['path'],'Original generic generated input bytes',row['sha256'])
 for n in ('audit-native.py','native/native_screen.py','native/run.py'):add(Path('/tmp/oriole-bulk-pgo-training-study')/n,'Prior exact four-engine reader/controller source; no prior elapsed campaign copied')
 manifest=Path('/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json');add(manifest,'Held-out real XML source and notice provenance')
 for project in load(manifest)['projects']:
  for row in project['files']:add(manifest.parent/row['path'],'Held-out XML input or required notice',row['sha256'])
 for n in ('LICENSE-APACHE','LICENSE-MIT','tests/c/UPSTREAM-NOTICES.txt'):add(W/n,'Selected source/derived C fixture notices')
 add(E/'expat-source/COPYING','Expat source license')
 add(Path('/home/dev-user/code/oss/oriole/benchmarks/native_driver.c'),'Canonical native driver source context; measured executable has independent hash identity')
 for n in ('assemble.py','verify.py','package-plan.json'):add(R/n,'Package producer/verifier or scope plan')
 archive=R/'evidence.tar.gz'
 with archive.open('xb') as f:
  with gzip.GzipFile(filename='',fileobj=f,mode='wb',mtime=0,compresslevel=9) as z:
   with tarfile.open(fileobj=z,mode='w',format=tarfile.PAX_FORMAT) as t:
    for h,data in sorted(payloads.items()):
     m=tarfile.TarInfo('payload/'+h);m.size=len(data);m.mode=0o644;m.uid=m.gid=m.mtime=0;t.addfile(m,io.BytesIO(data))
 index={'schema_version':1,'archive_sha256':sha(archive.read_bytes()),'archive_bytes':archive.stat().st_size,'stored_members':len(payloads),'origins':[records[k] for k in sorted(records)],'omitted':[omitted[k] for k in sorted(omitted)],'scope':'Every original raw native worker/report is stored intact. Content-identical files share a member by hash; no row/value compaction. Source capsules expanded, binaries excluded with recorded identities. No Cargo/CMake build trees or unrelated historical archives.'}
 (R/'members.json.gz').write_bytes(gzip.compress((json.dumps(index,indent=2)+'\n').encode(),mtime=0,compresslevel=9))
 for row in records.values():
  if '#' not in row['origin']:assert sha(Path(row['origin']).read_bytes())==row['sha256'],row['origin']
 print(json.dumps({'status':'assembled_original_origins_read_back','archive_sha256':index['archive_sha256'],'archive_bytes':index['archive_bytes'],'stored_members':len(payloads),'origins':len(records),'aliases':len(records)-len(payloads),'omitted':len(omitted),'members_sha256':sha((R/'members.json.gz').read_bytes())},indent=2))
if __name__=='__main__':main()
