"""Independent saved source/compiler/training audit; no target execution."""
from pathlib import Path
import argparse,hashlib,json,re,shlex,subprocess,tarfile
O=Path('/tmp/oriole-deferred-c-text-raw-pgo-study');B=Path('/tmp/oriole-cdata-finder-pgo-study');W=Path('/home/dev-user/code/oss/oriole-deferred-c-text-raw');S=Path('/tmp/oriole-deferred-c-text-raw-study');P=O/'pipeline'
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--released',action='store_true',help='Explicit root release after exclusive elapsed session is reaped')
parser.add_argument('--expected-freeze',required=True)
parser.add_argument('--expected-pair',required=True)
parser.add_argument('--expected-count',required=True,type=int)
args=parser.parse_args()
if not args.released:parser.error('Saved artifact scan remains held; root must release it after B elapsed completes')
assert re.fullmatch('[0-9a-f]{64}',args.expected_freeze) and re.fullmatch('[0-9a-f]{64}',args.expected_pair)
assert args.expected_count==85
assert args.expected_freeze=='1ae202d5daf63fddc25b93b60cf3916f99f8be73e056c90ccd384ea24d9ba4a4'
assert args.expected_pair=='f0dcbef4a1d0559bae00ad5ce8b79d0156e7c36a72b9ada2e3de1084a43ebc49'
load=lambda p:json.loads(Path(p).read_text())
def sha(p):
 with Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
f=load(O/'pair-freeze.json');assert sha(O/'pair-freeze.json')==args.expected_freeze
assert len(f['files'])==f['count']==args.expected_count
for name,v in f['files'].items():assert sha(O/name)==v['sha256'] and (O/name).stat().st_size==v['bytes'],name
c,a=load(B/'pair-report.json'),load(O/'pair-report.json');cm,am=[load(x['pgo_manifest']['path']) for x in (c,a)];cr,ar=Path(cm['run']),Path(am['run'])
assert a['status']==c['status']==am['status']==cm['status']=='passed'
assert sha(O/'pair-report.json')==args.expected_pair
assert all(a[k] for k in ('source_unchanged','scripts_unchanged','tools_unchanged'))
assert a['tools_before']==a['tools_after']==c['tools_before']
for key in ('cargo_config_sha256','base_rustflags','cargo_args','toolchain','host','rustc_version','cargo_version','profdata_version','python_version','tools_sha256'):assert am[key]==cm[key],key
for path,h in (a['tools_before']|am['tools_sha256']).items():assert sha(path)==h,path
for root,pair,m in ((O,a,am),(B,c,cm)):
 assert sha(pair['pgo_manifest']['path'])==pair['pgo_manifest']['sha256']
 for command in pair['commands']:
  assert command['exit']==0 and sha(root/(command['label']+'.log'))==command['log_sha256']
 for command in m['commands']:
  assert command['status']=='passed' and command['returncode']==0
  assert sha(Path(m['run'])/command['log'])==command['log_sha256']
metadata={'oriole_storage':'8ae6edb74b4afbee','oriole':'e41f1ff64bd2ed1c','oriole_expat':'f10a79f8fbc60887'}
def read_vectors(root,pair,m,phase):
 log=root/'normal-build.log' if phase=='normal' else Path(m['run'])/next(c['log'] for c in m['commands'] if c['label']=='build-'+phase)
 rows={}
 for line in log.read_text().splitlines():
  if 'Running `' not in line or '--crate-name ' not in line:continue
  args=shlex.split(line.split('Running `',1)[1].rsplit('`',1)[0]);name=args[args.index('--crate-name')+1]
  if name not in metadata:continue
  assert name not in rows;rows[name]=args
 assert set(rows)==set(metadata)
 expected=pair['normal_vectors'] if phase=='normal' else pair['pgo_vectors'][phase]
 assert rows=={r['crate']:r['argv'] for r in expected}
 return rows
