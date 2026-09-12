"""Read the 18 completed callback workers; never load a parser library."""
from pathlib import Path
import argparse, hashlib, json, os
D = Path(__file__).parent
H = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
J = lambda p: json.loads(Path(p).read_text())
assert __debug__ and os.sched_getaffinity(0) == {6}
args = argparse.ArgumentParser(); args.add_argument('--report-sha256',required=True); args=args.parse_args()
bound=J(D/'bound.json'); assert all(H(p)==h for p,h in bound['pins'].items())
R=D/'callbacks'; assert H(R/'report.json')==args.report_sha256
report=J(R/'report.json'); old=J('/tmp/oriole-unknown-encoding-callback-diagnostic/report.json')
assert report['status']=='completed' and report['all_children_reaped']
assert len(report['rows'])==len(report['processes'])==len(old['rows'])==18
for path,digest in report['worker_pins_sha256'].items(): assert H(path)==digest
key=lambda r:(r['engine'],r['context'],r['case'])
expected=[(engine,context,case) for engine in ['oriole','expat'] for context in ['root','general_child','subset_child'] for case in ['malformed_order','valid_order_unknown','uppercase_pi']]
assert [key(row) for row in report['rows']]==expected==[key(row) for row in old['rows']]
assert report['controller_command']==[report['processes'][0]['command'][3],'-I',str(R/'run.py')]
differences=[]
for row, prior, process in zip(report['rows'],old['rows'],report['processes'],strict=True):
    assert process['returncode']==0 and process['reaped'] and not process['timed_out'] and process['timeout_seconds']==10
    assert process['case']=='-'.join(key(row))
    assert process['command']==['/usr/bin/taskset','-c','3','/home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3.12','-I',str(R/'worker.py'),*key(row)]
    for stream in ['stdout','stderr']: assert H(process[stream]['path'])==process[stream]['sha256']
    assert J(process['stdout']['path'])==row and not Path(process['stderr']['path']).read_bytes()
    assert row['input_hex']==prior['input_hex'] and row['input_sha256']==prior['input_sha256']
    assert row['status']==0 and row['parse_calls']==1 and row['final_input'] and row['cpu_affinity']==[3]
    assert row['freed_in_order']==(['parent'] if row['context']=='root' else ['child','parent'])
    assert row['loaded_xml_parse_object']==row['library']
    assert H(row['library'])==row['library_sha256']
    if row['engine']=='oriole':
        assert row['library_sha256']==bound['source']['candidate_libraries']['liboriole_expat.so']
        if row['case']=='malformed_order':
            assert row['callbacks']==[]
            assert row['error']==(30 if row['context']=='root' else 31)
        elif row['case']=='valid_order_unknown':
            assert row['callbacks']==prior['callbacks']==[{'name':'UTF8','name_hex':'55544638','returned':0}]
            assert row['error']==18
        else:
            assert row['callbacks']==[] and row['error']==4
    else: assert row==prior, 'Pinned normal Expat first outcomes changed'
    changed={field:{'selected':prior[field],'candidate':row[field]} for field in prior if prior[field]!=row[field]}
    if changed: differences.append({'key':key(row),'fields':changed})
# Compare all semantic fields to the qualified isolated declaration outcomes.
qualified=J('/tmp/oriole-declaration-bootstrap-grammar-diagnostics/callbacks/report.json')['rows']
identity={'library','library_sha256','loaded_xml_parse_object'}
assert len(qualified)==len(report['rows'])==18
for current, prior in zip(report['rows'],qualified,strict=True):
    assert {k:v for k,v in current.items() if k not in identity}=={k:v for k,v in prior.items() if k not in identity}, 'New callback outcome versus qualified declaration source'
out={'status':'passed_saved_callback_order_diagnostic','targets_executed':False,'bound_sha256':H(D/'bound.json'),'report_sha256':args.report_sha256,'rows':18,'all_children_reaped':True,'selected_differences':differences,'rows_preserved_in':str(R/'report.json'),'limitations':['Positions and error strings are retained, not required to equal Expat.','Rejecting unknown handler only; accepted custom maps and split feeds belong to ordinary Rust regressions.','No performance or full compatibility claim.'],'reader_sha256':H(__file__)}
with (D/'callback-readback.json').open('x') as f: json.dump(out,f,indent=2); f.write('\n')
print(json.dumps({'status':out['status'],'sha256':H(D/'callback-readback.json'),'changed_rows':len(differences)}))
