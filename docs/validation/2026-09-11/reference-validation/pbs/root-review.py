from pathlib import Path
import gzip,hashlib,io,json,tarfile,zipfile
p=Path('/tmp/oriole-reference-frame-pbs-package')
sha=lambda b:hashlib.sha256(b).hexdigest()
for f in json.loads((p/'files.json').read_text())['files']:
    data=(p/f['name']).read_bytes()
    assert (len(data),sha(data))==(f['bytes'],f['sha256'])
i=json.loads((p/'archive-members.json').read_text())
data=(p/i['archive']).read_bytes()
assert len(data)==i['bytes'] and sha(data)=='db6445709c231d29216183723c1349b63645d4b54b3dcd4851d6fd31ff2cf5e7'
expected={r['name']:r for r in i['members']}
assert len(expected)==len(i['members'])==149
members={}
with tarfile.open(fileobj=io.BytesIO(data),mode='r:gz') as t:
    for m in t:
        assert m.isfile() and m.name in expected and m.name not in members
        assert not m.name.startswith('/') and '..' not in Path(m.name).parts
        b=t.extractfile(m).read(); r=expected[m.name]
        assert (len(b),sha(b))==(r['bytes'],r['sha256'])
        if 'path' in r['origin']:
            physical=Path(r['origin']['path']).read_bytes()
            assert (len(physical),sha(physical))==(r['origin']['source_bytes'],r['origin']['source_sha256'])
            if 'transformation' in r['origin']:
                assert r['origin']['transformation']=='gzip level 3, mtime 0'
                assert gzip.decompress(b)==physical
            else:
                assert b==physical
        members[m.name]=b
assert set(members)==set(expected)
zips=[(n,b) for n,b in members.items() if n.endswith('.zip')]
assert len(zips)==1
name,data=zips[0]
assert sha(data)=='7f29ea2ebb4afee2f422da6109bbefd4cf5442ff274adce45e6ef92b22b6fbf0'
with zipfile.ZipFile(io.BytesIO(data)) as z:
    assert len(z.infolist())==42 and z.testzip() is None
r=json.loads((p/'report.json').read_text())
assert r['run']['head']=='4064b0653534aff690c0e0f395a6b3b48da878fc' and r['run']['conclusion']=='failure'
assert r['outcomes']['failure_methods']==['test.test_pyexpat.BufferTextTest.test1','test.test_sax.CDATAHandlerTest.test_handlers']
assert r['outcomes']['glibc217']['thread_probe']['threaded_parses']==1024
first=Path('/tmp/oriole-reference-frame-pbs-root-package-review-first-failure.json')
out={'status':'passed','reader_sha256':sha(Path(__file__).read_bytes()),'first_failure_sha256':sha(first.read_bytes()),'reader_correction':'Initial inline reader compared compressed members directly to uncompressed physical origins. This version verifies the four explicitly indexed gzip transformations before byte comparison. No artifact, target or producer rerun.','archive_sha256':sha((p/'evidence.tar.gz').read_bytes()),'members':len(members),'validation_zip_members':42,'report_sha256':sha((p/'report.json').read_bytes()),'scope':'Root separately rehashed all top-level files, indexed archive members and physical origins; checked nested original validation ZIP and selected source/outcomes. Saved bytes only.'}
Path('/tmp/oriole-reference-frame-pbs-root-package-review.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out))
