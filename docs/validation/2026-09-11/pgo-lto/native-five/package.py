"""Archive complete build/native evidence; keep the readable handoff compact."""
from pathlib import Path
import hashlib,json,tarfile,io,shutil
O=Path(__file__).parent;sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();load=lambda p:json.loads(Path(p).read_text())
roots={
 'thin-pair':Path('/tmp/oriole-native-byte-count-pgo-study'),
 'fat-normal':Path('/tmp/oriole-native-byte-count-fat-lto-study'),
 'fat-pgo':Path('/tmp/oriole-native-byte-count-fat-pgo-study'),
 'native':Path('/tmp/oriole-current-pgo-native-study'),
 'author-proofs':Path('/tmp/oriole-current-pgo-build-evidence'),
 'expat-reuse-author':Path('/tmp/oriole-current-pgo-expat-reuse-proof'),
 'reviews/pair-build':Path('/tmp/oriole-native-byte-count-pgo-build-independent-review'),
 'reviews/pair-training':Path('/tmp/oriole-native-byte-count-pgo-training-independent-review'),
 'reviews/fat-normal':Path('/tmp/oriole-native-byte-count-fat-lto-independent-review'),
 'reviews/expat-reuse':Path('/tmp/oriole-current-pgo-expat-reuse-independent-review'),
 'reviews/native':Path('/tmp/oriole-current-pgo-native-independent-review'),
}
for root,freeze in [(roots['thin-pair'],'pair-freeze.json'),(roots['fat-normal'],'freeze.json'),(roots['fat-pgo'],'freeze.json'),(roots['native'],'freeze.json')]:
 assert all(sha(root/p)==h for p,h in load(root/freeze)['files_sha256'].items())
files={};excluded=[]
def add(name,p):
 assert p.is_file() and name not in files,(name,p)
 if p.suffix in ['.so','.a'] or '.so.' in p.name:
  excluded.append({'member':name,'path':str(p),'sha256':sha(p),'bytes':p.stat().st_size});return
 files[name]=p
for prefix,root in roots.items():
 for p in sorted(root.rglob('*')):
  rel=p.relative_to(root)
  if p.is_file() and not any(x in ['targets','__pycache__'] for x in rel.parts):add(prefix+'/'+rel.as_posix(),p)
# Historical Expat bytes needed to inspect the reused profile/compiler/training chain.
H=Path('/tmp/oriole-pgo-study')
for dirname in ['expat-source','expat-raw-profiles','inputs']:
 for p in sorted((H/dirname).rglob('*')):
  if p.is_file():add('expat-history/'+p.relative_to(H).as_posix(),p)
names=['expat-source.json','source.json','expat-profile.json','expat-profile-counts.txt','training-inputs.json','training-launch-provenance.json','build_expat.py','train.py','generate_training.py']
for phase in ['control','generate','use']:
 names.extend([f'expat-{phase}-build.json',f'expat-{phase}-configure.log',f'expat-{phase}-compile.log',f'expat-{phase}-training.json',f'expat-{phase}-build/expat_config.h',f'expat-{phase}-liboriole_expat.so'])
for n in names:add('expat-history/'+n,H/n)
manifest={'schema':1,'status':'prepared','files':{n:{'path':str(p),'sha256':sha(p),'bytes':p.stat().st_size} for n,p in sorted(files.items())},'excluded_binaries':excluded,'external_tool_identity_scope':'Compiler/Python/LLVM/GCC executable bytes are not bundled; original before/after hashes and versions are retained in build reports. Compiler targets/cache contents excluded.','scope':'Complete current four-mode build evidence, initial five-engine native campaign and existing Expat reuse. Fat-PGO has generated replay only; actual Python study is root-owned and separate.'}
archive=O/'evidence.tar.gz';assert not archive.exists()
with tarfile.open(archive,'w:gz',compresslevel=6) as tf:
 for name,p in sorted(files.items()):
  data=p.read_bytes();assert hashlib.sha256(data).hexdigest()==manifest['files'][name]['sha256'];info=tarfile.TarInfo(name);info.size=len(data);info.mode=0o644;info.mtime=0;tf.addfile(info,io.BytesIO(data))
