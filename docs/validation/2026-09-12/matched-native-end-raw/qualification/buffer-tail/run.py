"""Held root-only 12-row buffer-tail diagnostic; no existing output is reused."""
from pathlib import Path
import argparse, collections, ctypes, hashlib, json, os, re, shutil, signal, subprocess, sys, time
H=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
J=lambda p:json.loads(Path(p).read_text())
P=Path(__file__).resolve().parent
parser=argparse.ArgumentParser()
parser.add_argument('--released-preparation-sha',required=True)
args=parser.parse_args()
assert __debug__ and os.sched_getaffinity(0)=={3}
assert H(P/'prepared.json')==args.released_preparation_sha
prepared=J(P/'prepared.json');output=Path(prepared['output']);runout=output/'run01'
assert Path(sys.executable).resolve()==Path(prepared['python_path']).resolve()
assert H(P/'commands.json')==prepared['commands_sha256']
assert H(P/'diagnostic.patch')==prepared['diagnostic_patch_sha256']
assert H(P/'observation-transform.json')==prepared['observation_transform_sha256']
assert H(P/'scripts.json')==prepared['scripts_sha256']
seal=J(P/'scripts.json')
for name,digest in seal.items():assert H(P/name)==digest,name
for name,digest in prepared['pins'].items():assert H(name)==digest,name
for name,digest in prepared['source_files'].items():assert H(Path(prepared['worktree'])/name)==digest,name
for name,digest in prepared['held_files'].items():assert H(P/'held'/name)==digest,name
base=Path(prepared['original_base'])
def original_unchanged():
    return prepared['original_files']=={str(p.relative_to(base)):H(p) for p in sorted(base.rglob('*')) if p.is_file()}
assert original_unchanged()
runtime_checks=[]
assert prepared['runtime_commit'] is None
for tail in [['rev-parse','HEAD'],['diff','--no-ext-diff','--binary',prepared['base_commit'],'--',*sorted(prepared['source_files'])]]:
    argv=['/usr/bin/git','--no-optional-locks','-C',prepared['worktree'],*tail]
    r=subprocess.run(argv,capture_output=True,timeout=30,check=True)
    runtime_checks.append(dict(argv=argv,exit=r.returncode,reaped=True,stdout=r.stdout.decode(),stderr=r.stderr.decode()))
assert runtime_checks[0]['stdout'].strip()==prepared['base_commit']
assert hashlib.sha256(runtime_checks[1]['stdout'].encode()).hexdigest()==prepared['candidate_patch_sha256']
libc=ctypes.CDLL(None,use_errno=True)
if libc.prctl(36,1,0,0,0)!=0:raise OSError(ctypes.get_errno(),'PR_SET_CHILD_SUBREAPER')
assert not output.exists()
output.mkdir();runout.mkdir()
report=dict(status='incomplete',scope=prepared['scope'],preparation_sha256=H(P/'prepared.json'),controller_sha256=H(__file__),scripts_sha256=H(P/'scripts.json'),runtime_commit=prepared['runtime_commit'],base_commit=prepared['base_commit'],candidate_patch_sha256=prepared['candidate_patch_sha256'],runtime_checks=runtime_checks,commands=[],commands_sha256=H(P/'commands.json'),original_before=True)
def save():
    (runout/'report.json').write_text(json.dumps(report,indent=2)+'\n')
def interrupt(signum,frame):
    raise KeyboardInterrupt('SIGTERM')
def reap_descendants(receipt):
    """Kill and reap adopted generations, including detached process sessions."""
    deadline = time.monotonic() + 10
    children_file = Path(f'/proc/self/task/{os.getpid()}/children')
    receipt.update(status='incomplete', signalled=[], reaped=[])
    while time.monotonic() < deadline:
        for pid in map(int, children_file.read_text().split()):
            try:
                os.kill(pid, signal.SIGKILL)
                receipt['signalled'].append(pid)
            except ProcessLookupError:
                pass
        no_children = False
        while True:
            try:
                pid, status = os.waitpid(-1, os.WNOHANG)
            except ChildProcessError:
                no_children = True
                break
            if pid == 0:
                break
            receipt['reaped'].append({'pid': pid, 'wait_status': status})
        if no_children and not children_file.read_text().strip():
            receipt['status'] = 'completed'
            return
        time.sleep(0.01)
    receipt['status'] = 'cleanup_timeout'
    raise RuntimeError('Owned descendants were not fully reaped within 10 seconds')


