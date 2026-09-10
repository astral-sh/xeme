import os,re,json,hashlib,subprocess
from pathlib import Path
out=Path('/tmp/oriole-foreign-dtd/upstream-focused');out.mkdir(exist_ok=True)
source=Path('/tmp/oriole-upstream-final-values'); manifest=json.loads((source/'manifest.json').read_text()); names=[n for n in manifest['public_tests_in_source']if 'foreign_dtd' in n or 'not_standalone' in n];lib=Path('/home/dev-user/.cache/oriole/foreign-dtd-target/debug/liboriole_expat.so');binary=source/'runtests'
env=dict(os.environ,LD_PRELOAD=str(lib),ORIOLE_SELECTED_TESTS=','.join(names),ORIOLE_CHUNK_MASK='63',ORIOLE_DEFERRAL_MASK='3',ORIOLE_MEMORY_MIB='4096',ORIOLE_RSS_MIB='768',ORIOLE_TEST_TIMEOUT='15')
with (out/'tests.log').open('w')as f:r=subprocess.run(['taskset','-c','5',str(binary),'--verbose'],env=env,stdout=f,stderr=subprocess.STDOUT,timeout=120)
text=(out/'tests.log').read_text(); rows=[dict(context=c,test=t,outcome=o,code=int(n))for c,t,o,n in re.findall(r'^ORIOLE_RESULT\t([^\t]+)\t([^\t]+)\t([^\t]+)\t(\d+)$',text,re.M)]
origins=re.findall(r'^ORIOLE_LIBRARY\t(.+)$',text,re.M);assert origins==[str(lib)]
report={'library':str(lib),'library_sha256':hashlib.sha256(lib.read_bytes()).hexdigest(),'runner':str(binary),'runner_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'unchanged_upstream_adapter_manifest':str(source/'manifest.json'),'selected_names':names,'selection_complete':len(rows)==len(names)*12,'library_origins':origins,'returncode':r.returncode,'passed':sum(r['outcome']=='pass'for r in rows),'failed':sum(r['outcome']!='pass'for r in rows),'results':rows}
(out/'results.json').write_text(json.dumps(report,indent=2)+'\n');print({k:v for k,v in report.items()if k not in ('results',)})
