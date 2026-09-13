"""Reconstruct native results from every saved worker; no target execution."""
from pathlib import Path
import csv, hashlib, json, math, os, random, statistics
assert __debug__ and os.sched_getaffinity(0)=={6}
NATIVE=Path(__file__).parent; SCREEN=NATIVE/'native-screen';OUT=NATIVE;pins={}
def sha(path):
    path=Path(path);value=hashlib.sha256(path.read_bytes()).hexdigest()
    assert str(path) not in pins or pins[str(path)]==value
    pins[str(path)]=value;return value
def load(path):sha(path);return json.loads(Path(path).read_text())
binding=load(NATIVE.parent/'binding.json');prep=load(NATIVE.parent/'preparation.json')
assert binding['status']=='passed' and binding['preparation_sha256']==sha(NATIVE.parent/'preparation.json')
for path,h in prep['pins'].items():assert sha(path)==h,path
assert sha(binding['build_record']['path'])==binding['build_record']['sha256']
protocol=load(SCREEN/'protocol.json');preflight=load(SCREEN/'preflight.json');results=load(SCREEN/'results.json');controller=load(NATIVE/'controller.json')
assert preflight['status']==results['status']==controller['status']=='passed'
assert results['affinity']==[0]
assert preflight['protocol_sha256']==results['protocol_sha256']==sha(SCREEN/'protocol.json')
assert results['preflight_sha256']==sha(SCREEN/'preflight.json')
assert protocol['hashes']==preflight['hashes_before']==preflight['hashes_after']==results['hashes_before']==results['hashes_after']
for path,h in protocol['hashes'].items():assert sha(path)==h,path
libraries=protocol['libraries'];assert libraries==binding['libraries']
conditions=protocol['conditions']
prior=load('/tmp/oriole-native-start-end-namespace-declaration-confirmation/native/native-screen/protocol.json')
assert conditions==prior['conditions'] and len(conditions)==28
assert protocol['pairs']==7 and protocol['seed']==2026091003
assert protocol['preflights']==84 and protocol['timed_workers']==588
assert controller['controller_sha256']==sha(NATIVE/'native_screen.py') and controller['wrapper_sha256']==sha(NATIVE/'run.py')
assert len(controller['commands'])==2
for row,phase,cpu,bound in zip(controller['commands'],['preflight','time'],[5,0],[600,1200],strict=True):
    assert row['phase']==phase and row['exit']==0 and 'exception' not in row
    assert row['command']==['taskset','-c',str(cpu),'python3',str(NATIVE/'native_screen.py'),phase]
    assert row['timeout']==bound and row['end']>=row['start']
    assert sha(NATIVE/(phase+'-controller.log'))==row['log_sha256']
assert controller['commands'][0]['end']<=controller['commands'][1]['start']
driver = '/tmp/oriole-grammar-bench-plain/native-driver'
workers = sorted(SCREEN.glob('worker-*.json'))
assert [p.name for p in workers] == [f'worker-{n:04d}.json' for n in range(672)]
ordinal = 0
observations = {}
counts = {'preflight_workers': 0, 'timed_workers': 0, 'preflight_samples': 0, 'timed_warmups': 0, 'measured_samples': 0}

def condition_key(condition):
    return json.dumps(condition, sort_keys=True)

def worker(condition, engine, pair, cpu, count):
    global ordinal
    raw = load(workers[ordinal])
    ordinal += 1
    argv = ['taskset', '-c', str(cpu), driver, libraries[engine]['path'], condition['input'], str(condition['chunk']), str(count)]
    if condition['namespaces']:
        argv.append('namespaces')
    assert raw['condition'] == condition and raw['engine'] == engine and raw['pair'] == pair
    assert raw['command'] == argv and raw['returncode'] == 0 and raw['stderr'] == ''
    data = json.loads(raw['stdout'])
    assert data['version'] == ('expat_2.8.4' if engine == 'expat' else 'oriole_compat_2.8.4')
    samples = data['samples']
    assert len(samples) == count + 1
    assert [s['iteration'] for s in samples] == list(range(count + 1))
    assert [s['warmup'] for s in samples] == [True] + [False] * count
    assert all(math.isfinite(s['seconds']) and s['seconds'] > 0 for s in samples)
    seen = sorted({(s['hash'], s['elements'], s['text_bytes']) for s in samples})
    assert len(seen) == 1
    observed = [list(x) for x in seen]
    key = condition_key(condition)
    observations.setdefault(key, observed)
    assert observations[key] == observed
    median = statistics.median(s['seconds'] for s in samples[1:])
    if pair == -1:
        counts['preflight_workers'] += 1
        counts['preflight_samples'] += len(samples)
    else:
        counts['timed_workers'] += 1
        counts['timed_warmups'] += 1
        counts['measured_samples'] += count
    return raw | {'observations': observed, 'median_seconds': median}

