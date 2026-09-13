"""Read-only review of publication claims and saved arithmetic. No target runs."""
from pathlib import Path
from statistics import median
from urllib.parse import unquote, urlsplit
import hashlib, json, math, re, subprocess

O=Path(__file__).parent
R=Path('/home/dev-user/code/oss/oriole-native-byte-count')
D=R/'docs/validation/2026-09-11/native-byte-count'
checked={}
def read(p):
    p=Path(p);b=p.read_bytes();checked[str(p)]=hashlib.sha256(b).hexdigest();return b
def js(p):return json.loads(read(p))
def git(*args):return subprocess.check_output(['git','-C',str(R),*args])
def close(a,b):assert math.isclose(a,b,rel_tol=1e-14,abs_tol=1e-15),(a,b)
def geom(xs):return math.exp(sum(map(math.log,xs))/len(xs))
texts={str(p):read(p).decode() for p in [R/'README.md',D/'README.md',R/'benchmarks/results/2026-09-11/native-byte-count/README.md',R/'benchmarks/README.md',Path('/tmp/oriole-native-byte-count-pr-body.md')]}
human=texts[str(D/'README.md')]; top=texts[str(R/'README.md')]
parent=git('show','079fb66dc159cdc0e1c3ad16397f149751783014:README.md').decode()
headings=lambda t:re.findall(r'^#{1,6} .+$',t,re.M)
warning=lambda t:'\n'.join(x for x in t.splitlines() if x.startswith('>'))
assert headings(top)==headings(parent) and warning(top)==warning(parent)
assert top[top.index('## License'):]==parent[parent.index('## License'):]
assert top[:top.index('| Project XML')]==parent[:parent.index('| Project XML')]
assert top[top.index('## Installation'):]==parent[parent.index('## Installation'):]
toucan=read('/home/dev-user/code/oss/toucan/README.md').decode()
assert headings(top)==[x.replace('Toucan','Oriole') for x in headings(toucan)]
assert top[top.index('## License'):]==toucan[toucan.index('## License'):].replace('Toucan','Oriole')
for name in ['LICENSE-APACHE','LICENSE-MIT']:
    assert read(R/name)==git('show','079fb66dc159cdc0e1c3ad16397f149751783014:'+name)
ci={}
for name in git('ls-tree','-r','--name-only','079fb66dc159cdc0e1c3ad16397f149751783014','.github').decode().splitlines():
    current=read(R/name);assert current==git('show','be22a271f8a5c004d13e515cd4024a68afe57887:'+name)==git('show','079fb66dc159cdc0e1c3ad16397f149751783014:'+name)
    ci[name]=hashlib.sha256(current).hexdigest()
assert not git('diff','079fb66dc159cdc0e1c3ad16397f149751783014','be22a271f8a5c004d13e515cd4024a68afe57887','--','.github').strip()
assert len(git('diff','--name-only','079fb66dc159cdc0e1c3ad16397f149751783014','be22a271f8a5c004d13e515cd4024a68afe57887').decode().splitlines())==1
links=[]
for path,text in texts.items():
    for href in re.findall(r'(?<!!)\[[^\]]+\]\(([^)]+)\)',text):
        url=href.strip('<>');parts=urlsplit(url)
        if parts.scheme or url.startswith('#'):continue
        dest=(Path(path).parent/unquote(parts.path)).resolve()
        assert dest.exists(),(path,href)
        links.append({'from':path,'href':href,'exists':True})

# Reconstruct native process medians from the complete saved embedded outputs.
native=js('/tmp/oriole-raw-len-split-thinlto-timing-study/native-screen/results.json')
assert len(native['rows'])==196 and len(native['summary'])==28
paired={}; by_condition={}; nmeasured=0; nwarm=0
for row in native['rows']:
    c=row['condition'];key=(c['name'],c['chunk'],c['namespaces']);pair=row['pair']
    assert (key,pair) not in paired
    med={}
    for proc in row['processes']:
        assert proc['returncode']==0
        data=json.loads(proc['stdout']); samples=data['samples']
        assert samples[0]['warmup'] and not any(x['warmup'] for x in samples[1:])
        assert len(samples)==c['iterations']+1
        m=median(x['seconds'] for x in samples[1:]);close(m,proc['median_seconds'])
        med[proc['engine']]=m;nmeasured+=len(samples)-1;nwarm+=1
    assert set(med)=={'published','candidate','expat'};paired[key,pair]=med
    by_condition.setdefault(key,[]).append(med)
