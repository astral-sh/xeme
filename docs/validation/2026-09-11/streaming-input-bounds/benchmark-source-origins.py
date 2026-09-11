"""Map actual benchmark inputs to package members or explicit excluded binaries."""
from pathlib import Path
import gzip,hashlib,json
repo=Path('/home/dev-user/code/oss/oriole-streaming-input-bounds');doc=repo/'docs/validation/2026-09-11/streaming-input-bounds'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
load=lambda p:json.loads(Path(p).read_text())
index=json.loads(gzip.decompress((doc/'benchmark-evidence-members.json.gz').read_bytes()));by_hash={r['sha256']:r['destination'] for r in index['members']}
rows=[]
for phase in ['normal','pgo']:
 for kind in ['native','python']:
  root=Path('/tmp/oriole-streaming-work-'+phase+'-'+kind+'-study'+('-attempt02' if kind=='python' else ''))
  protocol=root/('native-screen/protocol.json' if kind=='native' else 'screen/preflight.json');p=load(protocol)
  for path,h in p['hashes' if kind=='native' else 'hashes_before'].items():
   assert sha(path)==h
   r={'campaign':kind+'-'+phase,'actual_source_path':path,'sha256':h}
   if h in by_hash:r.update(status='archived_identical_bytes',member=by_hash[h])
   else:
    with Path(path).open('rb') as f:assert f.read(4)==b'\x7fELF',path
    r.update(status='excluded_compiled_binary',reason='Explicit binary exclusion; library/extension/interpreter/driver identity remains in the archived original protocol and worker origin receipts.')
   rows.append(r)
p=doc/'benchmark-source-origins.json';report={'status':'passed','archive_sha256':index['archive_sha256'],'member_index_sha256':sha(doc/'benchmark-evidence-members.json.gz'),'scope':'Every immediate native protocol or Python preflight input hash mapped to identical archived bytes, including equivalent corpus checkout paths, or verified ELF binary excluded from publication. No runtime executed.','rows':rows,'generator_sha256':sha(__file__)};p.write_text(json.dumps(report,indent=2)+'\n');print(str(p),sha(p),len(rows))
