from pathlib import Path
import hashlib,json,tarfile,io,shutil,subprocess
W=Path('/home/dev-user/code/oss/oriole-end-frame-cells');S=Path('/tmp/oriole-pr115-windows-portability-study');B=Path('/tmp/oriole-pr115-windows-portability-build');D=W/'docs/validation/2026-09-11/detached-end-windows-portability'
assert not D.exists();D.mkdir(parents=True)
sha=lambda b:hashlib.sha256(b).hexdigest()
source=json.loads((S/'source.json').read_bytes());checks=json.loads((S/'checks.json').read_bytes());build=json.loads((B/'report.json').read_bytes());assert checks['status']==build['status']=='passed' and checks['source_libraries_unchanged'] and build['libraries_identical_to_measured']
assert build['fresh_workspace_compiles']==3
prior=json.loads((S/'prior-source.json').read_bytes())['source_sha256'];current=source['source_sha256'];changed=[n for n in prior if prior[n]!=current[n]];assert changed==['crates/oriole_expat/src/tests.rs']
old=subprocess.check_output(['git','show',source['base']+':'+changed[0]],cwd=W);new=(W/changed[0]).read_bytes();assert old.replace(b'opening.to_bytes().len() as i64',b'opening.to_bytes().len() as c_long')==new
entries={};excluded=[]
for label,root in [('source',S),('build',B)]:
 for p in sorted(root.rglob('*')):
  if not p.is_file() or '__pycache__' in p.parts:continue
  b=p.read_bytes();rec={'origin':str(p),'bytes':len(b),'sha256':sha(b)}
  if b.startswith(b'\x7fELF') or b.startswith(b'!<arch>\n'):excluded.append({'member':label+'/'+str(p.relative_to(root))}|rec)
  else:entries[label+'/'+str(p.relative_to(root))]=rec
with tarfile.open(D/'evidence.tar.gz','w:gz',compresslevel=9) as t:
 for name,rec in entries.items():
  data=Path(rec['origin']).read_bytes();assert sha(data)==rec['sha256'];info=tarfile.TarInfo(name);info.size=len(data);info.mode=0o644;info.mtime=0;t.addfile(info,io.BytesIO(data))
with tarfile.open(D/'evidence.tar.gz','r:gz') as t:
 members=t.getmembers();assert len(members)==len(entries) and all(m.isfile() for m in members)
 nested=[]
 for m in members:
  data=t.extractfile(m).read();assert sha(data)==entries[m.name]['sha256']
  if m.name.endswith('source.tar.gz'):
   with tarfile.open(fileobj=io.BytesIO(data),mode='r:gz') as inner:
    mapped={x.name:sha(inner.extractfile(x).read()) for x in inner.getmembers() if x.isfile()};assert mapped==current and len(mapped)==70;nested.append({'member':m.name,'source_files':70,'map_exact':True})
(D/'archive-members.json').write_text(json.dumps(entries,indent=2)+'\n')
report={'status':'local_portability_supplement_passed','scope':'Test-only Windows C-long cast correction. Preserved historical source/gates/measurements; platform execution belongs to PR115 CI. No local Windows execution or full Linux re-gate claimed.','base_commit':source['base'],'prior_source_manifest_sha256':sha((S/'prior-source.json').read_bytes()),'source_manifest_sha256':sha((S/'source.json').read_bytes()),'selected_source_files':70,'other_selected_files_unchanged':69,'runtime_implementation_unchanged':True,'changed_file':changed[0],'change':'Expected XML_GetCurrentByteIndex byte offset casts to imported c_long instead of i64. Fixture, expected value, callback budget and all assertions preserved.','patch_sha256':sha((S/'candidate.patch').read_bytes()),'original_windows_failure':{'run':34553541588,'job':103121238410,'url':'https://github.com/astral-sh/oriole/actions/runs/34553541588/job/103121238410','error':'E0308: expected i32, found i64 in lib test compilation','raw_log_sha256':sha((S/'windows-failure.log').read_bytes())},'local_checks':{'focused_tests':1,'focused_name':'tests::detached_end_charges_the_name_before_handlers_or_default_fallback','fmt':'passed','command_exits':[r['exit'] for r in checks['commands']],'checks_sha256':sha((S/'checks.json').read_bytes())},'fresh_c_only_build':{'workspace_compiles':3,'compiler_vectors_identical':build['compiler_comparison']['matched_after_private_paths_only'],'libraries_identical_to_measured':True,'libraries':build['libraries'],'build_report_sha256':sha((B/'report.json').read_bytes())},'archive':{'sha256':sha((D/'evidence.tar.gz').read_bytes()),'members':len(entries),'nested_sources':nested,'excluded_binaries':excluded},'author_role':'The patch author performed the local focused test, source-map/byte-identity checks and archive readback. Historical independent reviews remain separately preserved.'}
(D/'report.json').write_text(json.dumps(report,indent=2)+'\n');shutil.copy2(S/'source.json',D/'source.json');shutil.copy2(S/'candidate.patch',D/'test-only.patch')
(D/'README.md').write_text('''# Detached-end Windows test portability supplement

PR115's [Windows job](https://github.com/astral-sh/oriole/actions/runs/34553541588/job/103121238410) failed while compiling the callback-budget test: `XML_GetCurrentByteIndex` returns `c_long`, which is `i32` on Windows, but the expected offset was cast to `i64`. We cast the expected value to the already imported `c_long`. The fixture, expected offset, budget and assertions are unchanged.

This is a supplement to the [detached-end report](../detached-end-frames/). The prior source manifest `34473026` and all historical archives remain unchanged. The new 70-file manifest is `eab8c027`; only the test file differs. No runtime implementation changes.

The focused End callback-budget test passes on Linux, and formatting passes. A fresh C-only ThinLTO build emitted all three workspace compiler invocations with matching flags and produced byte-identical shared (`02fcab59`) and static (`69ebed41`) libraries. This preserves the measured C-library identity without repeating the full Linux gates. Windows and other platform checks run in [PR115 CI](https://github.com/astral-sh/oriole/pull/115/checks).

[The structured report](report.json), [one-line patch](test-only.patch), [source manifest](source.json), and [evidence archive](evidence.tar.gz) retain the original Windows failure, commands, raw local logs and source/build lineage. The archive contains no executable libraries; both binary hashes and full member readback are recorded. Local validation and packaging were performed by the patch author.
''')
shutil.copy2(__file__,D/'package.py')
files={p.name:{'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())} for p in D.iterdir() if p.is_file()};(D/'files.json').write_text(json.dumps(files,indent=2,sort_keys=True)+'\n')
p=W/'README.md';s=p.read_text();marker='## Installation\n';assert s.count(marker)==1;s=s.replace(marker,'The [Windows test portability supplement](docs/validation/2026-09-11/detached-end-windows-portability/) corrects an expected-value cast and verifies byte-identical C libraries.\n\n'+marker);p.write_text(s)
print(json.dumps({'source':report['source_manifest_sha256'],'archive':report['archive']['sha256'],'report':sha((D/'report.json').read_bytes()),'members':len(entries),'files':len(files)},indent=2))
