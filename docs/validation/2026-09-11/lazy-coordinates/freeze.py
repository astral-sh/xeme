import hashlib, json, pathlib, re, subprocess
root = pathlib.Path('/home/dev-user/code/oss/oriole-lazy-coordinates')
out = pathlib.Path(__file__).parent
sha = lambda b: hashlib.sha256(b).hexdigest()
base = subprocess.check_output(['git','rev-parse','HEAD'], cwd=root, text=True).strip()
paths = sorted([*json.loads(pathlib.Path('/tmp/oriole-context-text-frame-fixed-pgo-study/source.json').read_text())['source_sha256'], 'crates/oriole_expat/src/coordinate_tests.rs'])
assert len(paths) == len(set(paths)) == 73
hashes = {p:sha((root/p).read_bytes()) for p in paths}
deltas = {}
changed = subprocess.check_output(['git','diff','--name-only'], cwd=root, text=True).splitlines()
for p in changed:
    if p in hashes:
        prior = subprocess.check_output(['git','show',f'HEAD:{p}'], cwd=root)
        deltas[p] = {'before':sha(prior),'after':hashes[p]}
deltas['crates/oriole_expat/src/coordinate_tests.rs'] = {'before':None,'after':hashes['crates/oriole_expat/src/coordinate_tests.rs']}
source = dict(status='frozen_uncommitted_prototype_after_local_checks', base_runtime=base, worktree=str(root), source_file_count=len(hashes), source_sha256=hashes, delta_from_base=deltas)
source_path = out/'source.json'
assert not source_path.exists()
source_path.write_text(json.dumps(source,indent=2)+'\n')
first = dict(command=['env','-u','RUSTFLAGS','-u','CARGO_ENCODED_RUSTFLAGS','-u','CARGO_UNSTABLE_OHM_PROC_MACRO_TRUST','-u','CARGO_UNSTABLE_OHM_NATIVE_TOOL_TRUST','CARGO_HOME=/home/dev-user/.cache/toucan/cargo','CARGO_TARGET_DIR=/home/dev-user/.cache/oriole/targets/lazy-coordinates','CARGO_BUILD_BUILD_DIR=/home/dev-user/.cache/ohm/verified-build/{workspace-path-hash}','CARGO_INCREMENTAL=0','taskset','-c','4','cargo','+ohm','-Zohm-defaults=no','check','--offline','-p','oriole','-p','oriole_expat','--all-targets','--jobs','1'],cwd=str(root), returncode=101, note='Reconstructed from exact tool invocation; compiler missed remaining DTD enum assignment, no parser/test executed.')
for suffix in ('stdout','stderr'):
    p = out/f'check-attempt01.{suffix}'
    first[suffix] = dict(bytes=p.stat().st_size,sha256=sha(p.read_bytes()))
(out/'check-attempt01.json').write_text(json.dumps(first,indent=2)+'\n')
reports = {}
for p in sorted(out.glob('*attempt*.json')):
    data=json.loads(p.read_text())
    for suffix in ('stdout','stderr'):
        path=out/f'{p.stem}.{suffix}'
        assert data[suffix]['sha256']==sha(path.read_bytes())
    reports[p.name]=dict(sha256=sha(p.read_bytes()), **data)
assert all(d['returncode']==0 for name,d in reports.items() if name!='check-attempt01.json')
full=(out/'tests-attempt01.stdout').read_text()
counts=[tuple(map(int,m)) for m in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',full)]
assert sum(c[0] for c in counts)==443 and len(counts)==32
summary=dict(status='local_checks_passed_runtime_frozen_benchmark_and_miri_pending', source_manifest=dict(path=str(source_path),sha256=sha(source_path.read_bytes())), source_file_count=73, reports=reports, validation=dict(all_targets_tests=443,all_targets_groups=32,doctests=1,failed=0,ignored=0,subsequent_change='Only allocation.rs fixture strengthened to assert a real failed Start after unresolved native Text; affected test, strict Clippy and fmt rechecked. Runtime unchanged after full suite.',actual_miri_executed=False,actual_elapsed_benchmark_executed=False), layouts_x86_64=dict(Source=520,Parser=2416,AdapterFrame=264,CParser=3176,AdapterLocation=40), notes=dict(path=str(out/'attempt-notes.md'),sha256=sha((out/'attempt-notes.md').read_bytes())), controller=dict(path=str(out/'run.py'),sha256=sha((out/'run.py').read_bytes())))
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
for p in (source_path,out/'summary.json'):
    print(p,sha(p.read_bytes()))
