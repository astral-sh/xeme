from pathlib import Path
import hashlib,json,subprocess,time
O=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
receipt={'started_utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'command':['taskset','-c','4','python3',str(O/'generator.py')],'scope':'Saved C-gate audit only; no target execution.'}
with (O/'attempt-01.stdout').open('xb') as out,(O/'attempt-01.stderr').open('xb') as err:p=subprocess.run(receipt['command'],stdout=out,stderr=err,timeout=120)
receipt['exit']=p.returncode;receipt['generator_sha256']=sha(O/'generator.py')
for s in ['stdout','stderr']:receipt[s+'_sha256']=sha(O/f'attempt-01.{s}')
(O/'attempt-01.json').write_text(json.dumps(receipt,indent=2)+'\n');print((O/'attempt-01.stdout').read_text());print((O/'attempt-01.stderr').read_text());raise SystemExit(p.returncode)
