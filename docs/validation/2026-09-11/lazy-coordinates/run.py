import hashlib, json, os, pathlib, signal, subprocess, sys, time
root = pathlib.Path('/home/dev-user/code/oss/oriole-lazy-coordinates')
output = pathlib.Path(__file__).parent
label, *args = sys.argv[1:]
report = output / f'{label}.json'
assert not report.exists()
for suffix in ('stdout', 'stderr'):
    assert not (output / f'{label}.{suffix}').exists()
env = os.environ.copy()
for key in list(env):
    if key in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS') or key.startswith('CARGO_UNSTABLE_OHM_'):
        env.pop(key)
overrides = {
    'CARGO_HOME': '/home/dev-user/.cache/toucan/cargo',
    'CARGO_TARGET_DIR': '/home/dev-user/.cache/oriole/targets/lazy-coordinates',
    'CARGO_BUILD_BUILD_DIR': '/home/dev-user/.cache/ohm/verified-build/{workspace-path-hash}',
    'CARGO_INCREMENTAL': '0',
    'CARGO_BUILD_JOBS': '1',
}
env.update(overrides)
command = ['taskset', '-c', '4', 'cargo', '+ohm', '-Zohm-defaults=no', *args]
start = time.monotonic()
with (output / f'{label}.stdout').open('xb') as stdout, (output / f'{label}.stderr').open('xb') as stderr:
    process = subprocess.Popen(command, cwd=root, env=env, stdout=stdout, stderr=stderr, start_new_session=True)
    try:
        code = process.wait(timeout=600)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        code = 'timeout'
data = dict(command=command, cwd=str(root), env=overrides, returncode=code, seconds=time.monotonic()-start)
for suffix in ('stdout', 'stderr'):
    path = output / f'{label}.{suffix}'
    data[suffix] = {'bytes': path.stat().st_size, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
report.write_text(json.dumps(data, indent=2)+'\n')
print(json.dumps(data))
sys.exit(0 if code == 0 else 1)
