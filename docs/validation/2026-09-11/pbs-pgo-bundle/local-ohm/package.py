"""Package the historical first Ohm bundle result; never recompile or rerun."""
from pathlib import Path
import gzip,hashlib,io,json,shutil,subprocess,tarfile
W=Path('/home/dev-user/code/oss/oriole-pbs-pgo-bundle');O=Path('/tmp/oriole-pbs-pgo-bundle-local-attempt01');S=Path('/tmp/oriole-pbs-pgo-bundle-study');P=Path('/tmp/oriole-pbs-pgo-bundle-local-handoff')
P.mkdir(exist_ok=False)
sha=lambda b:hashlib.sha256(b).hexdigest()
load=lambda p:json.loads(Path(p).read_text())
launch=load('/tmp/oriole-pbs-pgo-bundle-local-attempt01-launch.json');prep=load(S/'preparation.json');review=load('/tmp/oriole-pbs-pgo-bundle-source-independent-review.json');manifest=load(O/'manifest.json');validation=load(O/'local-validation/report.json');run=Path(manifest['pgo']['manifest']['run'])
assert launch['status']==validation['status']=='passed';assert sha((O/'manifest.json').read_bytes())==launch['manifest_sha256']
base=review['base'];current=subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()
entries={};excluded=[]
def add(name,path):
    p=Path(path);data=p.read_bytes()
    if data.startswith((b'\x7fELF',b'!<arch>\n')):
        excluded.append({'origin':str(p),'bytes':len(data),'sha256':sha(data),'reason':'compiled ELF or static archive'});return
    assert name not in entries
    entries[name]=(data,{'kind':'file','path':str(p)})
source_pins=launch['source_before'] | prep['files'] | review['unchanged_related_sources']
restacked=[]
for name,expected in sorted(source_pins.items()):
    path=W/name;data=path.read_bytes()
    if sha(data)==expected:origin={'kind':'file','path':str(path)}
    else:
        data=subprocess.check_output(['git','show',base+':'+name],cwd=W)
        assert sha(data)==expected,(name,'cannot reconstruct frozen source')
        origin={'kind':'git_blob','repository':str(W),'commit':base,'path':name};restacked.append(name)
    assert sha(data)==expected;entries['source/'+name]=(data,origin)
for path in sorted(S.rglob('*')):
    if path.is_file() and '__pycache__' not in path.parts:add('source-checks/'+str(path.relative_to(S)),path)
for name in ('manifest.json','build.log','expat.h','expat.pc','native-static-libs.txt','LICENSE.oriole.txt','cpython-external-parser.patch','libexpat.a'):
    add('bundle/'+name,O/name)
add('bundle/pgo/latest.json',O/'pgo/latest.json')
for path in sorted(run.rglob('*')):
    if path.is_file():add('bundle/pgo/run/'+str(path.relative_to(run)),path)
for path in sorted((O/'local-validation').iterdir()):
    if path.is_file():add('local-validation/'+path.name,path)
for path in sorted(Path('/tmp').glob('oriole-pbs-pgo-bundle-local-attempt01-*')):
    if path.is_file():add('collector/'+path.name,path)
for path in [Path('/tmp/oriole-pbs-pgo-bundle-source-independent-review.json'),Path('/tmp/oriole-pbs-pgo-bundle-local-root-review.json')]:add('reviews/'+path.name,path)
# The raw compiler vectors are retained verbatim; this only indexes the six workspace invocations.
pgo=manifest['pgo']['manifest'];vectors={}
for phase in ('generate','use'):
    c=next(c for c in pgo['commands'] if c['label']=='build-'+phase)
    log=(run/c['log']).read_text()
    rows=[line for line in log.splitlines() if 'Running `' in line and any('--crate-name '+n+' ' in line for n in ('oriole_storage','oriole','oriole_expat'))]
    assert len(rows)==3;vectors[phase]=rows
vector_data=(json.dumps(vectors,indent=2)+'\n').encode();entries['collector/workspace-vectors.json']=(vector_data,{'kind':'derived','inputs':[str(run/next(c for c in pgo['commands'] if c['label']=='build-'+phase)['log']) for phase in ('generate','use')],'method':'Unmodified workspace compiler lines selected by exact --crate-name values; three each.'})
index=[];buffer=io.BytesIO()
with tarfile.open(fileobj=buffer,mode='w',format=tarfile.PAX_FORMAT) as tar:
    for name,(data,origin) in sorted(entries.items()):
        entry=tarfile.TarInfo(name);entry.size=len(data);entry.mode=0o644;entry.mtime=0;tar.addfile(entry,io.BytesIO(data))
        index.append({'member':name,'bytes':len(data),'sha256':sha(data),'origin':origin})
def gz(data):
    b=io.BytesIO()
    with gzip.GzipFile(fileobj=b,filename='',mode='wb',mtime=0,compresslevel=9) as f:f.write(data)
    return b.getvalue()
