"""Frozen source/red-green audit only; no compiler or parser execution."""
from pathlib import Path
import hashlib,json,subprocess,os,re
os.sched_setaffinity(0,{6})
P=Path('/tmp/oriole-arena-capacity-bound-study');OUT=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest=P/'source.json';assert sha(manifest)=='857cca1585ce8168ffe3f20c9c8b1ba90f6b8bea0e09b69c5352a677207ca203'
s=json.loads(manifest.read_text());wt=Path(s['worktree']);assert len(s['source_sha256'])==72
for n,h in s['source_sha256'].items():assert sha(wt/n)==h,n
assert sha(P/'fix.patch')==s['patch_sha256']=='3778d7c148adf38c14081fdc493cc1ba899fa17a53114ee28662443460f410a4'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=wt,text=True).strip()==s['head']
assert subprocess.check_output(['git','diff','--name-only'],cwd=wt,text=True).splitlines()==['crates/oriole/src/arena.rs']
assert subprocess.check_output(['git','diff','--','crates/oriole/src/arena.rs'],cwd=wt)==(P/'fix.patch').read_bytes()
base=subprocess.check_output(['git','show',s['head']+':crates/oriole/src/arena.rs'],cwd=wt,text=True)
red=(P/'arena-red.rs').read_text();fixed=(wt/'crates/oriole/src/arena.rs').read_text()
marker='    #[test]\n    fn start_after_text_stays_within_the_reserved_frame_capacity()'
assert marker in red and marker in fixed
red_prefix=red[:red.index(marker)].rstrip()+'\n}'
assert red_prefix==base.rstrip()
# Red and fixed test bodies are identical modulo the separately recorded fmt wrapping.
normalize=lambda t:re.sub(r'\s+','',t)
assert normalize(red[red.index(marker):])==normalize(fixed[fixed.index(marker):])
rows=[]
for stage,status,code in [('red','failed',101),('green','passed',0)]:
 report=json.loads((P/stage/'report.json').read_text());assert report['status']==status and report['source_unchanged']
 assert len(report['commands'])==1
 row=report['commands'][0];assert row['exit']==code and row['reaped']
 assert row['timeout_seconds']==900 and report['cpu']==2
 assert row['argv']==['cargo','+ohm','-Zohm-defaults=no','test','--locked','--offline','-p','oriole','--lib','arena::tests::start_after_text_stays_within_the_reserved_frame_capacity','--','--exact']
 assert sha(P/stage/(stage+'.log'))==row['log_sha256']
 log=(P/stage/(stage+'.log')).read_text()
 if stage=='red':assert 'active frame retains 10096 bytes against its 8192-byte reservation' in log and '0 passed; 1 failed;' in log
 else:assert '1 passed; 0 failed;' in log
 rows.append({'stage':stage,'status':status,'exit':code,'reaped':True,'report_sha256':sha(P/stage/'report.json'),'log_sha256':row['log_sha256'],'arena_sha256':report['source_sha256']['crates/oriole/src/arena.rs']})
names=[f'a{i}' for i in range(128)];tag='<n'+''.join(f" {n}='"+'v'*22+"'" for n in names)+'/>'
packed=2+sum(len(n)+22+2 for n in names)
assert len(tag)==3734 and packed==3476
refs=[manifest,P/'fix.patch',P/'arena-red.rs',P/'red/report.json',P/'red/red.log',P/'green/report.json',P/'green/green.log',P/'format/report.json',wt/'crates/oriole/src/arena.rs',wt/'crates/oriole/src/lib.rs',wt/'crates/oriole/src/recycling.rs',wt/'crates/oriole_storage/src/string.rs',wt/'crates/oriole_storage/src/lib.rs',wt/'crates/oriole_storage/src/allocator.rs',Path('/home/dev-user/.cache/toucan/cargo/registry/src/socket-firewall-registry.gateway.admin-0.internal.api.openai.org-42b3aaa0d2956984/allocator-api2-0.2.21/src/stable/raw_vec.rs')]
result={'status':'passed','role':'Independent frozen source and retained focused red/green review; not collector validation','base_head':s['head'],'source_manifest_sha256':sha(manifest),'source_files_checked':72,'patch_sha256':sha(P/'fix.patch'),'only_live_delta':'crates/oriole/src/arena.rs','selected_source_delta':s['delta_against_selected'],'red_green':rows,'witness_arithmetic':{'raw_tag_bytes':len(tag),'packed_arena_length':packed,'old_byte_capacity':6000,'record_capacity_bytes_x86_64':4096,'old_total':10096,'fixed_total_ceiling':8192},'full_checks':'Owned by author; not claimed by this source receipt.','scope':['No full64KiB aggregate excess or global allocation-limit bypass is demonstrated.','Growth request sizes and exact custom-allocator fail-at-N identity may differ.','No compiler/parser/profiler executed by reviewer.'],'pins':{str(x):sha(x) for x in refs}}
with (OUT/'source-readback.json').open('x') as f:json.dump(result,f,indent=2);f.write('\n')
print('passed',sha(OUT/'source-readback.json'))
