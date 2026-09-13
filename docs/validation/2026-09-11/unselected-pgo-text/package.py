"""Package saved evidence only; never load a parser or execute a build."""
from pathlib import Path
import argparse
import csv
import gzip
import hashlib
import io
import json
import os
import tarfile

ROOT = Path(__file__).resolve().parent
TMP = Path('/tmp')
SELECTED = Path('/home/dev-user/code/oss/oriole-cdata-finder')
STUDIES = {
    'additive': TMP/'oriole-real-pgo-training-study',
    'bulk': TMP/'oriole-bulk-pgo-training-study',
    'text/source-study': TMP/'oriole-deferred-c-text-raw-study',
    'text/build': TMP/'oriole-deferred-c-text-raw-pgo-study',
    'text/native-normal': TMP/'oriole-deferred-c-text-raw-normal-native-study',
    'text/native-pgo': TMP/'oriole-deferred-c-text-raw-pgo-native-study',
}
NATIVE = {
    'additive/native/native-screen': ('additive', 'oriole-gr_over_oriole-g', 4),
    'bulk/native/native-screen': ('bulk-native', 'oriole-b_over_oriole-g', 4),
    'text/native-normal/native-screen': ('text-normal', 'candidate_over_control', 3),
    'text/native-pgo/native-screen': ('text-pgo', 'candidate_over_control', 3),
}

def sha(data): return hashlib.sha256(data).hexdigest()
def load(p): return json.loads(Path(p).read_bytes())
def save(p, d):
    with Path(p).open('x') as f: f.write(json.dumps(d, indent=2)+'\n')

