"""Seal independent selected9815 reviews; standalone and ThinLTO remain separate."""
from pathlib import Path
import gzip,hashlib,io,json,shutil,tarfile
out=Path('/tmp/oriole-start-frame-integration-review-handoff')
roots={n:Path('/tmp/oriole-start-frame-integration-'+n+'-independent-review')for n in ['native','python','cpython','assembly','gate']}
roots['composition']=Path('/tmp/oriole-start-frame-source-independent-review')
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();load=lambda p:json.loads(Path(p).read_bytes())
native=load(roots['native']/'review.json');py=load(roots['python']/'review.json');cpp=load(roots['cpython']/'review.json');assembly=load(roots['assembly']/'review.json');workspace=load(roots['gate']/'workspace-review.json')
assert native['regressions']==py['regressions']==[]and workspace['all_target_tests_passed']==398and workspace['doc_tests_passed']==1
out.mkdir(exist_ok=False);entries=[]
for label,r in roots.items():
    for p in sorted(r.rglob('*')):
        if p.is_file():entries.append({'path':label+'/'+p.relative_to(r).as_posix(),'origin':str(p),'sha256':h(p),'bytes':p.stat().st_size})
with(out/'reviews.tar.gz').open('wb')as f:
    with gzip.GzipFile(fileobj=f,mode='wb',mtime=0)as gz:
        with tarfile.open(fileobj=gz,mode='w')as t:
            for row in entries:
                data=Path(row['origin']).read_bytes();assert hashlib.sha256(data).hexdigest()==row['sha256'];m=tarfile.TarInfo(row['path']);m.size=len(data);m.mode=0o644;t.addfile(m,io.BytesIO(data))
with tarfile.open(out/'reviews.tar.gz')as t:
    members=t.getmembers();assert len(members)==len(entries)
    for m,row in zip(members,entries,strict=True):assert m.isfile()and m.name==row['path']and m.size==row['bytes']and hashlib.sha256(t.extractfile(m).read()).hexdigest()==row['sha256']==h(row['origin'])
(out/'archive-members.json').write_text(json.dumps(entries,indent=2)+'\n')
summary={
    'status':'independent_reviews_passed','scope':'Selected combined9815/92ea against direct503a/dd16 and pinnedExpat. Saved source/build/Git, fullgates, native/Python and strictCPython audits only; no reviewer parser/build/timing reruns. Standalonec435 and ThinLTOa55c studies remain separate.',
    'runtime_commit':assembly['runtime_commit'],'source_files':70,'libraries':assembly['libraries'],
    'native_groups':native['groups'],'python_groups':py['groups'],'strict_cpython':cpp['cpython'],
    'counts':{'native':native['counts'],'python':{'preflights':py['preflights'],'timed_workers':py['timed_workers'],'cohorts':py['cohorts'],'sample_counts':py['sample_counts']}},
    'regressions':{'native':native['regressions'],'python':py['regressions']},
    'workspace':{'all_target_executables':workspace['all_target_executables'],'all_target_tests_passed':workspace['all_target_tests_passed'],'doc_tests_passed':workspace['doc_tests_passed']},
    'compiler':'Three actual fresh workspace rustc command vectors matchdd16 after normalizing only private intermediate directories. Commit50c20source70 equals live/archive measured source;503a→382doc-only parent touches none of thosefiles. No explicit cross-crateLTO flag in these matched normal builds.',
    'limitations':['Shared-host point estimates on six original XML inputs; no full-project claim. All conditions retained.','Native includes create/setup/parse/callback/free; Python sums parsing and explicit destruction with GCenabled. Canonical text output is not exact callback-grouping proof.','Original393API failures and two strictCPython failures remain. Overall native andPython results remain slower thanExpat.','Three measured Wayland Python conditions beatExpat;64KiB ElementTree margin is about0.5%, not an unqualified whole-project speed claim.','Successful custom-alias raw traces are not individually retained; executed controllers/templates/zero-difference reports remain scoped.','C harnesses are sanitizer-instrumented with LSanoff; Rust normalrelease, sustainedfuzz/PBS evidence belongs to prior sources.','Raw labels calledpublished identify the explicitdd16control here, not an unqualified currentrelease. Standalonec435, ThinLTOa55c and unimplementedaccountingproposal do not inherit this timing claim.'],
    'reviews':{label:{'path':str(r/'review.json'),'sha256':h(r/'review.json')}for label,r in roots.items()},
    'workspace_review_sha256':h(roots['gate']/'workspace-review.json'),
    'correctness_gate_handoff':{'path':'/tmp/oriole-start-frame-integration-gates-handoff','manifest_sha256':h('/tmp/oriole-start-frame-integration-gates-handoff/manifest.json')},
    'readback':{'members':len(entries),'all_stored_and_origin_bytes_rehashed':True},
}
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');shutil.copyfile(__file__,out/'package.py')
files={p.name:{'sha256':h(p),'bytes':p.stat().st_size}for p in sorted(out.iterdir())if p.is_file()};(out/'manifest.json').write_text(json.dumps({'status':'complete','files':files},indent=2)+'\n')
for n,row in files.items():assert h(out/n)==row['sha256']
print(json.dumps({'path':str(out),'manifest_sha256':h(out/'manifest.json'),'archive_sha256':h(out/'reviews.tar.gz'),'archive_bytes':(out/'reviews.tar.gz').stat().st_size,'members':len(entries),'outer_files':len(files)+1}))
