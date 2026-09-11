"""Read completed long-default C records and inspect ELF symbols; never run consumers."""
from pathlib import Path
import ast,hashlib,json,re,subprocess
R=Path('/tmp/oriole-long-default-regression');D=R/'run02';W=Path('/home/dev-user/code/oss/oriole-long-default-regression')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={}
def pin(p):
 p=Path(p);h=sha(p);assert str(p) not in pins or pins[str(p)]==h;pins[str(p)]=h;return h
def load(p):pin(p);return json.loads(Path(p).read_text())
r=load(D/'report.json');assert r['status']=='passed'
for p,h in r['pins'].items():assert pin(p)==h
source=load('/tmp/oriole-suffix-element-names-pgo-study/source.json')
assert source['candidate_base_commit']==r['source_commit']=='a0acaf1bf8c8ae3f55030332be794cb8b9b5bea9'
changed=[n for n,h in source['source_sha256'].items() if pin(W/n)!=h]
assert changed==['tests/c/integration.c'] and len(source['source_sha256'])==72
assert (W/'tests/c/integration.c').read_bytes()==(R/'integration.c').read_bytes()
assert (W/'tests/c/UPSTREAM-NOTICES.txt').read_bytes()==(R/'UPSTREAM-NOTICES.txt').read_bytes()
for p in [R/'integration.c',R/'UPSTREAM-NOTICES.txt',R/'long-default.patch',R/'run-attempt01.py',R/'attempt01.md']:pin(p)
assert pin(R/'long-default.patch')=='d191732f4500538fc46088cc692ba48788235e9d41cab49fee59cc6946da44f9'
up=Path('/home/dev-user/.cache/oriole/upstream/expat-2.8.4/expat/tests/nsalloc_tests.c');pin(up)
c=(W/'tests/c/integration.c').read_text();u=up.read_text()
def literal(text):
 text=re.sub(r'/\*.*?\*/','',text,flags=re.S)
 declaration=re.match(r'\s*=\s*((?:"(?:\\.|[^"\\])*"\s*)+);',text);assert declaration
 return ''.join(ast.literal_eval(q) for q in re.findall(r'"(?:\\.|[^"\\])*"',declaration[1]))
a=literal(c.split('static const char long_default_root[]',1)[1])
b=literal(u.split('START_TEST(test_nsalloc_long_default_in_ext)',1)[1].split('const char *text',1)[1])
assert a==b and len(a)==1111 and a.count('ABCDEFGHIJKLMNOP')==64
assert '<e/>' in c.split('static int XMLCALL long_default_external',1)[1].split('static void external_entity_long_default',1)[0]
headers=[W/'include/expat.h',up.parents[1]/'lib/expat.h']
for header in headers:
 pin(header);v=tuple(int(re.search(r'#define XML_'+name+r'_VERSION\s+(\d+)',header.read_text())[1]) for name in ['MAJOR','MINOR','MICRO']);assert v==(2,8,4)
expected_labels=[engine+'-'+phase for engine in ['expat','shared','static'] for phase in ['build','elf','ldd','run']]
assert [row['label'] for row in r['commands']]==expected_labels
commands={row['label']:row for row in r['commands']}
for row in r['commands']:
 assert row['exit']==0 and row['reaped'] and row['end']>=row['start'] and row['timeout_seconds']==120
 assert pin(D/(row['label']+'.log'))==row['log_sha256']
 assert 'exception' not in row
