"""Package selected reference-frame PBS evidence using the prior stable PBS format.

Saved files only; no downloaded interpreter, compiler, parser or container runs.
"""
import gzip,hashlib,io,json,os,tarfile,zipfile
from pathlib import Path
assert os.sched_getaffinity(0)=={6}
ROOT=Path('/tmp/oriole-reference-frame-pbs-results')
PREP=Path('/tmp/oriole-reference-frame-pbs-review-prep')
REVIEW=Path('/tmp/oriole-reference-frame-pbs-publication-prep')
OUTPUT=Path('/tmp/oriole-reference-frame-pbs-package')
SOURCE=Path('/home/dev-user/code/oss/oriole-reference-frame')
def digest(data): return hashlib.sha256(data).hexdigest()
def dump(path,data): path.write_text(json.dumps(data,indent=2,sort_keys=True)+'\n')
def read(path): return json.loads(path.read_bytes())
assert not OUTPUT.exists(),'Keep first package attempts intact.'
review=read(REVIEW/'independent-review.json')
assert review['status']=='passed saved-record review; remote workflow retains two strict XML failures'
assert digest((ROOT/'report.json').read_bytes())==review['report_sha256']=='df565b6fecfbae6045a7b7653fec180047f412b1016fbff942ad50af525cf6e7'
OUTPUT.mkdir()
payloads={};origins={}
def add_bytes(name,data,origin):
    assert name not in payloads and not name.startswith('/') and '..' not in Path(name).parts
    assert not data.startswith((b'\x7fELF',b'!<arch>\n')),'Compiled file: '+name
    payloads[name]=data;origins[name]=origin
def add(name,path,compressed=False):
    data=path.read_bytes()
    origin={'path':str(path),'source_bytes':len(data),'source_sha256':digest(data)}
    if compressed:
        data=gzip.compress(data,compresslevel=3,mtime=0);origin['transformation']='gzip level 3, mtime 0'
    add_bytes(name,data,origin)
for name in ['report.json','run.json','jobs.json','artifacts.json','validation.zip','pbs.yml','test-old-glibc.py','python-dynamic.txt','libpython-dynamic.txt','audit01.stdout','audit01.stderr','inspect01.stdout','inspect01.stderr','decompression.stderr']:
    add(name,ROOT/name)
for name in ['workflow.log','distribution-members.json','python-dynsym.txt','libpython-dynsym.txt']:
    add(name+'.gz',ROOT/name,compressed=True)
for name in ['PLAN.md','audit.py','previous-audit.py','audit-adaptation.patch','inspect_distribution.py','expected-source.json']:
    add('original-review/'+name,PREP/name)
for name in ['README.md','independent-review.json','review.py','review01.stdout','review01.stderr','package.py']:
    add(name,REVIEW/name)
add('package-readback.py',REVIEW/'package-readback.py')
add('previous-stable-package.py',SOURCE/'docs/validation/2026-09-11/pgo-trial-results/package.py')
add('selected-runtime-source.json',Path('/tmp/oriole-reference-frame-pgo-study/source.json'))
for name,pin in sorted(review['source_sha256'].items()):
    path=SOURCE/name;assert digest(path.read_bytes())==pin
    add('source/'+name,path)
for name in ['THIRD_PARTY_LICENSES.md','licenses/expat.txt','licenses/libxml2.txt']:
    assert 'source/'+name not in payloads
    add('source/'+name,SOURCE/name)
# Select only data/source/notices from the independently bound distribution tar.
# Keep all its license files and the two vendored COPYING notices. Binary members
# are represented only by their independently checked inventory hashes.
inventory=read(ROOT/'distribution-members.json')
selected=review['distribution_selected_members']
with (ROOT/'distribution.tar').open('rb') as stream:
    assert hashlib.file_digest(stream,'sha256').hexdigest()==review['distribution_tar_sha256']