native_rows={}
for row in native['summary']:
    c=row['condition'];key=(c['name'],c['chunk'],c['namespaces'])
    assert len(by_condition[key])==7
    ratios={}
    for label,(a,b) in {'candidate_over_published':('candidate','published'),'candidate_over_expat':('candidate','expat'),'published_over_expat':('published','expat')}.items():
        assert {p for k,p in paired if k==key}==set(range(7))
        values=[m[a]/m[b] for m in by_condition[key]]
        assert values==row['paired_ratios'][label]
        ratios[label]=median(values);close(ratios[label],row['median_ratios'][label])
    native_rows[key]=ratios
assert (nmeasured,nwarm)==(105420,588)
ns=js(D/'native-summary.json')
for group in ['real','generated']:
    vals=[v for k,v in native_rows.items() if k[0].startswith('generated')==(group=='generated')]
    assert len(vals)==ns['groups'][group]['conditions']
    for label,expected in ns['groups'][group]['ratios'].items():
        close(geom([v[label] for v in vals]),expected['geomean'])
        assert sum(v[label]<1 for v in vals)==expected['below_one']
labels={'vulkan':'Vulkan registry','wayland':'Wayland protocol','maven':'Maven POM','batik':'Batik SVG','gtk':'GTK UI','docbook':'DocBook XSL'}
display=[]
for name in labels:
    k=(name,4096,False); times={e:median(x[e] for x in by_condition[k])*1000 for e in ['candidate','expat']}
    display.append(f"| {labels[name]} | {times['candidate']:.3f} ms | {times['expat']:.3f} ms | {native_rows[k]['candidate_over_expat']:.2f}× |")
table='\n'.join(display)
assert table==read(D/'table.md').decode().strip() and table in human and table in top

# Consumer per-process samples are already origin/raw-file audited separately;
# repeat their arithmetic here to tie the published table to this exact study.
py=js('/tmp/oriole-raw-len-split-thinlto-python-root-study/screen/results.json')
assert len(py['rows'])==len(py['processes'])==504 and len(py['preflights'])==72
pymed={};pycounts=0
for row in py['rows']:
    ss=row['samples'];assert ss[0]['warmup'] and not any(s['warmup'] for s in ss[1:])
    m=median(x['seconds'] for x in ss[1:]);close(m,row['median_seconds'])
    k=(row['key'],row['pair'],row['engine']);assert k not in pymed;pymed[k]=m;pycounts+=len(ss)-1
assert pycounts==25788
pyrows={}
for row in py['summary']:
    c=row['condition'];key=f"{c['name']}/{c['chunk']}/{c['mode']}";v={}
    for label,(a,b) in {'candidate_over_published':('candidate','published'),'candidate_over_expat':('candidate','expat'),'published_over_expat':('published','expat')}.items():
        rr=[pymed[key,p,a]/pymed[key,p,b] for p in range(7)]
        assert rr==row['paired_ratios'][label];v[label]=median(rr);close(v[label],row['median_ratios'][label])
    pyrows[key]=v
ps=js(D/'python-summary.json')
for group in ['all','elementtree','pyexpat-events']:
    vals=[v for k,v in pyrows.items() if group=='all' or k.endswith('/'+group)]
    assert len(vals)==ps['groups'][group]['conditions']
    for label,expected in ps['groups'][group]['ratios'].items():
        close(geom([v[label] for v in vals]),expected['geomean']);assert sum(v[label]<1 for v in vals)==expected['below_one']
assert [k for k,v in pyrows.items() if v['candidate_over_expat']<1]==['wayland/4096/pyexpat-events','wayland/65536/elementtree','wayland/65536/pyexpat-events']
for label,group,rowname in [('native','real','Project XML'),('native','generated','Generated controls'),('python','elementtree','ElementTree'),('python','pyexpat-events','pyexpat events'),('python','all','Combined')]:
    g=(ns if label=='native' else ps)['groups'][group];a=g['ratios']['candidate_over_published'];b=g['ratios']['candidate_over_expat']
    assert f"| {rowname} | {g['conditions']} | {a['geomean']:.6f} | {b['geomean']:.6f} | {a['below_one']} | {b['below_one']} |" in human
