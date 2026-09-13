from pathlib import Path
import datetime, hashlib, json, subprocess, time

ROOT = Path('/tmp/oriole-core-owned-text-raw-ci/pbs')
CWD = '/home/dev-user/code/oss/oriole-core-owned-text-raw'
GH = '/home/dev-user/.local/bin/gh-auto'
API = 'repos/astral-sh/oriole'
RUN = 34712464503
HEAD = '47776625cfee5cb5f0258eec93bbb1248209113c'
start = time.monotonic()
last = None
errors = 0
records = []

def get(label, endpoint, poll):
    proc = subprocess.run([GH, 'api', endpoint], cwd=CWD, capture_output=True, timeout=60)
    stem = ROOT / f'{poll:03d}-{label}'
    stem.with_suffix('.json').write_bytes(proc.stdout)
    stem.with_suffix('.stderr').write_bytes(proc.stderr)
    row = {'poll': poll, 'label': label, 'endpoint': endpoint,
           'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
           'returncode': proc.returncode, 'stdout_sha256': hashlib.sha256(proc.stdout).hexdigest(),
           'stderr_sha256': hashlib.sha256(proc.stderr).hexdigest()}
    records.append(row)
    (ROOT / 'reads.json').write_text(json.dumps(records, indent=2) + '\n')
    if proc.returncode:
        raise RuntimeError(f'{label}: GET exited {proc.returncode}: {proc.stderr.decode()[:300]}')
    return json.loads(proc.stdout)

for poll in range(1, 132):
    if time.monotonic() - start > 195 * 60:
        raise SystemExit('Bounded monitor expired; run not declared complete.')
    try:
        run = get('run', f'{API}/actions/runs/{RUN}', poll)
        assert run['head_sha'] == HEAD and run['run_attempt'] == 1
        assert run['event'] == 'workflow_dispatch' and run['path'] == '.github/workflows/pbs.yml'
        jobs = get('jobs', f'{API}/actions/runs/{RUN}/attempts/1/jobs?per_page=100', poll)
        assert jobs['total_count'] == len(jobs['jobs']) == 1
        job, = jobs['jobs']
        assert job['head_sha'] == HEAD and job['name'] == 'distribution'
        state = [run['status'], run['conclusion'], job['status'], job['conclusion'],
                 [[x['name'], x['status'], x['conclusion']] for x in job['steps']]]
        if state != last:
            print(json.dumps({'poll': poll, 'run': state[:2], 'job': state[2:4],
                              'steps': state[4]}), flush=True)
            last = state
        errors = 0
        if run['status'] == 'completed':
            artifacts = get('artifacts', f'{API}/actions/runs/{RUN}/artifacts?per_page=100', poll)
            assert artifacts['total_count'] == len(artifacts['artifacts'])
            check = get('check', job['check_run_url'].removeprefix('https://api.github.com/'), poll)
            assert check['head_sha'] == HEAD
            summary = {'run_id': RUN, 'head_sha': HEAD, 'attempt': 1, 'event': run['event'],
                       'run_conclusion': run['conclusion'], 'job_conclusion': job['conclusion'],
                       'html_url': run['html_url'], 'final_poll': poll,
                       'artifacts': [{k:a.get(k) for k in ['id','name','size_in_bytes','digest','expired','archive_download_url']} for a in artifacts['artifacts']],
                       'workflow_execution_or_reruns_by_monitor': False, 'downloads_by_monitor': False}
            (ROOT / 'completed-metadata.json').write_text(json.dumps(summary, indent=2) + '\n')
            print('COMPLETED ' + json.dumps(summary), flush=True)
            break
    except (RuntimeError, subprocess.TimeoutExpired) as exc:
        errors += 1
        print('READ_ERROR ' + str(exc), flush=True)
        if errors >= 3:
            raise
    time.sleep(90)
else:
    raise SystemExit('Bounded monitor exhausted; no completion claimed.')
