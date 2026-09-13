from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parent;W=Path('/home/dev-user/code/oss/oriole-compiler-experiments');N=Path('/tmp/oriole-x86-64-v3-native-independent-review');b=Path('/tmp/oriole-allocator-bolt-native-independent-audit/review.json')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();load=lambda p:json.loads(p.read_text())
a,p,h,bolt=[load(x) for x in [N/'normal.json',N/'pgo.json',N/'posthoc.json',b]]
doc=W/'docs/validation/2026-09-11/compiler-experiments/README.md';text=doc.read_text();main=W/'README.md';before=subprocess.check_output(['git','-C',str(W),'show','HEAD:README.md']).decode();after=main.read_text()
added=next(x for x in after.split('\n\n') if x.startswith('A later [CPU-targeting and BOLT study]'))
assert after.replace(added+'\n\n','')==before
checks=[(a['groups']['real']['ratios']['oriole-v3_over_oriole-generic'],-2.26),(p['groups']['real']['ratios']['oriole-v3_over_oriole-generic'],-4.16),(a['groups']['generated']['ratios']['oriole-v3_over_oriole-generic'],-7.98),(p['groups']['generated']['ratios']['oriole-v3_over_oriole-generic'],-8.57),(bolt['groups']['real']['ratios']['candidate_over_control'],-1.47),(bolt['groups']['generated']['ratios']['candidate_over_control'],2.00),(bolt['groups']['real']['ratios']['candidate_over_relocation'],-4.85),(bolt['groups']['real']['ratios']['relocation_over_control'],3.69)]
assert all(round((ratio-1)*100,2)==expected for ratio,expected in checks)
for value in ['−2.26%','−4.16%','−7.98%','−8.57%','−1.47%','+2.00%','4.85%','3.69%','1.3820×','1.2599×','1.3263×','32.63%']:assert value in text,value
assert sum(x['all_workers'] for x in (a,p,bolt))==2688 and sum(x['total_samples'] for x in (a,p,bolt))==424704
assert h['modes']['pgo']['groups']['real']['oriole_v3_lower']==6
assert 'before the [Context Text change]' in text and 'percentages cannot be combined' in text and 'Full API, CPython and sanitizer gates have not run on the v3 artifacts' in text
assert 'post-hoc' in text and 'original paired worker samples' in text
report={'status':'passed_bounded_prose_review','overview_sha256':sha(doc),'main_readme_sha256':sha(main),'main_change':'Exactly one paragraph added; removing it reproduces HEAD README byte-for-byte. Existing headings, warning, license and benchmark table unchanged.',
 'numerical_sources':{str(x):sha(x) for x in [N/'normal.json',N/'pgo.json',N/'posthoc.json',b]},'verified':'Overview ISA/BOLT changes and practical post-hoc comparator, counts2688/424704, sourcee1 before Context, separate experiment scope and missing new compatibility/Python/PBS gates. Main paragraph rounded4.2%,1.5%,1.33x agrees.',
 'scope_limits':['Existing historical PGO/O2/allocator paragraphs are unchanged and were not re-audited in this bounded pass.','Child package links are publication destinations owned by root; no claim that not-yet-copied child packages were present at review time.','This is prose/source review and saved numerical comparison, not a new target run.']}
(R/'overview-review.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'status':report['status'],'sha256':sha(R/'overview-review.json')}))