def plan():
    """Inventory/hash retained files, verify existing pins, and define exact aliases."""
    files, excluded, aliases, notices = {}, {}, [], []
    expected = {}
    audits = {
        'additive/build-arms-independent-review.json': STUDIES['additive']/'build-arms-independent-review.json',
        'additive/native-independent-review.json': STUDIES['additive']/'native-independent-review.json',
        'bulk/build-arms-independent-review.json': STUDIES['bulk']/'build-arms-independent-review.json',
        'bulk/native-independent-review.json': STUDIES['bulk']/'native-independent-review.json',
        'text/audits/native.json': TMP/'oriole-deferred-c-text-raw-native-independent-review.json',
    }
    for p in audits.values():
        d=load(p)
        for field in ('raw_files_sha256','input_sha256'):
            for name,h in d.get(field,{}).items():
                assert expected.setdefault(name,h)==h, name

    def include(path, logical, pin=None):
        p=Path(path); assert not logical.startswith('/') and '..' not in Path(logical).parts
        data=p.read_bytes(); h=sha(data)
        assert not data.startswith((b'\x7fELF',b'!<arch>\n')), p
        pin=pin or expected.get(str(p))
        if pin: assert h==pin, ('saved hash differs',p,h,pin)
        row={'source':str(p),'bytes':len(data),'sha256':h}
        assert logical not in files or files[logical]==row, logical
        files[logical]=row

    def exclude(path, reason, pin=None):
        p=Path(path)
        excluded[str(p)]={'bytes':p.stat().st_size,'sha256':pin or sha(p.read_bytes()),'reason':reason}

    for logical, directory in STUDIES.items():
        for base, ds, fs in os.walk(directory):
            ds[:]=sorted(d for d in ds if d not in {'targets','uv-tools','__pycache__','.git','generate-build','use-build'})
            for name in sorted(fs):
                p=Path(base)/name; rel=p.relative_to(directory)
                if name.startswith('worker-') and name.endswith('.json') and p.parent.name=='native-screen':continue
                if '.so' in name or p.suffix in {'.a','.o','.rlib','.rmeta'}:
                    exclude(p,'Compiled artifact; hash retained, bytes omitted.');continue
                include(p,logical+'/'+str(rel))
        for builddir in ('generate-build','use-build'):
            for p in directory.glob('expat-*/'+builddir+'/**/CMakeCache.txt'):
                include(p,logical+'/'+str(p.relative_to(directory)))
            for name in ('expat_config.h','flags.make','link.txt'):
                for p in directory.glob('expat-*/'+builddir+'/**/'+name):include(p,logical+'/'+str(p.relative_to(directory)))

    # Each native raw worker is already embedded unchanged in a complete report.
    for prefix,(campaign,primary,nengine) in NATIVE.items():
        for phase in ('preflight','results'):
            container=prefix+'/'+phase+'.json';d=load(files[container]['source'])
            rows=[(['rows',i],x) for i,x in enumerate(d['rows'])] if phase=='preflight' else [(['rows',i,'processes',j],x) for i,row in enumerate(d['rows']) for j,x in enumerate(row['processes'])]
            for index,(pointer,row) in enumerate(rows):
                ordinal=index+(0 if phase=='preflight' else 28*nengine)
                name=f'worker-{ordinal:04}.json';p=Path(files[container]['source']).parent/name
                data=(json.dumps({k:v for k,v in row.items() if k not in ['observations','median_seconds']},indent=2)+'\n').encode()
                assert data==p.read_bytes(),p
                aliases.append({'source':str(p),'logical_path':prefix+'/'+name,'bytes':len(data),'sha256':sha(data),'container':container,'json_pointer_components':pointer,'transform':'native-worker-v1: remove observations and median_seconds, preserve order, json.dumps(indent=2)+newline','byte_equivalence':'verified_exact'})
    assert len(aliases)==3136

    # Selected source is separate from the rejected Text source archive/patch.
    prep=load(STUDIES['additive']/'preparation.json')
    assert prep['runtime_revision']=='78c748d9de5c497858458cbf7a1229148781b2bf'
    assert len(prep['oriole_source_sha256'])==70
    for rel,h in prep['oriole_source_sha256'].items():include(SELECTED/rel,'shared/source-selected/'+rel,h)
    for rel,h in prep['expat_source_sha256'].items():include(TMP/'oriole-pgo-study/expat-source'/rel,'shared/expat-source/'+rel,h)
    for rel in ['LICENSE-APACHE','LICENSE-MIT','THIRD_PARTY_LICENSES.md','tests/c/UPSTREAM-NOTICES.txt']:
        include(SELECTED/rel,'shared/oriole-notices/'+rel);notices.append('shared/oriole-notices/'+rel)
    for p,h in prep['inputs_sha256'].items():
        if '/oriole-real-pgo-training-inputs/' not in p:include(p,'shared/preparation-inputs/'+Path(p).name,h)
    # Exact original G helper source; modified additive and B copies live above.
    base=load(STUDIES['additive']/'base.json')
    for rel,h in base['files_sha256'].items():include(Path(base['source'])/rel,'shared/original-G-pipeline/'+rel,h)

    corpus=Path('/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json')
    include(corpus,'shared/heldout/corpus-manifest.json',prep['heldout_manifest_sha256'])
    for project in load(corpus)['projects']:
        for entry in project['files']:
            if entry['role'] in ('input','license','notice'):
                logical='shared/heldout/'+entry['path'];include(corpus.parent/entry['path'],logical,entry['sha256'])
                if entry['role']!='input':notices.append(logical)
    include(corpus.parent/'README.md','shared/heldout/README.md')
    include(corpus.parent.parent/'native_driver.c','shared/native_driver.c')
    for prefix in NATIVE:
        protocol=load(files[prefix+'/protocol.json']['source'])
        for c in protocol['conditions']:
            if c['name'].startswith('generated-'):include(c['input'],'shared/generated-timing/'+Path(c['input']).name,protocol['hashes'][c['input']])
        for engine,entry in protocol['libraries'].items():exclude(entry['path'],'Compiled parser named in '+prefix+'/'+engine,entry['sha256'])
    # Explicit corpus author boundary excludes rejected unlicensed artwork bodies.
    real=TMP/'oriole-real-pgo-training-inputs'
    for rel,h in prep['real_inputs']['files_sha256'].items():include(real/rel,'shared/real-training/'+rel,h)
    for rel in ['manifest.json','selection.json','README.md','NOTICE.md','freeze.json','independent-review.json']:
        include(real/rel,'shared/real-training/'+rel)
    for p in sorted(real.glob('*.py')):include(p,'shared/real-training/'+p.name)
    boundary=load(ROOT/'corpus-publication-boundary.json')
    for rel in boundary['include']:include(real/rel,'shared/real-training/'+rel)
    for rel in boundary['exclude']:
        p=real/rel
        if p.is_file():exclude(p,'Rejected prospective input/acquisition body, never used in training; exclusion inventory retained.')
    for rel in prep['real_inputs']['files_sha256']:
        if rel.startswith('licenses/'):notices.append('shared/real-training/'+rel)
    # Actual consumer source plus its redistribution notice; no CPython executable.
    py=Path('/home/dev-user/.cache/oriole/upstream/cpython-3.12.13')
    for rel in ['Modules/pyexpat.c','Modules/_elementtree.c','LICENSE']:
        include(py/rel,'shared/cpython/'+rel)
    notices.append('shared/cpython/LICENSE')

    externals = [
        'oriole-real-pgo-training-runner-independent-review.json',
        'oriole-deferred-c-text-raw-build-independent-review.json',
        'oriole-deferred-c-text-raw-checks-independent-review.json',
        'oriole-deferred-c-text-raw-preparation-independent-review.json',
        'oriole-deferred-c-text-raw-native-independent-review.json',
        'oriole-bulk-pgo-python-independent-source-review.json',
        'oriole-bulk-pgo-python-independent-source-review.py',
        'oriole-review-bulk-python-elapsed.py',
        'oriole-prepare-bulk-python-elapsed-audit.py',
        'oriole-bulk-python-elapsed-audit-preparation.json',
        'oriole-bulk-python-elapsed-audit.patch',
        'oriole-bulk-python-elapsed-audit-attempt01.stdout',
        'oriole-bulk-python-elapsed-audit-attempt01.stderr',
    ]
    for name in externals:include(TMP/name,'audits/'+name)
    for directory,prefix in [('oriole-deferred-c-text-raw-native-audit','text/audits/native-reader'),('oriole-bulk-pgo-python-elapsed-independent-review','bulk/python-audit')]:
        for p in sorted((TMP/directory).rglob('*')):
            if p.is_file():include(p,prefix+'/'+str(p.relative_to(TMP/directory)))
    for p,h in load(TMP/'oriole-deferred-c-text-raw-native-independent-review.json')['input_sha256'].items():
        if 'oriole-compact-delivery-status-' in p:include(p,'text/parent-controller/'+Path(p).parts[2]+'/'+Path(p).name,h)
    codegen=TMP/'oriole-deferred-c-text-raw-codegen-study'
    for p in sorted(codegen.rglob('*')):
        if not p.is_file():continue
        rel=p.relative_to(codegen)
        if (rel.parts[0]=='assembly-attempt01' and p.suffix=='.asm') or p.name=='readelf-info.txt.gz':
            exclude(p,'Large full disassembly/DWARF omitted; exact report/excerpts/layout and collection commands retained.');continue
        include(p,'text/codegen/'+str(rel))
    design=TMP/'oriole-c-context-deferred-text-design/DESIGN.md'
    include(design,'text/design/DESIGN.md')
    include(TMP/'oriole-c-context-deferred-text-independent-review.json','text/design/independent-review.json')
    for mode in ('normal','pgo'):
        pair=TMP/'oriole-cdata-finder-pgo-study'
        for rel in ['pair-report.json','pair-freeze.json','source.json','tool-source.json']:
            include(pair/rel,'shared/finder-control/'+rel)

    # Summary is derived only from the already audited condition rows.
    condition_rows=[];summaries={}
    for tag in ('additive','bulk'):
        d=load(STUDIES[tag]/'native-independent-review.json');summaries[tag]=d['groups']
        primary='oriole-'+('gr' if tag=='additive' else 'b')+'_over_oriole-g'
        for r in d['all_condition_paired_ratios']:condition_rows.append({'campaign':tag if tag=='additive' else 'bulk-native','condition':r['condition'],'median_ratios':r['median_ratios'],'primary_ratio':primary})
    d=load(TMP/'oriole-deferred-c-text-raw-native-independent-review.json')
    for mode,row in d['native'].items():
        summaries['text-'+mode]=row['groups']
        for r in row['all_conditions']:condition_rows.append({'campaign':'text-'+mode,'condition':r['condition'],'median_ratios':r['median_ratios'],'primary_ratio':'candidate_over_control'})
    d=load(TMP/'oriole-bulk-pgo-python-elapsed-independent-review/review.json');summaries['bulk-python']=d['groups']
    details=json.loads(gzip.decompress((TMP/'oriole-bulk-pgo-python-elapsed-independent-review/details.json.gz').read_bytes()))
    for r in details['conditions']:condition_rows.append({'campaign':'bulk-python','condition':r['condition'],'median_ratios':r['median_ratios'],'primary_ratio':'oriole-b_over_oriole-g'})
    assert len(condition_rows)==136
    for r in condition_rows:r['adverse_primary']=r['median_ratios'][r['primary_ratio']]>1
    assert sum(r['adverse_primary'] for r in condition_rows)==54
    summary={'status':'unselected_completed_experiments','selected_runtime':'78c748d9de5c497858458cbf7a1229148781b2bf','decisions':{'additive':'Rejected: real Oriole G+R/G1.006678837, generated1.021897744.','bulk':'Not adopted: Oriole native B/G0.990319732 and Python0.998938820, with DocBook regressions; no new full compatibility gate.','text':'Rejected: normal real0.962975115, PGO real1.012515; no Python/full gate after rejection.'},'groups':summaries,'conditions':136,'all_workers':3904,'all_samples':530640,'native_worker_aliases':3136,'adverse_primary_conditions':54,'all_conditions':condition_rows,'limits':['No comparison across separately run cohorts. Lower paired-ratio geomeans are faster; no confidence interval or hardware mechanism claim.','Native driver pins explicit library path and hashes, without same-process dladdr. Python workers verify module and XML_Parse origins.','Unmodified CPython benchmark consumers only; canonical coalesced callbacks do not establish strict compatibility.','G, G+R and B affect parser training only. No production stable-toolchain, full application, API or ASan result transfers.']}
    save(ROOT/'summary.json',summary)
    with (ROOT/'conditions.csv').open('x',newline='') as f:
        w=csv.writer(f,lineterminator='\n');w.writerow(['campaign','name','chunk','namespaces','consumer','primary_ratio','ratio','adverse','all_ratios_json'])
        for r in condition_rows:
            c=r['condition'];w.writerow([r['campaign'],c['name'],c['chunk'],c.get('namespaces',''),c.get('mode','native'),r['primary_ratio'],r['median_ratios'][r['primary_ratio']],r['adverse_primary'],json.dumps(r['median_ratios'],sort_keys=True)])
    save(ROOT/'inventory-plan.json',{'status':'planned_verified_saved_inputs','files':files,'raw_worker_aliases':aliases,'exclusions':excluded,'redistribution_notices':notices,'skipped_tree_policy':['targets: all Cargo build intermediates','uv-tools: installed lint tool environments','__pycache__ and .git','generate-build/use-build: object trees; CMakeCache,expat_config,flags.make,link.txt selectively retained'],'payload_bytes':sum(r['bytes'] for r in files.values()),'native_alias_bytes':sum(r['bytes'] for r in aliases),'source_maps':{'selected':prep['oriole_source_sha256'],'expat':prep['expat_source_sha256'],'text':load(STUDIES['text/build']/'source.json')['source_sha256']}})
    print(json.dumps({'status':'planned','files':len(files),'payload_bytes':sum(r['bytes'] for r in files.values()),'native_aliases':len(aliases),'native_alias_bytes':sum(r['bytes'] for r in aliases),'excluded_files':len(excluded)}))

