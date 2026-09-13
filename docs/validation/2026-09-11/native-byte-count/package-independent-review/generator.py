"""Read archives, recorded origins, Git blobs and saved workspace logs only."""
from pathlib import Path,PurePosixPath
from collections import Counter
import ast,hashlib,io,json,os,re,subprocess,tarfile
assert os.sched_getaffinity(0)=={4}
O=Path(__file__).resolve().parent;W=Path('/home/dev-user/code/oss/oriole-native-byte-count');D=W/'docs/validation/2026-09-11/native-byte-count';PACK=Path('/tmp/oriole-package-native-byte-count.py')
tracked={};checks=[];inventories=[];origins=[];git_reads=[]
def ck(n,v):checks.append({'name':n,'passed':bool(v)});assert v,n
def digest(b):return hashlib.sha256(b).hexdigest()
def read(p):
 p=Path(p);b=p.read_bytes();row={'bytes':len(b),'sha256':digest(b)};assert tracked.setdefault(str(p),row)==row;return b
def sha(p):read(p);return tracked[str(p)]['sha256']
def js(p):return json.loads(read(p))
def binary(b):return 'ELF' if b.startswith(b'\x7fELF') else 'ar' if b.startswith(b'!<arch>\n') else None
def entries(b,label,depth=0):
 ck('bounded archive depth '+label,depth<=10);files={};rows=[]
 with tarfile.open(fileobj=io.BytesIO(b),mode='r:*') as a:
  for m in a.getmembers():
   p=PurePosixPath(m.name);ck('safe unique regular member '+label+'!'+m.name,m.isfile() and not p.is_absolute() and '..' not in p.parts and m.name not in files)
   raw=a.extractfile(m).read();ck('member length '+m.name,len(raw)==m.size);files[m.name]=raw;rows.append({'name':m.name,'bytes':len(raw),'sha256':digest(raw),'binary':binary(raw)})
 inventories.append({'archive':label,'sha256':digest(b),'depth':depth,'members':rows})
 for n,raw in files.items():
  if n.endswith(('.tar.gz','.tgz','.tar.xz','.tar')):entries(raw,label+'!'+n,depth+1)
 return files
def shallow(b):
 with tarfile.open(fileobj=io.BytesIO(b),mode='r:*') as a:return {m.name:a.extractfile(m).read() for m in a.getmembers()}
def verify(row,b,label):ck('row bytes/hash '+label,row['bytes']==len(b) and row['sha256']==digest(b))
def origin(row,label,path=None):
 p=path or row.get('source') or row.get('origin') or row.get('path');ck('origin exists '+label,bool(p));b=read(p);verify(row,b,label);origins.append({'label':label,'source':str(p),'bytes':len(b),'sha256':digest(b)});return b
def git(*args):
 cmd=['git','-C',str(W),*args];b=subprocess.check_output(cmd);git_reads.append({'command':cmd,'bytes':len(b),'sha256':digest(b)});return b
index=js(D/'archive-members.json');summary=js(D/'summary.json');raw=read(D/'evidence.tar.gz')
ck('main archive pin',digest(raw)=='bc63cf87e4733c2b05e7391e0ef782747a6ba0ea1046efae92843a53485b6dd0' and len(raw)==13178538)
main=entries(raw,'evidence.tar.gz');members=index['members'];excluded=index['excluded_binaries']
ck('main exact member and exclusion count/order',len(main)==len(members)==1863 and len(excluded)==17 and list(main)==[r['path'] for r in members] and len({r['path'] for r in members+excluded})==1880)
ck('main archive summary exact',summary['archive']=={'sha256':digest(raw),'bytes':len(raw),'members':1863,'binary_exclusions':17,'all_direct_members_and_origins_read_back':True})
for row in members:
 b=main[row['path']];verify(row,b,row['path']);ck('main origin bytes '+row['path'],b==origin(row,row['path']) and binary(b) is None)
