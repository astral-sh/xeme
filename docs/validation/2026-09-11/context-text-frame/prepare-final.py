from pathlib import Path
import ast,difflib,hashlib,json
R=Path(__file__).resolve().parent
base=(R/'allocator-reference-package.py').read_text()
head=base[:base.index('ROOT =')]+'''ROOT = Path(__file__).resolve().parent
WORK = Path('/home/dev-user/code/oss/oriole-context-text-frame')
NATIVE = 'context-text-frame-fixed-native-study'
AUDIT = 'context-text-frame-fixed-independent-review'
PYTHON = 'context-text-frame-fixed-python-study'
PY_AUDIT = 'context-text-frame-fixed-python-elapsed-independent-review'
STUDIES = {name: Path('/tmp/oriole-' + name) for name in (
 'context-text-frame-fixed-pgo-study', AUDIT, NATIVE,
 'context-text-frame-fixed-thin-pgo-gates',
 'context-text-frame-fixed-thin-pgo-cpython-gates',
 'context-text-frame-fixed-thin-pgo-cpython-preparation',
 'context-text-frame-fixed-strict-outcome-review', PYTHON, PY_AUDIT,
)}
STUDIES.update({'combined-ci':Path('/tmp/oriole-context-text-frame-ci-combined-attempt01'),
 'historical-local':Path('/tmp/oriole-context-text-frame-local-publication'),
 'historical-notices':Path('/tmp/oriole-context-text-frame-local-publication-notices'),
 'historical-reviewer-attempts':Path('/tmp/oriole-context-text-frame-local-publication-reviewer-attempts')})
'''
head+=base[base.index('\ndef digest'):base.index('\ndef main')]
main='''
def main():
    if not __debug__:
        raise RuntimeError('Assertions must remain enabled')
    assert not (ROOT / 'evidence.tar.gz').exists()
    pgo=STUDIES['context-text-frame-fixed-pgo-study']
    source=read(pgo/'source.json');checks=read(pgo/'source-checks.json')
    assert source['candidate_commit']==checks['head']=='0f28139f83b04290e62301c38d8ed489f183a88d'
    assert source['base_runtime']=='e1d263a711e862e4f0f0018b15de9ac83a87a342'
    assert checks['status']=='passed' and len(checks['jobs'])==16
    assert all(x['conclusion']=='success' for x in checks['jobs'])
    assert read(pgo/'pair-report.json')['status']=='passed'
    assert digest((pgo/'source.json').read_bytes())=='8a7da2759b67ca85f82fb29bd775392e532b3ff355340eb10f92ea7764bbe642'
    ci=read(pgo/'combined-ci-independent-review.json')
    assert ci['head']==checks['head'] and ci['all_ci_jobs_successful']==16
    restack=read(pgo/'allocator-docs-restack.json')
    assert restack['old_head']==checks['head'] and restack['source_files_byte_exact']==72
    assert restack['new_head']=='20eda6ccaa22bd65b0ef1a4b3cf0b15d382a5f33'
    gates=read(STUDIES['context-text-frame-fixed-thin-pgo-gates']/'summary.json')
    assert gates['api']['all4740rows_exact'] and (gates['api']['passed'],gates['api']['failed'])==(4347,393)
    compatibility=read(STUDIES[AUDIT]/'compatibility/review.json')
    for row in compatibility['CPython'].values():
        assert row['methods']==802 and row['raw_exit']==2 and row['baseline_equal']
    native=read(STUDIES[AUDIT]/'native/review.json')
    assert native['status']=='passed'
    py=read(STUDIES[PY_AUDIT]/'review.json')
    assert digest((STUDIES[PY_AUDIT]/'review.json').read_bytes())=='2448a869cf73fcc7861b3ab5919df91fde65b3ee2591c546fa1ecfae0087d581'
    assert py['status']=='passed_independent_both_Python_campaigns'
    assert py['counts']['all_workers']==1152 and py['counts']['all_samples']==52728
    for mode in ('normal','pgo'):
        assert read(STUDIES[NATIVE]/mode/'controller.json')['status']=='passed'
        assert read(STUDIES[PYTHON]/mode/'time-controller.json')['status']=='passed'
        assert read(STUDIES[PY_AUDIT]/(mode+'-review.json'))['status']=='passed_independent_raw_elapsed_reconstruction'
'''
inv=base[base.index('    sources = {}'):base.index('    report = {')]
a=inv.index('    for name in (\'oriole-allocation');b=inv.index("    for name in ('COPYING'",a)
inv=inv[:a]+'''    sources['reviews/cgate-outcome.json']=Path('/tmp/oriole-context-text-frame-fixed-cgate-outcome-review.json')
    for path in sorted(Path('/tmp').glob('oriole-context-text-frame-fixed-python-independent-preflight-review*')):
        if path.is_file(): sources['reviews/python-preflight/'+path.name]=path
'''+inv[b:]
inv=inv.replace("('root-draft.py', 'package.py', 'verify.py', 'replay_native.py')", "('PLAN.json','README.draft.md','allocator-reference-package.py','allocator-reference-verify.py','prepare-final.py','package.py','verify.py','replay_native.py','replay_python.py','recompute-native-readme.py','native-readme-table.json','native-readme-table.md')")
inv=inv.replace("STUDIES[AUDIT] / 'worker-aliases.json'", "STUDIES[AUDIT] / 'native/worker-aliases.json'")
inv=inv.replace("STUDIES[AUDIT] / 'conditions.csv'", "STUDIES[AUDIT] / 'native/conditions.csv'")
report='''    report={
        'status':'selected_context_text_frame_evidence',
        'runtime_commit':checks['head'],'publication_restack':restack,
        'source_manifest_sha256':digest((pgo/'source.json').read_bytes()),
        'source_files':72,'runtime_delta':source['runtime_changed_files'],'all_changed_paths':source['changed_files'],
        'combined_CI':{'all_jobs_successful':16,'Miri_tests_per_model':45,'head':checks['head'],'warning_retained':True},
        'historical_local_scope':'Unmodified source06 packet retains226 focused local tests and original warm-test/empty-raw/Clippy/Miri failures. Current combined0f results are separate.',
        'api':{'rows':4740,'passed':4347,'assertion_failures':391,'timeouts':2,'raw_exit':1,'all_outcomes_match_corrected_allocator':True},
        'cpython_compatibility':compatibility['CPython'],
        'native':{mode:native['native'][mode]['groups'] for mode in ('normal','pgo')},
        'native_totals':{'workers':1344,'samples':212352,'measured_timed_samples':210840,'timed_warmups':1176,'preflight_samples':336},
        'python':py['modes'],'python_totals':py['counts'],
        'selection':'Keep Context Text on the corrected allocator baseline. Native normal/PGO and Python PGO improve; normal Python and generated native PGO regress. All adverse cases retained. No new PBS or BOLT claim.',
        'roles':'Runtime/source and CI reviewers are separate. Build readback uses a separate established reader but shares candidate build author; compatibility/native/Python raw audits are independent of their collectors.',
        'archive_sha256':digest((ROOT/'evidence.tar.gz').read_bytes()),
        'archive_index_sha256':digest(index_bytes),'archive_bytes':(ROOT/'evidence.tar.gz').stat().st_size,
        'logical_files':len(files),'unique_files':len(objects),'excluded_binaries_and_caches':len(excluded),'native_worker_aliases':len(aliases),
        'python_alias_scope':'JSON-value aliases for reconstructed rows only. Full original Python report JSON, raw stdout/stderr gzip files, specs and process records are retained. No Python whole-file byte alias or omission is asserted.',
    }
    (ROOT/'report.json').write_bytes(encoded(report))
    names=('README.md','package.py','verify.py','replay_native.py','replay_python.py','evidence.tar.gz','archive-index.json','report.json','conditions.csv','native-readme-table.json','native-readme-table.md')
    seal={name:{'sha256':digest((ROOT/name).read_bytes()),'bytes':(ROOT/name).stat().st_size} for name in names}
    (ROOT/'files.json').write_bytes(encoded({'format':'sha256-file-manifest-v1','files':seal}))
    print(json.dumps({k:report[k] for k in ('status','archive_sha256','archive_bytes','logical_files','unique_files','excluded_binaries_and_caches','native_worker_aliases')}))

if __name__=='__main__':
    main()
'''
pkg=head+main+inv+report
pkg=pkg.replace('saved allocator evidence','saved Context Text evidence')
(R/'package.py').write_text(pkg);ast.parse(pkg)
old=(R/'allocator-reference-verify.py').read_text();new=old.replace('allocator evidence','Context Text evidence').replace("native = 'allocator-provenance-native-study'", "native = 'context-text-frame-fixed-native-study'").replace("audit = 'allocator-provenance-native-independent-review'", "audit = 'context-text-frame-fixed-independent-review/native'").replace('allocator-provenance-pgo-study','context-text-frame-fixed-pgo-study').replace("len(source) == 70", "len(source) == 72")
needle="    report = json.loads((root / 'report.json').read_bytes())\n"
pyblock='''    py_study='context-text-frame-fixed-python-study'
    py_audit='context-text-frame-fixed-python-elapsed-independent-review'
    assert (root/'replay_python.py').read_bytes()==get(py_audit+'/audit.py')
    python_replays={}
    with tempfile.TemporaryDirectory(prefix='oriole-python-records-') as temporary:
        temp=Path(temporary);study=temp/'study';output=temp/'reviews'
        for name in records:
            if name.startswith(py_study+'/'):
                relative=name.removeprefix(py_study+'/');safe(relative)
                target=study/relative;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(get(name))
        for mode in ('normal','pgo'):
            command=[sys.executable,'-I','-S',str((root/'replay_python.py').resolve()),'--root',str(study),'--mode',mode,'--output',str(output)]
            done=subprocess.run(command,capture_output=True,text=True,timeout=180,check=False)
            assert done.returncode==0 and done.stderr=='',(done.returncode,done.stderr)
            actual=json.loads((output/(mode+'-review.json')).read_bytes())
            expected=json.loads(get(py_audit+'/'+mode+'-review.json'))
            for key in ('status','mode','counts','aggregates','summary','libraries','seed','pair_order_verified','raw_rows_exactly_reconstructed','all_callback_canonical_outputs_and_module_dladdr_records_verified','source_results_sha256','source_preflight_sha256','row_aliases_sha256'):
                assert actual[key]==expected[key],(mode,key)
            assert (output/(mode+'-row-aliases.json')).read_bytes()==get(py_audit+'/'+mode+'-row-aliases.json')
            python_replays[mode]={'counts':actual['counts'],'aggregates':actual['aggregates'],'row_aliases_sha256':actual['row_aliases_sha256']}
'''
assert needle in new;new=new.replace(needle,pyblock+needle)
new=new.replace("    receipt = {'status': 'passed'", "    assert report['python']=={mode:python_replays[mode]['aggregates'] for mode in ('normal','pgo')}\n    assert sum(v['counts']['all_workers'] for v in python_replays.values())==report['python_totals']['all_workers']==1152\n    assert sum(v['counts']['all_samples'] for v in python_replays.values())==report['python_totals']['all_samples']==52728\n    assert (root/'native-readme-table.json').read_bytes()==get('reproduce/native-readme-table.json')\n    receipt = {'status': 'passed'")
new=new.replace("'native': replay, 'reader_stdout': outcome.stdout}", "'native': replay, 'python':python_replays,'reader_stdout': outcome.stdout}")
new=new.replace('worker aliases and saved native arithmetic only','native byte aliases, Python value-alias replay and saved native/Python arithmetic only')
(R/'verify.py').write_text(new);ast.parse(new)
(R/'replay_python.py').write_bytes(Path('/tmp/oriole-context-text-frame-fixed-python-elapsed-independent-review/audit.py').read_bytes())
for name,old in [('package.py',base),('verify.py',(R/'allocator-reference-verify.py').read_text())]:
 (R/(name+'.adaptation.patch')).write_text(''.join(difflib.unified_diff(old.splitlines(keepends=True),(R/name).read_text().splitlines(keepends=True),fromfile='allocator-reference-'+name,tofile=name)))
print(json.dumps({name:hashlib.sha256((R/name).read_bytes()).hexdigest() for name in ('package.py','verify.py','replay_python.py')}))
