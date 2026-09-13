from pathlib import Path
import hashlib,json,os
assert os.sched_getaffinity(0)=={4}
O=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
checked=json.loads((O/'checked-files.json').read_text());r=json.loads((O/'review.json').read_text());a=json.loads((O/'attempt-01.json').read_text())
assert len(checked)==3299 and all({'bytes':Path(p).stat().st_size,'sha256':sha(p)}==v for p,v in checked.items())
assert sha(O/'review.json')=='8cabf7ec6dd062f01e69fb2b2a26d255eaa94e6a21d096e45eb247add224b0f0'
assert r['passed_checks']==20995 and len(json.loads((O/'checks.json').read_text()))==20995 and r['origin_rows']==len(json.loads((O/'origin-readback.json').read_text()))==3270
assert a['exit']==0 and a['generator_sha256']==r['generator_sha256']==sha(O/'generator.py')
for s in ['stdout','stderr']:assert a[s+'_sha256']==sha(O/f'attempt-01.{s}')
assert (O/'attempt-01.stderr').read_bytes()==b''
receipt={'status':'sealed_native_byte_count_package_workspace_review','review_sha256':sha(O/'review.json'),'generator_sha256':sha(O/'generator.py'),'all3299_inspected_files_unchanged':True,'audit_attempts':1,'first_attempt_exit':0,'target_executions':0,'final_outer_index_certified':False,'scope':'Main and nested tar members/origins/exclusions/manifests/source/workspace raw counts. Later outer attachments and files.json require a separate readback.'}
(O/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
files={p.name:{'bytes':p.stat().st_size,'sha256':sha(p)} for p in sorted(O.iterdir()) if p.is_file() and p.name!='files.json'}
(O/'files.json').write_text(json.dumps(files,indent=2)+'\n')
for n,row in files.items():assert row=={'bytes':(O/n).stat().st_size,'sha256':sha(O/n)}
assert all({'bytes':Path(p).stat().st_size,'sha256':sha(p)}==v for p,v in checked.items())
print(json.dumps({'review_sha256':sha(O/'review.json'),'receipt_sha256':sha(O/'receipt.json'),'files_sha256':sha(O/'files.json'),'indexed_output_files':len(files)},indent=2))
