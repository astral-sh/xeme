from pathlib import Path
import hashlib,json,os,signal,subprocess,time
P=Path(__file__).parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert json.loads((P/'adaptation.json').read_text())['status']=='passed'
report={'status':'incomplete','commands':[],'controller_sha256':sha(P/'native_screen.py'),'wrapper_sha256':sha(__file__),'scope':'Sequential preflightCPU5 then exclusive native elapsedCPU0; one attempt each, original90-second workers and1200-second aggregate timing bound. Controller process-group cleanup retains failure logs; no tuning/rerun.'}
def save():(P/'controller.json').write_text(json.dumps(report,indent=2)+'\n')
try:
 for phase,cpu,timeout in [('preflight',5,600),('time',0,1200)]:
  cmd=['taskset','-c',str(cpu),'python3',str(P/'native_screen.py'),phase];row={'phase':phase,'command':cmd,'start':time.time(),'timeout':timeout};report['commands'].append(row);save();proc=None
  with (P/(phase+'-controller.log')).open('wb') as log:
   try:
    proc=subprocess.Popen(cmd,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);row['exit']=proc.wait(timeout=timeout)
    if row['exit']:raise RuntimeError(phase+' failed')
   except BaseException as e:
    row['exception']=repr(e)
    if proc is not None:
     try:os.killpg(proc.pid,signal.SIGKILL)
     except ProcessLookupError:pass
     row['exit']=proc.wait()
    raise
   finally:log.flush();row['end']=time.time();row['log_sha256']=sha(P/(phase+'-controller.log'));save()
  print(phase,row['exit'],flush=True)
 report['status']='passed'
finally:save()