expected_preflights = [worker(c, e, -1, 5, 1) for c in conditions for e in ['control', 'candidate', 'expat']]
assert expected_preflights == preflight['rows']
rng = random.Random(protocol['seed'])
jobs = [(c, pair) for c in conditions for pair in range(7)]
rng.shuffle(jobs)
expected_rows = []
for condition, pair in jobs:
    order = ['control', 'candidate', 'expat']
    rng.shuffle(order)
    processes = [worker(condition, e, pair, 0, condition['iterations']) for e in order]
    expected_rows.append({'condition': condition, 'pair': pair, 'order': order, 'processes': processes})
assert expected_rows == results['rows'] and ordinal == 672
assert counts == {'preflight_workers': 84, 'timed_workers': 588, 'preflight_samples': 168, 'timed_warmups': 588, 'measured_samples': 105420}
counts.update(all_workers=672, all_samples=106176, cohorts=196, conditions=28)
summary = []
for condition in conditions:
    relevant = [r for r in expected_rows if r['condition'] == condition]
    assert sorted(r['pair'] for r in relevant) == list(range(7))
    values = {key: [] for key in ['candidate_over_control', 'candidate_over_expat', 'control_over_expat']}
    for row in relevant:
        medians = {p['engine']: p['median_seconds'] for p in row['processes']}
        for key in values:
            a, b = key.split('_over_')
            values[key].append(medians[a] / medians[b])
    summary.append({'condition': condition, 'paired_ratios': values, 'median_ratios': {k: statistics.median(v) for k, v in values.items()}})
assert summary == results['summary']
groups = {}
for label, predicate in [('real', lambda c: not c['name'].startswith('generated-')), ('generated', lambda c: c['name'].startswith('generated-')), ('real_namespaces_off', lambda c: not c['name'].startswith('generated-') and not c['namespaces']), ('real_namespaces_on', lambda c: not c['name'].startswith('generated-') and c['namespaces'])]:
    selected = [s for s in summary if predicate(s['condition'])]
    groups[label] = {'conditions': len(selected), 'geomean_ratios': {k: math.exp(sum(math.log(r['median_ratios'][k]) for r in selected) / len(selected)) for k in values}, 'candidate_faster_than_control': sum(r['median_ratios']['candidate_over_control'] < 1 for r in selected), 'adverse': [r for r in selected if r['median_ratios']['candidate_over_control'] > 1]}
with (OUT / 'conditions.csv').open('x', newline='') as stream:
    fieldnames = ['name', 'chunk', 'namespaces', *values]
    writer = csv.DictWriter(stream, fieldnames=fieldnames)
    writer.writeheader()
    for row in summary:
        writer.writerow({k: row['condition'][k] for k in ['name', 'chunk', 'namespaces']} | row['median_ratios'])
for path, value in pins.items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == value
result={'status':'passed','counts':counts,'groups':groups,'all_conditions':summary,'libraries':libraries,
 'raw_schedule_process_medians_ratios_callback_equality_exact':True,
 'reader_sha256':sha(__file__),'conditions_csv_sha256':sha(OUT/'conditions.csv'),
 'limitations':['All 28 conditions and adverse results retained. Median of seven paired process-median ratios, equally weighted geometric mean across conditions.','Shared host and CPU affinity do not establish isolation or whole-application performance.','Native callback hashes merge adjacent text fragments and do not establish exact callback fragmentation compatibility.'],
 'inputs':pins}
dest=OUT/'review.json'
with dest.open('x') as stream:stream.write(json.dumps(result,indent=2)+'\n')
print(json.dumps({'status':'passed','report':str(dest),'sha256':sha(dest),'counts':counts,'groups':{k:{key:value for key,value in v.items() if key!='adverse'} for k,v in groups.items()},'adverse':[r for r in summary if r['median_ratios']['candidate_over_control']>1]},indent=2))
