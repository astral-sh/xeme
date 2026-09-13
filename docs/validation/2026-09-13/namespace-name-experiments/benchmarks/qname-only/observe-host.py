"""Record a bounded host observation before root releases a benchmark."""
from pathlib import Path
import json
import os
import sys
import time

assert os.sched_getaffinity(0) == {6}
assert len(sys.argv) == 2
out = Path(__file__).parent / sys.argv[1]
assert out.parent == Path(__file__).parent and not out.exists()
samples = []
for sample in range(2):
    if sample:
        time.sleep(10)
    cpus = {}
    for line in Path('/proc/stat').read_text().splitlines():
        parts = line.split()
        if parts and parts[0] in ('cpu', 'cpu0'):
            values = list(map(int, parts[1:9]))
            cpus[parts[0]] = {'total': sum(values), 'idle': values[3] + values[4]}
    samples.append({'time': time.time(), 'cpu': cpus})
fractions = {}
for cpu in ('cpu', 'cpu0'):
    before, after = [x['cpu'][cpu] for x in samples]
    fractions[cpu] = (after['idle'] - before['idle']) / (after['total'] - before['total'])
report = {'samples': samples, 'idle_plus_iowait_fraction': fractions,
          'scope': 'Root released this observation after all local Oriole parser/compiler processes were reaped. Shared host; CPU counters and affinity do not establish OS isolation.'}
out.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'path': str(out), 'idle_plus_iowait_fraction': fractions}))