archive=gz(buffer.getvalue());(P/'evidence.tar.gz').write_bytes(archive)
(P/'members.json.gz').write_bytes(gz((json.dumps(index,indent=2)+'\n').encode()))
with tarfile.open(fileobj=io.BytesIO(archive),mode='r:gz') as tar:
    assert len(tar.getmembers())==len(index)
    for row in index:
        assert sha(tar.extractfile(row['member']).read())==row['sha256'];origin=row['origin']
        if origin['kind']=='file':assert sha(Path(origin['path']).read_bytes())==row['sha256']
        elif origin['kind']=='git_blob':assert sha(subprocess.check_output(['git','show',origin['commit']+':'+origin['path']],cwd=W))==row['sha256']
summary={'status':'passed','role':'Collector-owned build, C validation and packaging. Independent source and root saved-result reviews included.',
    'toolchain':'Ohm 1.98.1-1, +ohm -Zohm-defaults=no; not the production stable workflow',
    'source_base':base,'source_review_patch_sha256':review['patch_sha256'],'current_head_after_sessions_reaped':current,'historical_sources_reconstructed_from_base':restacked,
    'scope':'One fresh PIC/unwind/ThinLTO generate-use PBS bundle build. Three actual workspace compiler commands and 288 generated-only training rows per phase; both phase rows equal. Existing C integration against use shared library and exact bundled static archive passed first attempt. No installed PBS distribution, glibc/thread runtime validation, strict XML suite or elapsed benchmark.',
    'bundle_manifest_sha256':sha((O/'manifest.json').read_bytes()),'pgo_manifest_sha256':manifest['pgo']['manifest_sha256'],'payloads':manifest['files'],
    'shared_sha256':pgo['libraries_sha256']['use/liboriole_expat.so'],'exact_use_static_equals_bundle':validation['archive_identity'],
    'native_static_libraries':validation['native_static_libraries'],'weak_tls_references':validation['weak_tls_references'],'static_archive_members':validation['archive_members'],
    'pipeline_commands':len(pgo['commands']),'validation_commands':len(validation['commands']),'c_consumers':2,'all_command_outcomes_zero':all(c['returncode']==0 for c in pgo['commands']+validation['commands']),
    'first_check_attempts_retained':prep['checks']['original_attempts'],'native_build_attempts':1,'native_validation_attempts':1,'native_failures':0,
    'reviews':{p.name:sha(p.read_bytes()) for p in [Path('/tmp/oriole-pbs-pgo-bundle-source-independent-review.json'),Path('/tmp/oriole-pbs-pgo-bundle-local-root-review.json')]},
    'archive':{'name':'evidence.tar.gz','sha256':sha(archive),'bytes':len(archive),'members':len(index),'all_members_and_origins_read_back':True},
    'member_index':{'name':'members.json.gz','sha256':sha((P/'members.json.gz').read_bytes())},'excluded_compiled_artifacts':excluded,
    'excluded_caches':['bundle/pgo/targets (entire reusable Cargo target cache; not traversed)'],'original_artifacts_unchanged':True}
(P/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
(P/'README.md').write_text('''# Local Ohm PBS PGO bundle

The first local build and both existing C integration consumers passed. This is an **Ohm** result, not a production stable build or an installed PBS distribution result.

The bridge used fresh generated-only profiles with PIC, unwinding and ThinLTO. Each phase recorded three workspace compiler invocations and 288 generated parses; all generated callbacks agreed. The bundled static archive is byte-identical to the final use archive. Its native dependencies came from that exact compile, and the archive retained only weak references to the optional TLS destructor hook. The existing shared and exact-static C consumers passed without retries. The final PBS linker TLS treatment and glibc/thread compatibility still need installed-distribution validation.

[Summary](summary.json) records exact identities, scope, exclusions and independent reviews. [Evidence](evidence.tar.gz) retains frozen source, all earlier Python-check attempts, first build/training/profile records, manifests, raw C checks and both reviews. [Member index](members.json.gz) records every member hash and origin; all were read back. Compiled ELFs/static archives and target caches are excluded; selected artifact hashes are retained. Raw and merged profiles and generated input bytes are included.

The worktree was advanced from PR124 to PR125 only after all target sessions ended. The compiled C consumer is the earlier `tests/c/integration.c` at commit `4d603c7487ca93cc73dbd4acfe6fa5714ffc2151`, reconstructed from that exact Git blob. Historical before/after source pins and raw outcomes are unchanged. The newer nested-entity fixture is not part of these two runs.
''')
shutil.copyfile(__file__,P/'package.py')
files={p.name:sha(p.read_bytes()) for p in sorted(P.iterdir()) if p.is_file()};(P/'files.json').write_text(json.dumps(files,indent=2)+'\n')
print(json.dumps({'directory':str(P),'archive':summary['archive'],'summary_sha256':sha((P/'summary.json').read_bytes()),'files_sha256':sha((P/'files.json').read_bytes()),'source_reconstructed':restacked,'excluded_binaries':len(excluded)},indent=2))