selected_data=[]
with tarfile.open(ROOT/'distribution.tar','r:') as archive:
    for item in archive:
        include=(item.name in selected and not selected[item.name]['compiled_binary']) or (item.isfile() and Path(item.name).name=='COPYING')
        if not include: continue
        data=archive.extractfile(item).read()
        if item.name in selected:
            assert len(data)==selected[item.name]['bytes'] and digest(data)==selected[item.name]['sha256']
        add_bytes('selected/'+item.name,data,{'archive':str(ROOT/'distribution.tar'),'archive_sha256':review['distribution_tar_sha256'],'member':item.name,'source_bytes':len(data),'source_sha256':digest(data)})
        selected_data.append(item.name)
assert len(selected_data)==25
raw_members=[]
with zipfile.ZipFile(io.BytesIO(payloads['validation.zip'])) as original:
    assert original.testzip() is None
    for item in original.infolist():
        assert not item.is_dir()
        data=original.read(item.filename)
        assert data==(ROOT/'validation'/item.filename).read_bytes()
        assert not data.startswith((b'\x7fELF',b'!<arch>\n'))
        row={'name':item.filename,'bytes':len(data),'sha256':digest(data)}
        assert review['api_zip_members']['validation.zip/'+item.filename]=={k:row[k] for k in ['bytes','sha256']}
        raw_members.append(row)
assert len(raw_members)==42
add_bytes('validation-members.json',(json.dumps(raw_members,indent=2,sort_keys=True)+'\n').encode(),{'derived_from':'validation.zip','scope':'All42 original ZIP members match saved raw logs, input/profile bytes and records.'})
assert digest(payloads['validation.zip'])=='7f29ea2ebb4afee2f422da6109bbefd4cf5442ff274adce45e6ef92b22b6fbf0'
for name in payloads:
    assert name!='distribution.zip' and not name.endswith(('.so','.a','.tar.zst'))
members=[];buffer=io.BytesIO()
with tarfile.open(fileobj=buffer,mode='w',format=tarfile.PAX_FORMAT) as archive:
    for name,data in sorted(payloads.items()):
        item=tarfile.TarInfo(name);item.size=len(data);item.mode=0o644;item.uid=item.gid=item.mtime=0
        archive.addfile(item,io.BytesIO(data))
        members.append({'name':name,'bytes':len(data),'sha256':digest(data),'origin':origins[name]})
packed=gzip.compress(buffer.getvalue(),compresslevel=3,mtime=0)
(OUTPUT/'evidence.tar.gz').write_bytes(packed)
with tarfile.open(fileobj=io.BytesIO(packed),mode='r:gz') as archive:
    assert archive.getnames()==sorted(payloads)
    for item in archive: assert archive.extractfile(item).read()==payloads[item.name]
report=read(ROOT/'report.json')
dump(OUTPUT/'archive-members.json',{
 'archive':'evidence.tar.gz','bytes':len(packed),'sha256':digest(packed),'members':members,
 'raw_validation_members':len(raw_members),'source_run':report['run']['url'],
 'source_snapshot_files':90,'selected_nonbinary_distribution_members':selected_data,
 'binary_exclusions':report['binary_publication_exclusions']+['distribution.tar'],
 'readback':'Every archive member read back byte-for-byte; all42 original validation ZIP members matched their originals. Full source/support snapshot87 plus three upstream notice files;25 nonbinary distribution members include licenses and COPYING notices.',
})
for name in ['README.md','report.json','independent-review.json','package.py']:
    (OUTPUT/name).write_bytes(payloads[name])
dump(OUTPUT/'files.json',{'files':[{'name':p.name,'bytes':p.stat().st_size,'sha256':digest(p.read_bytes())} for p in sorted(OUTPUT.iterdir()) if p.name!='files.json']})
print(json.dumps({'output':str(OUTPUT),'archive_sha256':digest(packed),'archive_bytes':len(packed),'members':len(members),'nested_raw_members':len(raw_members)},indent=2))
