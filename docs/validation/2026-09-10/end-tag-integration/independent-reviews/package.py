"""Compact immutable receipt package; raw study data remain in their original locations."""
from pathlib import Path
import gzip,hashlib,io,json,shutil,tarfile
out=Path('/tmp/oriole-end-tag-integration-timing-review-handoff');out.mkdir(exist_ok=False)
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();load=lambda p:json.loads(Path(p).read_bytes())
roots={n:Path('/tmp/oriole-end-tag-integration-'+n+'-independent-review')for n in ['native','python','cpython','assembly']};roots['composition']=Path('/tmp/oriole-end-tag-integration-independent-review')
entries=[]
for label,r in roots.items():
 for p in sorted(r.rglob('*')):
  if p.is_file():entries.append({'path':label+'/'+p.relative_to(r).as_posix(),'origin':str(p),'sha256':h(p),'bytes':p.stat().st_size})
with (out/'reviews.tar.gz').open('wb')as f:
 with gzip.GzipFile(fileobj=f,mode='wb',mtime=0)as gz:
  with tarfile.open(fileobj=gz,mode='w')as t:
   for row in entries:
    data=Path(row['origin']).read_bytes();assert hashlib.sha256(data).hexdigest()==row['sha256'];info=tarfile.TarInfo(row['path']);info.size=len(data);info.mode=0o644;t.addfile(info,io.BytesIO(data))
with tarfile.open(out/'reviews.tar.gz')as t:
 members=t.getmembers();assert len(members)==len(entries)
 for m,row in zip(members,entries,strict=True):assert m.isfile()and m.name==row['path']and hashlib.sha256(t.extractfile(m).read()).hexdigest()==row['sha256']and h(row['origin'])==row['sha256']
(out/'archive-members.json').write_text(json.dumps(entries,indent=2)+'\n')
native=load(roots['native']/'review.json');py=load(roots['python']/'review.json');cpp=load(roots['cpython']/'review.json');assembly=load(roots['assembly']/'review.json')
summary={'status':'independent_reviews_passed','scope':'Combined dd16/fbb on64a integrated parent3694 and pinnedExpat; all saved-data only, no parser/build or timing reruns. Prior isolated matching-end evidence is separate.','runtime_commit':assembly['runtime_commit'],'source_files':assembly['source_files'],'libraries':assembly['libraries'],'native_groups':native['groups'],'python_groups':py['groups'],'strict_cpython':cpp['cpython'],'counts':{'native':native['counts'],'python':{'preflights':py['preflights'],'timed_workers':py['timed_workers'],'cohorts':py['cohorts'],'sample_counts':py['sample_counts']}},'regressions':{'native':native['regressions'],'python':py['regressions']},'compiler':'Three actual fresh workspace rustc command vectors match3694 after normalizing only private intermediate directories. Commit503 source70 equals live and archived measured source;64a→71bf parent advance leaves all70 files unchanged.','limitations':['Shared-host point estimates on six original real XML inputs; no full-project claim. All conditions and adverse comparisons retained.','Native includes parser create/setup/parse/callback/free; Python sums parse and explicit destruction with GC enabled. Canonicalized Python outputs are not exact callback-grouping proofs.','Native handle-based library loading and Python loaded identities are preserved; strict fresh-import finder checked separately.','Original393 API failures and two CPython grouping failures remain. Candidate is still slower than Expat overall.','Labels named published in inherited raw protocols identify the explicit3694 control in this study, not an unqualified current release.'],'reviews':{label:{'path':str(r/'review.json'),'sha256':h(r/'review.json')}for label,r in roots.items()},'correctness_gate_handoff':{'path':'/tmp/oriole-end-tag-integration-gates-handoff','manifest_sha256':h('/tmp/oriole-end-tag-integration-gates-handoff/manifest.json')},'readback':{'members':len(entries),'all_stored_and_origin_bytes_rehashed':True}}
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');shutil.copyfile(__file__,out/'package.py')
files={p.name:{'sha256':h(p),'bytes':p.stat().st_size}for p in sorted(out.iterdir())if p.is_file()};(out/'manifest.json').write_text(json.dumps({'status':'complete','files':files},indent=2)+'\n')
for n,row in files.items():assert h(out/n)==row['sha256']
print(json.dumps({'path':str(out),'manifest_sha256':h(out/'manifest.json'),'archive_sha256':h(out/'reviews.tar.gz'),'archive_bytes':(out/'reviews.tar.gz').stat().st_size,'archive_members':len(entries),'outer_files':len(files)+1}))