def assemble():
    """Create deterministic content-deduplicated tar; preserve original files untouched."""
    p=load(ROOT/'inventory-plan.json');out=ROOT/'bundle';out.mkdir(exist_ok=False)
    files={};payloads={};canonical={}
    for logical,row in sorted(p['files'].items()):
        data=Path(row['source']).read_bytes();assert len(data)==row['bytes'] and sha(data)==row['sha256'],logical
        member=canonical.setdefault(row['sha256'],logical)
        if member in payloads:assert payloads[member]==data
        else:payloads[member]=data
        files[logical]=row|{'archive_member':member}
    archive=out/'evidence.tar.gz'
    with archive.open('xb') as f,gzip.GzipFile(filename='',mode='wb',fileobj=f,mtime=0,compresslevel=9) as gz,tarfile.open(fileobj=gz,mode='w') as tar:
        for member,data in payloads.items():
            info=tarfile.TarInfo(member);info.size=len(data);info.mode=0o644;info.mtime=0;tar.addfile(info,io.BytesIO(data))
    assert archive.stat().st_size<25_000_000,archive.stat().st_size
    inventory=p|{'status':'assembled','files':files,'archive_sha256':sha(archive.read_bytes()),'archive_bytes':archive.stat().st_size,'unique_members':len(payloads),'unique_payload_bytes':sum(map(len,payloads.values()))}
    save(out/'inventory.json',inventory)
    for name in ('README.md','summary.json','conditions.csv','verify.py','package.py','corpus-publication-boundary.json'):(out/name).write_bytes((ROOT/name).read_bytes())
    print(json.dumps({'status':'assembled_pending_readback','archive_bytes':inventory['archive_bytes'],'archive_sha256':inventory['archive_sha256'],'unique_members':len(payloads)}))

if __name__=='__main__':
    assert os.sched_getaffinity(0)=={6}
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('phase',choices=['plan','assemble']);args=parser.parse_args()
    {'plan':plan,'assemble':assemble}[args.phase]()
