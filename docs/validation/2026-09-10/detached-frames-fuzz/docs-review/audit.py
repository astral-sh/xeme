"""Final docs/index/compressed-review checks. No replay of already verified campaigns."""
from pathlib import Path
from urllib.parse import urlsplit,unquote
import hashlib,json,re,subprocess,tarfile
R=Path('/home/dev-user/code/oss/oriole-frame-fuzz-evidence');B=R/'docs/validation/2026-09-10/detached-frames-fuzz';O=Path(__file__).parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();load=lambda p:json.loads(Path(p).read_bytes())
base='d53dc6c2545420713c6af5d38f58f8702d7c1f0f';assert subprocess.check_output(['git','-C',str(R),'rev-parse','HEAD']).decode().strip()==base
paths=['README.md','docs/review.md',str((B/'README.md').relative_to(R))]
texts={p:(R/p).read_text() for p in paths};top=texts['README.md'];doc=texts[paths[-1]]
old=subprocess.check_output(['git','-C',str(R),'show',base+':README.md']).decode()
assert re.findall(r'^#{1,6} .+$',top,re.M)==re.findall(r'^#{1,6} .+$',old,re.M)
assert top.split('## Highlights')[0]==old.split('## Highlights')[0]
assert top[top.index('## Installation'):]==old[old.index('## Installation'):]
assert set(subprocess.check_output(['git','-C',str(R),'diff','--name-only']).decode().splitlines())==set(paths[:2])
assert '14.62 million executions on `5bc806e`' in top and 'these precede the latest text-scanning change' in top
assert 'applies to the earlier `4b11ace` runtime' in top
assert '[earlier sanitizer report]' in texts['docs/review.md']
assert 'No campaign reruns are recorded in this evidence.' in doc
links=[]
for name,text in texts.items():
 for target in re.findall(r'\[[^\]]*\]\(([^\s)]+)\)',text):
  parts=urlsplit(target)
  if parts.scheme or parts.netloc:continue
  p=(R/name).parent/unquote(parts.path) if parts.path else R/name
  assert p.exists(),(name,target)
  if parts.fragment and p.suffix=='.md':
   slug=lambda x:re.sub(r'[^\w\- ]','',x.lower()).replace(' ','-')
   assert parts.fragment in [slug(x) for x in re.findall(r'^#{1,6} (.+)$',p.read_text(),re.M)]
  links.append({'source':name,'target':target})
index=load(B/'files.json')['files'];assert len(index)==14
assert {x['path'] for x in index}=={str(p.relative_to(B)) for p in B.rglob('*') if p.is_file() and p.name!='files.json'}
for x in index:assert sha(B/x['path'])==x['sha256'] and (B/x['path']).stat().st_size==x['bytes']
assert sha(B/'evidence.tar.gz')=='3bdd18ae72a22f770c653b9a32270da23f454d4f6fa9bd31eb5b6b2d244fedda'
assert sha(B/'final-corpus.tar.gz')=='d8154f6185e1a39f054962184f503edfe9529e204ce46fbf6271fed236e7ef85'
for p in Path('/tmp/oriole-frame-fuzz-handoff').iterdir():
 if p.is_file():assert sha(p)==sha(B/p.name)
assert sha(B/'independent-review/review.json')=='a6407d6e06686790bdf78b3599c67b4dab8499618b49b225ce8265a2c7465c57'
# New compression must retain every byte of the eight original review files.
files=load(B/'independent-review-members.json')['files'];assert len(files)==8
with tarfile.open(B/'independent-review.tar.gz') as t:
 members=[m for m in t if m.isfile()];assert {m.name for m in members}==set(files)
 for m in members:
  d=t.extractfile(m).read();assert hashlib.sha256(d).hexdigest()==files[m.name]
  rel=Path(m.name).relative_to('independent-review');assert d==(Path('/tmp/oriole-frame-fuzz-independent-review')/rel).read_bytes()
summary=load(B/'summary.json');review=load(B/'independent-review/review.json');stats=load('/tmp/oriole-frame-fuzz-stats-independent-review/review.json')
assert summary['source_commit']==review['source_commit']=='5bc806e5fc2a545f22c75f2a1bcbef45cfa3632b'
assert summary['total_executed_units']==sum(t['executed_units'] for t in summary['targets'])==stats['totals']['campaign_executions']==14617069
assert sum(t['initial_inputs'] for t in summary['targets'])==26689 and sum(t['corpus_files'] for t in summary['targets'])==16272
for t in summary['targets']:
 line=f"| {t['target']} | {t['initial_inputs']:,} | {t['executed_units']:,} | {t['corpus_files']:,} |";assert line in doc
assert '| Total | 26,689 | 14,617,069 | 16,272 |' in doc
for text in ['26,686 nonempty','three empty','26,692 separate replay','600-second','690-second','180-second','10-second','64 KiB','1,536 MiB','20260911','601.141–601.192','357 tracked','55 regular members','16,272 resulting disk files','no new LeakSanitizer','UndefinedBehaviorSanitizer claim']:
 assert text in doc,text
assert 'initialization is included in the campaign budget' in doc and 'not a proof of memory safety' in doc
assert 'build and outer\ncontroller do not provide complete process-tree cleanup coverage' in doc
for name in paths:
 dest=O/'reviewed-docs'/name;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes((R/name).read_bytes())
result={'status':'passed_no_remaining_findings','scope':'Final human-doc/link/index/hash review only. Existing main evidence and corpus archives were not expensively re-audited because their byte hashes exactly match the prior complete independent readback; new compressed independent-review archive was fully read back.',
'base':base,'readme_warning_headings_installation_license_exact':True,'local_links_verified':len(links),'outer_index_entries':14,'outer_index_total_bytes':sum(x['bytes'] for x in index),'compressed_review_members_exact':8,
'counts':stats['totals'],'limits':stats['limits'],
'interpretation':['14.62million is rounded from14,617,069 initialization/repetition-inclusive executions on5bc806e; separate26,692 replay count, raw empty inputs and retained disk-vs-live corpus distinctions are correct.','FreshASan3workspace+6targets and representative machine-code evidence remain accurately scoped; externalClang runtime/std/libc limitations and LSanoff explicit.','Later scanner changes and PBS4b results remain separate. Existing393API and2strictCPython failures are not reclassified. No throughput or broader compatibility claim.','Per-target cleanup and outer/build cleanup limitations remain explicit. The final no-reruns statement is qualified to recorded evidence.'],
'wording_corrections':['Earlier4b report label changed current→earlier in docs/review.md.','No-reruns negative-history statement now says no campaign reruns are recorded in this evidence.'],
'evidence_sha256':{str(p):sha(p) for p in [*(R/n for n in paths),B/'files.json',B/'independent-review.tar.gz',B/'independent-review-members.json',B/'evidence.tar.gz',B/'final-corpus.tar.gz',Path(__file__),Path('/tmp/oriole-frame-fuzz-stats-independent-review/final-docs/review.json')]},
'limits_of_review':['Parent may add this audit/generator and refresh outer files.json; original evidence and final corpus hashes stay unchanged.','The initial source-only documentation follow-up recorded the two now-resolved wording nits. Its original review stays preserved.']}
(O/'links.json').write_text(json.dumps(links,indent=2)+'\n');(O/'review.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'status':result['status'],'sha256':sha(O/'review.json'),'links':len(links),'outer_bytes':result['outer_index_total_bytes']}))
