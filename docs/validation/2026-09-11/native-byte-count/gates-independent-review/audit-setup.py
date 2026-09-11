
S=L;prior_root=Path('/tmp/oriole-end-frame-cells-thinlto-gates');initial_root=Path('/tmp/oriole-native-byte-count-gates-initial-cpu4')
source=j(R/'source.json');preparation=j(R/'preparation.json');initial_preparation=j(initial_root/'preparation.json');build=j(R/'build.json');summary=j(R/'summary.json')
source_review_path=Path('/tmp/oriole-native-byte-count-source-build-independent-review/review.json')
ck('source build review pinned',h(source_review_path)=='f1f801a46736344b8fae130ed3ca815c9d71c542c0dda15b098cd5e784c0d886')
ck('source70 identity',len(source['source_sha256'])==70 and h(R/'source.json')==h(S/'source.json')==build['source_manifest_sha256']=='735afb9491158c3a34fd376109edc2312727382b5fa61e5e8b084796d7294f1d')
normal_source=j(B/'source.json');changed=sorted(n for n,v in source['source_sha256'].items() if normal_source['source_sha256'][n]!=v)
ck('summary source delta exact',changed==summary['source']['changed_files_from_control']==['crates/oriole/src/encoding.rs','crates/oriole_expat/src/tests.rs'])
ck('all70 live source bytes',all(h(W/n)==v for n,v in source['source_sha256'].items()))
for p in [R/'source.tar.gz',S/'source.tar.gz']:
 with tarfile.open(fileobj=io.BytesIO(read(p)),mode='r:gz') as a:
  ms=a.getmembers();ck('source archive70 '+str(p),len(ms)==len({m.name for m in ms})==70 and all(m.isfile() for m in ms) and {m.name:hashlib.sha256(a.extractfile(m).read()).hexdigest() for m in ms}==source['source_sha256'])
expected_libraries={'liboriole_expat.so':'ccfb22956a11c1b241ec755f46245c61f088187ef93143b77a7afec748983821','liboriole_expat.a':'f58d5bc6ed28077933fcbcb84ad876cc7208e0102c9008347223da91a328b03c'}
normal_libraries={'liboriole_expat.so':'02fcab59f6e3818c128da6706d67987ae7450dd8b59e97cd28ce497a547f28f5','liboriole_expat.a':'69ebed414946cbaef1a940c052a636be277c809902943d676e824deab45d22d5'}
ck('build alias exact',read(R/'build.json')==read(S/'report.json') and build['status']=='passed' and build['source_unchanged'] and build['libraries']==expected_libraries)
for root,libmap in [(S,expected_libraries),(B,normal_libraries)]:ck('library identities '+str(root),all(h(root/n)==v for n,v in libmap.items()))
ck('all81 prepared inputs unchanged',len(preparation['before'])==81 and hashes(preparation['before']))
ck('initial preparation receipt',h(initial_root/'preparation.json')==preparation['initial_cpu4_preparation']['sha256']=='e292da0ce5fce28123c94f79ea79efb139d82540b7a25b1a532ac22230f4b72e' and initial_preparation['status']=='prepared_not_executed')
repls=[(str(prior_root),str(R)),(str(B),str(S)),('/tmp/oriole-ffi-family-cell-thinlto-study',str(B)),('/tmp/oriole-ffi-family-cell-thinlto-gates/api-native/upstream-api',str(prior_root/'api-native/upstream-api')),('/home/dev-user/code/oss/oriole-end-frame-cells',str(W)),(normal_libraries['liboriole_expat.so'],expected_libraries['liboriole_expat.so']),('0cfd610876f1bfd26a4d37a7f86adc9961896dfe24177c180ec7d15e0bae04e9',normal_libraries['liboriole_expat.so'])]
ck('seven original substitutions declared',preparation['replacements']==[list(x) for x in repls]==initial_preparation['replacements'])
def adapt(value):
 for i,(a,b) in enumerate(repls):value=value.replace(a,'__NATIVE_COUNT_'+str(i)+'__')
 for i,(a,b) in enumerate(repls):value=value.replace('__NATIVE_COUNT_'+str(i)+'__',b)
 return value
def cpu1(value):
 return value.replace('CPU4','CPU1').replace("'4'","'1'").replace("'api':4,'other':4","'api':1,'other':1").replace('=={4}','=={1}').replace('owned by structural agent','owned by the byte-count source author')
