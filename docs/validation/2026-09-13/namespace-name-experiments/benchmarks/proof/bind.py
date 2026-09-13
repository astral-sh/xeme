"""Pin an explicitly released candidate; no compilation or target execution."""
from pathlib import Path
import argparse, hashlib, json
p=argparse.ArgumentParser()
p.add_argument('--library',type=Path,required=True)
p.add_argument('--sha256',required=True)
p.add_argument('--build-record',type=Path,required=True)
a=p.parse_args();root=Path(__file__).parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert not (root/'binding.json').exists()
prep=json.loads((root/'preparation.json').read_text())
for path,h in prep['pins'].items():assert sha(path)==h,path
assert a.library.name=='liboriole_expat.so' and sha(a.library)==a.sha256
assert len(a.sha256)==64
record=json.loads(a.build_record.read_text())
assert record['status']=='passed'
libraries={
 'control':{'path':'/tmp/oriole-native-start-end-namespace-declaration-study/normal/liboriole_expat.so','sha256':'c3e6533900cf0b1ec6b127fd173f7e25be210cb5afe23ad9963c84873f6ea025'},
 'candidate':{'path':str(a.library.resolve()),'sha256':a.sha256},
 'expat':{'path':'/tmp/oriole-pgo-study/expat-control-liboriole_expat.so','sha256':'7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478'}}
for v in libraries.values():assert sha(v['path'])==v['sha256']
binding={'status':'passed','mode':'normal','libraries':libraries,
 'build_record':{'path':str(a.build_record.resolve()),'sha256':sha(a.build_record)},
 'preparation_sha256':sha(root/'preparation.json'),
 'consumer_directories':{
  'control':'/tmp/oriole-native-start-end-namespace-declaration-study/python/consumers/candidate',
  'expat':'/tmp/oriole-expanded-start-capacity-study/python/consumers/expat',
  'candidate':str(root/'python/consumers/candidate')},
 'scope':'Root explicitly releases a passing build and supplies its exact library hash. This command verifies saved bytes; it does not reconstruct compiler provenance.'}
(root/'binding.json').write_text(json.dumps(binding,indent=2)+'\n')
print(json.dumps({'path':str(root/'binding.json'),'sha256':sha(root/'binding.json'),'libraries':libraries},indent=2))
