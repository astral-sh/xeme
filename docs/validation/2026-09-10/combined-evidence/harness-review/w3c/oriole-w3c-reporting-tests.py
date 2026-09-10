import importlib.util,json,subprocess,sys
from pathlib import Path
from unittest.mock import patch
spec=importlib.util.spec_from_file_location('harness','/tmp/oriole-w3c-run.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
rows=[]
for kind in ['timeout','launch','exit','missing_output','malformed_output']:
 out=Path('/tmp/oriole-w3c-reporting-'+kind)
 def fake(command,**kwargs):
  if kind=='timeout':raise subprocess.TimeoutExpired(command,120)
  if kind=='launch':raise OSError('injected launch failure')
  if kind=='exit':return subprocess.CompletedProcess(command,17)
  if kind=='malformed_output':(out/'reference.json').write_text('{broken')
  return subprocess.CompletedProcess(command,0)
 args=['/tmp/oriole-w3c-run.py','--suite','/tmp/oriole-w3c-cli-review-fixture','--library','/tmp/oriole-values-final-source/liboriole_expat.so','--reference','/home/dev-user/.cache/oriole/expat-build/libexpat.so','--output',str(out)]
 with patch.object(sys,'argv',args),patch.object(module.subprocess,'run',fake): status=module.main()
 report=json.loads((out/'summary.json').read_text());assert status==1 and 'reference' in report['worker_failures'];assert not report['engines'];assert len(report['commands'])==1
 assert report['commands'][0]['timed_out']==(kind=='timeout')
 rows.append({'case':kind,'status':status,'failure':report['worker_failures']['reference'],'command':report['commands'][0]})
Path('/tmp/oriole-w3c-reporting-tests.json').write_text(json.dumps(rows,indent=2)+'\n');print('reporting failures recorded',len(rows))