deltas=j(R/'controller-deltas.json')
ck('13 original controllers and retained deltas',len(preparation['controllers'])==13 and set(preparation['controllers'])==set(deltas) and read(R/'controller-deltas.json')==read(initial_root/'controller-deltas.json'))
for name,value in preparation['controllers'].items():
 original_path=prior_root/name;original=read(original_path).decode();initial=read(initial_root/name).decode();current=read(R/name).decode()
 ck('prior/current/initial controller pins '+name,h(original_path)==preparation['prior_controllers'][str(original_path)] and h(initial_root/name)==initial_preparation['controllers'][name] and h(R/name)==value)
 ck('initial exact seven substitutions '+name,adapt(original)==initial)
 ck('initial exact retained delta '+name,deltas[name]==''.join(difflib.unified_diff(original.splitlines(True),initial.splitlines(True),fromfile=str(original_path),tofile=str(R/name))))
 ck('current only CPU/prose substitutions '+name,cpu1(initial)==current)
ck('initial combined patch retained',read(R/'initial-controller.patch')==read(initial_root/'controller.patch')==''.join(deltas.values()).encode())
ck('CPU lane patch pin',h(R/'cpu-lane.patch')==preparation['cpu_lane_patch_sha256'])
for name in ['probe.c','original-allocations.c','expat.h']:ck('End OOM original assertions '+name,read(R/'end-oom-final'/name)==read(prior_root/'end-oom-final'/name))
for lane,status,names,codes in [('api','completed_zero_exits',['api_native.py'],[0]),('other','failed',['other_controls.py','run_publication.py','run_malformed.py','end-lifecycle/controller.py'],[0,0,0,1]),('end','completed_zero_exits',['end-lifecycle-final/controller.py','end-oom-final/run.py'],[0,0])]:
 report=j(R/(lane+'-controller.json'))
 ck('lane identity/status '+lane,report['status']==status and report['unchanged'] and report['before']==report['after']==preparation['before'] and not report['hash_errors'] and [r['script'] for r in report['jobs']]==names and [r['exit'] for r in report['jobs']]==codes and report['cpu']==1)
 for row in report['jobs']:
  ck('lane command bound '+row['script'],row['timeout']==660 and 'exception' not in row and row['command']==['taskset','-c','1','/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3.12','-I','-S',str(R/row['script'])] and h(R/row['script'])==row['sha256'] and 0<row['end']-row['start']<660)
  for stream in ['stdout','stderr']:ck('lane stream '+row['script']+stream,h(R/(row['script'].replace('/','-')+'.'+stream))==row[stream+'_sha256'])
  ck('lane child stderr empty '+row['script'],read(R/(row['script'].replace('/','-')+'.stderr'))==b'')
failed=j(R/'end-lifecycle/controller.json');fix=j(R/'end-affinity-fix.json')
ck('initial End guard failed before any parser runs',failed['status']=='failed' and failed['runs']==fix['initial_runs']==[] and 'assert sorted(os.sched_getaffinity(0)) == [4]' in failed['error'] and h(R/'end-lifecycle/controller.json')==fix['initial_controller_sha256'])
fixdiff=''
for name in ['controller.py','oracle.py']:
 old=read(R/'end-lifecycle'/name).decode();new=read(R/'end-lifecycle-final'/name).decode()
 expected=old.replace("'cpu': 4","'cpu': 1").replace('== [4]','== [1]')
 ck('End final only CPU guard/metadata '+name,expected==new)
 fixdiff+=''.join(difflib.unified_diff(old.splitlines(True),new.splitlines(True),fromfile='initial/'+name,tofile='final/'+name))
ck('End final correction patch exact',fixdiff==read(R/'end-affinity-fix.patch').decode() and h(R/'end-affinity-fix.patch')==fix['patch_sha256']=='da24ac6eb69eacdaf58fbd0395059ca355513a74ac27b82262608a25f14ef899')
ck('End SOURCE_REVIEW unchanged',read(R/'end-lifecycle-final/SOURCE_REVIEW.md')==read(R/'end-lifecycle/SOURCE_REVIEW.md'))
lane_code=read(R/'run_lane.py').decode();remaining=read(R/'run_remaining.py').decode()
expected_remaining=lane_code.replace("cpu={'api':1,'other':1}","cpu={'end':1}").replace("names={'api':['api_native.py'],'other':['other_controls.py','run_publication.py','run_malformed.py','end-lifecycle/controller.py','end-oom-final/run.py']}[lane]","names={'end':['end-lifecycle-final/controller.py','end-oom-final/run.py']}[lane]")
ck('remaining controller only changed job selection',expected_remaining==remaining)
ck('malformed worker exact inherited preparation',adapt(read(prior_root/'malformed/malformed_probe.py').decode())==read(R/'malformed/malformed_probe.py').decode())
for p in [R/'prepare.py',R/'remaining-controller.patch',R/'controller.patch',R/'summary-adaptation.patch']:h(p)
