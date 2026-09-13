"""Independent documentation/package checks over saved evidence, without parser runs."""
import collections,hashlib,json,re,statistics,subprocess,tarfile
from pathlib import Path
from urllib.parse import unquote,urlsplit

root=Path('/home/dev-user/code/oss/oriole-frame-evidence')
out=Path(__file__).parent
base=root/'docs/validation/2026-09-10/detached-frames'
bench=root/'benchmarks/results/2026-09-10/detached-frames'
hashes={}
def read(p):
 p=Path(p);b=p.read_bytes();hashes[str(p)]=hashlib.sha256(b).hexdigest();return b
def sha(p):read(p);return hashes[str(Path(p))]
def load(p):return json.loads(read(p))
texts={}
files=['README.md','benchmarks/README.md','docs/review.md','docs/compatibility.md','benchmarks/results/2026-09-10/detached-frames/README.md','docs/validation/2026-09-10/detached-frames/README.md']
for f in files:texts[f]=read(root/f).decode()
prior=subprocess.check_output(['git','show','5bc806e:README.md'],cwd=root).decode()
readme=texts['README.md']
assert re.findall(r'^#{1,6} .+$',readme,re.M)==re.findall(r'^#{1,6} .+$',prior,re.M)
assert readme.split('## Highlights')[0]==prior.split('## Highlights')[0]
assert readme[readme.index('## Installation'):]==prior[prior.index('## Installation'):]
assert readme[readme.index('## License'):]==prior[prior.index('## License'):]
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip()=='5bc806e5fc2a545f22c75f2a1bcbef45cfa3632b'
tracked=subprocess.check_output(['git','diff','--name-only'],cwd=root).decode().splitlines()
assert set(tracked)==set(files[:4])
links=[]
for name,text in texts.items():
 for target in re.findall(r'\[[^\]]*\]\(([^\s)]+)\)',text):
  parts=urlsplit(target)
  if parts.scheme or parts.netloc:continue
  path=(root/name).parent/unquote(parts.path) if parts.path else root/name
  assert path.exists(),(name,target)
  if parts.fragment and path.is_file() and path.suffix=='.md':
   headings=re.findall(r'^#{1,6} (.+)$',path.read_text(),re.M)
   slug=lambda x:re.sub(r'[^\w\- ]','',x.lower()).replace(' ','-')
   assert parts.fragment in [slug(x) for x in headings],(name,target)
  links.append({'file':name,'target':target})
index=load(base/'files.json')['files'];assert len(index)==30
assert {e['path'] for e in index}=={str(p.relative_to(base)) for p in base.rglob('*') if p.is_file() and p.name!='files.json'}
for e in index:
 b=read(base/e['path']);assert len(b)==e['bytes'] and hashlib.sha256(b).hexdigest()==e['sha256']
ci=load(base/'ci.json')
assert [(p['number'],p['headRefOid']) for p in ci]==[(103,'ebb9aefd566ad7178b6b869aefdfc231ad45e4e1'),(105,'fdb7afb157fb020c9d2fa775d82b9087112c7e0b'),(106,'5bc806e5fc2a545f22c75f2a1bcbef45cfa3632b')]
assert all(len(p['statusCheckRollup'])==12 and all(c['conclusion']=='SUCCESS' and c['status']=='COMPLETED' for c in p['statusCheckRollup']) for p in ci)
summary=load(base/'summary.json')
assert summary['workspace_tests']==392 and summary['workspace_executables']==33
checks=load('/tmp/oriole-text-frames-pr/checks.json');assert checks['status']=='passed' and checks['source_unchanged'] and all(j['exit']==0 for j in checks['jobs'])
test=read('/tmp/oriole-text-frames-pr/test.log').decode();counts=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed;',test)
assert len(counts)==33 and sum(int(p) for p,f in counts)==392 and all(int(f)==0 for p,f in counts)
for label,count in [('grammar-layer',377),('start-layer',386)]:
 with tarfile.open(base/'evidence.tar.gz') as tar:
  tests=[m for m in tar if m.isfile() and m.name.startswith(label+'/') and m.name.endswith(('test.log','tests.log'))]
  tallies=[]
  for m in tests:
   t=tar.extractfile(m).read().decode(errors='replace');c=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed;',t)
   tallies.append(sum(int(p) for p,f in c))
  assert count in tallies,(label,tallies)