def canonical(args,name,run):
 assert args.count('opt-level=3')==1
 assert [s for s in args if s.startswith('metadata=')]==['metadata='+metadata[name]]
 assert args[args.index('--target')+1]=='x86_64-unknown-linux-gnu'
 kinds=[args[i+1] for i,v in enumerate(args[:-1]) if v=='--crate-type']
 assert kinds==(['cdylib','staticlib'] if name=='oriole_expat' else ['lib'])
 assert ('lto=thin' if name=='oriole_expat' else 'linker-plugin-lto') in args
 out=[]
 for s in args:
  s=s.replace(str(run),'<fresh-run>')
  s=re.sub(r'/home/dev-user/\.cache/ohm/verified-build/[0-9a-f]{2}/[0-9a-f]{14}(?=/)', '<private-build>',s)
  s=re.sub(r'(?<=extra-filename=-)[0-9a-f]{16}$','<suffix>',s)
  s=re.sub(r'(lib\w+)-[0-9a-f]{16}(\.(?:rlib|rmeta))$',r'\1-<suffix>\2',s)
  out.append(s)
 return out
counts={}
for phase in ('normal','generate','use'):
 left=read_vectors(B,c,cm,phase);right=read_vectors(O,a,am,phase)
 for name in metadata:
  assert len(left[name])==len(right[name])
  assert canonical(left[name],name,cr)==canonical(right[name],name,ar),(phase,name)
 counts[phase]=len(right)
reference=load(B/'normal-training.json');training=[]
for root,pair,m in ((B,c,cm),(O,a,am)):
 for phase in ('normal','generate','use'):
  p=root/'normal-training.json' if phase=='normal' else Path(m['run'])/(phase+'-training.json');r=load(p)
  assert r['status']=='passed' and len(r['rows'])==288 and r['rows']==reference['rows']
  assert all(row['status']==1 and row['error']==0 and not row['callback_exceptions'] for row in r['rows'])
  assert r['inputs_sha256']==reference['inputs_sha256']
  assert sha(r['xml_parse_origin']['path'])==r['xml_parse_origin']['sha256']==r['library_sha256']
  if phase!='normal':assert sha(p)==m['training_sha256'][phase]
  training.append({'path':str(p),'sha256':sha(p),'rows':288})
assert {p.name:sha(p) for p in (ar/'inputs').iterdir()}=={p.name:sha(p) for p in (cr/'inputs').iterdir()}
for name,digest in am['profiles_sha256'].items():assert sha(ar/name if name=='merged.profdata' else ar/'raw-profiles'/name)==digest
assert am['profiles_sha256']['merged.profdata']!=cm['profiles_sha256']['merged.profdata']
source_record=load(O/'source.json');source=source_record['source_sha256'];baseline=load(B/'source.json')['source_sha256'];assert len(source)==70 and source.keys()==baseline.keys()
changed={'crates/oriole/src/dtd.rs','crates/oriole/src/dtd/attlist.rs','crates/oriole/src/lib.rs','crates/oriole/tests/adapter_frame.rs','crates/oriole/tests/allocation.rs','crates/oriole_expat/src/lib.rs','crates/oriole_expat/src/tests.rs'}
runtime=['crates/oriole/src/dtd.rs','crates/oriole/src/dtd/attlist.rs','crates/oriole/src/lib.rs','crates/oriole_expat/src/lib.rs']
assert {p for p in source if source[p]!=baseline[p]}==changed
assert all(sha(W/p)==v for p,v in source.items())
base='78c748d9de5c497858458cbf7a1229148781b2bf';assert load(O/'adapter-preparation.json')['base_commit']==base
base_files={p:hashlib.sha256(subprocess.check_output(['git','show',base+':'+p],cwd=W)).hexdigest() for p in source}
assert {p for p in source if source[p]!=base_files[p]}==changed
patch=subprocess.check_output(['git','diff',base,'--',*sorted(changed)],cwd=W)
assert patch==(O/'source.patch').read_bytes()==(S/'source-attempt02.patch').read_bytes()
assert sha(O/'source.patch')=='7c3e093695826f92ba09c84907365e3bf473924203a30d4963cda04db590bcdd'
assert sha(O/'source.json')==sha(S/'source-attempt02.json')=='dfa4fb42c7c6601d402d06adc17a4d0efa1208f57cd11658df65d9bb25c74009'
assert len(am['source_sha256'])==67 and 'tests/c/integration.c' not in am['source_sha256']
assert all(source[p]==v for p,v in am['source_sha256'].items())
assert {p for p in am['source_sha256'] if am['source_sha256'][p]!=cm['source_sha256'][p]}==changed
assert source['tests/c/integration.c']==baseline['tests/c/integration.c']
with tarfile.open(O/'source.tar.gz') as t:assert {m.name:hashlib.sha256(t.extractfile(m).read()).hexdigest() for m in t if m.isfile()}==source
scripts=load(O/'tool-source.json');assert len(scripts)==6 and scripts==load(B/'tool-source.json') and all(sha(P/p)==v for p,v in scripts.items())
assert am['script_sha256']==cm['script_sha256']=={Path(p).name:h for p,h in scripts.items()}
for path,h in scripts.items():
 blob=subprocess.check_output(['git','show','144e69e08c5462217d54791374e579e3ef390a87:'+path],cwd=W)
 assert hashlib.sha256(blob).hexdigest()==h
