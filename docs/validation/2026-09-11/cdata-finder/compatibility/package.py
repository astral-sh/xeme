"""Package immutable Finder compatibility evidence; no target execution."""
from pathlib import Path
import ast,gzip,hashlib,io,json,re,tarfile
O=Path(__file__).parent;C=Path('/tmp/oriole-cdata-finder-thin-pgo-gates');P=Path('/tmp/oriole-cdata-finder-thin-pgo-cpython-gates');B=Path('/tmp/oriole-streaming-work-thin-pgo-gates-cpu5');BP=Path('/tmp/oriole-streaming-work-thin-pgo-cpython-gates');W=Path('/home/dev-user/code/oss/oriole-cdata-finder');BUILD=Path('/tmp/oriole-cdata-finder-pgo-study')
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();j=lambda p:json.loads(Path(p).read_text())
assert not (O/'evidence.tar.gz').exists();entries={};excluded={}
def add(name,p):
 p=Path(p);assert name not in entries
 data=p.read_bytes();why=''
 if data.startswith(b'\x7fELF') or data.startswith(b'!<arch>\n'):why='compiled library/object/executable excluded'
 elif '__pycache__' in p.parts or p.suffix=='.pyc':why='interpreter cache excluded'
 record={'origin':str(p),'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)}
 if why:excluded[name]={**record,'reason':why}
 else:entries[name]=record
for prefix,directory in [('c-gates',C),('strict-gates',P),('strict-preparation',Path('/tmp/oriole-cdata-finder-thin-pgo-cpython-preparation')),('strict-outcome-review',Path('/tmp/oriole-cdata-finder-strict-outcome-review'))]:
 for p in sorted(directory.rglob('*')):
  if p.is_file():add(prefix+'/'+str(p.relative_to(directory)),p)
for p in [Path('/tmp/oriole-prepare-cdata-finder-gates.py'),Path('/tmp/oriole-prepare-cdata-finder-gates.log'),Path('/tmp/oriole-cdata-finder-cgate-launch.json'),Path('/tmp/oriole-cdata-finder-cgate-outcome-review.json'),Path('/tmp/oriole-cdata-finder-cgate-outcome-audit.py'),Path('/tmp/oriole-cdata-finder-cgate-outcome-audit.log'),Path('/tmp/oriole-cdata-finder-cgate-outcome-audit-attempt01.py'),Path('/tmp/oriole-cdata-finder-cgate-outcome-audit-attempt01.log'),Path('/tmp/oriole-cdata-finder-root-compat-review.py'),Path('/tmp/oriole-cdata-finder-root-compat-review.json'),Path('/tmp/oriole-cdata-finder-independent-audit.py'),Path('/tmp/oriole-cdata-finder-independent-review.json')]:add('reviews-and-controllers/'+p.name,p)
for n in ['source.json','source.patch','base-source.json','pair-report.json','pair-freeze.json','tool-source.json','vector-training-proof.json']:
 add('build-pins/'+n,BUILD/n)
add('build-pins/pgo-manifest.json',j(BUILD/'pair-report.json')['pgo_manifest']['path'])
source=j(BUILD/'source.json')['source_sha256'];assert len(source)==70
for n,v in source.items():assert h(W/n)==v;add('source/'+n,W/n)
for n in ['LICENSE-APACHE','LICENSE-MIT','THIRD_PARTY_LICENSES.md','licenses/cpython.txt','licenses/expat.txt','tests/c/UPSTREAM-NOTICES.txt','tools/cpython/run.py','tools/cpython/extension_loader.py','tools/cpython/check_extension_imports.py','integration/python-build-standalone/consumer-fix/provenance.json','integration/python-build-standalone/consumer-fix/cpython-3.12.13-external-parser.patch']:
 if 'source/'+n not in entries:add('source/'+n,W/n)
add('notices/expat-COPYING','/home/dev-user/.cache/oriole/upstream/expat-2.8.4/COPYING');add('notices/CPython-LICENSE','/home/dev-user/.cache/oriole/upstream/cpython-3.12.13/LICENSE')
for n in ['summary.json','source.json','api-native/upstream-api/results.json','api-native/upstream-api/manifest.json','other-gates/differential/oriole.json','other-gates/differential/corpus.json','malformed/observations.jsonl.gz','publication-probe/observations.json.gz','end-lifecycle-final/baseline.json','end-lifecycle-final/candidate.json','end-lifecycle-final/expat.json','end-oom-final/candidate.stdout']:add('baseline-c-gates/'+n,B/n)
for n in ['source.json','pair-report.json','pair-freeze.json']:add('baseline-build-pins/'+n,Path('/tmp/oriole-streaming-work-pgo-study-attempt02')/n)
for n in ['crates/oriole/src/lib.rs','tests/c/integration.c']:add('baseline-source/'+n,Path('/home/dev-user/code/oss/oriole-streaming-input-bounds')/n)
for linkage in ['shared','static']:
 for n in ['tests.log','summary.json','origin.log','consumer-fix.log']:add('baseline-strict/'+linkage+'/'+n,BP/linkage/n)
add('baseline-strict/controller.json',BP/'controller.json');add('baseline-strict/outcome-review.json','/tmp/oriole-streaming-work-thin-pgo-cpython-outcome-review/review.json')
# Include the original six unchanged CPython test modules and complete method maps.
modulepins=j('/tmp/oriole-pbs-version-followup/report.json')['test_count_and_coverage']['upstream_six_module_hashes_exact']
for n,v in modulepins.items():
 p=Path('/home/dev-user/.cache/oriole/upstream/cpython-3.12.13/Lib/test')/(n+'.py');assert h(p)==v;add('consumer-tests/'+n+'.py',p)
reference=Path('/tmp/oriole-native-byte-count-thinlto-cpython-root-review/audit.py');tree=ast.parse(reference.read_text());node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='methods');read=lambda p:Path(p).read_bytes();exec(compile(ast.Module(body=[node],type_ignores=[]),str(reference),'exec'))
(O/'method-maps').mkdir()
for label,d in [('candidate',P),('baseline',BP)]:
 for linkage in ['shared','static']:
  rows=methods(d/linkage/'tests.log');assert len(rows)==802
  p=O/'method-maps'/(label+'-'+linkage+'.json');p.write_text(json.dumps(rows,indent=2,sort_keys=True)+'\n');add('method-maps/'+p.name,p)
