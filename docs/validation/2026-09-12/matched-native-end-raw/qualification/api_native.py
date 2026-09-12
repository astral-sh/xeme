"""Strict original API and current C sanitizer consumers; all children on CPU3."""
if not __debug__: raise RuntimeError('Assertions required')
from pathlib import Path
import hashlib,json
_pins=json.loads((Path(__file__).parent/'pins.json').read_text())
assert all(hashlib.sha256(Path(p).read_bytes()).hexdigest()==h for p,h in _pins.items()), 'Prepared inputs changed'

from pathlib import Path
import hashlib,json,os,shutil,signal,subprocess,time
R=Path('/tmp/oriole-matched-native-end-raw-correctness'); W=Path('/home/dev-user/code/oss/oriole-matched-native-end-raw'); L=Path('/tmp/oriole-matched-native-end-raw-study/normal'); O=R/'api-native'; O.mkdir(exist_ok=False)
B=Path('/tmp/oriole-reference-frame-api-gates/api-native/upstream-api'); T=O/'tools';T.mkdir()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for n in ['run.py','runner.c','bridge.c','bridge.h','memcheck.patch','README.md','allocator_repro.c']:shutil.copyfile(W/'tools/upstream-expat'/n,T/n)
shutil.copyfile('/tmp/oriole-unique-accounting-evidence/api-tools/run_clean.py',O/'run_clean.py')
shutil.copyfile(B/'adapted/expat_config.h',O/'expat_config.h')
observed=[*T.iterdir(),O/'run_clean.py',O/'expat_config.h',L/'liboriole_expat.so',L/'liboriole_expat.a',B/'results.json',B/'manifest.json',W/'crates/oriole/src/lib.rs']
report={'status':'incomplete','scope':'Original4740 unchanged assertions/ceilings:3s per case,1GiB AS,768MiB RSS,240s total. Native C ASan/UBSan only; Rust release library uninstrumented; leak checking disabled.','before':{str(p):sha(p) for p in observed},'commands':[]}
def save(): (O/'report.json').write_text(json.dumps(report,indent=2)+'\n')
def run(label,args,timeout,env=None):
 row={'label':label,'command':['taskset','-c','3',*args],'timeout':timeout,'start':time.time()};report['commands'].append(row);save()
 with (O/(label+'.stdout')).open('wb') as out,(O/(label+'.stderr')).open('wb') as err:
  p=None
  try:
   p=subprocess.Popen(row['command'],stdout=out,stderr=err,env=env,start_new_session=True)
   row['exit']=p.wait(timeout=timeout)
  except BaseException as e:
   row['exception']=repr(e)
   if p is not None:
    if p.poll() is None:os.killpg(p.pid,signal.SIGKILL)
    row['exit']=p.wait()
   raise
  finally:
   row['reaped']=p is not None and p.poll() is not None;row['end']=time.time();out.flush();err.flush();row['stdout_sha256']=sha(O/(label+'.stdout'));row['stderr_sha256']=sha(O/(label+'.stderr'));save()
 print(label,row['exit'],flush=True);return row['exit']
try:
 py='/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3.12'
 code=run('api',[py,'-I','-S',str(O/'run_clean.py'),str(T/'run.py'),'--source','/home/dev-user/.cache/oriole/upstream/expat-2.8.4','--config',str(O/'expat_config.h'),'--library',str(L/'liboriole_expat.so'),'--output',str(O/'upstream-api'),'--test-timeout','3','--memory-mib','1024','--rss-mib','768','--timeout','240'],300)
 assert code in [0,1]
 a=json.loads((B/'results.json').read_text());b=json.loads((O/'upstream-api/results.json').read_text())
 am=json.loads((B/'manifest.json').read_text());bm=json.loads((O/'upstream-api/manifest.json').read_text())
 assert b['selection_complete'] and b['library_origin_verified'] and not b['timed_out']
 assert len(a['results'])==len(b['results'])==4740
 norm=lambda m,p:{k:([x.replace(str(p),'<OUTPUT>') for x in v] if k=='compile_command' else v) for k,v in m.items() if k not in ['library_sha256','binary_sha256']}
 assert norm(am,B)==norm(bm,O/'upstream-api')
 aa={(r['test'],r['context']):r for r in a['results']};bb={(r['test'],r['context']):r for r in b['results']};assert aa.keys()==bb.keys()
 changes=[{'baseline':aa[k],'candidate':bb[k]} for k in aa if aa[k]!=bb[k]]
 comparison={'baseline':str(B),'baseline_results_sha256':sha(B/'results.json'),'candidate_results_sha256':sha(O/'upstream-api/results.json'),'passed':b['passed'],'failed':b['failed'],'all4740rows_exact':not changes,'changes':changes,'manifests_equal_except_library_binary_and_output_paths':True}
 (O/'api-comparison.json').write_text(json.dumps(comparison,indent=2)+'\n');report['api_comparison']=comparison
 native=O/'native';native.mkdir()
 for n in ['integration','adversarial','allocations']:shutil.copyfile(W/'tests/c'/(n+'.c'),native/(n+'.c'))
 shutil.copyfile(W/'include/expat.h',native/'expat.h')
 report['native_sources']={str(p):sha(p) for p in native.iterdir()}
 for linkage in ['dynamic','static']:
  for name in ['integration','adversarial','allocations']:
   exe=native/(name+'-'+linkage);label='native-'+name+'-'+linkage
   cmd=['cc','-std=c11','-O1','-g','-Wall','-Wextra','-Werror','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I',str(native),str(native/(name+'.c'))]
   cmd+=['-L',str(L),'-loriole_expat','-Wl,-rpath,'+str(L)] if linkage=='dynamic' else [str(L/'liboriole_expat.a'),'-ldl','-lpthread','-lm']
   assert run(label+'-build',cmd+['-o',str(exe)],120)==0
   assert run(label,[str(exe)],120,{**os.environ,'ASAN_OPTIONS':'detect_leaks=0:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'})==0
   report['commands'][-1]['binary_sha256']=sha(exe);save()
 report['status']='passed' if not changes else 'api_differences_native_passed'
finally:
 report['after']={p:sha(p) for p in report['before']};report['unchanged']=report['before']==report['after'];save()
assert report['unchanged']
