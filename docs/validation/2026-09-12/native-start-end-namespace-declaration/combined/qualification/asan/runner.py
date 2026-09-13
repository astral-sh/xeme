"""Held one-shot Rust ASan build, retained-corpus replay and six fresh fuzz campaigns."""
from pathlib import Path
import argparse, hashlib, json, os, re, shlex, shutil, signal, subprocess, time


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def load(path):
    return json.loads(Path(path).read_text())


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


def clean_env():
    env = dict(os.environ)
    exact = {'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTDOCFLAGS',
             'CARGO_ENCODED_RUSTDOCFLAGS', 'RUSTC', 'RUSTDOC', 'RUSTUP_TOOLCHAIN',
             'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_RUSTFLAGS',
             'CARGO_BUILD_RUSTC', 'CARGO_BUILD_RUSTDOC', 'CARGO_BUILD_RUSTC_WRAPPER',
             'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER', 'LD_PRELOAD', 'LD_LIBRARY_PATH',
             'LLVM_PROFILE_FILE', 'ASAN_OPTIONS', 'LSAN_OPTIONS', 'UBSAN_OPTIONS',
             'TSAN_OPTIONS', 'MSAN_OPTIONS', 'CC', 'CXX', 'CFLAGS', 'CXXFLAGS',
             'CPPFLAGS', 'LDFLAGS', 'AR', 'RANLIB',
             'CARGO_UNSTABLE_OHM_PROC_MACRO_TRUST', 'CARGO_UNSTABLE_OHM_NATIVE_TOOL_TRUST'}
    removed = []
    for name in list(env):
        if name in exact or name.startswith('CARGO_PROFILE_') or (
            name.startswith('CARGO_TARGET_') and name.endswith(('_RUSTFLAGS', '_LINKER', '_RUNNER'))
        ):
            removed.append(name)
            del env[name]
    return env, sorted(removed)


class Runner:
    """Own and reap each command's process group, including timeout/signal exits."""
    def __init__(self, out, manifest):
        self.out, self.manifest = out, manifest
        self.active = {}
        self.abort = None
        self.started = time.monotonic()
        self.last_progress = self.started
        self.report = {'status': 'running', 'phase': 'binding', 'commands': [], 'targets': {}}
        for signum in (signal.SIGTERM, signal.SIGINT):
            signal.signal(signum, lambda number, _: setattr(self, 'abort', number))

    def save(self):
        save(self.out / 'report.json', self.report)

    def check(self):
        if self.abort is not None:
            raise KeyboardInterrupt(f'signal {self.abort}')
        if time.monotonic() - self.started > self.manifest['limits']['overall_seconds']:
            raise TimeoutError('whole Rust ASan validation deadline')
        if time.monotonic() - self.last_progress >= 30:
            print(json.dumps({'phase': self.report['phase'], 'active': [x['row']['label'] for x in self.active.values()]}), flush=True)
            self.last_progress = time.monotonic()

    def launch(self, label, argv, timeout, env, cwd):
        self.check()
        log = self.out / (label + '.log')
        log.parent.mkdir(parents=True, exist_ok=True)
        row = {'label': label, 'argv': argv, 'cwd': str(cwd), 'outer_timeout_seconds': timeout,
               'start': time.time(), 'reaped': False}
        self.report['commands'].append(row)
        self.save()
        with log.open('xb') as stream:
            process = subprocess.Popen(argv, cwd=cwd, env=env, stdout=stream,
                                       stderr=subprocess.STDOUT, start_new_session=True)
        row['pid'] = process.pid
        self.active[process.pid] = {'process': process, 'row': row, 'log': log,
                                    'started': time.monotonic(), 'timeout': timeout}
        self.save()
        self.check()  # Handles a signal arriving between spawn and registration.
        print(json.dumps({'started': label, 'pid': process.pid}), flush=True)
        return process.pid

    def finish(self, pid, aborted=False):
        item = self.active[pid]
        process, row = item['process'], item['row']
        # Kill surviving descendants even when the group leader has already exited.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        code = process.wait()
        row.update(exit_code=code, end=time.time(), reaped=True,
                   elapsed_seconds=time.monotonic() - item['started'], log_sha256=digest(item['log']))
        if aborted:
            row['controller_aborted'] = True
        del self.active[pid]
        self.save()
        print(json.dumps({'completed': row['label'], 'exit': code}), flush=True)
        return row

    def poll(self):
        self.check()
        completed = []
        for pid, item in list(self.active.items()):
            if item['process'].poll() is not None:
                completed.append(self.finish(pid))
            elif time.monotonic() - item['started'] > item['timeout']:
                item['row']['outer_timeout'] = True
                completed.append(self.finish(pid))
        return completed

    def run(self, label, argv, timeout, env, cwd):
        pid = self.launch(label, argv, timeout, env, cwd)
        while pid in self.active:
            for row in self.poll():
                assert row['exit_code'] == 0 and not row.get('outer_timeout'), row['label']
            time.sleep(0.25)
        return (self.out / (label + '.log')).read_text(errors='replace')