assert methods(P/'shared/tests.log')==methods(P/'static/tests.log')==methods(BP/'shared/tests.log')==methods(BP/'static/tests.log')
add('method-maps/parser-source.py',reference)
# Exact excluded candidate/control .so/.a identities remain available for artifact reconstruction.
for directory in [Path(j(BUILD/'pair-report.json')['pgo_manifest']['path']).parent/'use',Path('/tmp/oriole-streaming-work-pgo-study-attempt02/pgo/runs/run-psjiwn9z/use')]:
 for n in ['liboriole_expat.so','liboriole_expat.a']:
  add(('excluded-candidate/' if 'cdata-finder' in str(directory) else 'excluded-control/')+n,directory/n)
add('packaging/package.py',Path(__file__));add('packaging/verify.py',O/'verify.py')
with (O/'evidence.tar.gz').open('wb') as raw:
 with gzip.GzipFile(filename='',fileobj=raw,mode='wb',mtime=0,compresslevel=9) as compressed:
  with tarfile.open(fileobj=compressed,mode='w') as archive:
   for name,meta in sorted(entries.items()):
    data=Path(meta['origin']).read_bytes();assert hashlib.sha256(data).hexdigest()==meta['sha256']
    info=tarfile.TarInfo(name);info.size=len(data);info.mode=0o644;info.uid=info.gid=info.mtime=0;info.uname=info.gname='';archive.addfile(info,io.BytesIO(data))
