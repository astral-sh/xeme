"""Run matched consumer preparation or one bounded elapsed study."""
from pathlib import Path
import hashlib
import json
import signal
import subprocess
import sys
import time

out = Path(__file__).parent
python = '/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3'
adaptation = json.loads((out.parent / 'binding.json').read_text())
assert adaptation['status'] == 'passed'
libraries = adaptation['libraries']
for entry in libraries.values():
    assert hashlib.sha256(Path(entry['path']).read_bytes()).hexdigest() == entry['sha256']
assert len(sys.argv) == 2 and sys.argv[1] in ('prepare', 'time')
phase = sys.argv[1]
common = ['--kind', 'python', '--build', str(out / 'consumers/build.json'), '--input', '/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json', '--output', str(out / 'screen')]
if phase == 'prepare':
    jobs = [
        ('preflight', 600, ['taskset', '-c', '3', python, '-I', '-S', str(out / 'consumer_screen.py'), '--phase', 'prepare', *common]),
    ]
else:
    assert json.loads((out / 'prepare-controller.json').read_text())['status'] == 'passed'
    jobs = [('time', 1200, ['taskset', '-c', '0', python, '-I', '-S', str(out / 'consumer_screen.py'), '--phase', 'time', *common])]
report_path = out / (phase + '-controller.json')
assert not report_path.exists()
report = {'status': 'incomplete', 'jobs': [], 'controller_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
          'scope': 'All six existing matched extensions reused; fresh canonical preflights on CPU3; separate elapsed study on leased CPU0. Worker and measurement boundaries unchanged. Per-worker300s and aggregate timing1200s retained. Process group killed on outer timeout/interruption.'}
def save():
    report_path.write_text(json.dumps(report, indent=2) + '\n')
try:
    for label, timeout, command in jobs:
        row = {'label': label, 'command': command, 'timeout_seconds': timeout, 'start': time.time()}
        report['jobs'].append(row)
        save()
        process = None
        log_path = out / (label + '-controller.log')
        with log_path.open('wb') as log:
            try:
                process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                row['exit'] = process.wait(timeout=timeout)
                if row['exit']:
                    raise RuntimeError(label + ' failed')
            except BaseException as exc:
                row['exception'] = repr(exc)
                if process is not None:
                    try:
                        signal_process = process.pid
                        import os
                        os.killpg(signal_process, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    row['exit'] = process.wait()
                raise
            finally:
                log.flush()
                row['end'] = time.time()
                row['log_sha256'] = hashlib.sha256(log_path.read_bytes()).hexdigest()
                save()
        print(label, row['exit'], flush=True)
    report['status'] = 'passed'
finally:
    save()
