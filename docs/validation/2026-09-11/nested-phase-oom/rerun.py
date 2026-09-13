"""Relocate the original five-consumer run without changing source or binary pins."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
assert os.sched_getaffinity(0) == {3}
assert len(sys.argv) in (2, 3)
prep = json.loads((HERE / 'checks-preparation.json').read_text())
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(HERE / 'run_checks.py') == 'e805b5e30f2a94ac7a5896f2903d20517609f16db31917404ee1b458f9668464'
overrides = json.loads(Path(sys.argv[2]).read_text()) if len(sys.argv) == 3 else {}
assert not (set(overrides) - {arm['name'] for arm in prep['arms']})
for arm in prep['arms']:
    arm['path'] = str(Path(overrides.get(arm['name'], arm['path'])).resolve())
    assert sha(arm['path']) == arm['sha256'], arm['name']
out = Path(sys.argv[1]).resolve()
out.mkdir(exist_ok=False)
source = out / 'source'
mapping = {'tests/c/integration.c': 'integration.c', 'tests/c/UPSTREAM-NOTICES.txt': 'UPSTREAM-NOTICES.txt', 'include/expat.h': 'expat.h', 'CONTRIBUTING.md': 'CONTRIBUTING.md'}
pins = {}
for relative, stored in mapping.items():
    old = str(Path(prep['worktree']) / relative)
    expected = prep['source_pins'][old]
    assert sha(HERE / 'source' / stored) == expected
    dest = source / relative
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(HERE / 'source' / stored, dest)
    pins[str(dest)] = expected
old_wrapper = next(p for p in prep['source_pins'] if p.endswith('/consumer.c'))
assert sha(HERE / 'consumer.c') == prep['source_pins'][old_wrapper]
for name in ['consumer.c', 'run_checks.py']:
    shutil.copyfile(HERE / name, out / name)
pins[str(out / 'consumer.c')] = prep['source_pins'][old_wrapper]
prep['source_pins'] = pins
prep['worktree'] = str(source)
(out / 'checks-preparation.json').write_text(json.dumps(prep, indent=2) + '\n')
raise SystemExit(subprocess.call([sys.executable, '-I', '-S', str(out / 'run_checks.py')]))
