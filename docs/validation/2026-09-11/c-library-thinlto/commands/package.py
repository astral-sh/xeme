from pathlib import Path
import json,hashlib,tarfile,shutil,subprocess,io
W=Path('/home/dev-user/code/oss/oriole-c-library-lto');O=Path('/tmp/oriole-c-library-lto-command-handoff');O.mkdir(exist_ok=False)
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();j=lambda p:json.loads(Path(p).read_text());put=lambda p,v:Path(p).write_text(json.dumps(v,indent=2)+'\n')
final=Path('/tmp/oriole-c-library-lto-production-implementation');phase2=Path('/tmp/oriole-c-library-lto-final-implementation');phase1=Path('/tmp/oriole-c-library-lto-implementation');s=j(final/'source.json')
assert all(h(W/n)==v for n,v in s['input_sha256'].items());assert j(final/'checks/report.json')['status']=='passed';pgo=j(phase2/'pgo-launch.json');pbs=j(final/'pbs-fresh/report.json');assert pgo['status']==pbs['status']=='passed' and pbs['unchanged'] and pgo['unchanged'];assert j(final/'pbs-nonempty.json')['exit']==2
before=j(phase2/'source.json');assert [n for n,v in before['input_sha256'].items() if s['input_sha256'][n]!=v]==['integration/python-build-standalone/prepare.py']
for n in ['candidate.patch','source.json','source.tar.gz','phase3-delta.json']:shutil.copyfile(final/n,O/n)
roots={
 'phase1':phase1,'phase2':phase2,'phase3':final,
 'warning-diagnosis':Path('/tmp/oriole-c-library-lto-warning-diagnosis'),
 'failed-pgo':Path('/tmp/oriole-c-library-lto-pgo-validation/runs/run-_rdl2z8c'),
 'successful-pgo':Path('/tmp/oriole-c-library-lto-final-pgo-validation/runs/run-0gjyf0_i'),
 'pbs-01-empty-rustc':Path('/tmp/oriole-c-library-lto-pbs-validation'),
 'pbs-02-empty-encoded-unhardened':Path('/tmp/oriole-c-library-lto-pbs-final-validation'),
 'pbs-03-clean-unhardened':Path('/tmp/oriole-c-library-lto-pbs-clean-validation'),
 'pbs-04-hardened-cache-reuse':Path('/tmp/oriole-c-library-lto-pbs-production-validation'),
 'pbs-05-hardened-fresh':Path('/tmp/oriole-c-library-lto-pbs-production-fresh-validation'),
}
entries={};excluded={}
for prefix,root in roots.items():
 for p in sorted(root.rglob('*')):
  if not p.is_file() or '__pycache__' in p.parts:continue
  name=prefix+'/'+str(p.relative_to(root));rec={'sha256':h(p),'bytes':p.stat().st_size,'source':str(p)}
  if p.suffix in ('.so','.a'):excluded[name]=rec
  else:entries[name]=rec
# The originally failed use build had no published use/ directory; its copied binaries are indexed under warning-diagnosis.
put(O/'excluded-binaries.json',excluded)
with tarfile.open(O/'evidence.tar.gz','w:gz') as t:
 for name,v in entries.items():
  p=Path(v['source']);assert h(p)==v['sha256'];ti=tarfile.TarInfo(name);ti.size=v['bytes'];ti.mode=0o644
  with p.open('rb') as f:t.addfile(ti,f)