# Current publication arithmetic and scoped evidence, independently of doc generator.
assert f"{(1-ns['groups']['real']['ratios']['candidate_over_published']['geomean'])*100:.1f}%"=='3.1%'
assert f"{(1-ps['groups']['all']['ratios']['candidate_over_published']['geomean'])*100:.1f}%"=='1.5%'
assert f"{(1-ns['groups']['generated']['ratios']['candidate_over_published']['geomean'])*100:.1f}%"=='4.0%'
regressions=[k for k,v in pyrows.items() if v['candidate_over_published']>1]
assert regressions==['docbook/65536/pyexpat-events']
assert f"{(pyrows[regressions[0]]['candidate_over_published']-1)*100:.2f}%"=='0.59%'
assert ns['regressions']==[]
for t in texts.values():
 if 'elapsed time' in t:
  assert all(x in t for x in ['3.1%','1.5%','4.0%','0.59%','1.59','1.27'])
profile=js('/tmp/oriole-raw-len-split-profile-independent-review/review.json')
assert profile['profile_processes']==profile['preflight_processes']==32 and profile['parser_samples']==128
assert profile['project_conditions_lower']==12 and profile['generated_conditions_lower']==4
assert f"{profile['project_instruction_ratio_geomean']:.6f}"=='0.977682'
assert f"{profile['generated_instruction_ratio_geomean']:.6f}"=='0.972419'
codegen=js('/tmp/oriole-raw-len-split-codegen-study/report.json')
assert codegen['engines']['control']['text_section_bytes']==784461
assert codegen['engines']['candidate']['text_section_bytes']==787405
assert codegen['text_byte_delta']==2944
for name,h in codegen['evidence_sha256'].items():assert hashlib.sha256(read(Path('/tmp/oriole-raw-len-split-codegen-study')/name)).hexdigest()==h
assert codegen['engines']['candidate']['selected_symbols']['<oriole::encoding::Source>::raw_len'] is None
assert codegen['engines']['candidate']['selected_symbols']['<oriole::encoding::Source>::converted_raw_len'] is not None
classification=js(R/'docs/validation/2026-09-10/detached-frames/api-classification/classification.json')
assert (classification['passes'],classification['failures'],classification['total'],classification['failed_test_names'])==(4347,393,4740,38)
assert sorted(g['failed_configurations'] for g in classification['groups'].values())==[1,2,12,12,12,12,44,298]
assert sum(g['failed_configurations'] for g in classification['groups'].values())==393
summary=js(D/'summary.json');workspace=js(D/'workspace.json');gates=js(D/'gates.json')
assert summary['workspace']==workspace and summary['api']==gates['api']
assert summary['native']==ns['groups'] and summary['python']==ps
assert summary['api']['bounds']=={'per_test_seconds':3,'per_test_address_space_bytes':1073741824,'per_test_rss_limit_bytes':805306368,'total_timeout_seconds':240}
assert summary['cpython_each_linkage']=={'methods':802,'failures':2,'reported_skips':14,'expected_failures':3}
assert (workspace['all_target_tests'],workspace['target_inventory_rows'],workspace['doc_tests'],workspace['new_or_removed_tests'])==(407,33,1,0)
package_review=js('/tmp/oriole-native-byte-count-package-independent-review/review.json')
assert not package_review.get('blocking_findings',package_review.get('blockers',[]))
source=js(D/'source.json');source_review=js(D/'source-build-review.json');build=js(D/'build.json')
assert hashlib.sha256(read(D/'source.json')).hexdigest()==summary['source_manifest_sha256']==source_review['source_manifest_sha256']
assert len(source['source_sha256'])==70
for name,h in source['source_sha256'].items():
 assert hashlib.sha256(read(R/name)).hexdigest()==h
 assert hashlib.sha256(git('show',summary['runtime_commit']+':'+name)).hexdigest()==h
