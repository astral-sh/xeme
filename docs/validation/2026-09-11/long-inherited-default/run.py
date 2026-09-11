from pathlib import Path
import hashlib,json,os,signal,subprocess,time
W=Path('/home/dev-user/code/oss/oriole-long-default-regression')
O=Path('/tmp/oriole-long-default-regression/run02');O.mkdir()
B=Path('/tmp/oriole-suffix-element-names-pgo-study')
L=B/'pgo/runs/run-kj_rp37s/use'
E=Path('/tmp/oriole-pgo-study/expat-use-build')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert os.sched_getaffinity(0)=={3}
source=json.loads((B/'source.json').read_text())
assert source['candidate_base_commit']=='a0acaf1bf8c8ae3f55030332be794cb8b9b5bea9'
assert [n for n,h in source['source_sha256'].items() if sha(W/n)!=h]==['tests/c/integration.c']
assert sha(W/'tests/c/integration.c')=='1e77767b31d4aa2c4bf16321dc545f0faa6ce6e401d5080f96993bbca978d399'
expected={L/'liboriole_expat.so':'5872374889ad673fcf7f397057de33a95a1e8f72be591b9bb996b8dbc1fd9280',L/'liboriole_expat.a':'0cfe94b96fce7d8b0063146343c1d95bb8b7728c663a4990abc35c9e4b541605',E/'libexpat.so.1.12.4':'12d33ad26315e8b46a02e598df8581679d0561fb2d98a3d4744e483a573fbdd0'}
assert all(sha(p)==h for p,h in expected.items())
pins={str(p):sha(p) for p in [W/'tests/c/integration.c',W/'tests/c/UPSTREAM-NOTICES.txt',W/'include/expat.h',B/'source.json',Path(__file__),*expected]}
r={'status':'incomplete','pins':pins,'source_commit':source['candidate_base_commit'],'commands':[],'scope':'Focused added C semantic regression plus existing integration assertions; same selected Rust libraries. C ASan/UBSan only; Rust and Expat release libraries uninstrumented. Leak detection disabled; explicit custom allocator balance asserted.'}
def save():(O/'report.json').write_text(json.dumps(r,indent=2)+'\n')
def run(label,cmd,env=None):
 row={'label':label,'argv':[str(x) for x in cmd],'start':time.time(),'timeout_seconds':120};r['commands'].append(row);save()
 with (O/(label+'.log')).open('wb') as log:
  p=subprocess.Popen(row['argv'],cwd=W,stdout=log,stderr=subprocess.STDOUT,env=env,start_new_session=True)
  try:row['exit']=p.wait(timeout=120)
  except BaseException:
   os.killpg(p.pid,signal.SIGKILL);row['exit']=p.wait();raise
  finally:row.update(reaped=p.poll() is not None,end=time.time());save()
 row['log_sha256']=sha(O/(label+'.log'));save();assert row['exit']==0,row
for name in ['expat','shared','static']:
 exe=O/name
 include=Path('/home/dev-user/.cache/oriole/upstream/expat-2.8.4/expat/lib') if name=='expat' else W/'include'
 cmd=['cc','-std=c11','-O1','-g','-Wall','-Wextra','-Werror','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I',include,W/'tests/c/integration.c']
 if name=='expat':cmd+=['-L',E,'-lexpat','-Wl,-rpath,'+str(E)]
 elif name=='shared':cmd+=['-L',L,'-loriole_expat','-Wl,-rpath,'+str(L)]
 else:cmd+=[L/'liboriole_expat.a','-ldl','-lpthread','-lm']
 run(name+'-build',cmd+['-o',exe])
 run(name+'-elf',['readelf','-d',exe]);run(name+'-ldd',['ldd',exe])
 env=os.environ.copy();env.pop('LD_PRELOAD',None);env.pop('LD_LIBRARY_PATH',None)
 env.update(ASAN_OPTIONS='detect_leaks=0:abort_on_error=1',UBSAN_OPTIONS='halt_on_error=1')
 run(name+'-run',[exe],env)
 print(name,'passed',flush=True)
assert all(sha(p)==h for p,h in pins.items())
r['status']='passed';save()