read={};nested={}
with tarfile.open(O/'evidence.tar.gz','r:gz') as t:
 for m in t:
  assert m.isfile() and m.name not in read and not m.name.startswith('/') and '..' not in Path(m.name).parts
  data=t.extractfile(m).read();v=entries[m.name];assert hashlib.sha256(data).hexdigest()==v['sha256'] and len(data)==v['bytes'];read[m.name]={'sha256':v['sha256'],'bytes':v['bytes']}
  if m.name.endswith('source.tar.gz'):
   rows={}
   with tarfile.open(fileobj=io.BytesIO(data),mode='r:gz') as st:
    for n in st:
     assert n.isfile() and n.name not in rows and not n.name.startswith('/') and '..' not in Path(n.name).parts
     b=st.extractfile(n).read();rows[n.name]={'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)}
   expected=j(roots[m.name.split('/')[0]]/'source.json')['input_sha256'];assert {n:v['sha256'] for n,v in rows.items()}==expected;nested[m.name]=rows
assert set(read)==set(entries);put(O/'archive-members.json',read);put(O/'nested-source-members.json',nested)
retained=[
 'Phase1: initial Ruff environment/unused-suppression failures and successful checks retained. Double-verbose PGO generated 288 successful parses; optimized compile succeeded but unchanged strict warning gate rejected a real allocator-api2 warning before optimized replay. No dependency or lint gate changed.',
 'Single-v proof retains actual compiler command and standard Cargo dependency cap-lints allow; double-v adds dependency warnings/cap-lints warn. No manually injected lint suppressions or filtered build output. Upstream warning/fix/source and failed output binaries hashes retained.',
 'PBS01: controller supplied RUSTC empty; Cargo could not launch compiler. PBS02: empty encoded Rust flags hid explicit PIC/unwind, despite bundle/TLS checks passing; this does not prove required flags. PBS03: absent encoded var passed with actual flags. PBS04: hardened helper completed, but compilation-count postprocessing correctly rejected cached reuse. PBS05: fresh hardened helper with intentionally empty encoded var passed explicit PIC/unwind/ThinLTO, native-library parsing and weak TLS checks.',
 'Warning diagnosis parser development failures were postprocessing only: initial extraction omitted attached -C options, second version omitted last attached argument. Prior generator revisions retained; final artifact reconstructs all flags.',
 'Fast-forward first attempt hit read-only Git metadata; authorized escalation completed doc-only FF to 1cb326c. No runtime edits. Root-authored README/new report files are outside this nine-file command patch and excluded.',
]
summary={'status':'passed_with_initial_attempts_retained','base':s['base'],'patch_sha256':h(O/'candidate.patch'),'source_manifest_sha256':h(O/'source.json'),'changed_files':s['changed_files'],'source_files':70,'tracked_inputs':81,'runtime_header_and_cargo_bytes_unchanged':True,'checks':{'pgo_unit_tests':13,'ruff':'passed','ruff_format':'passed','ty':'passed','pbs_recipe':'passed','diff':'passed'},'pgo':{'manifest_sha256':pgo['manifest_sha256'],'generate_parses':288,'optimized_replay_parses':288,'rows_exact':True,'fresh_workspace_compiles_per_phase':3,'final_crate_types':['cdylib','staticlib'],'effective_lto':'thin','libraries':pgo['libraries'],'scope':'Ohm defaults disabled, fresh profile, generated inputs only. No performance or full compatibility gate on these optimized artifacts. Phase3 only changes PBS helper; PGO input/helper identities unchanged.'},'pbs':{'manifest_sha256':pbs['bundle_manifest_sha256'],'fresh_workspace_compiles':3,'native_libraries':pbs['native_static_libraries'],'weak_tls_references':pbs['tls_references'],'explicit_pic_and_unwind':True,'empty_encoded_flags_tested':True,'nonempty_encoded_flags_exit':2,'scope':'Actual local bundle smoke using Ohm default experimental options; stable CI smoke authored, not executed here. No PBS distribution/glibc or new C compatibility test claim.'},'retained_attempts':retained,'archive_sha256':h(O/'evidence.tar.gz'),'archive_members':len(entries),'nested_source_members':sum(map(len,nested.values())),'executable_libraries_in_archive':False,'excluded_libraries':len(excluded),'parent_historical_evidence_unchanged':True}
put(O/'summary.json',summary)
(O/'HANDOFF.md').write_text('''# Production C-library commands

The nine-file patch uses `cargo rustc --lib --crate-type cdylib,staticlib` for active C builds while keeping `rlib` in Cargo.toml. CI runner and uv commands are unchanged. No Rust implementation, header, dependency or resource-limit changes.

PGO preserves global Cargo option provenance and uses single verbosity without changing its strict warning gate. PBS now removes an empty encoded-flags variable so required PIC/unwind flags take effect; nonempty values still reject. CI adds a stable Rust/rust-docs PBS bundle smoke. The historical PGO human README qualifies its unsupported effective-ThinLTO claim; archived historical bytes stay unchanged.

Validation: 13 unit tests, Ruff/format/type checks, PBS recipe validation; fresh effective-ThinLTO PGO generate/replay 288+288 exact parses; actual PBS bundle with three fresh workspace compiler commands, PIC/unwind, native-library extraction and weak TLS hook. PGO used Ohm defaults disabled; PBS used default Ohm. Stable CI and full PBS distribution are not claimed.

Read `summary.json` for initial failures and limited attempts. They remain separately retained, including the double-verbosity warning rejection and PBS controller/environment/cache cases. The final PGO and PBS successful runs are distinct from earlier normal/ThinLTO correctness and timing studies.

`candidate.patch` is against PR112 base 1cb326c. Apply only these nine files; root-authored current README/report changes are outside this package. No commit or push by the author.
''')
shutil.copyfile(__file__,O/'package.py');put(O/'artifact-manifest.json',{str(p.relative_to(O)):{'sha256':h(p),'bytes':p.stat().st_size} for p in sorted(O.iterdir()) if p.is_file() and p.name!='artifact-manifest.json'})
print(json.dumps({k:summary[k] for k in ['status','archive_sha256','archive_members','nested_source_members','excluded_libraries']}))
