from pathlib import Path
import hashlib,json,os,signal,subprocess,sys,time
root=Path(__file__).parent
commands=json.loads((root/'commands.json').read_text())
report={'status':'incomplete','commands':[],'started':time.time()}
def save():(root/'execution.json').write_text(json.dumps(report,indent=2)+'\n')
def run(label):
 command=commands[label];row={'label':label,'command':command,'start':time.time()};report['commands'].append(row);save()
 with (root/(label+'.log')).open('wb') as log:
  p=subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  try:
   row['exit']=p.wait(timeout=1950)
  except BaseException as e:
   row['exception']=repr(e)
   try:os.killpg(p.pid,signal.SIGKILL)
   except ProcessLookupError:pass
   row['exit']=p.wait();raise
  finally:row['end']=time.time();row['log_sha256']=hashlib.sha256((root/(label+'.log')).read_bytes()).hexdigest();save()
 assert row['exit']==0,(label,row['exit'])
 print(label,'passed',flush=True)
try:
 for label in ['host_before_native','native','native_readback','python_prepare_only_after_root_native_decision','python_preflight_readback','host_before_python','python_time','python_readback']:run(label)
 report['status']='passed'
finally:report['ended']=time.time();save()
