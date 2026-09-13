"""Bounded saved archive/origin/coverage review; never runs target code."""
from pathlib import Path
import collections,gzip,hashlib,json,os,subprocess,tarfile
assert os.sched_getaffinity(0)=={6}
root=Path('/tmp/oriole-cdata-finder-perf-handoff'); repo=Path('/home/dev-user/code/oss/oriole-cdata-finder')
H=lambda data:hashlib.sha256(data).hexdigest()
load=lambda path:json.loads(Path(path).read_text())
idx=json.loads(gzip.decompress((root/'members.json.gz').read_bytes())); summary=load(root/'summary.json'); files=load(root/'files.json')
for r in files['files']:
 b=(root/r['path']).read_bytes(); assert H(b)==r['sha256'] and len(b)==r['bytes'],r['path']
assert H((root/'evidence.tar.gz').read_bytes())==summary['archive_sha256']==idx['archive_sha256']=='c1d57a40b866fc4f19293dcfeb821aa1dc8c536ccac3def82b714f615979ddb7'
assert H((root/'members.json.gz').read_bytes())==summary['member_index_sha256']
rows=idx['members']+idx['aliases']; dest={r['destination']:r for r in rows}; stored={r['destination']:r for r in idx['members']}; assert len(dest)==len(rows)==5424
seen=set()
with tarfile.open(root/'evidence.tar.gz','r|gz') as stream:
 for m in stream:
  assert m.isfile() and not m.name.startswith('/') and '..' not in Path(m.name).parts and m.name not in seen
  seen.add(m.name); r=stored[m.name]; b=stream.extractfile(m).read(); assert len(b)==r['bytes'] and H(b)==r['sha256']; assert not b.startswith((b'\x7fELF',b'!<arch>\n'))
assert seen==stored.keys() and len(seen)==4388
for r in idx['aliases']:
 assert not r['destination'].startswith('/') and '..' not in Path(r['destination']).parts
 target=stored[r['same_bytes_as_destination']]; assert r['sha256']==target['sha256'] and r['bytes']==target['bytes']
byorigin=collections.defaultdict(list)
for r in rows:
 b=Path(r['source']).read_bytes(); assert H(b)==r['sha256'] and len(b)==r['bytes'],r['source']; byorigin[r['source']].append(r)
 if r.get('origin','').startswith('git:'):
  _,commit,path=r['origin'].split(':',2); assert H(subprocess.check_output(['git','show',commit+':'+path],cwd=repo))==r['sha256']
excluded=set(idx['excluded_paths']); assert len(excluded)==36
coverage={}
for mode in ('normal','pgo'):
 for kind in ('native','python'):
  p=Path(f'/tmp/oriole-cdata-finder-{mode}-{kind}-study'); actual={str(f) for f in p.rglob('*') if f.is_file()}; selected=actual-excluded
  assert selected<=byorigin.keys(),selected-byorigin.keys(); coverage[f'{kind}-{mode}']={'included_origins':len(selected),'excluded_paths':len(actual&excluded)}
  if kind=='python':
   review=load(f'/tmp/oriole-cdata-finder-python-elapsed-independent-review/{mode}/review.json'); result=p/'screen/results.json'; assert H(result.read_bytes())==review['results_sha256']; assert review['workers']['timed']==504 and review['workers']['preflight']==72
  else:
   result=p/'native-screen/results.json'; review=load('/tmp/oriole-cdata-finder-native-independent-review.json'); assert str(result) in review['input_sha256']; assert H(result.read_bytes())==review['input_sha256'][str(result)]
source=load('/tmp/oriole-cdata-finder-study/source.json')['source_sha256']; assert len(source)==70
for name,h in source.items(): assert dest['source/candidate/'+name]['sha256']==h
manifests={}
for label,p in [('candidate',Path('/tmp/oriole-cdata-finder-pgo-study')),('control',Path('/tmp/oriole-streaming-work-pgo-study-attempt02'))]:
 freeze=load(p/'pair-freeze.json')
 for name,rec in freeze['files'].items():
  f=str(p/name); assert f in byorigin or f in excluded,(label,name)
  if f in byorigin: assert all(r['sha256']==rec['sha256'] for r in byorigin[f]),f
 pair=load(p/'pair-report.json'); manifests[label]=load(pair['pgo_manifest']['path'])
 assert pair['pgo_manifest']['path'] in byorigin