assert git('diff',summary['parent_commit'],summary['runtime_commit'],'--','crates/oriole/src/encoding.rs')==read('/tmp/oriole-raw-len-split-study/candidate.patch')
assert git('diff','--numstat',summary['parent_commit'],summary['runtime_commit']).decode().strip()=='6\t0\tcrates/oriole/src/encoding.rs'
assert source_review['shared_and_static_whole_bytes_equal_to_measured_prototype']
assert build['fresh_workspace_compiles']==source_review['actual_workspace_compiler_vectors']==3
assert build['compiler_comparison']['matched_after_private_paths_only']
assert '-C lto=thin' in build['compiler_comparison']['normalized']['candidate']['oriole_expat']
assert source_review['recorded_environment_differences']=={'CARGO_TARGET_DIR':['/home/dev-user/.cache/oriole/raw-len-split-target','/home/dev-user/.cache/oriole/native-byte-count-target']}
prototype=js('/tmp/oriole-raw-len-split-independent-review/review.json')
assert set(prototype['environment_differences'])=={'CARGO_TARGET_DIR','CARGO_UNSTABLE_OHM_NATIVE_TOOL_TRUST','CARGO_UNSTABLE_OHM_PROC_MACRO_TRUST'}
assert 'measured prototype omits two optional Ohm trust settings relative to the control' in human
assert 'integrated rebuild retains that environment' in human
assert read(D/'oriole-write-native-byte-count-docs.py')==read('/tmp/oriole-write-native-byte-count-docs.py')
assert summary['libraries']==source_review['libraries']
for name,h in summary['libraries'].items():assert hashlib.sha256(read(Path('/tmp/oriole-native-byte-count-thinlto-study')/name)).hexdigest()==h
# Separate prior independent complete raw audits support controller, origin and sample provenance.
for orig,copied in [('source-build-review.json','/tmp/oriole-native-byte-count-source-build-independent-review/review.json'),('python-review.json','/tmp/oriole-raw-len-split-python-independent-review/review.json')]:
 assert read(D/orig)==read(copied)
nr=js('/tmp/oriole-raw-len-split-native-independent-review/review.json')
pr=js(D/'python-review.json')
assert nr['groups']['real']==ns['groups']['real'] and nr['groups']['generated']==ns['groups']['generated']
for group in ps['groups']:
 for ratio,row in ps['groups'][group]['ratios'].items():
  assert pr['groups'][group][ratio]==dict(row,conditions=ps['groups'][group]['conditions'])
assert nr['controller_review']['worker_and_timing_branch_AST_exact']
assert pr['controller']['complete_ast_after_reversal_exact']
assert pr['history']['collision_receipt']['status']=='failed_before_any_consumer_build_or_parser_run'
assert gates['native']['consumers']==6 and gates['native']['allocation_scenarios_per_linkage']==327
assert (gates['strict_baseline_traces'],gates['custom_alias_baseline_comparisons'],gates['malformed_baseline_comparisons'])==(3318,36456,2392)
assert (gates['publication']['logical_cases'],gates['publication']['parser_executions'])==(1304,3912)
assert gates['end_lifecycle']['logical_cases']==38 and gates['end_oom']['failed_indices']==4 and gates['end_oom']['success_controls']==2
assert gates['retained_harness_failure']['initial_end_parser_executions']==0
cgate=js(D/'gates-independent-review/review.json');assert not cgate.get('blocking_findings',[])
cp=js(D/'cpython-root-review.json')
assert cp['findings']==[]
for linkage,c in cp['cpython'].items():
 assert (c['methods'],c['expected_failures'],c['reported_skips'],c['whole_method_skips'],c['subtest_skips'],c['class_setup_skips'])==(802,3,14,5,8,1)
 assert c['failures']==['test.test_pyexpat.BufferTextTest.test1','test.test_sax.CDATAHandlerTest.test_handlers']
 assert c['all_method_statuses_equal_corrected_baseline']