assert {p.name for p in (P/'tools/pgo').iterdir() if p.is_file()}=={Path(p).name for p in scripts}
pipeline_command=next(x['argv'] for x in a['commands'] if x['label']=='pgo-pipeline')
assert pipeline_command[3]==str(P/'tools/pgo/build.py')
assert pipeline_command[pipeline_command.index('--source')+1]==str(W)
assert am['invocation'][1]==str(P/'tools/pgo/build.py') and am['source']==str(W)
for phase in ('generate','use'):
 command=next(x for x in am['commands'] if x['label']==phase)
 assert command['argv'][3]==str(P/'tools/pgo/train.py')
normal_replay=next(x['argv'] for x in a['commands'] if x['label']=='normal-generated-replay')
assert normal_replay[3]==str(P/'tools/pgo/train.py')
prepared=load(O/'adapter-preparation.json')
assert sha(O/'independent-source-review.json')=='647539d5117bc2f55d4a2315f43e59ac100ff530c5143e8f9d15def475da721e'
assert sha(O/'independent-source-review-attempt02.json')=='b11f95a8427ec3d798f342671fdd84b351037638db7c7e5ba9768d53913951f6'
assert sha('/tmp/oriole-deferred-c-text-raw-preparation-independent-review.json')=='52f025aaa81eb1f2cd06381389551145eb6cfffe0db1db57a7525266d002bb5b'
delta=load(O/'source-delta.json')
assert delta['status']=='reviewed' and set(delta['changed_files'])==changed and delta['runtime_changed_files']==runtime
assert delta['source_manifest_sha256']==sha(O/'source.json') and delta['control_source_manifest_sha256']==sha(B/'source.json')
for name,digest in a['normal_libraries'].items():assert sha(O/'normal'/name)==digest
for name,digest in am['libraries_sha256'].items():assert sha(ar/name)==digest
checks=load(S/'checks-attempt02/results.json');assert checks['status']=='passed' and checks['source_unchanged']
assert sha(O/'source-checks.json')==sha(S/'checks-attempt02/results.json')
focused_path=Path('/tmp/oriole-deferred-c-text-raw-checks-independent-review.json')
assert sha(focused_path)=='9e6a399dc0b9c95db2561f53d2c1ca59f27c4d478d2d8ab7a79fdb26cf38d1aa'
focused=load(focused_path)
assert focused['status']=='passed_saved_data_review' and focused['results_sha256']==sha(O/'source-checks.json')
assert focused['source_manifest_sha256']==sha(O/'source.json') and focused['total_passed']==226
for row in checks['commands']:
 assert row['exit']==0
 for stream in ('stdout','stderr'):assert sha(S/'checks-attempt02'/(row['label']+'.'+stream))==row[stream+'_sha256']