for row in excluded:ck('main exclusion bytes/magic '+row['path'],row['path'] not in main and binary(origin(row,'excluded!'+row['path'])) is not None)
# Evaluate only path/dict/list literals in packaging source; never import or run it.
def value(node,env):
 if isinstance(node,ast.Constant):return node.value
 if isinstance(node,ast.Name):return env[node.id]
 if isinstance(node,ast.Call) and isinstance(node.func,ast.Name) and node.func.id=='Path' and len(node.args)==1 and not node.keywords:return Path(value(node.args[0],env))
 if isinstance(node,ast.BinOp) and isinstance(node.op,ast.Div):return value(node.left,env)/value(node.right,env)
 if isinstance(node,ast.Attribute) and isinstance(node.value,ast.Name) and node.value.id=='args':return env['args'][node.attr]
 if isinstance(node,ast.Dict):return {value(k,env):value(v,env) for k,v in zip(node.keys,node.values,strict=True)}
 if isinstance(node,(ast.List,ast.Tuple)):return [value(v,env) for v in node.elts]
 raise AssertionError(ast.dump(node))
def assignment(code,name,env):
 for n in ast.parse(code).body:
  if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id==name for t in n.targets):return value(n.value,env)
 raise AssertionError(name)
def selection(roots):
 out={}
 for prefix,root in roots.items():
  for p in sorted(Path(root).rglob('*')):
   if p.is_file() and not p.is_symlink() and '__pycache__' not in p.parts:out[prefix+'/'+str(p.relative_to(root))]=str(p)
 return out
rowmap={r['path']:r for r in members};gate_root=Path(rowmap['gates/members.json']['source']).parent;python_review_root=Path(rowmap['python-review/review.json']['source']).parent
env={'source':Path('/tmp/oriole-native-byte-count-study'),'build':Path('/tmp/oriole-native-byte-count-thinlto-study'),'workspace':Path('/tmp/oriole-native-byte-count-workspace-gates'),'gates':Path('/tmp/oriole-native-byte-count-gates'),'native':Path('/tmp/oriole-raw-len-split-thinlto-timing-study'),'python':Path('/tmp/oriole-raw-len-split-thinlto-python-root-study'),'cpython':Path('/tmp/oriole-native-byte-count-thinlto-cpython-gates'),'source_review':Path('/tmp/oriole-native-byte-count-source-build-independent-review'),'cpython_review':Path('/tmp/oriole-native-byte-count-thinlto-cpython-root-review'),'args':{'gates_handoff':gate_root,'python_review':python_review_root},'__file__':str(PACK)}
pack=read(PACK).decode();roots=assignment(pack,'directories',env);controllers=assignment(pack,'controllers',env);copies=assignment(pack,'copies',env)
expected=selection(roots);expected.update({'controllers/'+p.name:str(p) for p in controllers})
ck('complete main source-directory/controller selection',expected=={r['path']:r['source'] for r in members+excluded})
ck('exact archived main packager',main['controllers/'+PACK.name]==read(PACK))
copy_rows=[]
for name,p in copies.items():
 b=read(D/name);ck('top copy '+name,b==read(p));copy_rows.append({'name':name,'origin':str(p),'bytes':len(b),'sha256':digest(b)})