readback={};nested={}
with tarfile.open(archive,'r:gz') as tf:
 members=tf.getmembers();assert len(members)==len(files) and len({m.name for m in members})==len(members)
 for m in members:
  assert m.isfile() and not m.name.startswith('/') and '..' not in Path(m.name).parts
  data=tf.extractfile(m).read();entry=manifest['files'][m.name];assert hashlib.sha256(data).hexdigest()==entry['sha256'] and len(data)==entry['bytes'];readback[m.name]=entry['sha256']
  if m.name.endswith('.tar.gz'):
   with tarfile.open(fileobj=io.BytesIO(data),mode='r:gz') as inner:
    rows={}
    for item in inner.getmembers():
     assert item.isfile() and item.name not in rows and not item.name.startswith('/') and '..' not in Path(item.name).parts
     b=inner.extractfile(item).read();rows[item.name]=hashlib.sha256(b).hexdigest()
    nested[m.name]=rows
assert all(sha(p)==manifest['files'][n]['sha256'] for n,p in files.items())
manifest.update(status='passed',archive={'path':str(archive),'sha256':sha(archive),'bytes':archive.stat().st_size,'members':len(readback)},nested_archive_members=sum(map(len,nested.values())))
(O/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
(O/'readback.json').write_text(json.dumps({'status':'passed','archive_sha256':sha(archive),'outer_members':len(readback),'all_outer_member_hashes_verified':True,'nested_member_sha256':nested,'all_original_files_unchanged':True,'generator_sha256':sha(__file__)},indent=2)+'\n')
summary=load(roots['native']/'summary.json');pair=load(roots['thin-pair']/'pair-report.json');fat=load(roots['fat-normal']/'report.json');fatpgo=load(roots['fat-pgo']/'report.json')
report={'status':'passed','source_commit':'be22a271f8a5c004d13e515cd4024a68afe57887','source_manifest_sha256':pair['source_manifest_sha256'],'source_files':70,'script_source_files':6,'normal_thin':pair['normal_libraries'],'pgo_thin':{k:v for k,v in pair['pgo_libraries'].items() if k.startswith('use/')},'normal_fat':fat['libraries'],'pgo_fat':{k:v for k,v in fatpgo['libraries'].items() if k.startswith('use/')},'generated_parses':{'normal_thin':288,'generate_thin':288,'use_thin':288,'normal_fat':288,'generate_fat':288,'use_fat':288},'native_counts':summary['counts'],'native_groups':summary['groups'],'native_regressions':summary['regressions'],'manifest_sha256':sha(O/'manifest.json'),'archive':manifest['archive'],'readback_sha256':sha(O/'readback.json'),'role':'Study author; independent scoped reviews are archived. Fat-PGO independent review may arrive as a separate supplement.','limitations':['No fresh full API/sanitizer/CPython test certification of new optimized binaries.','Native records are lifecycle timings with original native C callbacks, not application timings.','Actual Python benchmark is a separate root-owned study.','Fat-PGO is not included in the initial five-engine native timing campaign.','All four real and one generated no-PGO fat regressions retained.','Expat historical launch/loaded-origin/generic-manifest limitations retained; compiler parity is within each engine.','Explicit-target normal differs from prior implicit-target ccfb/f58; current comparisons use the fresh normal.','Shared host; CPU4 consumer preparation overlapped native CPU0 campaign.']}
(O/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'status':'passed','archive':manifest['archive'],'manifest_sha256':sha(O/'manifest.json'),'report_sha256':sha(O/'report.json'),'readback_sha256':sha(O/'readback.json')}),flush=True)
