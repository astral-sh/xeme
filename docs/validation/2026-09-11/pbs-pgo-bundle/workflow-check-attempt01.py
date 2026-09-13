"""Validate workflow structure and shell argument flow using inert command stubs."""
import json
import os
import pathlib
import subprocess
import tempfile
import hashlib
import yaml

root = pathlib.Path('/home/dev-user/code/oss/oriole-pbs-pgo-bundle')
out = pathlib.Path(__file__).resolve().parent
path = root / '.github/workflows/pbs.yml'
workflow = yaml.safe_load(path.read_text())
old = yaml.safe_load(subprocess.check_output(['git', 'show', 'HEAD:.github/workflows/pbs.yml'], cwd=root, text=True))
event = workflow.get('on', workflow.get(True))
assert event['workflow_dispatch']['inputs']['pgo'] == {'description': 'Train and bundle a fresh Oriole PGO library', 'type': 'boolean', 'required': False, 'default': False}
job = workflow['jobs']['distribution']
assert job['runs-on'] == old['jobs']['distribution']['runs-on']
steps = {s['name']: s for s in job['steps'] if 'name' in s}
old_steps = {s['name']: s for s in old['jobs']['distribution']['steps'] if 'name' in s}
changed = {'Build the frozen Oriole bundle', 'Retain build and validation evidence'}
for name, value in old_steps.items():
    if name not in changed:
        assert steps[name] == value, name
condition = "${{ github.event_name == 'workflow_dispatch' && inputs.pgo }}"
assert steps['Prepare optional PGO tools']['if'] == condition
assert steps['Build the frozen Oriole bundle']['env']['ORIOLE_PGO'] == condition
assert 'cargo fetch --locked --target x86_64-unknown-linux-gnu' in steps['Prepare optional PGO tools']['run']
assert 'pgo/targets' not in steps['Retain build and validation evidence']['with']['path']
for suffix in ('a', 'so', 'dylib'):
    assert '!${{ runner.temp }}/oriole-bundle/pgo/runs/**/*.' + suffix in steps['Retain build and validation evidence']['with']['path']
for step in job['steps']:
    if 'run' in step:
        subprocess.run(['bash', '-n'], input=step['run'], text=True, check=True, capture_output=True)
cases = []
with tempfile.TemporaryDirectory(prefix='oriole-workflow-') as directory:
    temporary = pathlib.Path(directory)
    binary = temporary / 'bin'
    binary.mkdir()
    sysroot = temporary / 'sysroot with spaces'
    profdata = sysroot / 'lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-profdata'
    profdata.parent.mkdir(parents=True)
    profdata.touch()
    profdata.chmod(0o755)
    stub = '#!/usr/bin/python3\nimport json,os,sys\nwith open(os.environ["CALLS"],"a") as f:f.write(json.dumps(sys.argv)+"\\n")\nif os.path.basename(sys.argv[0])=="rustc":print(os.environ["FAKE_SYSROOT"])\n'
    for name in ('rustup', 'cargo', 'rustc', 'python3', 'git'):
        (binary / name).write_text(stub)
        (binary / name).chmod(0o755)
    for selected in (False, True):
        calls_path = temporary / ('calls-' + str(selected) + '.jsonl')
        github_env = temporary / ('env-' + str(selected))
        environment = os.environ | {'PATH': str(binary) + ':/usr/bin:/bin', 'CALLS': str(calls_path), 'FAKE_SYSROOT': str(sysroot), 'GITHUB_ENV': str(github_env), 'RUNNER_TEMP': str(temporary), 'GITHUB_WORKSPACE': str(temporary / 'workspace with spaces'), 'ORIOLE_PGO': str(selected).lower()}
        environment.pop('ORIOLE_PGO_PROFDATA', None)
        if selected:
            subprocess.run(['bash', '-e', '-o', 'pipefail', '-c', steps['Prepare optional PGO tools']['run']], env=environment, check=True, capture_output=True)
            key, value = github_env.read_text().strip().split('=', 1)
            assert key == 'ORIOLE_PGO_PROFDATA' and value == str(profdata)
            environment[key] = value
        subprocess.run(['bash', '-e', '-o', 'pipefail', '-c', steps['Build the frozen Oriole bundle']['run']], env=environment, check=True, capture_output=True)
        calls = [json.loads(line) for line in calls_path.read_text().splitlines()]
        prepare, = [call for call in calls if 'integration/python-build-standalone/prepare.py' in call]
        expected = [str(binary / 'python3'), 'integration/python-build-standalone/prepare.py', '--output', str(temporary / 'oriole-bundle')]
        if selected:
            expected += ['--pgo', '--llvm-profdata', str(profdata)]
            fetch, = [call[1:] for call in calls if pathlib.Path(call[0]).name == 'cargo']
            assert fetch == ['fetch', '--locked', '--target', 'x86_64-unknown-linux-gnu']
        else:
            assert not any(pathlib.Path(call[0]).name in ('rustc', 'cargo') for call in calls)
        assert prepare == expected
        cases.append({'pgo': selected, 'calls': calls})
report = {'status': 'passed', 'scope': 'YAML parse, unchanged runner and independent gates, bash -n for all run steps, false/true argument flow with inert command stubs; no Cargo/compiler/parser/container/workflow execution.', 'pyyaml': yaml.__version__, 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(), 'workflow_sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'cases': cases}
(out / 'workflow-check.json').write_text(json.dumps(report, indent=2) + '\n')
print(report['status'], report['workflow_sha256'])