results=[]
for engine in ['expat','shared','static']:
 exe=D/engine;binary_sha=pin(exe);argv=commands[engine+'-build']['argv']
 assert argv[:9]==['cc','-std=c11','-O1','-g','-Wall','-Wextra','-Werror','-fsanitize=address,undefined','-fno-omit-frame-pointer']
 assert '-DNDEBUG' not in argv and argv[-2:]==['-o',str(exe)] and str(W/'tests/c/integration.c') in argv
 assert commands[engine+'-run']['argv']==[str(exe)]
 run=(D/(engine+'-run.log')).read_text();assert run.strip()==('C ABI full integration passed (expat_2.8.4)' if engine=='expat' else 'C ABI full integration passed (oriole_compat_2.8.4)')
 elf=(D/(engine+'-elf.log')).read_text();ldd=(D/(engine+'-ldd.log')).read_text()
 assert '[libasan.so.8]' in elf and '[libubsan.so.1]' in elf
 if engine=='expat':
  assert '[libexpat.so.1]' in elf and 'Library runpath: [/tmp/oriole-pgo-study/expat-use-build]' in elf
  resolved=Path(re.search(r'libexpat.so.1 => (\S+)',ldd)[1]).resolve();assert pin(resolved)=='12d33ad26315e8b46a02e598df8581679d0561fb2d98a3d4744e483a573fbdd0'
 elif engine=='shared':
  assert '[liboriole_expat.so]' in elf
  resolved=Path(re.search(r'liboriole_expat.so => (\S+)',ldd)[1]).resolve();assert pin(resolved)=='5872374889ad673fcf7f397057de33a95a1e8f72be591b9bb996b8dbc1fd9280'
 else:
  assert 'liboriole_expat' not in elf and 'libexpat' not in elf
  assert str(Path('/tmp/oriole-suffix-element-names-pgo-study/pgo/runs/run-kj_rp37s/use/liboriole_expat.a')) in argv
 # ELF metadata inspection, not consumer execution; retain the exact output.
 symbols=subprocess.check_output(['/usr/bin/readelf','--wide','--syms',str(exe)],text=True)
 path=R/(engine+'-review-symbols.txt');assert not path.exists();path.write_text(symbols);pin(path)
 parse=[line.split() for line in symbols.splitlines() if line.split() and line.split()[-1]=='XML_Parse']
 assert parse and all((row[-2]!='UND')==(engine=='static') for row in parse)
 results.append({'engine':engine,'binary_sha256':binary_sha,'exit':0,'contexts':12,'assertions_active':True,'sanitizers':'C ASan/UBSan','xml_parse_symbol_kind':'defined' if engine=='static' else 'imported'})
for p,h in pins.items():assert sha(p)==h
out={'status':'passed_independent_review','runtime_source':'a0acaf1bf8c8ae3f55030332be794cb8b9b5bea9','runtime_and_other_source_files_unchanged':71,'intended_source_delta':changed,'upstream_root_xml_bytes':1111,'default_bytes':1024,'fixture_exact_upstream':True,'runs':results,'total_long_default_contexts':36,'findings':[],'assertion_review':['Sole child attribute a1 is null-terminated with exactly 1024 independently checked A-through-P bytes, and zero specified attributes.','Parent/child start and end counts each equal one; exactly one child is created, completes final parsing successfully, and is freed before parent end. Child user data and handlers are inherited.','Each of six root chunk widths and two deferral modes checks successful final XML_Parse/error NONE and final selected malloc/free balance, independently per case.'],'limitations':['This is successful value/lifecycle coverage, not failure-injection coverage or a changed upstream allocation retry schedule.','Namespace mode is configured but the literal has no namespace-qualified names; deferral settings are exercised without a separate getter or callback-timing assertion.','Final completion is checked through successful final XML_Parse and callbacks; XML_GetParsingStatus is not separately asserted.','The allocator checks balanced successful block counts, not address-level ownership or process RSS. Release Rust/Expat libraries are uninstrumented and ASan leak checking is disabled.','Saved ELF dependencies/RUNPATH/ldd plus actual ELF symbols and library hashes establish linkage; the consumer does not log same-process dladdr origins.','The root run01 assertion failed before compilation because it included the intentional integration.c difference; its script/note are retained.'],'targets_executed':False,'elf_metadata_inspection_only':True,'input_sha256':pins}
p=R/'independent-review.json';assert not p.exists();p.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({'status':out['status'],'report':str(p),'sha256':sha(p),'contexts_per_engine':12,'total_contexts':36,'pins':len(pins)}))