native=[]
for mode,library,expected_hash in [('normal',O/'normal/liboriole_expat.so','47bd479857f6b14006a9ec1cf60419a89ab5a32f0d4ac701a4841ce6f0ace253'),('pgo',ar/'use/liboriole_expat.so','d3a1851bf87bce9558366cf99bd81cddb1fc0773ac755674c4cd6ec12467762d')]:
 root=Path('/tmp/oriole-deferred-c-text-raw-'+mode+'-native-study')
 template=load(root/'template-preparation.json');binding=load(root/'adaptation.json')
 assert binding['status']=='passed' and binding['prepared_not_executed'] is True
 assert binding['source_manifest_sha256']==sha(O/'source.json') and binding['build_freeze_sha256']==args.expected_freeze
 assert binding['template_preparation_sha256']==sha(root/'template-preparation.json')
 assert sha(root/'native_screen.py.in')==template['template_sha256']
 assert sha(root/'run.py')==sha(root/'run.py.in')==template['wrapper_template_sha256']==binding['wrapper_sha256']
 assert binding['candidate_library']=={'path':str(library),'sha256':expected_hash} and sha(library)==expected_hash
 assert binding['control_library']==template['control_library'] and sha(binding['control_library']['path'])==binding['control_library']['sha256']
 text=(root/'native_screen.py.in').read_text();assert text.count(template['placeholder'])==1
 assert text.replace(template['placeholder'],str(library))==(root/'native_screen.py').read_text()
 assert sha(root/'native_screen.py')==binding['controller_sha256']
 assert sha(S/'seal_native.py')==binding['seal_controller_sha256']
 native.append({'mode':mode,'adaptation_sha256':sha(root/'adaptation.json'),'controller_sha256':binding['controller_sha256'],'wrapper_sha256':binding['wrapper_sha256'],'candidate_library':binding['candidate_library'],'control_library':binding['control_library']})
result={
 'status':'passed',
 'role':'Independent saved raw source/compiler/training/profile/library audit; runtime implementer, build adapter author and build collector are other agents. Reviewer proposed the earlier deferred-raw design and independently reviewed prepared controllers and focused outputs. No target execution.',
 'source_sha256':sha(O/'source.json'),'patch_sha256':sha(O/'source.patch'),'base_commit':base,
 'runtime_delta':runtime,'test_only_files':sorted(changed-set(runtime)),
 'source_review_sha256':sha(O/'independent-source-review.json'),'source_review_supplement_sha256':sha(O/'independent-source-review-attempt02.json'),
 'focused_checks':{'receipt_sha256':sha(S/'checks-attempt02/results.json'),'tests':226,'independent_review_sha256':sha(focused_path),'first_attempt':'Initial new compaction oracle failure retained; test/comment-only correction, not a runtime retry.'},'native_preparation_bindings':native,
 'freeze_sha256':sha(O/'pair-freeze.json'),'frozen_files':f['count'],'pair_sha256':sha(O/'pair-report.json'),
 'actual_workspace_compilations':counts,
 'compiler_comparison':'All nine complete actual vectors match O3 Finder control after only exact private build/run paths and Cargo artifact suffixes. Metadata, target, opt-level3, PIC/unwind/cgu1/ThinLTO arguments and order remain significant.',
 'external_pipeline':{'directory':str(P/'tools/pgo'),'git_commit':'144e69e08c5462217d54791374e579e3ef390a87','exact_six_hashes':scripts,'actual_build_and_three_training_invocations_verified':True,'pipeline_scope':'Exact six historical Git144 helpers; no B or additive real-input runner. Live production pipeline was not executed.'},
 'candidate_generated_records':864,'training':training,'fresh_profiles':am['profiles_sha256'],
 'normal_libraries':a['normal_libraries'],'pgo_use_libraries':{p:v for p,v in am['libraries_sha256'].items() if p.startswith('use/')},
 'blockers':[],
 'tools_current_bytes_verified':am['tools_sha256'],
 'limitations':['Ohm build and generated-only replay, not production stable verification.','Focused checks and build provenance only; no full compatibility, code-generation improvement or elapsed performance claim.','Native scripts are reviewed bindings only; this audit does not inspect or claim any native elapsed outcome.'],
 'reviewer_sha256':sha(__file__),
}
out=Path('/tmp/oriole-deferred-c-text-raw-build-independent-review.json');out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'path':str(out),'sha256':sha(out),'status':'passed'}))