def main():
    args = argparse.ArgumentParser()
    args.add_argument('--manifest', type=Path, required=True)
    args.add_argument('--sha256', required=True)
    opts = args.parse_args()
    assert __debug__ and os.sched_getaffinity(0) == {6}
    assert digest(opts.manifest) == opts.sha256
    m = load(opts.manifest)
    assert digest(__file__) == m['runner_sha256']
    assert m['release']['status'] == 'released', 'Root must bind the selected source/libraries and release this held manifest.'
    release = m['release']
    assert release['source_commit'] is None or re.fullmatch('[0-9a-f]{40}', release['source_commit'])
    assert re.fullmatch('[0-9a-f]{40}', release['base_commit'])
    root, out, target = (Path(m[k]) for k in ['source_root', 'output', 'target_dir'])
    assert root.is_dir() and not out.exists() and not target.exists()
    assert root.resolve() == root and str(root).startswith('/home/dev-user/code/oss/')
    assert str(target).startswith('/home/dev-user/.cache/oriole/')
    assert str(out).startswith('/tmp/') and out != root
    for parent in [out.parent, target.parent]:
        space = os.statvfs(parent)
        assert space.f_bavail * space.f_frsize >= m['limits']['min_free_bytes'], parent
        assert space.f_favail >= m['limits']['min_free_inodes'], parent
    source_path, binding_path = Path(m['normal_source_manifest']), Path(m['normal_build_binding'])
    assert digest(source_path) == release['normal_source_manifest_sha256']
    assert digest(binding_path) == release['normal_build_binding_sha256']
    source, binding = load(source_path), load(binding_path)
    assert binding['status'] == 'passed' and binding['source_manifest_sha256'] == digest(source_path)
    assert binding['tests'] == dict(passed=483, groups=35, failures=0, ignored=0)
    assert digest(m['prepared_source']['path']) == m['prepared_source']['sha256'] == binding['prepared_source_sha256']
    assert source['sha256'] == load(m['prepared_source']['path'])['sha256']
    assert len(source['sha256']) == 74 and source['worktree'] == str(root)
    assert binding['candidate_libraries'] == release['normal_libraries']
    for name, value in release['normal_libraries'].items():
        assert digest(binding_path.parent / 'normal' / name) == value
    for path, value in m['tool_sha256'].items():
        assert digest(path) == value, path
    assert all(m['tool_sha256'].get(path) == value for path, value in binding['tool_hashes'].items())
    git = lambda *argv: subprocess.check_output(['/usr/bin/git', '--no-optional-locks', '-C', str(root), *argv], timeout=30)
    assert source['base'] == binding['base'] == release['base_commit']
    assert sorted(binding['source_delta']) == release['source_delta'] and len(release['source_delta']) == 12
    head = git('rev-parse', 'HEAD').decode().strip()
    assert head == (release['source_commit'] or release['base_commit'])
    patch = git('diff', '--no-ext-diff', '--binary', release['base_commit'], '--', *release['source_delta'])
    patch_hash = hashlib.sha256(patch).hexdigest()
    assert patch_hash == release['candidate_patch_sha256'] == binding['complete_patch_sha256']
    assert digest(binding_path.parent / 'candidate.patch') == patch_hash
    source_identity = {'source_commit': release['source_commit'], 'base_commit': release['base_commit'], 'candidate_patch_sha256': patch_hash,
                       'source_delta': release['source_delta'], 'unchanged_base_blobs_verified': True}
    for name in binding['auxiliary_files'].keys() & m['fuzz_source_sha256'].keys():
        assert binding['auxiliary_files'][name] == m['fuzz_source_sha256'][name]
    pins = {**source['sha256'], **binding['auxiliary_files'], **m['fuzz_source_sha256']}
    for name, value in pins.items():
        assert digest(root / name) == value, name
        if name not in binding['source_delta']:
            assert hashlib.sha256(git('show', release['base_commit'] + ':' + name)).hexdigest() == value, name
    def check_source():
        assert all(digest(root / name) == value for name, value in pins.items())
        assert digest(opts.manifest) == opts.sha256 and digest(__file__) == m['runner_sha256']
    out.mkdir()
    shutil.copyfile(opts.manifest, out / 'command-manifest.json')
    shutil.copyfile(__file__, out / 'runner.py')
    shutil.copyfile(source_path, out / 'normal-source.json')
    shutil.copyfile(binding_path, out / 'normal-build-binding.json')
    shutil.copyfile(binding_path.parent / 'candidate.patch', out / 'normal-candidate.patch')
    r = Runner(out, m)
    r.report.update(manifest_sha256=opts.sha256, runner_sha256=m['runner_sha256'],
                    release=release, source_identity=source_identity, source_sha256=pins, normal_libraries_are_provenance_only=True,
                    scope='Fresh Rust ASan fuzz binaries, not the uninstrumented normal libraries. Six unchanged harnesses; historical corpora are seeds only.')
    r.save()
    try:
        fuzz = out / 'fuzz-source'
        (fuzz / 'fuzz_targets').mkdir(parents=True)
        for name in m['targets']:
            shutil.copyfile(root / 'fuzz/fuzz_targets' / (name + '.rs'), fuzz / 'fuzz_targets' / (name + '.rs'))
        cargo = (root / 'fuzz/Cargo.toml').read_text()
        for crate in ['oriole', 'oriole_expat']:
            old = f'path = "../crates/{crate}"'
            assert cargo.count(old) == 1
            cargo = cargo.replace(old, f'path = "{root}/crates/{crate}"')
        (fuzz / 'Cargo.toml').write_text(cargo)
        shutil.copyfile(root / 'fuzz/Cargo.lock', fuzz / 'Cargo.lock')
        fuzz_pins = {str(p.relative_to(fuzz)): digest(p) for p in fuzz.rglob('*') if p.is_file()}
        r.report['copied_fuzz_source_sha256'] = fuzz_pins
        env, removed = clean_env()
        env.update(m['environment'])
        env['PATH'] = m['fuzz_tools_directory'] + ':' + env['PATH']
        env['CARGO_TARGET_DIR'] = str(target)
        env['ASAN_OPTIONS'] = m['asan_options']
        assert Path(shutil.which('clang', path=env['PATH'])).resolve() == Path(m['clang']).resolve()
        assert Path(shutil.which('cargo-fuzz', path=env['PATH'])).resolve() == Path(m['fuzz_tools_directory'], 'cargo-fuzz').resolve()
        r.report.update(environment=m['environment'], removed_environment=removed,
                        asan_options=m['asan_options'], path_prefix=m['fuzz_tools_directory'])
        r.report['phase'] = 'build'
        for label, argv in m['version_commands']:
            r.run(label, argv, 60, env, root)
        command = [arg.replace('@TARGET@', str(target)).replace('@FUZZ_SOURCE@', str(fuzz)) for arg in m['build_command']]
        r.run('build', command, m['limits']['build_seconds'], env, root)
        assert all(digest(fuzz / name) == value for name, value in fuzz_pins.items())
        check_source()
        vectors = []
        for line in (out / 'build.log').read_text().splitlines():
            if 'Running `' in line and '/rustc ' in line:
                vectors.append(shlex.split(line.split('Running `', 1)[1].rsplit('`', 1)[0]))
        proof = {}
        for crate in ['oriole_storage', 'oriole', 'oriole_expat'] + m['targets']:
            rows = [v for v in vectors if '--crate-name' in v and v[v.index('--crate-name') + 1] == crate]
            assert len(rows) == 1, (crate, len(rows))
            v = rows[0]
            assert '-Zsanitizer=address' in v, crate
            assert v[v.index('--target') + 1] == 'x86_64-unknown-linux-gnu'
            assert str(Path(v[0]).resolve()) in m['tool_sha256']
            assert not any(any(flag in arg for flag in ['profile-use', 'profile-generate', 'target-cpu', 'relink-only', 'public-api-hash', 'cache-proc-macros']) for arg in v)
            proof[crate] = v
        save(out / 'compiler-proof.json', {'all_invocations': vectors, 'required_asan_crates': proof})
        binaries = {}
        (out / 'binaries').mkdir()
        for name in m['targets']:
            binary = target / 'x86_64-unknown-linux-gnu/release' / name
            assert binary.is_file()
            shutil.copy2(binary, out / 'binaries' / name)
            binaries[name] = digest(out / 'binaries' / name)
            symbols = r.run('instrumentation/' + name, [m['nm'], '-D', str(out / 'binaries' / name)], 45, env, root)
            assert '__asan_init' in symbols and '__asan_report_load' in symbols, name
        r.report.update(binaries=binaries, compiler_proof_sha256=digest(out / 'compiler-proof.json'))
        r.report['phase'] = 'corpus-copy'
        for name in m['targets']:
            r.check()
            origin = m['corpora'][name]
            assert digest(origin['initial_manifest']) == origin['initial_manifest_sha256']
            assert digest(origin['result']) == origin['result_sha256']
            initial = load(origin['initial_manifest'])['inputs']
            previous = load(origin['result'])
            assert previous['status'] == 'passed'
            campaign = out / 'campaigns' / name
            seeds = campaign / 'initial-corpus'
            seeds.mkdir(parents=True)
            inputs, origins = {}, {}
            def add(data, expected, label):
                value = hashlib.sha256(data).hexdigest()
                assert value == expected and len(data) <= m['limits']['max_len']
                dest = seeds / value
                if not dest.exists():
                    dest.write_bytes(data)
                    dest.chmod(0o444)
                inputs[value] = len(data)
                origins.setdefault(value, []).append(label)
            for phase, entries in [('initial', {h: h for h in initial}), ('final', previous['final_corpus'])]:
                folder = Path(origin[phase + '_directory'])
                assert {p.name for p in folder.iterdir() if p.is_file()} == set(entries)
                for filename, expected in entries.items():
                    r.check()
                    data = (folder / filename).read_bytes()
                    if phase == 'initial': assert len(data) == initial[filename]
                    add(data, expected, str(folder / filename))
            for rel, expected in m['fuzz_source_sha256'].items():
                if rel.startswith('fuzz/seeds/' + name + '/'):
                    add((root / rel).read_bytes(), expected, str(root / rel))
            for seed in m['derived_seeds']:
                if seed['target'] == name:
                    data = bytes(seed['header']) + seed['xml'].encode()
                    add(data, seed['sha256'], 'manifest-derived:' + seed['name'])
            assert len(inputs) == origin['expected_union_count'], name
            save(campaign / 'initial-manifest.json', {'inputs': inputs, 'origins': origins})
            (campaign / 'corpus').mkdir()
            (campaign / 'artifacts').mkdir()
            r.report['targets'][name] = {'status': 'pending', 'initial_inputs': len(inputs),
                'replay_expected': sum(size != 0 for size in inputs.values()) + 1,
                'initial_manifest_sha256': digest(campaign / 'initial-manifest.json')}
        r.report['phase'] = 'replay-and-exploration'
        state_by_pid = {}
        pending = {cpu: list(names) for cpu, names in m['lanes']}
        def launch(cpu, name, phase):
            campaign = out / 'campaigns' / name
            binary = out / 'binaries' / name
            assert digest(binary) == binaries[name]
            row = r.report['targets'][name]
            row.update(status='running', cpu=cpu)
            command = ['taskset', '-c', str(cpu), str(binary), f"-seed={m['seed']}",
                       f"-max_len={m['limits']['max_len']}", f"-timeout={m['limits']['input_seconds']}",
                       f"-rss_limit_mb={m['limits']['rss_mb']}", f'-artifact_prefix={campaign}/artifacts/']
            if phase == 'replay':
                command += ['-runs=0', str(campaign / 'initial-corpus')]
                timeout = m['limits']['replay_seconds']
            else:
                command += [f"-max_total_time={m['limits']['exploration_seconds']}", '-print_final_stats=1',
                            str(campaign / 'corpus'), str(campaign / 'initial-corpus')]
                timeout = m['limits']['exploration_outer_seconds']
            pid = r.launch('campaigns/' + name + '/' + phase, command, timeout, env, root)
            state_by_pid[pid] = (cpu, name, phase)
        for cpu, names in pending.items():
            launch(cpu, names.pop(0), 'replay')
        while r.active:
            for command in r.poll():
                cpu, name, phase = state_by_pid.pop(command['pid'])
                row = r.report['targets'][name]
                campaign = out / 'campaigns' / name
                assert command['exit_code'] == 0 and not command.get('outer_timeout'), (name, phase)
                artifacts = {p.name: digest(p) for p in (campaign / 'artifacts').iterdir() if p.is_file()}
                row['artifacts'] = artifacts
                assert not artifacts, (name, 'finding artifacts')
                text = (campaign / (phase + '.log')).read_text(errors='replace')
                if phase == 'replay':
                    done = re.findall(r'^#(\d+)\s+DONE\b', text, re.M)
                    row['replay_executions'] = int(done[-1]) if done else None
                    assert row['replay_executions'] == row['replay_expected'], name
                    launch(cpu, name, 'exploration')
                else:
                    stats = {k: int(v) for k, v in re.findall(r'stat::(\w+):\s+(\d+)', text)}
                    assert stats.get('number_of_executed_units', 0) > 0, name
                    initial = load(campaign / 'initial-manifest.json')['inputs']
                    assert {p.name: digest(p) for p in (campaign / 'initial-corpus').iterdir()} == {h: h for h in initial}
                    row.update(status='passed', stats=stats,
                               final_corpus={p.name: digest(p) for p in (campaign / 'corpus').iterdir() if p.is_file()})
                    check_source()
                    r.save()
                    if pending[cpu]: launch(cpu, pending[cpu].pop(0), 'replay')
            time.sleep(0.25)
        assert all(row['status'] == 'passed' for row in r.report['targets'].values())
        assert len(r.report['targets']) == 6
        assert all(digest(out / 'binaries' / n) == h for n, h in binaries.items())
        check_source()
        r.report.update(status='passed', source_unchanged=True,
                        total_exploration_executions=sum(x['stats']['number_of_executed_units'] for x in r.report['targets'].values()),
                        total_replay_executions=sum(x['replay_executions'] for x in r.report['targets'].values()))
    except BaseException as error:
        r.report.update(status='failed', error=repr(error))
        raise
    finally:
        for pid in list(r.active):
            r.finish(pid, aborted=True)
        for name, row in r.report['targets'].items():
            if row['status'] == 'pending': row['unstarted'] = True
            campaign = out / 'campaigns' / name
            row['artifacts'] = {p.name: digest(p) for p in (campaign / 'artifacts').iterdir() if p.is_file()}
            if row['status'] != 'passed': row['status'] = 'incomplete'
        r.report.update(elapsed_seconds=time.monotonic() - r.started, all_children_reaped=not r.active,
                        unstarted_targets=[n for n in m['targets'] if n not in r.report['targets'] or r.report['targets'][n].get('unstarted', False)])
        r.save()
        print(json.dumps({'status': r.report['status'], 'report': str(out / 'report.json')}), flush=True)


if __name__ == '__main__':
    main()
