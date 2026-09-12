"""Saved-only raw readback; never compiles or starts a parser."""
from pathlib import Path
import argparse, collections, hashlib, json, os, re
H=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
J=lambda p:json.loads(Path(p).read_text())
parser=argparse.ArgumentParser()
parser.add_argument('--released-preparation-sha',required=True)
parser.add_argument('--released-controller-report-sha',required=True)
args=parser.parse_args()
assert __debug__ and os.sched_getaffinity(0)=={6}
P=Path(__file__).resolve().parent
assert H(P/'prepared.json')==args.released_preparation_sha
p=J(P/'prepared.json');D=Path(p['output']);R=D/'run01';base=Path(p['original_base'])
assert H(R/'report.json')==args.released_controller_report_sha
r=J(R/'report.json');commands=J(P/'commands.json')
assert H(P/'scripts.json')==p['scripts_sha256']
for name,digest in J(P/'scripts.json').items():assert H(P/name)==digest,name
assert H(P/'commands.json')==p['commands_sha256']==r['commands_sha256']
assert H(P/'run.py')==r['controller_sha256']
assert H(P/'prepared.json')==r['preparation_sha256']
assert r['runtime_commit']==p['runtime_commit'] is None
assert r['base_commit']==p['base_commit']
assert r['candidate_patch_sha256']==p['candidate_patch_sha256']
runtime_argv=[['rev-parse','HEAD'],['diff','--no-ext-diff','--binary',p['base_commit'],'--',*sorted(p['source_files'])]]
assert len(r['runtime_checks'])==len(runtime_argv)==2
for actual,tail in zip(r['runtime_checks'],runtime_argv,strict=True):
    assert actual['argv']==['/usr/bin/git','--no-optional-locks','-C',p['worktree'],*tail]
    assert actual['exit']==0 and actual['reaped']
assert r['runtime_checks'][0]['stdout'].strip()==p['base_commit']
assert hashlib.sha256(r['runtime_checks'][1]['stdout'].encode()).hexdigest()==p['candidate_patch_sha256']
assert r['environment']==commands['environment']
assert r['original_before'] and r['original_tree_unchanged_on_exit'] and r['source_and_inputs_unchanged'] and r['staged_sources_unchanged']
assert p['original_files']=={str(x.relative_to(base)):H(x) for x in sorted(base.rglob('*')) if x.is_file()}
for path,digest in p['pins'].items():assert H(path)==digest,path
for name,digest in p['source_files'].items():assert H(Path(p['worktree'])/name)==digest,name
for name,digest in p['held_files'].items():assert H(P/'held'/name)==H(D/name)==digest,name
assert H(D/'original-results.json')==H(base/'results.json')
assert H(D/'original-manifest.json')==H(base/'manifest.json')
assert H(D/'liboriole_expat.so')==r['library_sha256_before']==r['library_sha256_after']==p['candidate_library_sha256']
assert H(R/'runtests')==r['binary_sha256']
assert len(r['commands'])==len(commands['steps'])==2
for actual,expected in zip(r['commands'],commands['steps'],strict=True):
    assert actual['label']==expected['label'] and actual['argv']==expected['argv']
    assert actual['timeout_seconds']==expected['timeout_seconds'] and actual['reaped']
    assert actual['end']>=actual['start'] and actual['pid']>0
    assert actual['descendant_cleanup']['status']=='completed'
    assert H(R/expected['log'])==actual['log_sha256']
assert r['commands'][0]['exit']==0 and not r['commands'][0]['outer_timed_out']
# Prove the complete staged source change is exactly the allowlisted local maxima.
assert H(P/'retry-ceilings.patch')==p['retry_patch_sha256']
for fn in ('alloc_tests.c','nsalloc_tests.c'):
    before=(base/'adapted'/fn).read_text();after=(D/'adapted'/fn).read_text()
    for change in [x for x in p['ceiling_changes'] if x['file']=='adapted/'+fn]:
        m=re.search(r'START_TEST\('+change['test']+r'\).*?END_TEST',after,re.S);assert m
        body=m.group();assert hashlib.sha256(body.encode()).hexdigest()==change['body_sha256_after']
        needle=change['variable']+' = 512;';assert body.count(needle)==1
        old=body.replace(needle,change['variable']+' = '+str(change['old'])+';')
        assert hashlib.sha256(old.encode()).hexdigest()==change['body_sha256_before']
        after=after[:m.start()]+old+after[m.end():]
    assert after==before
log=(R/'tests.log').read_text()
begins=re.findall(r'^ORIOLE_BEGIN\t([^\t\n]+)\t([^\t\n]+)$',log,re.M)
rows=[dict(context=c,test=t,outcome=o,code=int(n)) for c,t,o,n in re.findall(r'^ORIOLE_RESULT\t([^\t\n]+)\t([^\t\n]+)\t([^\t\n]+)\t(\d+)$',log,re.M)]
origins=re.findall(r'^ORIOLE_LIBRARY\t([^\n]+)$',log,re.M)
assert origins and all(Path(x).resolve()==(D/'liboriole_expat.so').resolve() for x in origins)
expected=[(x['context'],x['test']) for x in p['expected_order']]
observed=[(x['context'],x['test']) for x in rows]
complete=begins==expected and observed==expected
assert rows==r['results'] and origins==r['library_origins'] and r['library_origin_verified']
assert complete==r['selection_complete'] and dict(collections.Counter(x['outcome'] for x in rows))==r['outcomes']
all_pass=complete and not r['commands'][1]['outer_timed_out'] and r['commands'][1]['exit']==0 and all(x['outcome']=='pass' and x['code']==0 for x in rows)
assert all_pass==r['all_diagnostic_contexts_passed']
result=dict(status='passed_saved_raw_readback',targets_executed=False,runtime_commit=p['runtime_commit'],base_commit=p['base_commit'],candidate_patch_sha256=p['candidate_patch_sha256'],candidate_library_sha256=p['candidate_library_sha256'],original_api_unchanged=True,semantic_assertions_unchanged=True,ceiling_changes=25,expected_configurations=300,observed_results=len(rows),complete=complete,all_diagnostic_contexts_passed=all_pass,outcomes=r['outcomes'],nonpasses=[x for x in rows if x['outcome']!='pass'],controller_report_sha256=H(R/'report.json'),preparation_sha256=H(P/'prepared.json'),reader_sha256=H(__file__),limits=p['limits'],limitations=p['limitations'])
out=R/'saved-readback.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
