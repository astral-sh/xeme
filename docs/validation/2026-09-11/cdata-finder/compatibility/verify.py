"""Verify portable archive/member hashes; never build or execute archived targets."""
from pathlib import Path,PurePosixPath
import gzip,hashlib,json,sys,tarfile
root=Path(__file__).resolve().parent
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report=json.loads((root/'report.json').read_text());index=json.loads(gzip.decompress((root/'members.json.gz').read_bytes()))
assert h(root/report['archive']['path'])==report['archive']['sha256'] and h(root/report['index']['path'])==report['index']['sha256']
with tarfile.open(root/report['archive']['path']) as archive:
 members=archive.getmembers();assert len(members)==report['archive']['members']==len(index['members']) and len({m.name for m in members})==len(members)
 assert {m.name for m in members}==set(index['members'])
 for m in members:
  path=PurePosixPath(m.name);assert not path.is_absolute() and '..' not in path.parts and m.isfile()
  meta=index['members'][m.name];assert m.size==meta['bytes'] and hashlib.sha256(archive.extractfile(m).read()).hexdigest()==meta['sha256']
if '--check-origins' in sys.argv[1:]:
 for meta in [*index['members'].values(),*index['excluded'].values()]:assert h(meta['origin'])==meta['sha256']
print(json.dumps({'status':'passed','members':len(index['members']),'excluded':len(index['excluded']),'origins_checked':'--check-origins' in sys.argv[1:]}))
