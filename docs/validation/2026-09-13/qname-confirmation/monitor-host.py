"""Record host CPU counters alongside one confirmation, without running targets."""
from pathlib import Path
import hashlib
import json
import os
import sys
import time

assert __debug__ and os.sched_getaffinity(0) == {6}
assert len(sys.argv) == 2 and sys.argv[1] in ('native', 'python')
root = Path(__file__).parent
phase = sys.argv[1]
controller = root/phase/('controller.json' if phase == 'native' else 'time-controller.json')
out = root/('host-during-'+phase+'.json')
assert not out.exists()
report = {'status': 'running', 'phase': phase, 'samples': [],
          'monitor_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
          'scope': 'Read-only CPU counters, not OS-enforced isolation. CPU0 carries the benchmark; other CPU work is recorded separately.'}
deadline = time.monotonic() + 1900
while True:
    cpus = {}
    for line in Path('/proc/stat').read_text().splitlines():
        parts = line.split()
        if parts and (parts[0] == 'cpu' or parts[0] == 'cpu0'):
            values = list(map(int, parts[1:9]))
            cpus[parts[0]] = {'total': sum(values), 'idle': values[3]+values[4]}
    report['samples'].append({'time': time.time(), 'cpu': cpus})
    if controller.exists():
        try:
            state = json.loads(controller.read_text())
        except json.JSONDecodeError:
            state = None
        if state and state['status'] != 'incomplete':
            report['status'] = 'completed'
            report['controller_status'] = state['status']
            report['controller_sha256'] = hashlib.sha256(controller.read_bytes()).hexdigest()
    if time.monotonic() >= deadline:
        report['status'] = 'monitor_timeout'
    out.write_text(json.dumps(report, indent=2)+'\n')
    if report['status'] != 'running':
        break
    time.sleep(5)
print(json.dumps({'status': report['status'], 'samples': len(report['samples']), 'path': str(out)}))