classification=load(base/'api-classification/classification.json')
assert classification['total']==4740 and classification['passes']==4347 and classification['failures']==393
assert classification['api_sha256']==sha('/tmp/oriole-frame-integrated-gates/upstream-api/results.json')
assert classification['api_sha256']==summary['api']['results_sha256']
groups=collections.Counter(r['category'] for r in classification['rows'])
assert sum(groups.values())==393 and len(classification['rows'])==393
assert groups=={k:v['failed_configurations'] for k,v in classification['groups'].items()}
assert sorted(groups.values())==[1,2,12,12,12,12,44,298]
for p in (base/'api-classification').iterdir():
 assert read(p)==read(Path('/tmp/oriole-final-allocation-assessment')/p.name)
reviews={
 'behavior-review.json':'e6777f520cdf2bbca2fcf547ded6813fbe44253f7d38b5150c0c0460ff4dff85',
 'python-review.json':'8fc9d02096d64e50d909d475fac818ca8c98ffcf7848009c4c340df1bb2808ad',
 'native-review.json':'863e4f00049fbccbfde1858d3ea114af4a332df8bc9243bc346f34461560a087',
 'build-equivalence-review.json':'05fd213cced1ccdba66400aaf0488eec9a156356347c50444ff62bb12dec68bd',
 'source.json':'6f6bfb5c076cbf34a18b742783591612fafa8f660efbfb9b8e989461fdeb4805',
 'measured-source.json':'e05d0c4f8de7978968e4b4dafc1ca747029f284fd4ac238064050b1ee5eec38c',
}
for p,d in reviews.items():assert sha(base/p)==d
n=load('/tmp/oriole-integrated-native-study/native-screen/results.json')
p=load('/tmp/oriole-integrated-consumer-study/screen/results.json')
ng=load(bench/'native-summary.json');pg=load(bench/'python-summary.json')
assert ng==load('/tmp/oriole-integrated-native-study/summary.json')==summary['native_benchmark']
assert pg==load('/tmp/oriole-integrated-consumer-study/summary.json')==summary['python_benchmark']
assert n['status']==p['status']=='passed'
# Reconstruct displayed native per-project table from process medians and paired ratios.
labels={'vulkan':'Vulkan registry','wayland':'Wayland protocol','maven':'Maven POM','batik':'Batik SVG','gtk':'GTK UI','docbook':'DocBook XSL'}
displayed=[]
for s in n['summary']:
 c=s['condition']
 if c['name'] not in labels or c['chunk']!=4096 or c['namespaces']:continue
 med={e:statistics.median([q['median_seconds'] for r in n['rows'] if r['condition']==c for q in r['processes'] if q['engine']==e]) for e in ['candidate','expat']}
 ratio=statistics.median(s['paired_ratios']['candidate_over_expat']);assert ratio==s['median_ratios']['candidate_over_expat']
 line=f"| {labels[c['name']]} | {med['candidate']*1000:.3f} ms | {med['expat']*1000:.3f} ms | {ratio:.2f}× |"
 assert line in readme and line in texts['benchmarks/results/2026-09-10/detached-frames/README.md'],line
 displayed.append({'project':c['name'],'candidate_ms':med['candidate']*1000,'expat_ms':med['expat']*1000,'paired_median_ratio':ratio})
assert len(displayed)==6
rows=[('Native real-project XML',ng['real']),('Native namespaces disabled',ng['namespaces-off']),('Native namespaces enabled',ng['namespaces-on']),('Actual CPython XML consumers',pg['all']),('ElementTree',pg['elementtree']),('pyexpat callbacks',pg['pyexpat-events']),('Native generated adverse fixtures',ng['generated'])]
for label,g in rows:
 cp=g['candidate_over_published'];ce=g['candidate_over_expat']
 line=f"| {label} | {cp['conditions']} | {cp['geomean']:.3f}× | {ce['geomean']:.3f}× | {cp['below_one']} | {ce['below_one']} |"
 assert line in texts['benchmarks/results/2026-09-10/detached-frames/README.md'],line