ck('nine original top copies',len(copy_rows)==9)
nested=[]
for prefix,count,excluded_count in [('prototype-native',1038,8),('gates',332,12)]:
 sub=shallow(main[prefix+'/evidence.tar.gz']);rows=json.loads(main[prefix+'/members.json']);omitted=json.loads(main[prefix+'/excluded-binaries.json']);manifest=json.loads(main[prefix+'/manifest.json']);report=json.loads(main[prefix+'/report.json'])
 ck(prefix+' member inventory exact',len(sub)==len(rows)==count and list(sub)==sorted(rows) and len(omitted)==excluded_count)
 for n,row in rows.items():ck(prefix+' nested member origin '+n,sub[n]==origin(row,prefix+'!'+n) and binary(sub[n]) is None)
 for row in omitted:ck(prefix+' nested exclusion magic',binary(origin(row,prefix+'!excluded')) is not None)
 for n,row in manifest.items():verify(row,main[prefix+'/'+n],prefix+' manifest '+n)
 ck(prefix+' manifest six siblings',set(manifest)=={'evidence.tar.gz','members.json','excluded-binaries.json','nested-readback.json','package.py','report.json'})
 archive_report=report['archive'];ck(prefix+' archive reported pin/count',archive_report['sha256']==digest(main[prefix+'/evidence.tar.gz']) and archive_report['bytes']==len(main[prefix+'/evidence.tar.gz']) and archive_report['members']==count)
 readback=[]
 for n,b in sub.items():
  if n.endswith('.tar.gz'):
   inner=shallow(b);readback.append({'archive':n,'members':[{'name':k,'bytes':len(v),'sha256':digest(v)} for k,v in inner.items()]})
   mapname=n[:-len('source.tar.gz')]+'source.json'
   if n.endswith('source.tar.gz') and mapname in sub:ck(prefix+' source archive map '+n,{k:digest(v) for k,v in inner.items()}==json.loads(sub[mapname])['source_sha256'])
 ck(prefix+' complete nested readback exact',readback==json.loads(main[prefix+'/nested-readback.json']) and archive_report['nested_archives']==len(readback) and archive_report['nested_members']==sum(len(r['members']) for r in readback))
 package=main[prefix+'/package.py'].decode()
 if prefix=='gates':
  e={'S':env['source'],'B':env['build'],'G':env['gates'],'W':env['workspace']};nestedroots=assignment(package,'roots',e);selected=selection(nestedroots)
  selected.update({'control-api/results.json':'/tmp/oriole-end-frame-cells-thinlto-gates/api-native/upstream-api/results.json','control-api/manifest.json':'/tmp/oriole-end-frame-cells-thinlto-gates/api-native/upstream-api/manifest.json','control-workspace/tests.log':js(env['workspace']/'workspace-checks/counts.json')['baseline_log'],**{'prototype-build/'+n:'/tmp/oriole-raw-len-split-thinlto-study/'+n for n in ['report.json','source.json','compiler-invocations.json']}})
  ck('gate nested complete source selection',set(selected.values())=={r['source'] for r in rows.values()}|{r['source'] for r in omitted} and {n:p for n,p in selected.items() if n in rows}=={n:r['source'] for n,r in rows.items()})
 else:
  nestedroots=assignment(package,'roots',{});selected=selection(nestedroots)
  selected.update({'control-build/'+n:'/tmp/oriole-end-frame-cells-final-thinlto-study/'+n for n in ['source.json','source.tar.gz','report.json','compiler-invocations.json','release.log','build.py']})
  selected.update({'original-control/'+n:'/tmp/oriole-end-frame-cells-thinlto-study/'+n for n in ['source.json','build.json']})
  corpus=Path('/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json');selected['benchmark-inputs/corpus-manifest.json']=str(corpus)
  for project in js(corpus)['projects']:
   f=next(f for f in project['files'] if f['role']=='input');selected['benchmark-inputs/'+f['path']]=str((corpus.parent/f['path']).resolve())
  selected.update({'benchmark-inputs/rare-declarations.xml':'/tmp/oriole-text-frame-prototype/text/screen/rare-declarations.xml','benchmark-inputs/entities.xml':'/tmp/oriole-grammar-bench-plain/entities.xml','driver/native_driver.c':'/home/dev-user/code/oss/oriole/benchmarks/native_driver.c','driver/original-build.json':'/tmp/oriole-grammar-bench-plain/summary.json'})
  selected_nonbinary={n:p for n,p in selected.items() if not binary(read(p))}
  ck('prototype nested complete member selection',selected_nonbinary=={n:r['source'] for n,r in rows.items()})
  extra_exclusions={'/tmp/oriole-end-frame-cells-final-thinlto-study/liboriole_expat.so','/tmp/oriole-end-frame-cells-final-thinlto-study/liboriole_expat.a','/tmp/oriole-grammar-bench-plain/native-driver','/home/dev-user/.cache/oriole/expat-build/libexpat.so.1.12.4'}
  ck('prototype complete binary selection',{p for n,p in selected.items() if n not in selected_nonbinary}|extra_exclusions=={r['path'] for r in omitted})
 nested.append({'prefix':prefix,'members':count,'exclusions':excluded_count,'nested_archives':len(readback),'nested_source_members':sum(len(r['members']) for r in readback),'all_member_origin_bytes_exact':True,'full_source_selection_checked':True})