index={'members':entries,'excluded':excluded,'deduplication':'None; repeated source/header or method-map identities remain explicit members. No aliases require reconstruction.'}
(O/'members.json.gz').write_bytes(gzip.compress(json.dumps(index,sort_keys=True,separators=(',',':')).encode(),mtime=0))
# Readback every member byte and every original, not just the archive footer.
with tarfile.open(O/'evidence.tar.gz') as t:
 assert {m.name for m in t}==set(entries)
 for m in t:
  meta=entries[m.name];assert m.isfile() and m.size==meta['bytes'] and hashlib.sha256(t.extractfile(m).read()).hexdigest()==meta['sha256']==h(meta['origin'])
rootreview=Path('/tmp/oriole-cdata-finder-root-compat-review.json');assert j(rootreview)['status'].startswith('passed')
report={'status':'passed_package_readback','role':'C/API and strict collector-owned compatibility handoff; includes separate root independent review of API/strict/source/stream outcomes and collector-owned detailed trace reconstruction. Packaging performs no target execution.','source_manifest_sha256':h(BUILD/'source.json'),'source_files':70,'runtime_delta':['crates/oriole/src/lib.rs'],'inherited_PR125_integration_sha256':source['tests/c/integration.c'],'archive':{'path':'evidence.tar.gz','sha256':h(O/'evidence.tar.gz'),'bytes':(O/'evidence.tar.gz').stat().st_size,'members':len(entries)},'index':{'path':'members.json.gz','sha256':h(O/'members.json.gz')},'excluded_files':len(excluded),'libraries':j(C/'summary.json')['libraries'],'outcomes':{'API_rows':4740,'API_pass':4347,'API_assertion_failures':391,'API_timeouts':2,'API_raw_exit':1,'API_rows_byte_exact_baseline':True,'C_consumer_runs':6,'allocation_scenarios_per_linkage':327,'strict_traces':3318,'custom_alias_comparisons':36456,'malformed_pairs':2392,'publication_cases':1304,'publication_parses':3912,'End_cases':38,'End_parses':114,'End_OOM_scenarios':6,'End_OOM_rows_per_engine':37,'CPython_methods_per_linkage':802,'CPython_raw_exits':[2,2],'CPython_method_maps_exact':True,'CPython_known_failures':['test.test_pyexpat.BufferTextTest.test1','test.test_sax.CDATAHandlerTest.test_handlers']},'reviews':{'collector_C_readback':h('/tmp/oriole-cdata-finder-cgate-outcome-review.json'),'collector_strict_readback':h('/tmp/oriole-cdata-finder-strict-outcome-review/review.json'),'root_independent_compatibility':h(rootreview)},'first_target_attempts':True,'all_sessions_reaped':True,'preserved_review_attempts':'Reviewer-only publication origin list/dict expectation corrected once; first script/log retained. No target retry.','limits':['Original API3s/1GiB AS/768MiB RSS/240s total and strict1200s per linkage unchanged.','C ASan/UBSan only; Rust PGO release library is uninstrumented and C leak checking is disabled. No current whole-Rust ASan claim.','Strict CPython uses the approved cleanup backport and retains its two failures,3 expected failures and14 reported skips; outcome parity is not a green suite.','Successful custom alias raw pairs are not emitted by the unchanged helper; counts/scripts/commands/difference summaries remain.','Historical End fixture review is provenance only. Reference publication callback-position differences and38 reference-different End cases retained.','No normal/PGO benchmark or full PBS distribution result is claimed by this compatibility handoff.'],'portable_verification':'python3 verify.py; optional --check-origins rehashes still-available original local paths. No target code is executed.'}
(O/'report.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'report_sha256':h(O/'report.json'),'archive':report['archive'],'index':report['index'],'excluded':len(excluded)}))
