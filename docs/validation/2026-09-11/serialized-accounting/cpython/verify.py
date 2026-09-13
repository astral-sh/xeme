"""Verify a copied CPython packet and replay saved records without parser imports."""
from pathlib import Path, PurePosixPath
import argparse, csv, hashlib, json, re, subprocess, sys, tarfile, tempfile
cli=argparse.ArgumentParser(description=__doc__)
cli.add_argument('--package',type=Path,required=True)
cli.add_argument('--output',type=Path,required=True)
args=cli.parse_args()
if not __debug__: raise RuntimeError('Run without -O: this verifier uses assertions')
assert sys.version_info[:2]==(3,12) and not args.output.exists()
package=args.package.resolve();sha=lambda b:hashlib.sha256(b).hexdigest();load=lambda p:json.loads(p.read_text())
for name,row in load(package/'files.json').items():
    b=(package/name).read_bytes();assert len(b)==row['bytes'] and sha(b)==row['sha256'],name
index=load(package/'index.json');report=load(package/'report.json')
assert sha((package/'index.json').read_bytes())==report['index_sha256']
assert sha((package/'evidence.tar.gz').read_bytes())==report['evidence_sha256']
expected={r['path']:r for r in index['files']};assert len(expected)==len(index['files'])==report['archive_files']
assert len(index['excluded_binaries'])==report['excluded_binary_files']
results={};csv_rows=[]
with tempfile.TemporaryDirectory(prefix='oriole-serialized-python-records-') as temp:
    root=Path(temp);seen=set()
    with tarfile.open(package/'evidence.tar.gz') as archive:
        for member in archive:
            name=member.name;p=PurePosixPath(name)
            assert member.isfile() and not p.is_absolute() and '..' not in p.parts
            assert name in expected and name not in seen;seen.add(name)
            b=archive.extractfile(member).read();r=expected[name]
            assert len(b)==r['bytes']==member.size and sha(b)==r['sha256']
            assert not b.startswith((b'\x7fELF',b'!<arch>'))
            destination=root/name;destination.parent.mkdir(parents=True,exist_ok=True);destination.write_bytes(b)
    assert seen==set(expected)
    sources={};pairs={}
    for label in ('candidate','selected'):
        directory=root/'builds'/label
        sources[label]=load(directory/'source.json')['source_sha256'];assert len(sources[label])==72
        for name,digest in sources[label].items():assert sha((directory/'source'/name).read_bytes())==digest
        pairs[label]=load(directory/'pair-report.json');assert pairs[label]['status']=='passed'
    delta={name:{'control':sources['selected'][name],'candidate':digest} for name,digest in sources['candidate'].items() if digest!=sources['selected'][name]}
    prepared=load(root/'builds/candidate/preparation.json')
    assert sorted(delta)==prepared['expected_source_delta'] and len(delta)==9
    assert prepared['head']==report['candidate'] and prepared['check_attempt']=='03' and prepared['expected_test_count']==440
    assert (package/'replay.py').read_bytes()==(root/'assembly/replay.py').read_bytes()
    for mode in ('normal','pgo'):
        directory=root/'study'/mode;adaptation=load(directory/'adaptation.json')
        assert adaptation['source_runtime_candidate']==report['candidate'] and adaptation['source_runtime_control']==report['control']
        assert adaptation['source_delta']==delta
        assert adaptation['candidate_source_manifest']['sha256']==sha((root/'builds/candidate/source.json').read_bytes())
        assert adaptation['candidate_pair_report']['sha256']==sha((root/'builds/candidate/pair-report.json').read_bytes())
        assert adaptation['selected_control_source_manifest']['sha256']==sha((root/'builds/selected/source.json').read_bytes())
        assert adaptation['selected_control_pair_report']['sha256']==sha((root/'builds/selected/pair-report.json').read_bytes())
        field,key=('normal_libraries','liboriole_expat.so') if mode=='normal' else ('pgo_libraries','use/liboriole_expat.so')
        for label,engine in [('candidate','candidate'),('selected','control')]:assert pairs[label][field][key]==adaptation['libraries'][engine]['sha256']
        build=load(directory/'consumers/build.json');original=root/'review/elapsed'/(mode+'-review.json')
        preflight=load(root/'review/elapsed'/(mode+'-preflight-review.json'))
        assert preflight['summary']['extension_compiles']==6 and preflight['summary']['preflight_workers']==72
        assert preflight['summary']['build_manifest_sha256']==sha((directory/'consumers/build.json').read_bytes())
        assert preflight['summary']['preflight_sha256']==sha((directory/'screen/preflight.json').read_bytes())
        assert preflight['source_delta']==sorted(delta)
        source_before=build['source_sha256_before'];assert source_before==build['source_sha256_after']
        vectors=[]
        for engine,consumer in build['consumers'].items():
            for module,command in zip(('pyexpat','_elementtree'),consumer['commands'],strict=True):
                source_key=next(p for p in source_before if p.endswith('/Modules/'+module+'.c'))
                assert source_before[source_key]==sha((root/'upstream/cpython/Modules'/(module+'.c')).read_bytes())
                upstream=str(Path(source_key).parent);inc='/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/include/python3.12'
                expected_command=['cc','-shared','-fPIC','-O2',*['-I'+p for p in ('/home/dev-user/code/oss/oriole-serialized-accounting/include',inc,inc+'/internal',upstream+'/expat',upstream)],source_key,consumer['library'],'-Wl,-rpath,'+consumer['directory'],'-o',consumer['directory']+'/'+module+'.cpython-312-x86_64-linux-gnu.so']
                assert command['command']==expected_command and command['returncode']==0
                assert command['log_sha256']==sha((directory/'consumers'/engine/(module+'-build.log')).read_bytes())
                vectors.append(expected_command)
        assert vectors==preflight['summary']['exact_compile_vectors']
        output=root/'replayed'/mode
        command=[sys.executable,'-I','-S',str(package/'replay.py'),'--root',str(root/'study'),'--mode',mode,'--output',str(output)]
        run=subprocess.run(command,capture_output=True,text=True,timeout=300)
        assert run.returncode==0,(run.returncode,run.stdout,run.stderr)
        actual=load(output/(mode+'-review.json'));original=load(original)
        for key in ('counts','aggregates','summary','libraries','seed','pair_order_verified','raw_rows_exactly_reconstructed','source_results_sha256','source_preflight_sha256'):
            assert actual[key]==original[key],(mode,key)
        for key in ('counts','aggregates','libraries'):assert actual[key]==report['modes'][mode][key]
        assert (output/(mode+'-row-aliases.json')).read_bytes()==(root/'review/elapsed'/(mode+'-row-aliases.json')).read_bytes()
        for row in actual['summary']:
            c,x=row['condition'],row['median_ratios'];csv_rows.append([mode,c['name'],str(c['chunk']),c['mode'],str(x['candidate_over_control']),str(x['candidate_over_expat']),str(x['control_over_expat'])])
        results[mode]={'counts':actual['counts'],'aggregates':actual['aggregates']}
    with (package/'conditions.csv').open() as stream:
        rows=list(csv.reader(stream));assert rows[0]==['build','project','chunk_bytes','consumer','candidate_over_control','candidate_over_expat','control_over_expat']
        assert rows[1:]==csv_rows
    # Preserve the two strict failures and compare every rendered outcome to suffix.
    strict_results={}
    for linkage in ('shared','static'):
        directory=root/'strict/candidate'/linkage
        summary=load(directory/'summary.json')
        assert summary['linkage']==linkage and summary['tests_exit_code']==summary['gate_exit_code']==2
        assert summary['text_fragmentation'] is None and summary['consumer_adaptation']=='CPython upstream allocation-failure fix'
        assert summary['probe_exit_code']==summary['origin_exit_code']==0
        origin=load(directory/'origin.log')
        assert origin['status']=='passed'
        assert set(origin['origins'])=={'initial:pyexpat','fresh:pyexpat','initial:_elementtree','fresh:_elementtree'}
        omitted={row['original']:row for row in index['excluded_binaries']}
        for role,identity in origin['origins'].items():
            assert identity['path']==f'/tmp/oriole-serialized-accounting-strict-cpython/{linkage}/'+role.split(':')[1]+'.cpython-312-x86_64-linux-gnu.so'
            assert identity['sha256']==omitted[identity['path']]['sha256']
        assert summary['source_sha256']==sha((root/'upstream/cpython/Modules/pyexpat.c').read_bytes())
        assert summary['compiled_source_sha256']==sha((directory/'pyexpat.c').read_bytes())
        library='use/liboriole_expat.'+('so' if linkage=='shared' else 'a')
        assert summary['library_sha256']==pairs['candidate']['pgo_libraries'][library]
        strict_results[linkage]={'suite_exit':2}
    strict_controller=load(root/'strict/candidate/report.json')
    assert strict_controller['status']=='completed_strict_results' and strict_controller['pins_unchanged']
    for command in strict_controller['commands']:
        assert command['exit']==2 and command['reaped'] and '--consumer-fix' in command['argv'] and '--allow-text-fragmentation' not in command['argv']
        linkage=command['linkage']
        for stream in ('stdout','stderr'):assert command[stream+'_sha256']==sha((root/'strict/candidate'/(linkage+'.'+stream)).read_bytes())
        assert command['summary_sha256']==sha((root/'strict/candidate'/linkage/'summary.json').read_bytes())
    for linkage in ('shared','static'):
        current=(root/'strict/candidate'/linkage/'tests.log').read_text()
        baseline=(root/'strict/selected'/linkage/'tests.log').read_text()
        pattern=r'^(.+?) \.\.\. (ok|FAIL|skipped.*)$'
        rendered=re.findall(pattern,current,re.MULTILINE)
        all_pattern=r'^(.+?) \.\.\. (ok|FAIL|skipped.*|expected failure)$'
        all_rendered=re.findall(all_pattern,current,re.MULTILINE)
        assert len(all_rendered)==809 and all_rendered==re.findall(all_pattern,baseline,re.MULTILINE)
        assert sum(outcome=='expected failure' for _,outcome in all_rendered)==3
        assert len(rendered)==806 and rendered==re.findall(pattern,baseline,re.MULTILINE)
        failures=[name for name,outcome in rendered if outcome=='FAIL']
        assert failures==['test1 (test.test_pyexpat.BufferTextTest.test1)', 'test_handlers (test.test_sax.CDATAHandlerTest.test_handlers)']
        assert sum(outcome.startswith('skipped') for _,outcome in rendered)==14
        assert re.findall(r'^Total tests: (.*)$',current,re.MULTILINE)==['run=802 failures=2 skipped=14']
        strict_results[linkage].update({'reported_tests':802,'failures':failures,'skips':14,'rendered_verbose_lines':806,'all_rendered_lines':809,'expected_failure_lines':3,'rendered_outcomes_equal_suffix':True})
out={'status':'passed_portable_records_replay','archive_files':len(expected),'archive_sha256':report['evidence_sha256'],'source_files':{'candidate':72,'selected':72},'modes':results,'strict':strict_results,'scope':'Saved source/build/consumer records and original raw worker arithmetic; no parser imports or original absolute-path reads. Actual compiler/library byte checks belong to the archived original reviews.'}
args.output.write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({'status':out['status'],'archive_files':len(expected),'workers':sum(v['counts']['all_workers'] for v in results.values()),'samples':sum(v['counts']['all_samples'] for v in results.values())},indent=2))