source=json.loads(main['source/source.json']);runtime='be22a271f8a5c004d13e515cd4024a68afe57887';parent='079fb66dc159cdc0e1c3ad16397f149751783014'
ck('runtime parent and source identities',summary['runtime_commit']==runtime and summary['parent_commit']==parent and git('rev-parse',runtime+'^').decode().strip()==parent and summary['source_manifest_sha256']==digest(main['source/source.json'])=='735afb9491158c3a34fd376109edc2312727382b5fa61e5e8b084796d7294f1d')
for n,h in source['source_sha256'].items():ck('committed/live source '+n,digest(git('show',runtime+':'+n))==h==sha(W/n))
patch=git('diff',parent,runtime,'--binary');ck('committed exact patch',patch==main['source/candidate.patch'] and digest(patch)==summary['committed_patch_sha256']=='faf56975ae27c070276be8bcc65d6f6178a78f6dcba360902769ec7e26128b68')
for n in ['build/source.tar.gz','source/source.tar.gz','workspace/workspace-checks/source.tar.gz']:ck('integrated archive matches70 source '+n,{k:digest(v) for k,v in shallow(main[n]).items()}==source['source_sha256'])
# Independently rederive the packaged workspace inventory, without running Cargo.
ws=json.loads(main['workspace/summary.json']);wr=json.loads(main['workspace/workspace-checks/report.json']);wc=json.loads(main['workspace/workspace-checks/counts.json'])
ck('workspace source/frozen identities',wr['status']=='passed' and wr['before']==wr['after']==source['source_sha256'] and wr['unchanged'] and wr['frozen_libraries_unchanged'] and wr['frozen_libraries_before']==wr['frozen_libraries_after']==ws['frozen_libraries'])
for p,h in wr['frozen_libraries_before'].items():ck('frozen workspace artifact '+p,sha(p)==h)
commands=[('tests',['test','--release','--locked','--workspace','--all-targets','--no-fail-fast']),('doc-tests',['test','--release','--locked','--workspace','--doc']),('clippy',['clippy','--release','--locked','--workspace','--all-targets','--','-D','warnings']),('fmt',['fmt','--all','--','--check'])]
for row,(label,args) in zip(wr['commands'],commands,strict=True):ck('workspace command '+label,row['label']==label and row['command']==['taskset','-c','3','cargo','+ohm','-Zohm-defaults=no',*args] and row['exit']==0 and row['timeout_seconds']==900 and 'exception' not in row and 0<row['end']-row['start']<900 and row['log_sha256']==digest(main['workspace/workspace-checks/'+label+'.log']))
def inventory(raw):
 rows=[];current=None
 for line in raw.decode().splitlines():
  if line.lstrip().startswith('Running ') and '(' in line:
   ck('no open previous target',current is None);desc=line.split('Running ',1)[1].rsplit(' (',1)[0];target=re.sub(r'-[0-9a-f]{16}$','',Path(line.rsplit('(',1)[1].rstrip(')')).name);current={'target':target,'description':desc,'tests':[]}
  elif line.lstrip().startswith('Doc-tests '):ck('no open doc target',current is None);current={'target':'doc:'+line.split('Doc-tests ',1)[1].strip(),'description':'doc','tests':[]}
  elif line.startswith('test ') and not line.startswith('test result:'):
   ck('named test has target',current is not None);name,result=line[5:].rsplit(' ... ',1);current['tests'].append({'name':name,'result':result})
  m=re.fullmatch(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;.*',line)
  if m:
   ck('result has target',current is not None);current.update(dict(zip(['passed','failed','ignored','measured','filtered'],map(int,m.groups()))));ck('named counts exact',current['passed']==len(current['tests']) and all(t['result']=='ok' for t in current['tests']) and len({t['name'] for t in current['tests']})==len(current['tests']));rows.append(current);current=None
 ck('complete unique targets',current is None and len({(r['target'],r['description']) for r in rows})==len(rows));return rows
actual=inventory(main['workspace/workspace-checks/tests.log']);docs=inventory(main['workspace/workspace-checks/doc-tests.log']);previous=inventory(read(wc['baseline_log']))
ck('all407 tests and33 targets with1 doc test',actual==wc['all_target_rows'] and docs==wc['doc_rows'] and len(actual)==len(previous)==33 and sum(r['passed'] for r in actual)==407 and sum(r['passed'] for r in docs)==1 and all(r[k]==0 for r in actual+docs for k in ['failed','ignored','measured','filtered']))
ck('same exact target/name inventories as baseline',[(r['target'],r['description'],{t['name'] for t in r['tests']}) for r in actual]==[(r['target'],r['description'],{t['name'] for t in r['tests']}) for r in previous] and not wc['added_tests'] and wc['no_removed_or_changed_baseline_tests'] and wc['baseline_sha256']==sha(wc['baseline_log']))
ck('workspace author summary exact',summary['workspace']==ws and ws['all_target_tests']==407 and ws['target_inventory_rows']==33 and ws['doc_tests']==1 and ws['raw_report_sha256']==digest(main['workspace/workspace-checks/report.json']) and ws['actual_counts_sha256']==digest(main['workspace/workspace-checks/counts.json']))
ck('top aggregate copies exact',summary['native']==js(D/'native-summary.json')['groups'] and summary['python']==js(D/'python-summary.json') and summary['api']==js(D/'gates.json')['api'] and summary['libraries']==js(D/'build.json')['libraries'])
binary_members=[{'archive':a['archive'],**m} for a in inventories for m in a['members'] if m['binary']]
outer=[]
for p in sorted(D.iterdir()):
 if p.is_file():b=read(p);outer.append({'path':p.name,'bytes':len(b),'sha256':digest(b)})
sha(Path(__file__));ck('all inspected files unchanged',all({'bytes':Path(p).stat().st_size,'sha256':digest(Path(p).read_bytes())}==v for p,v in tracked.items()))
for n,v in [('checked-files.json',tracked),('checks.json',checks),('archive-inventory.json',inventories),('origin-readback.json',origins),('copies.json',copy_rows),('outer-snapshot.json',outer),('workspace-inventory.json',{'targets':actual,'docs':docs}),('git-reads.json',git_reads)]: (O/n).write_text(json.dumps(v,indent=2)+'\n')
report={'status':'independent_native_byte_count_package_and_workspace_audit_passed','blocking_findings':[],'scope':'Saved archive/member/origin/copy/source/workspace audit; no target, compiler, packager, parser or timing execution. Human prose and later final outer index are separate.','runtime_commit':runtime,'parent_commit':parent,'main_archive':summary['archive'],'main_complete_selection':True,'main_origins_including_exclusions':1880,'nested_handoffs':nested,'recursive_tar_inventory':{'archives':len(inventories),'member_instances':sum(len(a['members']) for a in inventories),'max_depth':max(a['depth'] for a in inventories),'ELF_or_ar_members':binary_members,'scope':'Every .tar.gz/.tgz/.tar.xz/.tar reached by suffix was read, all member bytes hashed. Magic checks apply to those member bytes; no claim about other opaque formats or fresh semantic review of every nested study.'},'committed_live_and_archived_source_inputs':70,'top_original_copies':len(copy_rows),'workspace':{'all_target_tests':407,'target_rows':33,'doc_tests':1,'all_named_results_reconstructed':True,'same_baseline_target_test_names':True,'four_commands_exit0':True,'normal_workspace_distinct_from_C_only_libraries':True},'outer_snapshot':{'file_count':len(outer),'files_json_present':(D/'files.json').exists(),'scope':'Point-in-time file hashes only; later review sidecars and final outer index require a separate readback.'},'preserved_failures':'All selected package origins and nested records retain initial source-verifier, CPU4 preparation/End guard, summary postprocessing and Python directory-collision evidence. Original API393 and strict CPython2 failures remain in their separately reviewed records.','limits':['Direct archive excludes17 binary files; nested handoffs separately record8 and12 exclusions. Recursive tar magic inventory is explicit rather than an assumed blanket binary exclusion.','No new semantic review of nested CPython, instruction or Python timing results; separately sealed review artifacts and original hashes remain.','Workspace407/33/+1 reconstructed from saved raw logs and unchanged baseline inventory. No test or compiler run by this reviewer.','Outer final files index and subsequent attachments are intentionally not certified by this snapshot.'],'checked_files':len(tracked),'origin_rows':len(origins),'passed_checks':len(checks),'all_inspected_files_unchanged':True,'generator_sha256':sha(Path(__file__))}
(O/'review.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'review_sha256':digest((O/'review.json').read_bytes()),'status':report['status'],'checks':len(checks),'files':len(tracked),'origin_rows':len(origins),'recursive_archives':len(inventories),'recursive_members':sum(len(a['members']) for a in inventories),'nested_binaries':len(binary_members)},indent=2))