assert manifests['candidate']['script_sha256']==manifests['control']['script_sha256'] and len(manifests['candidate']['script_sha256'])==6
for name,h in manifests['candidate']['script_sha256'].items(): assert dest['source/measured-pipeline/'+name]['sha256']==h
# Summaries use the original independently reconstructed numerical results.
nat=load('/tmp/oriole-cdata-finder-native-independent-review.json')
for row in summary['measured_aggregates']:
 if row['consumer']=='Native':
  expected=nat['native'][row['profile']]['groups'][row['group']]; assert row['conditions']==expected['conditions']; assert row['lower_than_control']==expected['candidate_faster_than_control']; ratios=expected['ratios']
 else:
  expected=load(f"/tmp/oriole-cdata-finder-python-elapsed-independent-review/{row['profile']}/review.json")['groups']['all']; ratios={k:v['geomean'] for k,v in expected.items()}; assert row['lower_than_control']==expected['candidate_over_control']['below_one']
 assert all(row[k]==v for k,v in ratios.items())
required=['source/LICENSE-APACHE','source/LICENSE-MIT','source/candidate/tests/c/UPSTREAM-NOTICES','source/cpython/LICENSE','expat/COPYING','source/memchr/COPYING','source/memchr/LICENSE-MIT','source/memchr/UNLICENSE']
for name in required: assert name in dest,name
for p in [repo/'benchmarks/projects/corpus',Path('/tmp/oriole-cdata-finder-codegen-study'),Path('/tmp/oriole-streaming-current-profile'),Path('/tmp/oriole-cdata-finder-study/workspace-checks-attempt01')]:
 for f in p.rglob('*'):
  if f.is_file(): assert str(f) in byorigin or str(f) in excluded,str(f)
assert H(Path('/tmp/oriole-cdata-finder-native-decision/decision.json').read_bytes())==summary['decision_origin_sha256']
# Identify excluded artifacts using explicit recorded original hashes; caches are not recreated.
text_records=[]
for r in rows:
 if r['destination'].endswith('.json') and not any(t in r['destination'] for t in ['/stdout/','/stderr/','/raw/']):
  try: text_records.append(Path(r['source']).read_text())
  except UnicodeDecodeError: pass
unbound=[]
for p in sorted(excluded):
 digest=H(Path(p).read_bytes())
 if not any(digest in s for s in text_records): unbound.append({'path':p,'sha256':digest})
result={'status':'passed' if not unbound else 'needs_exclusion_scope_check','role':'Independent saved package/source/origin review against my earlier complete benchmark audits. No compiler/parser or benchmark rerun; numerical campaign audit not duplicated.','archive_sha256':summary['archive_sha256'],'member_index_sha256':summary['member_index_sha256'],'top_files_sha256':H((root/'files.json').read_bytes()),'stored_members':4388,'aliases':1036,'origins':5424,'excluded_paths':36,'all_original_origins_checked':True,'all_alias_targets_stored_and_byte_identical':True,'campaign_coverage':coverage,'measured_source_files':70,'historical_pipeline_files':6,'measured_pipeline_git_base':summary['measured_pipeline_git_base'],'publication_base':summary['publication_base'],'unbound_exclusions':unbound,'findings':[],'scope':['Complete four immediate campaign trees preserved except explicit compiled library/extension aliases; raw results identical to independently reviewed originals.','Both normal/fresh-PGO compiler/profile/training freezes represented by stored members or explicit exclusions.','All six original pipeline files reconstructed from Git144e69e match candidate and control manifests; new publication tooling not substituted.','Every held-out corpus file/notice, source70 and selected memchr/CPython/Expat notices retained; both instruction-profile trees and workspace checks covered.','Summary agrees with independent paired-ratio audits, including normal Python regression and all PGO adverse rows.','Portable verifier checks stored members and aliases without producer paths; independent review additionally checks actual current origins and historical Git bytes.'],'limitations':['Original absolute-path numerical readers require path reconstruction using the member/alias mapping; portable verifier validates bytes rather than rerunning arithmetic.','Excluded executable/build caches are not reconstructable from this archive alone; recorded binary identities and pinned external interpreter/tool provenance remain.','Canonical callback hashes/coalesced Python text are separate from strict compatibility.','Historical one-warmup/one-measured diagnostic captions remain preserved and corrected in final prose.']}
out=Path('/tmp/oriole-cdata-finder-perf-package-review.json'); out.write_text(json.dumps(result,indent=2)+'\n'); print(json.dumps(result,indent=2)); print('REVIEW_SHA256',H(out.read_bytes()))
