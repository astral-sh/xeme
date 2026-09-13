"""Repeat the diagnostic retry512 replay; original upstream failures stay counted."""
import gzip,hashlib,json,os,re,subprocess,sys
from pathlib import Path
base=Path('/home/dev-user/.cache/oriole/allocation-audit/contracts-raised512')
original=Path('/tmp/oriole-upstream-final-values')
output=Path(sys.argv[1]) if len(sys.argv)>1 else original/'allocation-raised512-replay'
output.mkdir(exist_ok=False)
audit=json.load(gzip.open('/home/dev-user/code/oss/oriole/tools/upstream-expat/results/2026-09-10/allocation-audit/audit.json.gz','rt'))
failed=json.loads((original/'failure-diagnostics.json').read_text())['counts']
names=[row['test']for row in audit['tests']if row['test']in failed]
lib=Path('/tmp/oriole-values-final-source/liboriole_expat.so'); binary=base/'runtests'
env=dict(os.environ,LD_PRELOAD=str(lib),ORIOLE_SELECTED_TESTS=','.join(names),ORIOLE_CHUNK_MASK='63',ORIOLE_DEFERRAL_MASK='3',ORIOLE_MEMORY_MIB='4096',ORIOLE_RSS_MIB='768',ORIOLE_TEST_TIMEOUT='15')
with (output/'tests.log').open('w')as log: process=subprocess.run(['taskset','-c','4',str(binary),'--verbose'],env=env,stdout=log,stderr=subprocess.STDOUT,timeout=120)
text=(output/'tests.log').read_text();rows=[dict(context=c,test=t,outcome=o,code=int(n))for c,t,o,n in re.findall(r'^ORIOLE_RESULT\t([^\t]+)\t([^\t]+)\t([^\t]+)\t(\d+)$',text,re.M)]
origins=re.findall(r'^ORIOLE_LIBRARY\t(.+)$',text,re.M);assert origins==[str(lib)]
report={'candidate_sha256':hashlib.sha256(lib.read_bytes()).hexdigest(),'runner_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'library_origins':origins,'selected_names':names,'passed':sum(r['outcome']=='pass'for r in rows),'failed':sum(r['outcome']!='pass'for r in rows),'results':rows}
assert len(rows)==len(names)*12
(output/'results.json').write_text(json.dumps(report,indent=2)+'\n');print(report['passed'],report['failed'])
