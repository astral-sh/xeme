"""Ordinary checks/build for the core-owned-text-raw candidate; no benchmarks."""
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import time

ROOT = Path('/home/dev-user/code/oss/oriole-core-owned-text-raw')
OUT = Path('/tmp/oriole-core-owned-text-raw-study')
TARGET = Path('/home/dev-user/.cache/oriole/core-owned-text-raw-target')
PREP = Path(__file__).parent
OUT.mkdir(exist_ok=True)
assert not (OUT / 'build.json').exists()
assert __debug__ and os.sched_getaffinity(0) == {4}
def interrupted(signum, frame):
    raise KeyboardInterrupt('SIGTERM')
signal.signal(signal.SIGTERM, interrupted)

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

env = os.environ.copy()
for key in list(env):
    if key.startswith('CARGO_PROFILE_') or (key.startswith('CARGO_TARGET_') and key.endswith('_RUSTFLAGS')) or key in {
        'RUSTC', 'RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'LLVM_PROFILE_FILE',
        'CARGO_UNSTABLE_OHM_PROC_MACRO_TRUST', 'CARGO_UNSTABLE_OHM_NATIVE_TOOL_TRUST',
        'LD_PRELOAD', 'LD_LIBRARY_PATH',
    }:
        env.pop(key)
env.update({
    'CARGO_HOME': '/home/dev-user/.cache/toucan/cargo',
    'CARGO_TARGET_DIR': str(TARGET),
    'CARGO_BUILD_BUILD_DIR': '/home/dev-user/.cache/ohm/verified-build/{workspace-path-hash}',
    'CARGO_BUILD_JOBS': '1', 'CARGO_INCREMENTAL': '0',
    'RUSTC_WRAPPER': '', 'RUSTC_WORKSPACE_WRAPPER': '',
    'CARGO_ENCODED_RUSTFLAGS': '', 'CARGO_TERM_COLOR': 'never',
})
inputs = json.loads((PREP / 'build-inputs.json').read_text())
assert sha(__file__) == inputs['build_script_sha256']
assert sha(PREP / 'source.json') == inputs['source_sha256']
assert sha(PREP / 'change.patch') == inputs['patch_sha256']
assert sha(PREP / 'candidate.patch') == inputs['git_patch_sha256']
prepared = json.loads((PREP / 'source.json').read_text())
base = subprocess.check_output(['/usr/bin/git', '--no-optional-locks', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
assert base == prepared['base'] == '2c4d21647efdefe0b7da977ca206b8552c32f0f3'
assert subprocess.check_output(['/usr/bin/git', '--no-optional-locks', 'config', 'user.email'], cwd=ROOT, text=True).strip() == 'charlie.r.marsh@gmail.com'
source = prepared['sha256']
assert len(source) == 74 and all(sha(ROOT / name) == value for name, value in source.items())
auxiliary = prepared['auxiliary_files']
assert len(auxiliary) == 4 and all(sha(ROOT / name) == value for name, value in auxiliary.items())
parent = json.loads((PREP / 'inputs.json').read_text())
assert all(sha(Path(parent['parent_worktree']) / name) == value for name, value in parent['source74'].items())
control = Path('/tmp/oriole-outer-whitespace-separated-dispatch-study')
old_binding = json.loads((control / 'build-binding.json').read_text())
metadata_review_path = Path('/tmp/oriole-text-lane-masks-preparation/metadata-independent-review.json')
metadata_receipt_path = Path('/tmp/oriole-text-lane-masks-preparation/format-metadata.json')
assert sha(metadata_review_path) == old_binding['dependency_review_sha256']
assert sha(metadata_receipt_path) == old_binding['metadata_receipt_sha256']
assert auxiliary == old_binding['auxiliary_files']
control_source = json.loads((control / 'source.json').read_text())['sha256']
for name in source:
    if name.endswith('Cargo.toml') or name.endswith('Cargo.lock'):
        assert source[name] == control_source[name]

(OUT / 'source.json').write_text(json.dumps({'base': base, 'worktree': str(ROOT), 'sha256': source}, indent=2) + '\n')
(OUT / 'candidate.patch').write_bytes((PREP / 'candidate.patch').read_bytes())
report = {'status': 'incomplete', 'source_sha256': sha(OUT / 'source.json'), 'commands': [],
          'dependency_review_sha256': sha(metadata_review_path), 'auxiliary_files': auxiliary,
          'metadata_receipt_sha256': sha(metadata_receipt_path),
          'scope': 'Ordinary O3 ThinLTO, one codegen unit, generic x86-64 release. No profile generation/use or compiler optimization changes.'}


def save():
    (OUT / 'build.json').write_text(json.dumps(report, indent=2) + '\n')


def run(label, argv):
    path = OUT / (label + '.log')
    row = {'label': label, 'argv': argv, 'start': time.time()}
    report['commands'].append(row)
    save()
    with path.open('wb') as log:
        proc = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            row['pid'] = proc.pid
            row['exit'] = proc.wait(timeout=900)
        finally:
            if proc.poll() is None:
                os.killpg(proc.pid, signal.SIGKILL)
                row['exit'] = proc.wait()
            row.update(reaped=True, end=time.time(), log_sha256=sha(path))
            save()
    print(label, row['exit'], flush=True)
    assert row['exit'] == 0
    return path.read_text()


try:
    report['rustc'] = run('rustc-version', ['rustc', '+ohm', '-vV'])
    cargo = ['cargo', '+ohm', '-Zohm-defaults=no']
    run('fmt', cargo + ['fmt', '--all', '--check'])
    run('tests', cargo + ['test', '--workspace', '--locked', '--offline'])
    run('clippy', cargo + ['clippy', '--workspace', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings'])
    run('benchmark-fmt', cargo + ['fmt', '--manifest-path', 'benchmarks/inprocess/Cargo.toml', '--check'])
    run('benchmark-clippy', cargo + ['clippy', '--manifest-path', 'benchmarks/inprocess/Cargo.toml', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings'])
    build = run('release', cargo + ['rustc', '--release', '--locked', '--offline', '--target', 'x86_64-unknown-linux-gnu', '-p', 'oriole_expat', '--lib', '--crate-type', 'cdylib,staticlib', '--verbose'])
    vectors = []
    dependency_vectors = []
    for line in build.splitlines():
        if 'Running `' in line and '/rustc ' in line:
            argv = shlex.split(line.strip()[len('Running `'):-1])
            if argv[argv.index('--crate-name') + 1] in {'wide', 'safe_arch', 'bytemuck'}:
                dependency_vectors.append(argv)
            if argv[argv.index('--crate-name') + 1] in {'oriole', 'oriole_expat', 'oriole_storage'}:
                assert not any('profile-generate' in arg or 'profile-use' in arg or 'target-cpu' in arg for arg in argv)
                vectors.append(argv)
    assert len(vectors) == 3
    for argv in vectors:
        crate = argv[argv.index('--crate-name') + 1]
        codegen = [argv[index + 1] for index, arg in enumerate(argv[:-1]) if arg == '-C']
        assert 'opt-level=3' in codegen and 'codegen-units=1' in codegen
        assert argv[argv.index('--target') + 1] == 'x86_64-unknown-linux-gnu'
        assert not any(any(flag in arg for flag in ['profile-generate', 'profile-use', 'target-cpu', 'target-feature', 'sanitizer']) for arg in argv)
        if crate == 'oriole_expat':
            assert 'lto=thin' in codegen
            assert [argv[index + 1] for index, arg in enumerate(argv[:-1]) if arg == '--crate-type'] == ['cdylib', 'staticlib']
        else:
            assert 'linker-plugin-lto' in codegen
    report['compiler_vectors'] = vectors
    report['dependency_compiler_vectors'] = dependency_vectors
    normal = OUT / 'normal'
    normal.mkdir()
    report['libraries'] = {}
    for name in ['liboriole_expat.so', 'liboriole_expat.a']:
        shutil.copy2(TARGET / 'x86_64-unknown-linux-gnu' / 'release' / name, normal / name)
        report['libraries'][name] = sha(normal / name)
    report['status'] = 'passed'
except BaseException as error:
    report['status'] = 'failed'
    report['failure'] = {'type': type(error).__name__, 'message': str(error)}
    raise
finally:
    report['source_unchanged'] = all(sha(ROOT / name) == value for name, value in source.items())
    report['auxiliary_unchanged'] = all(sha(ROOT / name) == value for name, value in auxiliary.items())
    assert report['auxiliary_unchanged']
    save()
assert report['source_unchanged']
