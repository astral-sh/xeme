"""Bounded direct-link C validation using frozen selected libraries; no Rust rebuild."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import signal
import subprocess
import time

if not __debug__:
    raise RuntimeError('Assertions required')
HERE = Path(__file__).resolve().parent
PREP = json.loads((HERE / 'checks-preparation.json').read_text())
OUT = HERE / 'checks-attempt01'
assert os.sched_getaffinity(0) == {3}
OUT.mkdir(exist_ok=False)
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
observed = {**PREP['source_pins'], **{a['path']: a['sha256'] for a in PREP['arms']}}
observed[str(HERE / 'checks-preparation.json')] = sha(HERE / 'checks-preparation.json')
observed[str(Path(__file__))] = sha(Path(__file__))
report = {'status': 'incomplete', 'preparation': PREP, 'before': {}, 'commands': [], 'workers': [], 'controller_sha256': sha(Path(__file__))}
save = lambda: (OUT / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
active = None
def stop_owned():
    global active
    if active is not None:
        if active.poll() is None:
            os.killpg(active.pid, signal.SIGKILL)
        active.wait()
        active = None
def interrupted(signum, _frame):
    stop_owned()
    raise KeyboardInterrupt(str(signum))
for sig in (signal.SIGTERM, signal.SIGINT):
    signal.signal(sig, interrupted)
env = dict(os.environ)
for name in ['LD_PRELOAD', 'LD_LIBRARY_PATH', 'ASAN_OPTIONS', 'LSAN_OPTIONS', 'UBSAN_OPTIONS', 'CFLAGS', 'CPPFLAGS', 'LDFLAGS', 'LLVM_PROFILE_FILE']:
    env.pop(name, None)
env.update(ASAN_OPTIONS='detect_leaks=0:abort_on_error=1', UBSAN_OPTIONS='halt_on_error=1')
def run(label, argv, seconds, run_env):
    global active
    row = {'label': label, 'command': ['taskset', '-c', '3', *argv], 'timeout': seconds, 'start': time.time()}
    report['commands'].append(row)
    save()
    with (OUT / (label + '.stdout')).open('xb') as out, (OUT / (label + '.stderr')).open('xb') as err:
        try:
            active = subprocess.Popen(row['command'], stdout=out, stderr=err, cwd=OUT, env=run_env, start_new_session=True)
            row['exit'] = active.wait(timeout=seconds)
        except BaseException as exc:
            row['exception'] = repr(exc)
            if active is not None:
                if active.poll() is None:
                    os.killpg(active.pid, signal.SIGKILL)
                row['exit'] = active.wait()
            raise
        finally:
            stop_owned()
            row['end'] = time.time()
            out.flush()
            err.flush()
            row['stdout_sha256'] = sha(OUT / (label + '.stdout'))
            row['stderr_sha256'] = sha(OUT / (label + '.stderr'))
            save()
    print(label, row['exit'], flush=True)
    assert row['exit'] == 0, label
    return row
try:
    report['before'] = {p: sha(p) for p in observed}
    assert report['before'] == observed
    save()
    W = Path(PREP['worktree'])
    for source, name in [(W / 'tests/c/integration.c', 'integration.c'), (W / 'tests/c/UPSTREAM-NOTICES.txt', 'UPSTREAM-NOTICES.txt'), (W / 'include/expat.h', 'expat.h'), (HERE / 'consumer.c', 'consumer.c')]:
        shutil.copyfile(source, OUT / name)
    report['compiler_sha256'] = sha('/usr/bin/cc')
    run('compiler-version', ['/usr/bin/cc', '--version'], 10, env)
    for arm in PREP['arms']:
        directory = OUT / arm['name']
        directory.mkdir()
        library = Path(arm['path'])
        exe = directory / 'integration'
        cmd = ['/usr/bin/cc', '-std=c11', '-O1', '-g', '-Wall', '-Wextra', '-Werror', '-fsanitize=address,undefined', '-fno-omit-frame-pointer', '-I', str(OUT), str(OUT / 'consumer.c')]
        cmd += [str(library), '-ldl', '-lpthread', '-lm', '-o', str(exe)]
        run(arm['name'] + '-build', cmd, PREP['compile_wall_seconds'], env)
        if arm['linkage'] == 'shared':
            (directory / arm['soname']).symlink_to(library)
            expected = str(library)
        else:
            expected = str(exe)
        worker_env = {**env, 'LD_LIBRARY_PATH': str(directory)}
        row = run(arm['name'], [str(exe), expected], PREP['worker_wall_seconds'], worker_env)
        text = (OUT / (arm['name'] + '.stdout')).read_text()
        assert 'Verified 34 API symbol origins: ' in text
        assert 'C ABI full integration passed (' in text
        assert 'Phase-relative OOM passed: successful_declarations=2 child_error=1 parent_error=21 rejected_allocations=' in text
        assert ' live_allocations=0\n' in text
        report['workers'].append({'arm': arm, 'binary_sha256': sha(exe), 'exit': row['exit'], 'stdout': text})
        save()
    assert len(report['workers']) == 5
    report['status'] = 'passed'
except BaseException as exc:
    report['status'] = 'failed'
    report['exception'] = repr(exc)
    raise
finally:
    stop_owned()
    report['after'] = {p: sha(p) for p in observed}
    report['unchanged'] = report['before'] == report['after']
    if not report['unchanged']:
        report['status'] = 'failed_identity'
    save()
assert report['unchanged']