for name,h in cp['evidence_sha256'].items():assert hashlib.sha256(read(name)).hexdigest()==h
assert summary['archive']=={'sha256':'bc63cf87e4733c2b05e7391e0ef782747a6ba0ea1046efae92843a53485b6dd0','bytes':13178538,'members':1863,'binary_exclusions':17,'all_direct_members_and_origins_read_back':True}
assert hashlib.sha256(read(D/'evidence.tar.gz')).hexdigest()==summary['archive']['sha256']
assert 'workspace inventory review is author-owned' in human
assert 'same root agent that collected these strict tests' in human
assert 'not every successful raw pair' in human
assert 'leak checking is disabled' in human and 'release libraries are uninstrumented' in human
assert 'Automatic GC stays enabled' in human and 'explicit `gc.collect` calls occur outside timing' in human
assert 'namespace-restoration fallback' in human and 'do not alone establish coverage of every detached End path' in human
assert 'This source has no new sustained Rust sanitizer campaign or full PBS distribution build.' in human
assert not git('diff','--check').strip()
# Avoid claiming publication text or historical reviewer role was authored by a human.
snap=O/'publication-snapshot';snap.mkdir(exist_ok=True)
for i,(p,t) in enumerate(texts.items()):(snap/f'{i}-{Path(p).name}').write_text(t)
(O/'native-display.json').write_text(json.dumps({'rows':display,'native_groups':ns['groups'],'python_groups':ps,'native_measured':nmeasured,'native_warmups':nwarm,'python_measured':pycounts},indent=2)+'\n')
manual=[
 'All displayed native times and paired ratios plus aggregate native/Python tables independently reconstructed from embedded saved samples; 28 native and24 Python conditions retained. Six table rows match main README and full report exactly.',
 '3.1% native,1.5% Python,4.0% generated reductions correctly round paired-median geometric ratios; sole DocBook64KiB pyexpat+0.59% remains explicit. Current ratios to Expat remain1.59/1.27 and generated4.99; no parity or production-readiness claim.',
 'Private six-added-line extraction, whole library identity,70 source blobs and committed prototype patch reviewed. Effective ThinLTO vectors are separate from normal workspace tests. Prototype-versus-control trust option difference and integration-versus-prototype byte identity are now unambiguous.',
 '407/33/+1 workspace result is independently reconstructed by the separate package reviewer; original author inventory retains its historical role. Canonical4740 bounds/raw4347/393, C sanitizer limits, custom successful raw-pair limit, End fallback scope and preserved CPU-guard/postprocessing failures remain explicit.',
 'Strict802 per linkage remains exactly two known failures,14 skips decomposed5methods8subtests1class and3expected. Root self-review role explicitly identified; evidence hashes checked without rerunning targets.',
 'Timing includes construction/feeds/destruction; automatic Python GC remains enabled and explicit collections remain untimed. Canonicalization/import/input and dynamic loading remain outside their declared timing scopes. Shared host and unloaded Batik DTD limits retained.',
 'Instruction ratios and text growth2944 bytes match the saved independent profile and author static-codegen records. Codegen addresses/instruction-site evidence does not establish complete elapsed causality.',
 'Main README headings, warning, license and installation onward exactly match parent079fb; Toucan headings/license agree after project-name substitution. CI runner bytes remain unchanged. All local links resolve.',
 'No new sustained ASan/fullPBS/PGO result is implied. Previous fuzz5bc and PBS4b scope remains unchanged. Initial Python collision is supported by source ordering and author declaration, not a saved raw traceback.'
]
report={'status':'independent_agent_publication_text_review_passed','scope':'Read-only independent agent review of publication prose, saved arithmetic, source identity and scope. No human/external audit claim, source edits, target tests, compilers, parsers or timings. Complete archive/origin/workspace inventory reconstruction is a separate independent package review.',
 'runtime_commit':summary['runtime_commit'],'parent_commit':summary['parent_commit'],'source_manifest_sha256':summary['source_manifest_sha256'],
 'blockers':[],'reviewed_findings':manual,'resolved_wording':'Explicitly identify measured prototype trust opt-ins relative to control; integrated rebuild retains prototype environment and byte identity.',
 'README_invariance':{'headings_warning_parent_exact':True,'toucan_headings_license_project_substitution_exact':True,'license_files_unchanged':True,'CI_files_unchanged':ci},
 'local_links_checked':links,'native_counts':{'conditions':28,'preflights':84,'workers':588,'cohorts':196,'measured':105420,'warmups':588},'python_counts':{'conditions':24,'preflights':72,'workers':504,'cohorts':168,'measured':25788,'warmups':504},
 'native_groups':ns['groups'],'python_groups':ps,'package_review_sha256':checked['/tmp/oriole-native-byte-count-package-independent-review/review.json'],
 'limitations':['Final attachment files.json remains a subsequent bounded snapshot audit.','Previous complete raw provenance/controller reviews are reused and pinned; this review repeats saved embedded-sample arithmetic, not every prior raw-file readback.','Two initial exploratory schema/file reads failed (missing historical classification README; Python row has no native processes field). These read-only tool errors precede the review generator, changed no evidence and ran no targets.'],
 'generator_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'checked_files':len(checked),'all_inspected_inputs_unchanged':True}
for p,h in checked.items():assert hashlib.sha256(Path(p).read_bytes()).hexdigest()==h,p
(O/'checked-files.json').write_text(json.dumps(checked,indent=2)+'\n')
(O/'review.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'status':report['status'],'files':len(checked),'links':len(links),'review_sha256':hashlib.sha256((O/'review.json').read_bytes()).hexdigest()}))
