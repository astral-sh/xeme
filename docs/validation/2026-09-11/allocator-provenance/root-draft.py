"""Publish the recorded allocator study without executables or build caches."""
from pathlib import Path
import gzip, hashlib, io, json, statistics, tarfile

ROOT=Path('/tmp/oriole-allocator-provenance-publication')
sha=lambda data:hashlib.sha256(data).hexdigest()
load=lambda p:json.loads(Path(p).read_text())

def main():
    assert not ROOT.exists()
    studies={name:Path('/tmp')/('oriole-'+name) for name in [
      'allocator-provenance-study','allocator-provenance-pgo-study',
      'allocator-provenance-build-independent-review','allocator-provenance-native-independent-review','allocator-provenance-compatibility-independent-review',
      'allocator-provenance-thin-pgo-gates','allocator-provenance-thin-pgo-cpython-gates',
      'allocator-provenance-thin-pgo-cpython-preparation','allocator-provenance-strict-outcome-review',
      'allocator-provenance-native-study']}
    checks=load(studies['allocator-provenance-study']/'checks-fix01/results.json')
    ci=load(studies['allocator-provenance-study']/'fixed-ci.json')
    assert ci['conclusion']=='success' and ci['headSha']=='e1d263a711e862e4f0f0018b15de9ac83a87a342'
    assert len(ci['jobs'])==14 and all(x['conclusion']=='success' for x in ci['jobs'])
    assert load(studies['allocator-provenance-pgo-study']/'pair-report.json')['status']=='passed'
    assert load(studies['allocator-provenance-thin-pgo-gates']/'summary.json')['api']['all4740rows_exact']
    native={}
    for mode in ['normal','pgo']:
        p=studies['allocator-provenance-native-study']/mode
        assert load(p/'controller.json')['status']=='passed'
        result=load(p/'native-screen/results.json');assert result['status']=='passed'
        groups={}
        for group,generated,count in [('real',False,24),('generated',True,4)]:
            rows=[r for r in result['summary'] if r['condition']['name'].startswith('generated-')==generated]
            assert len(rows)==count
            groups[group]={'conditions':count,'ratios':{k:statistics.geometric_mean(x['median_ratios'][k] for x in rows) for k in rows[0]['median_ratios']},'adverse_conditions':[x for x in rows if x['median_ratios']['candidate_over_control']>1]}
        native[mode]={'results_sha256':sha((p/'native-screen/results.json').read_bytes()),'groups':groups}
    ROOT.mkdir()
    sources={}
    for prefix,folder in studies.items():
        for p in sorted(folder.rglob('*')):
            if p.is_file() and '__pycache__' not in p.parts and 'targets' not in p.parts:
                sources[prefix+'/'+str(p.relative_to(folder))]=p
    for name in ['oriole-allocation-prefix-independent-review.json','oriole-allocator-provenance-fix-independent-review.json','oriole-allocator-provenance-cgate-outcome-review.json']:
        sources['reviews/'+name]=Path('/tmp')/name
    sources['reproduce/package.py']=Path(__file__)
    files=[];excluded=[];objects={}
    for logical,p in sources.items():
        data=p.read_bytes();item={'path':logical,'origin':str(p),'sha256':sha(data),'bytes':len(data)}
        if data.startswith((b'\x7fELF',b'!<arch>\n')) or p.suffix in {'.pyc','.o','.rlib','.rmeta'}:
            excluded.append(item|{'reason':'Executable, compiled archive or build output; identity retained.'});continue
        objects.setdefault(item['sha256'],data);files.append(item)
    index={'format':'sha256-object-archive-v1','files':files,'excluded_binaries':excluded,'objects':len(objects)}
    index_bytes=(json.dumps(index,indent=2)+'\n').encode()
    with (ROOT/'evidence.tar.gz').open('wb') as raw:
        with gzip.GzipFile(fileobj=raw,mode='wb',mtime=0,filename='') as gz:
            with tarfile.open(fileobj=gz,mode='w') as bundle:
                for name,data in [('index.json',index_bytes),*[(f'objects/{h}',data) for h,data in sorted(objects.items())]]:
                    info=tarfile.TarInfo(name);info.size=len(data);info.mode=0o644;bundle.addfile(info,io.BytesIO(data))
    (ROOT/'archive-index.json').write_bytes(index_bytes)
    report={'runtime_commit':ci['headSha'],'source_manifest_sha256':sha((studies['allocator-provenance-pgo-study']/'source.json').read_bytes()),'ci':ci,'native':native,'compatibility':load(studies['allocator-provenance-thin-pgo-gates']/'summary.json'),'archive_sha256':sha((ROOT/'evidence.tar.gz').read_bytes()),'logical_files':len(files),'unique_files':len(objects),'scope':'Allocator correctness repair on Finder runtime; optional fresh original-G ThinLTO PGO. Native elapsed only, no fresh CPython elapsed/PBS build claim. Baseline Miri failure and fixed successes retained. Full CPython shared/static raw failures remain in archive. No claim that Expat API matrix is all passing or that Miri proves general soundness.'}
    (ROOT/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'output':str(ROOT),'logical_files':len(files),'unique_files':len(objects),'archive_bytes':(ROOT/'evidence.tar.gz').stat().st_size,'archive_sha256':report['archive_sha256']}))

if __name__=='__main__': main()
