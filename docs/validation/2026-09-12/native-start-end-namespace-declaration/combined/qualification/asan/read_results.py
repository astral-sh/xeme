"""Saved-only current Rust-ASan audit, adapted from the reference-frame raw reader.

No producer imports, subprocesses, artifact tools, parser targets or corpus writes.
Root supplies a terminal report hash; no review output precedes that completion gate.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import shlex
import traceback

assert __debug__ and os.sched_getaffinity(0) == {6}
P = Path('/tmp/oriole-native-start-end-namespace-declaration-asan-preparation')
S = Path('/tmp/oriole-native-start-end-namespace-declaration-asan-study')
W = Path('/home/dev-user/code/oss/oriole-native-start-end-namespace-declaration')
OUT = Path('/tmp/oriole-native-start-end-namespace-declaration-asan-independent-review.json')
TARGETS = ['parse', 'streaming', 'ffi', 'ffi_family', 'multibyte', 'value_family']
RUNTIME = None
BASE = '9c632547af2253b9672a92fac320c70af3263bb8'
RELEASED_MANIFEST = None
HELD_MANIFEST = 'd6e586bfa240ce9bee114ee82399fc52a77a678d4791856bf6d797d6eebadd1d'
RUNNER = '5863138bc0b8663a7fe89b7e64e54ca01161ae41719c6e31bdca611b285357d4'
SOURCE = None
BINDING = None
pins = {}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    path = Path(path)
    data = path.read_bytes()
    value = hashlib.sha256(data).hexdigest()
    assert str(path) not in pins or pins[str(path)] == value, path
    pins[str(path)] = value
    return data


def load(path):
    return json.loads(read(path))


def vectors(text):
    return [{'raw': line.strip(), 'argv': shlex.split(line.split('Running `', 1)[1][:-1])}
            for line in text.splitlines() if 'Running `' in line and '/rustc ' in line]


def crate(row):
    args = row['argv']
    return args[args.index('--crate-name') + 1]


def hash_directory(path):
    files = list(path.iterdir())
    assert all(p.is_file() and not p.is_symlink() for p in files), path
    return {p.name: digest(p) for p in files}


def audit():
    global SOURCE, BINDING
    m = load(S/'command-manifest.json')
    assert pins[str(S/'command-manifest.json')] == RELEASED_MANIFEST == digest(P/'command-manifest.json')
    held = load(P/'command-manifest-held.json')
    assert pins[str(P/'command-manifest-held.json')] == HELD_MANIFEST
    restored = json.loads(json.dumps(m)); restored['release']['status'] = 'held'; restored['release']['source_commit'] = None
    for field in ('candidate_patch_sha256','normal_source_manifest_sha256','normal_build_binding_sha256'):
        assert re.fullmatch('[0-9a-f]{64}', m['release'][field])
        restored['release'][field] = None
    for name in ('liboriole_expat.so','liboriole_expat.a'):
        assert re.fullmatch('[0-9a-f]{64}', m['release']['normal_libraries'][name])
        restored['release']['normal_libraries'][name] = None
    SOURCE = m['release']['normal_source_manifest_sha256']
    BINDING = m['release']['normal_build_binding_sha256']
    assert restored == held and m['release']['status'] == 'released'
    assert digest(S/'runner.py') == digest(P/'runner.py') == RUNNER == m['runner_sha256']
    pins[str(S/'runner.py')] = RUNNER
    assert m['targets'] == TARGETS and m['seed'] == 20260912
    assert m['source_root'] == str(W) and m['output'] == str(S)
    assert m['target_dir'] == '/home/dev-user/.cache/oriole/native-start-end-namespace-declaration-asan-target'
    assert m['limits'] == dict(build_seconds=1200, replay_seconds=180, exploration_seconds=600,
        exploration_outer_seconds=690, input_seconds=10, rss_mb=1536, max_len=65536,
        overall_seconds=3600, min_free_bytes=8589934592, min_free_inodes=150000)
    assert m['asan_options'] == 'detect_leaks=0:abort_on_error=1'
    assert producer['manifest_sha256'] == RELEASED_MANIFEST and producer['runner_sha256'] == RUNNER
    assert producer['release'] == m['release'] and m['release']['source_commit'] == RUNTIME
    source = load(S/'normal-source.json'); binding = load(S/'normal-build-binding.json')
    assert pins[str(S/'normal-source.json')] == SOURCE == m['release']['normal_source_manifest_sha256']
    assert pins[str(S/'normal-build-binding.json')] == BINDING == m['release']['normal_build_binding_sha256']
    assert digest(m['normal_source_manifest']) == SOURCE and digest(m['normal_build_binding']) == BINDING
    assert binding['status'] == 'passed' and binding['tests'] == dict(passed=483, groups=35, failures=0, ignored=0)
    assert binding['candidate_libraries'] == m['release']['normal_libraries']
    assert digest(m['prepared_source']['path']) == m['prepared_source']['sha256'] == binding['prepared_source_sha256']
    assert source['sha256'] == load(m['prepared_source']['path'])['sha256']
    assert len(source['sha256']) == 74 and len(binding['auxiliary_files']) == 4 and len(m['fuzz_source_sha256']) == 254
    assert source['worktree'] == str(W) and source['base'] == BASE
    assert m['release']['source_commit'] == RUNTIME and m['release']['base_commit'] == BASE
    assert sorted(binding['source_delta']) == m['release']['source_delta'] and len(binding['source_delta']) == 12
    patch_hash = binding['complete_patch_sha256']
    assert patch_hash == m['release']['candidate_patch_sha256'] == digest(S/'normal-candidate.patch')
    assert digest(Path(m['normal_build_binding']).parent/'candidate.patch') == patch_hash
    assert producer['source_identity'] == {'source_commit': RUNTIME, 'base_commit': BASE,
        'candidate_patch_sha256': patch_hash, 'source_delta': m['release']['source_delta'],
        'unchanged_base_blobs_verified': True}
    merged = {**source['sha256'], **binding['auxiliary_files'], **m['fuzz_source_sha256']}
    for mapping in [source['sha256'], binding['auxiliary_files'], m['fuzz_source_sha256']]:
        assert all(merged[n] == h for n,h in mapping.items())
    assert len(merged) == 330 and producer['source_sha256'] == merged
    for n,h in merged.items():
        assert digest(W/n) == h, n
        pins[str(W/n)] = h
    for n,h in binding['candidate_libraries'].items():
        assert digest(Path(m['normal_build_binding']).parent/'normal'/n) == h
    for path,h in m['tool_sha256'].items():
        assert digest(path) == h, path
        pins[path] = h
    assert all(m['tool_sha256'].get(path) == h for path,h in binding['tool_hashes'].items())
    assert producer['normal_libraries_are_provenance_only'] is True

    commands = producer['commands']
    labels = [r['label'] for r in commands]
    assert len(labels) == len(set(labels))
    rows = dict(zip(labels, commands, strict=True))
    captured = []
    for row in commands:
        log = S/(row['label']+'.log')
        data = read(log)
        assert pins[str(log)] == row['log_sha256'] and row['reaped'] is True, row['label']
        assert isinstance(row['pid'], int) and row['pid'] > 0
        assert row['end'] >= row['start'] and row['elapsed_seconds'] >= 0
        assert row['cwd'] == str(W)
        captured.append({**row, 'log':str(log)})
    failure_artifacts = {}
    for name in TARGETS:
        folder = S/'campaigns'/name/'artifacts'
        if folder.exists(): failure_artifacts[name] = hash_directory(folder)
    result.update(command_records=captured, failure_artifacts=failure_artifacts,
                  producer_error=producer.get('error'), unstarted_targets=producer.get('unstarted_targets', []))
    if producer['status'] != 'passed':
        result.update(status='completed_campaign_failed', campaign_status=producer['status'],
                      full_success_checks_completed=False)
        return
    assert len(commands) == 22 and len({r['pid'] for r in commands}) == 22
    assert all(r['exit_code'] == 0 and not r.get('outer_timeout') and not r.get('controller_aborted') for r in commands)
    assert producer['source_unchanged'] and not producer['unstarted_targets']
    assert producer['environment'] == m['environment'] and producer['asan_options'] == m['asan_options']
    assert producer['path_prefix'] == m['fuzz_tools_directory']
    assert m['environment']['PATH'] == '/home/dev-user/.cargo/bin:/home/dev-user/.local/bin:/usr/bin:/bin'
    assert producer['elapsed_seconds'] < m['limits']['overall_seconds']
    assert m['environment']['RUSTFLAGS'] == '-Zexternal-clangrt -Clinker=clang -Clink-arg=-fsanitize=address'
    assert not any('TRUST' in k or k.startswith('CARGO_PROFILE_') for k in m['environment'])
    expected_initial = [(label, argv, 60) for label,argv in m['version_commands']]
    fuzz = S/'fuzz-source'; target_dir = Path(m['target_dir'])
    build_argv = [a.replace('@TARGET@',str(target_dir)).replace('@FUZZ_SOURCE@',str(fuzz)) for a in m['build_command']]
    expected_initial.append(('build',build_argv,1200))
    expected_initial += [('instrumentation/'+n,[m['nm'],'-D',str(S/'binaries'/n)],45) for n in TARGETS]
    assert labels[:10] == [n for n,_,_ in expected_initial]
    for i,(label,argv,bound) in enumerate(expected_initial):
        row = rows[label]
        assert row['argv'] == argv and row['outer_timeout_seconds'] == bound
        assert 0 <= row['elapsed_seconds'] < bound
        if i: assert commands[i-1]['end'] <= row['start']
    expected_fuzz = {'Cargo.lock':m['fuzz_source_sha256']['fuzz/Cargo.lock']}
    cargo = (W/'fuzz/Cargo.toml').read_text()
    for name in ['oriole','oriole_expat']:
        old = f'path = "../crates/{name}"'; assert cargo.count(old)==1
        cargo = cargo.replace(old,f'path = "{W}/crates/{name}"')
    assert (fuzz/'Cargo.toml').read_text() == cargo
    expected_fuzz['Cargo.toml'] = digest(fuzz/'Cargo.toml')
    expected_fuzz.update({f'fuzz_targets/{n}.rs':m['fuzz_source_sha256'][f'fuzz/fuzz_targets/{n}.rs'] for n in TARGETS})
    assert producer['copied_fuzz_source_sha256'] == expected_fuzz
    assert {str(p.relative_to(fuzz)):digest(p) for p in fuzz.rglob('*') if p.is_file()} == expected_fuzz
    actual = vectors((S/'build.log').read_text())
    saved_proof = load(S/'compiler-proof.json')
    assert pins[str(S/'compiler-proof.json')] == producer['compiler_proof_sha256']
    assert saved_proof['all_invocations'] == [r['argv'] for r in actual]
    required = {'oriole_storage','oriole','oriole_expat',*TARGETS}
    selected = [r for r in actual if crate(r) in required]
    assert len(selected)==9 and {crate(r) for r in selected}==required
    assert saved_proof['required_asan_crates'] == {crate(r):r['argv'] for r in selected}
    for item in selected:
        name,args = crate(item),item['argv']
        for flag in ['-Zsanitizer=address','-Zexternal-clangrt','-Clinker=clang',
                     '-Clink-arg=-fsanitize=address','-Cpasses=sancov-module',
                     '-Cllvm-args=-sanitizer-coverage-inline-8bit-counters',
                     '-Cllvm-args=-sanitizer-coverage-pc-table','-Ccodegen-units=1']:
            assert flag in args,(name,flag)
        assert args[args.index('--target')+1]=='x86_64-unknown-linux-gnu'
        assert str(Path(args[0]).resolve()) in m['tool_sha256']
        assert not any(s in ' '.join(args) for s in ['relink-only','public-api-hash','cache-proc-macros','profile-use','profile-generate','target-cpu','target-feature'])
        if name.startswith('oriole'): assert str(W/f'crates/{name}/src/lib.rs') in args
        else: assert f'fuzz_targets/{name}.rs' in args or str(fuzz/f'fuzz_targets/{name}.rs') in args
    assert set(producer['binaries']) == set(TARGETS)
    assert set(producer['targets']) == set(TARGETS)
    lane_map = {n:cpu for cpu,names in m['lanes'] for n in names}
    assert m['lanes']==[[1,['parse','multibyte']],[2,['streaming','value_family']],[4,['ffi','ffi_family']]]
    results=[]
    for name in TARGETS:
        binary=S/'binaries'/name
        assert digest(binary)==producer['binaries'][name]==digest(target_dir/'x86_64-unknown-linux-gnu/release'/name)
        pins[str(binary)] = digest(binary)
        symbols=read(S/'instrumentation'/(name+'.log')).decode()
        assert '__asan_init' in symbols and '__asan_report_load' in symbols
        d=S/'campaigns'/name; row=producer['targets'][name]
        initial=load(d/'initial-manifest.json')
        assert pins[str(d/'initial-manifest.json')]==row['initial_manifest_sha256']
        origin=m['corpora'][name]
        old_initial=load(origin['initial_manifest']);old_result=load(origin['result'])
        assert pins[origin['initial_manifest']]==origin['initial_manifest_sha256']
        assert pins[origin['result']]==origin['result_sha256'] and old_result['status']=='passed'
        expected_inputs={};expected_origins={}
        def add(data,h,label):
            assert hashlib.sha256(data).hexdigest()==h and len(data)<=65536
            expected_inputs[h]=len(data);expected_origins.setdefault(h,[]).append(label)
        for phase,entries in [('initial',{h:h for h in old_initial['inputs']}),('final',old_result['final_corpus'])]:
            folder=Path(origin[phase+'_directory']);assert hash_directory(folder)==entries
            for filename,h in entries.items():
                data=(folder/filename).read_bytes()
                if phase=='initial': assert len(data)==old_initial['inputs'][filename]
                add(data,h,str(folder/filename))
        for rel,h in m['fuzz_source_sha256'].items():
            if rel.startswith('fuzz/seeds/'+name+'/'):add((W/rel).read_bytes(),h,str(W/rel))
        for seed in m['derived_seeds']:
            if seed['target']==name:add(bytes(seed['header'])+seed['xml'].encode(),seed['sha256'],'manifest-derived:'+seed['name'])
        assert initial=={'inputs':expected_inputs,'origins':expected_origins}
        assert len(expected_inputs)==origin['expected_union_count']==row['initial_inputs']
        assert hash_directory(d/'initial-corpus')=={h:h for h in expected_inputs}
        assert all((d/'initial-corpus'/h).stat().st_size==size for h,size in expected_inputs.items())
        assert row['status']=='passed' and row['cpu']==lane_map[name]
        assert not row['artifacts'] and not failure_artifacts[name]
        replay_expected=sum(size!=0 for size in expected_inputs.values())+1
        coverage=[];executions={}
        for phase in ['replay','exploration']:
            command=rows[f'campaigns/{name}/{phase}']
            argv=['taskset','-c',str(lane_map[name]),str(binary),'-seed=20260912','-max_len=65536','-timeout=10','-rss_limit_mb=1536',f'-artifact_prefix={d}/artifacts/']
            argv += ['-runs=0',str(d/'initial-corpus')] if phase=='replay' else ['-max_total_time=600','-print_final_stats=1',str(d/'corpus'),str(d/'initial-corpus')]
            assert command['argv']==argv and command['outer_timeout_seconds']==(180 if phase=='replay' else 690)
            assert 0<command['elapsed_seconds']<command['outer_timeout_seconds']
            text=read(d/(phase+'.log')).decode(errors='replace')
            assert 'INFO: Seed: 20260912' in text
            assert not re.search(r'ERROR: (?:AddressSanitizer|libFuzzer)|SUMMARY: AddressSanitizer|ABORTING|thread .+ panicked',text)
            done=re.findall(r'^#(\d+)\s+DONE\b',text,re.M)
            ends=re.findall(r'^Done (\d+) runs in (\d+) second\(s\)$',text,re.M)
            assert len(done)==len(ends)==1 and int(done[0])==int(ends[0][0])
            count=int(done[0]);executions[phase]=count
            if phase=='replay': assert count==replay_expected==row['replay_expected']==row['replay_executions']
            else:
                stats={k:int(v) for k,v in re.findall(r'^stat::(\w+):\s+(\d+)',text,re.M)}
                assert stats==row['stats'] and count==stats['number_of_executed_units']>0
                assert int(ends[0][1])>=600 and command['elapsed_seconds']>=600 and stats['peak_rss_mb']<1536
            sizes=[size for size in expected_inputs.values() if size]
            seed_info=re.findall(r'^INFO: seed corpus: files: (\d+) min: (\d+)b max: (\d+)b total: (\d+)b',text,re.M)
            assert seed_info==[(str(len(sizes)),str(min(sizes)),str(max(sizes)),str(sum(sizes)))]
            counts=re.findall(r'^INFO: Loaded 1 modules\s+\((\d+) inline 8-bit counters\): (\d+) ',text,re.M)
            pc=re.findall(r'^INFO: Loaded 1 PC tables\s+\((\d+) PCs\): (\d+) ',text,re.M)
            assert len(counts)==1 and counts==pc and int(counts[0][0])>0 and counts[0][0]==counts[0][1]
            coverage.append(counts)
        assert coverage[0]==coverage[1]
        final=hash_directory(d/'corpus');assert final==row['final_corpus']
        for p in (d/'corpus').iterdir():
            data=p.read_bytes();assert len(data)<=65536 and hashlib.sha1(data).hexdigest()==p.name
        results.append({'target':name,'initial_inputs':len(expected_inputs),'replay_executions':executions['replay'],
            'exploration_executions':executions['exploration'],'final_corpus_files':len(final),
            'elapsed_exploration_seconds':rows[f'campaigns/{name}/exploration']['elapsed_seconds'],
            'coverage_counter_count':int(coverage[0][0][0]),'stats':row['stats'],'artifacts':{}})
    expected_labels={n for n,_,_ in expected_initial}|{f'campaigns/{n}/{phase}' for n in TARGETS for phase in ['replay','exploration']}
    assert set(labels)==expected_labels
    for cpu,names in m['lanes']:
        ordered=[rows[f'campaigns/{n}/{p}'] for n in names for p in ['replay','exploration']]
        assert [labels.index(r['label']) for r in ordered]==sorted(labels.index(r['label']) for r in ordered)
        assert rows['instrumentation/'+TARGETS[-1]]['end']<=ordered[0]['start']
        assert all(a['end']<=b['start'] for a,b in zip(ordered,ordered[1:]))
    totals={'targets':6,'initial_inputs':sum(r['initial_inputs'] for r in results),
        'replay_executions':sum(r['replay_executions'] for r in results),
        'exploration_executions':sum(r['exploration_executions'] for r in results),
        'final_corpus_files':sum(r['final_corpus_files'] for r in results),'failure_artifacts':0}
    assert totals['initial_inputs']==69201
    assert totals['replay_executions']==producer['total_replay_executions']
    assert totals['exploration_executions']==producer['total_exploration_executions']
    result.update(status='passed_independent_current_rust_asan_saved_review',campaign_status='passed',
        full_success_checks_completed=True,results=results,totals=totals,source_count=74,auxiliary_count=4,
        fuzz_source_files=254,unique_source_files=330,compiler_core_vectors=3,compiler_harness_vectors=6,
        full_compiler_invocations=len(actual),all_source_and_corpus_origins_exact=True)


parser=argparse.ArgumentParser()
parser.add_argument('--completed-report-sha256',required=True)
parser.add_argument('--released-manifest-sha256',required=True)
parser.add_argument('--completed',action='store_true',required=True)
args=parser.parse_args()
assert re.fullmatch('[0-9a-f]{64}',args.released_manifest_sha256)
RELEASED_MANIFEST=args.released_manifest_sha256
assert re.fullmatch('[0-9a-f]{64}',args.completed_report_sha256)
assert not OUT.exists()
producer=load(S/'report.json')
RUNTIME=producer['release']['source_commit']
assert RUNTIME is None or re.fullmatch('[0-9a-f]{40}',RUNTIME)
assert pins[str(S/'report.json')]==args.completed_report_sha256
assert producer['status'] in {'passed','failed'} and producer.get('all_children_reaped') is True
assert isinstance(producer.get('elapsed_seconds'),(int,float)) and producer['elapsed_seconds']>=0
result={'status':'incomplete','runtime_commit':RUNTIME,'base_commit':BASE,'released_manifest_sha256':RELEASED_MANIFEST,
    'completed_report_sha256':args.completed_report_sha256,'reader_sha256':digest(__file__),
    'role':'Saved-data reconstruction by the protocol adapter, independent of root target execution. No independent runtime-authorship claim. No producer or target execution.',
    'limitations':['Rust ASan for the three Oriole crates and six harnesses, using the external Clang runtime. detect_leaks=0; system libraries and Rust standard library are not claimed instrumented.',
        'Six retained seed corpora plus six prior FFI and five new declaration inputs are replay/exploration inputs, not exhaustive safety or branch-coverage proof. No speed claim.',
        'Actual compiler vectors and binary hashes are reconstructed, with saved nm captures; the reader does not rerun artifact tools.',
        'Saved terminal command records and the pinned controller establish recorded process-group cleanup and leader reaping. Successful runs do not test timeout/signal cleanup or independently prove global absence of orphan processes.'],
    'command_records':producer['commands'],'producer_error':producer.get('error'),
    'targets_executed':False,'cpu':6}
error=None
try:
    audit()
    for path,h in pins.items():assert digest(path)==h,path
except BaseException as exc:
    error=exc;result.update(status='failed_independent_saved_review',error=repr(exc),traceback=traceback.format_exc())
finally:
    result['pins_sha256']=pins
    with OUT.open('x') as stream:stream.write(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'status':result['status'],'path':str(OUT),'sha256':digest(OUT),'totals':result.get('totals')}))
if error is not None:raise error
if result['campaign_status']!='passed':raise SystemExit(1)
