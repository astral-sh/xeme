import hashlib,json,pathlib,signal,subprocess,sys,time,os
root=pathlib.Path('/tmp/oriole-namespace-name-storage-benchmark');commands=json.loads((root/'commands.json').read_text());phase=sys.argv[1]
report_path=root/f'execution-{phase}.json';assert not report_path.exists()
record={'status':'running','commands':[]}
sha=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def save():report_path.write_text(json.dumps(record,indent=2)+'\n')
def run(key, substitutions=None):
 argv=commands[key]
 if substitutions:argv=[substitutions.get(v,v) for v in argv]
 row={'key':key,'argv':argv,'start':time.time()};record['commands'].append(row);save();print(key,flush=True)
 with (root/f'{key}.log').open('xb') as log:
  proc=subprocess.Popen(argv,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  try:row['exit']=proc.wait(timeout=1900)
  except BaseException:
   os.killpg(proc.pid,signal.SIGKILL);row['exit']=proc.wait();raise
  finally:row.update(end=time.time(),reaped=True,pid=proc.pid);save()
 row['log_sha256']=sha(root/f'{key}.log');save()
 if row['exit']:raise RuntimeError((key,(root/f'{key}.log').read_text()[-5000:]))
 if key.startswith('host_'):print((root/f'{key}.log').read_text(),flush=True)
try:
 if phase=='native':
  build=pathlib.Path('/tmp/oriole-namespace-name-storage-study/build.json');data=json.loads(build.read_text());assert data['status']=='passed'
  lib='/tmp/oriole-namespace-name-storage-study/normal/liboriole_expat.so'
  run('bind_after_root_build_release',{'<frozen liboriole_expat.so>':lib,'<candidate sha256>':data['artifacts'][lib],'<passing build JSON>':str(build)})
  run('host_before_native')
 elif phase=='python':
  run('python_prepare_only_after_root_native_decision');run('python_preflight_readback');run('host_before_python')
 else:raise AssertionError(phase)
 key=f'monitor_{phase}_parallel_with_{phase}'
 with (root/f'{key}.log').open('xb') as log:
  proc=subprocess.Popen(commands[key],stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  try:
   run('native' if phase=='native' else 'python_time')
   rc=proc.wait(timeout=20);assert rc==0
  finally:
   if proc.poll() is None:os.killpg(proc.pid,signal.SIGKILL)
   rc=proc.wait();record['monitor']={'argv':commands[key],'exit':rc,'reaped':True,'log_sha256':sha(root/f'{key}.log')};save()
 run(f'{phase}_readback');record['status']='passed'
finally:save()
