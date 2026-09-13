"""Run the held three-harness ASan follow-up using the qualified supervisor."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import runpy
import shlex
import shutil
import time


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def load(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--build-sha256', required=True)
    parser.add_argument('--release', action='store_true', required=True)
    args = parser.parse_args()
    assert __debug__ and os.sched_getaffinity(0) == {6}
    assert args.release and sha(args.plan) == args.sha256
    plan = load(args.plan)
    assert sha(__file__) == plan['runner_sha256']
    root, out, target = (Path(plan[key]) for key in ['worktree', 'output', 'target_dir'])
    assert root.resolve() == root and not out.exists() and not target.exists()
    build_path = Path(plan['normal_build_binding'])
    assert sha(build_path) == args.build_sha256
    build = load(build_path)
    assert build['status'] == 'passed'
    assert [row['label'] for row in build['commands']] == ['rustc-version', 'fmt', 'tests', 'clippy', 'release']
    assert all(row['exit'] == 0 and row['reaped'] and not row.get('timed_out') for row in build['commands'])
    assert build['source'] == plan['source_binding']['source']
    assert build['base'] == plan['source_binding']['base']
    assert build['patch_sha256'] == plan['source_binding']['patch_sha256']
    assert all(sha(path) == value for path, value in build['artifacts'].items())
    pins = {**build['source'], **plan['fuzz_source_sha256']}

    def check_source():
        assert sha(args.plan) == args.sha256 and sha(__file__) == plan['runner_sha256']
        assert sha(build_path) == args.build_sha256
        assert all(sha(root / name) == value for name, value in pins.items())

    check_source()
    assert all(sha(path) == value for path, value in plan['tool_sha256'].items())
    controller = plan['existing_controller']
    assert sha(controller['path']) == controller['sha256']
    assert sha(plan['prior_report']) == plan['prior_report_sha256']
    prior = load(plan['prior_report'])
    assert prior['status'] == 'passed'
    for parent in [out.parent, target.parent]:
        space = os.statvfs(parent)
        assert space.f_bavail * space.f_frsize >= 8 * 1024**3
        assert space.f_favail >= 150000
    qualified = runpy.run_path(controller['path'])
    out.mkdir()
    shutil.copyfile(args.plan, out / 'plan.json')
    shutil.copyfile(__file__, out / 'run.py')
    shutil.copyfile(build_path, out / 'normal-build.json')
    runner = qualified['Runner'](out, {'limits': {'overall_seconds': 4500}})
    runner.report.update(plan_sha256=args.sha256, runner_sha256=plan['runner_sha256'],
                         normal_build_sha256=args.build_sha256, source_sha256=pins,
                         source_base=build['base'], source_patch_sha256=build['patch_sha256'],
                         limitations=plan['limitations'], prior_report_sha256=plan['prior_report_sha256'])
    runner.save()
    try:
        env, removed = qualified['clean_env']()
        for name in ['PYTHONPATH', 'PYTHONHOME']:
            if name in env:
                removed.append(name)
                del env[name]
        env.update(plan['environment'])
        assert Path(shutil.which('clang', path=env['PATH'])).resolve() == Path('/usr/bin/clang').resolve()
        runner.report.update(environment=plan['environment'], removed_environment=sorted(removed))
        fuzz = out / 'fuzz-source'
        (fuzz / 'fuzz_targets').mkdir(parents=True)
        for name in pins:
            if name.startswith('fuzz/fuzz_targets/'):
                shutil.copyfile(root / name, fuzz / 'fuzz_targets' / Path(name).name)
        cargo = (root / 'fuzz/Cargo.toml').read_text()
        for crate in ['oriole', 'oriole_expat']:
            old = f'path = "../crates/{crate}"'
            assert cargo.count(old) == 1
            cargo = cargo.replace(old, f'path = "{root}/crates/{crate}"')
        (fuzz / 'Cargo.toml').write_text(cargo)
        shutil.copyfile(root / 'fuzz/Cargo.lock', fuzz / 'Cargo.lock')
        fuzz_pins = {str(path.relative_to(fuzz)): sha(path) for path in fuzz.rglob('*') if path.is_file()}
        runner.report['copied_fuzz_source_sha256'] = fuzz_pins
        commands = {row['label']: row for row in plan['commands']}

        def command(label):
            row = commands[label]
            return runner.run(label, row['argv'], row['timeout_seconds'], env, root)

        runner.report['phase'] = 'build'
        vectors = []
        for name in plan['targets']:
            log = command('build-' + name)
            vectors.extend(shlex.split(line.split('Running `', 1)[1].rsplit('`', 1)[0])
                           for line in log.splitlines() if 'Running `' in line and '/rustc ' in line)
            check_source()
        proof = {}
        # Cargo-fuzz must instrument every target dependency, not just the three
        # local parser crates. Host build scripts do not carry a target argument.
        for argv in vectors:
            if '--target' in argv:
                assert argv[argv.index('--target') + 1] == 'x86_64-unknown-linux-gnu'
                assert '-Zsanitizer=address' in argv
        for crate in plan['instrumentation_validation']['required_crates']:
            rows = [v for v in vectors if '--crate-name' in v and v[v.index('--crate-name') + 1] == crate]
            assert rows, ('missing instrumented compiler invocation', crate)
            for argv in rows:
                assert '-Zsanitizer=address' in argv
                assert argv[argv.index('--target') + 1] == 'x86_64-unknown-linux-gnu'
                assert str(Path(argv[0]).resolve()) in plan['tool_sha256']
                assert not any(flag in arg for arg in argv for flag in plan['instrumentation_validation']['forbidden_flags'])
            proof[crate] = rows
        write(out / 'compiler-proof.json', {'required_asan_crates': proof, 'all_invocations': vectors})
        assert all(sha(fuzz / name) == value for name, value in fuzz_pins.items())
        (out / 'binaries').mkdir()
        binaries = {}
        for name in plan['targets']:
            shutil.copy2(target / 'x86_64-unknown-linux-gnu/release' / name, out / 'binaries' / name)
            symbols = command('symbols-' + name)
            assert '__asan_init' in symbols and '__asan_report_load' in symbols
            binaries[name] = sha(out / 'binaries' / name)
        runner.report.update(binaries=binaries, compiler_proof_sha256=sha(out / 'compiler-proof.json'))
        runner.report['phase'] = 'corpus-copy'
        for name in plan['targets']:
            campaign = out / 'campaigns' / name
            seeds = campaign / 'initial-corpus'
            seeds.mkdir(parents=True)
            (campaign / 'corpus').mkdir()
            (campaign / 'artifacts').mkdir()
            origin = plan['corpora'][name]
            assert sha(origin['initial_manifest']) == origin['initial_manifest_sha256']
            initial = load(origin['initial_manifest'])['inputs']
            inputs = {}

            def add(data, expected):
                runner.check()
                assert hashlib.sha256(data).hexdigest() == expected and len(data) <= 65536
                if expected not in inputs:
                    (seeds / expected).write_bytes(data)
                    (seeds / expected).chmod(0o444)
                    inputs[expected] = len(data)

            for phase, files in [('initial', {key: key for key in initial}),
                                 ('final', prior['targets'][name]['final_corpus'])]:
                directory = Path(origin[phase + '_directory'])
                assert {path.name for path in directory.iterdir() if path.is_file()} == set(files)
                for filename, expected in files.items():
                    data = (directory / filename).read_bytes()
                    if phase == 'initial':
                        assert len(data) == initial[filename]
                    add(data, expected)
            for seed in plan['derived_seeds']:
                if seed['target'] == name:
                    add(bytes(seed['header']) + seed['xml'].encode(), seed['sha256'])
            assert len(inputs) == origin['planned_union_count']
            write(campaign / 'initial-manifest.json', {'inputs': inputs, 'origin': origin})
            runner.report['targets'][name] = {
                'status': 'pending', 'initial_inputs': len(inputs),
                'replay_expected': sum(size != 0 for size in inputs.values()) + 1,
                'initial_manifest_sha256': sha(campaign / 'initial-manifest.json')}
            runner.save()
        runner.report['phase'] = 'replay-and-exploration'
        for name in plan['targets']:
            row = runner.report['targets'][name]
            campaign = out / 'campaigns' / name
            assert sha(out / 'binaries' / name) == binaries[name]
            row['status'] = 'running'
            replay = command('replay-' + name)
            done = re.findall(r'^#(\d+)\s+DONE\b', replay, re.M)
            assert done and int(done[-1]) == row['replay_expected']
            row['replay_executions'] = int(done[-1])
            assert not any((campaign / 'artifacts').iterdir())
            exploration = command('explore-' + name)
            stats = {key: int(value) for key, value in re.findall(r'stat::(\w+):\s+(\d+)', exploration)}
            assert stats.get('number_of_executed_units', 0) > 0
            assert not any((campaign / 'artifacts').iterdir())
            initial = load(campaign / 'initial-manifest.json')['inputs']
            assert {path.name: sha(path) for path in (campaign / 'initial-corpus').iterdir()} == {key: key for key in initial}
            row.update(status='passed', stats=stats,
                       final_corpus={path.name: sha(path) for path in (campaign / 'corpus').iterdir() if path.is_file()})
            check_source()
            runner.save()
        assert all(sha(out / 'binaries' / name) == value for name, value in binaries.items())
        assert all(sha(fuzz / name) == value for name, value in fuzz_pins.items())
        check_source()
        runner.report.update(status='passed', source_unchanged=True,
                             total_exploration_executions=sum(row['stats']['number_of_executed_units'] for row in runner.report['targets'].values()),
                             total_replay_executions=sum(row['replay_executions'] for row in runner.report['targets'].values()))
    except BaseException as error:
        runner.report.update(status='failed', error=repr(error))
        raise
    finally:
        for pid in list(runner.active):
            runner.finish(pid, aborted=True)
        for name, row in runner.report['targets'].items():
            row['artifacts'] = {path.name: sha(path) for path in (out / 'campaigns' / name / 'artifacts').iterdir() if path.is_file()}
        runner.report.update(elapsed_seconds=time.monotonic() - runner.started,
                             all_children_reaped=not runner.active)
        runner.save()
        print(json.dumps({'status': runner.report['status'], 'report': str(out / 'report.json')}), flush=True)


if __name__ == '__main__':
    main()
