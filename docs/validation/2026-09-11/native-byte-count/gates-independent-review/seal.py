from pathlib import Path
import hashlib,json,os
assert os.sched_getaffinity(0)=={4}
O=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
checked=json.loads((O/'checked-files.json').read_text());r=json.loads((O/'review.json').read_text());a=json.loads((O/'attempt-01.json').read_text())
assert len(checked)==375 and all(sha(p)==h for p,h in checked.items())
assert r['passed_checks']==15849 and r['checked_file_count']==375 and len(json.loads((O/'checks.json').read_text()))==15849
assert sha(O/'review.json')=='7d88712eb34eec26f9612bffe611aabdb5079ae5e5e269db5e5ada2e1145f619'
assert a['exit']==0 and a['generator_sha256']==r['generator_sha256']==sha(O/'generator.py')
for s in ['stdout','stderr']:assert a[s+'_sha256']==sha(O/f'attempt-01.{s}')
assert (O/'attempt-01.stderr').read_bytes()==b''
receipt={'status':'sealed_native_byte_count_saved_C_gate_review','review_sha256':sha(O/'review.json'),'generator_sha256':sha(O/'generator.py'),'all375_inspected_files_unchanged':True,'audit_attempts':1,'first_attempt_exit':0,'target_executions':0,'scope':'Saved raw C-gate/API/trace/publication/malformed/End evidence and inherited-controller/source hashes. Original393 API failures, reference differences, missing alias raw pairs and initial harness/postprocessing failures retained.'}
(O/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
files={p.name:{'bytes':p.stat().st_size,'sha256':sha(p)} for p in sorted(O.iterdir()) if p.is_file() and p.name!='files.json'}
(O/'files.json').write_text(json.dumps(files,indent=2)+'\n')
for n,row in files.items():assert row=={'bytes':(O/n).stat().st_size,'sha256':sha(O/n)}
assert all(sha(p)==h for p,h in checked.items())
print(json.dumps({'review_sha256':sha(O/'review.json'),'receipt_sha256':sha(O/'receipt.json'),'files_sha256':sha(O/'files.json'),'indexed_output_files':len(files)},indent=2))
