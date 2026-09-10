import argparse,gzip,hashlib,json,os,re,shutil,subprocess,tarfile,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('target');p.add_argument('cpu');p.add_argument('--seconds',type=int,default=600);a=p.parse_args()
root=Path('/tmp/oriole-value-fuzz');e=root/'final-evidence'/a.target;e.mkdir(parents=True,exist_ok=True)
manifest=json.loads((root/'final-evidence/source.json').read_text())
def sources_match():return all(hashlib.sha256((root/p).read_bytes()).hexdigest()==h for p,h in manifest['files'].items())
assert sources_match()
binary=Path('/home/dev-user/.cache/oriole/value-fuzz-target/x86_64-unknown-linux-gnu/release')/a.target
sha=hashlib.sha256(binary.read_bytes()).hexdigest()
with binary.open('rb') as src,gzip.open(e/(a.target+'.gz'),'wb') as dst:shutil.copyfileobj(src,dst)
initial=e/'initial-corpus';initial.mkdir(exist_ok=True)
inputs=[root/'fuzz/seeds'/a.target]
if a.target!='value_family':
 old=Path('/home/dev-user/.cache/oriole/fuzz-newfeatures-final')/(a.target+'-evidence')
 inputs += [old/'corpus',old/'initial-corpus']
initial_manifest={}
for folder in inputs:
 for path in sorted(folder.iterdir()):
  if not path.is_file():continue
  data=path.read_bytes();h=hashlib.sha256(data).hexdigest()
  if h not in initial_manifest:(initial/h).write_bytes(data)
  initial_manifest.setdefault(h,[]).append(str(path))
(e/'initial-manifest.json').write_text(json.dumps(initial_manifest,indent=2)+'\n')
with tarfile.open(e/'initial-corpus.tar.gz','w:gz') as tar:
 for path in sorted(initial.iterdir()):tar.add(path,arcname=path.name,recursive=False)
corpus=e/'corpus';corpus.mkdir(exist_ok=True);artifacts=e/'artifacts';artifacts.mkdir(exist_ok=True)
env=os.environ.copy();env['ASAN_OPTIONS']='detect_leaks=0'
common=['taskset','-c',a.cpu,str(binary),'-seed=20260910','-max_len=65536','-timeout=10','-rss_limit_mb=1536','-artifact_prefix='+str(artifacts)+'/']
result={'target':a.target,'binary_sha256':sha,'source_manifest_sha256':hashlib.sha256((root/'final-evidence/source.json').read_bytes()).hexdigest(),'initial_inputs':len(initial_manifest),'input_directories':[str(x) for x in inputs],'sanitizer':'AddressSanitizer with external Clang runtime','runtime_environment':{'ASAN_OPTIONS':env['ASAN_OPTIONS']}}
replay=common+['-runs=0',str(initial)];result['replay_command']=replay
with (e/'replay.log').open('w') as log:result['replay_exit_code']=subprocess.run(replay,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
(e/'result.json').write_text(json.dumps(result,indent=2)+'\n')
if result['replay_exit_code']:
 print(json.dumps(result),flush=True);raise SystemExit(result['replay_exit_code'])
command=common+[f'-max_total_time={a.seconds}','-print_final_stats=1',str(corpus),str(initial)]
result['campaign_command']=command;result['started_at_utc']=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime());(e/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'started':a.target,'initial_inputs':len(initial_manifest),'binary_sha256':sha}),flush=True)
start=time.monotonic()
with (e/'campaign.log').open('w') as log:result['campaign_exit_code']=subprocess.run(command,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
result['campaign_elapsed_seconds']=time.monotonic()-start
result['source_unchanged']=sources_match();result['binary_unchanged']=hashlib.sha256(binary.read_bytes()).hexdigest()==sha
result['stats']={k:int(v) for k,v in re.findall(r'stat::(\w+):\s+(\d+)',(e/'campaign.log').read_text())}
result['artifacts']=[{'name':p.name,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in artifacts.iterdir()]
result['final_corpus']={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in corpus.iterdir() if p.is_file()}
(e/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='final_corpus'}),flush=True)