assert f"{100*(1-ng['real']['candidate_over_published']['geomean']):.1f}%"=='18.3%'
assert f"{100*(1-pg['all']['candidate_over_published']['geomean']):.1f}%"=='8.2%'
assert ng['real']['candidate_over_expat']['below_one']==0 and pg['all']['candidate_over_expat']['below_one']==2
wins=[s for s in p['summary'] if s['median_ratios']['candidate_over_expat']<1]
assert len(wins)==2 and all(s['condition']['name']=='wayland' and s['condition']['mode']=='pyexpat-events' for s in wins)
assert sorted(round(s['median_ratios']['candidate_over_expat'],3) for s in wins)==[.932,.935]
# Verify the atomic archive's complete top-level member map independently.
a=base/'unselected-accounting';expected=load(a/'members.json')
with tarfile.open(a/'evidence.tar.gz') as tar:
 actual={}
 for m in tar:
  assert m.isfile() and m.name not in actual
  b=tar.extractfile(m).read();actual[m.name]={'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}
assert actual==expected
validation=texts['docs/validation/2026-09-10/detached-frames/README.md']
assert 'callback waiver' not in validation and 'ceilings are unchanged' in validation
assert 'stale prose `method` label' in validation
assert 'section-by-section\nequivalence comparison below covers the shared libraries' in validation
assert '773d48c79dd4b9d0f47327e103ae0daf706d4d23fa36239fe046f69798756e79' in validation
assert '303988d9a1f6abe34e5aec54a682b6140a69d2a170f0b312e324a88e7208b8f8' in validation
assert 'The Oriole baseline and candidate use fresh normal builds with identical compiler settings' in texts['benchmarks/results/2026-09-10/detached-frames/README.md']
# Save exact reviewed mutable docs; parent may add this audit and regenerate index later.
for f in files:
 target=out/'final'/f;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((root/f).read_bytes())
readback=load(out/'readback.json');assert readback['status']=='passed'
report={
 'status':'passed','findings':[],
 'scope':'Independent source/documentation and saved-data package audit only; no parser runs, builds, timings, repository edits or live GitHub query. Saved CI snapshot checked against exact heads.',
 'base_commit':'5bc806e5fc2a545f22c75f2a1bcbef45cfa3632b',
 'readme_invariants':{'headings':re.findall(r'^#{1,6} .+$',readme,re.M),'warning_and_intro_exact':True,'installation_through_license_exact':True,'license_verbatim':True},
 'relative_links_checked':len(links),'outer_index_entries_checked':len(index),'ci_snapshot_successes':36,
 'main_archive':{'sha256':readback['main_archive_sha256'],'members':2357,'bytes':10786733,'uncompressed_bytes':88773781,'excluded_binary_hashes_verified':50,'all_member_and_live_origin_hashes_verified':True,'nested_archives_checked':len(readback['nested_archives'])},
 'atomic_archive_members_verified':len(actual),'api_classification':{'total':4740,'passed':4347,'failed':393,'categories':dict(groups)},
 'workspace':{'tests':392,'executables':33,'fmt_clippy':'passed','earlier_grammar_tests':377,'earlier_start_tests':386},
 'native_display_table':displayed,'native_aggregates':ng,'python_aggregates':pg,
 'reviewed_interpretation':[
  '18.3% native and8.2% Python reductions are geometric aggregates of paired condition medians versus fresh4b, not elapsed totals or attribution to one frame layer. Every real condition improves, both generated rare-declaration regressions remain explicit.',
  'No broad faster-than-Expat claim: native0/24 wins, Python2/24 Wayland pyexpat wins; overall2.132/1.478 slower. Normal builds remain separate from prior PGO and full applications.',
  'Measured/behavior-tested shared9277 and static773d identities remain explicit. Assembled5af shared code/data is metadata-equivalent except build ID and18 panic lines. Static303988 has build provenance only; shared section comparison is not claimed for static archive.',
  'Original393 API failures and both CPython grouping failures remain failures. Exact resource bounds/ceilings, separate semantic checks, fresh-import coverage and C-only sanitizer scope are retained.',
  'Saved earlier Rust fuzz/PBS results apply to4b, not the new runtime. Initial author-test/lint/controller/LSan failures and stale-build/objcopy incidents are disclosed without relabeling old binaries.',
  'The archived Python method label remains stale but exact hashes and independent audit identify9277; final documentation explicitly resolves it. Compiler-equality wording is now correctly limited to the Oriole baseline/candidate.',
 ],
 'limits':[
  'This audit reuses prior full raw benchmark/behavior/source reviews, rehashes their packaged copies, and independently reconstructs displayed tables and aggregates. It does not rerun their parser workloads.',
  '14 CPython reported skips comprise5 whole methods,8 subtests and1 class setup, as retained behavior review details; they are not14 skipped test methods.',
  'Saved malformed matching streams and callback-time return values have the retention limits stated in behavior-review.json. No exact full Expat conformance claim follows from the comparison with the preceding Oriole source.',
  'Shared-host variability, parser/callback consumer scope, normalized text and unloaded Batik external DTD remain material benchmark limits.',
  'files.json covers the30 files present at this review. Parent may add this review/generator and regenerate that outer index; archive hash is unchanged.',
 ],
 'artifacts_sha256':hashes,
}
(out/'links.json').write_text(json.dumps(links,indent=2)+'\n')
(out/'review.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'status':'passed','review_sha256':sha(out/'review.json'),'outer_files':len(index),'links':len(links),'atomic_members':len(actual)}))