signal.signal(signal.SIGTERM,interrupt)
try:
    for folder in ('adapted','lib'):shutil.copytree(P/'held'/folder,output/folder)
    shutil.copyfile(prepared['library_path'],output/'liboriole_expat.so')
    for name in ('manifest.json','results.json'):shutil.copyfile(base/name,output/('original-'+name))
    assert H(output/'liboriole_expat.so')==prepared['candidate_library_sha256']
    report['library_sha256_before']=H(output/'liboriole_expat.so')
    commands=J(P/'commands.json')
    report['environment']=commands['environment'];save()
    for step in commands['steps']:
        row=dict(label=step['label'],argv=step['argv'],timeout_seconds=step['timeout_seconds'],start=time.time(),outer_timed_out=False)
        report['commands'].append(row);save()
        log=runout/step['log']
        with log.open('xb') as stream:
            proc=subprocess.Popen(step['argv'],cwd=output,env=commands['environment'],stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
            row['pid']=proc.pid
            try:
                row['exit']=proc.wait(timeout=step['timeout_seconds'])
            except subprocess.TimeoutExpired:
                row['outer_timed_out']=True
            finally:
                handlers={sig:signal.signal(sig,signal.SIG_IGN) for sig in (signal.SIGTERM,signal.SIGINT)}
                try:
                    if proc.poll() is None:
                        try:os.killpg(proc.pid,signal.SIGKILL)
                        except ProcessLookupError:pass
                    row['exit']=proc.wait(timeout=10)
                    row['descendant_cleanup']={}
                    reap_descendants(row['descendant_cleanup'])
                finally:
                    row.update(reaped=proc.poll() is not None,end=time.time())
                    stream.flush();row['log_sha256']=H(log);save()
                    for sig,handler in handlers.items():signal.signal(sig,handler)
        if step['label']=='compile':
            assert row['exit']==0 and not row['outer_timed_out'],row
            report['binary_sha256']=H(runout/'runtests')
    log=(runout/'tests.log').read_text()
    begins=re.findall(r'^ORIOLE_BEGIN\t([^\t\n]+)\t([^\t\n]+)$',log,re.M)
    rows=[dict(context=c,test=t,outcome=o,code=int(n)) for c,t,o,n in re.findall(r'^ORIOLE_RESULT\t([^\t\n]+)\t([^\t\n]+)\t([^\t\n]+)\t(\d+)$',log,re.M)]
    expected=[(x['context'],x['test']) for x in prepared['expected_order']]
    origins=re.findall(r'^ORIOLE_LIBRARY\t(.+)$',log,re.M)
    report.update(results=rows,outcomes=dict(collections.Counter(r['outcome'] for r in rows)),selection_complete=begins==expected and [(r['context'],r['test']) for r in rows]==expected,library_origins=origins)
    report['library_origin_verified']=bool(origins) and all(Path(p).resolve()==(output/'liboriole_expat.so').resolve() for p in origins)
    observations = []
    for block in re.finditer(r'^ORIOLE_BEGIN\t([^\t\n]+)\t([^\t\n]+)\n(.*?)^ORIOLE_RESULT\t\1\t\2\t([^\n]+)\n', log, re.M | re.S):
        for status, error, allocation_count in re.findall(r'^ORIOLE_EMPTY_BUFFER_OBSERVATION\tstatus=(-?\d+)\terror=(-?\d+)\tallocation_count=(-?\d+)$', block[3], re.M):
            observations.append({'context': block[1], 'test': block[2], 'status': int(status), 'error': int(error), 'allocation_count': int(allocation_count)})
    report['empty_buffer_observations'] = observations
    report['observation_selection_complete'] = [(r['context'],r['test']) for r in observations] == expected
    report['all_diagnostic_contexts_passed']=report['selection_complete'] and report['observation_selection_complete'] and report['library_origin_verified'] and not report['commands'][-1]['outer_timed_out'] and report['commands'][-1]['exit']==0 and all(r['outcome']=='pass' and r['code']==0 for r in rows)
    report['status']='complete' if report['all_diagnostic_contexts_passed'] else 'diagnostic_failures_or_incomplete'
except BaseException as error:
    report['status']='error';report['error']=repr(error)
    raise
finally:
    report['original_tree_unchanged_on_exit']=original_unchanged()
    report['library_sha256_after']=H(output/'liboriole_expat.so') if (output/'liboriole_expat.so').exists() else None
    report['source_and_inputs_unchanged']=all(H(p)==v for p,v in prepared['pins'].items()) and all(H(Path(prepared['worktree'])/p)==v for p,v in prepared['source_files'].items())
    report['staged_sources_unchanged']=all((output/p).is_file() and H(output/p)==v for p,v in prepared['held_files'].items())
    save()
    print(json.dumps({k:report[k] for k in ('status','outcomes','selection_complete','original_tree_unchanged_on_exit') if k in report}))
assert report['original_tree_unchanged_on_exit'] and report['source_and_inputs_unchanged'] and report['staged_sources_unchanged']
assert report['library_sha256_after']==prepared['candidate_library_sha256']
if not report['all_diagnostic_contexts_passed']:sys.exit(1)
