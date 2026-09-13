from pathlib import Path
import hashlib,json,datetime
p=Path(__file__).parent
sha=lambda b:hashlib.sha256(b).hexdigest()
readme='''# Independent publication-text review

Passed: source/build scope, README invariants, links, all six displayed native rows, native/Python aggregate arithmetic, retained regression and compatibility limitations. This is an independent agent review, not an external human audit. No targets or compilers were run.

The trust-environment sentence now distinguishes the measured prototype from its control and the integrated rebuild from that prototype. The root-proposed final package/publication-review links are approved subject to final attachment/index readback.

`review.json`, `checked-files.json`, source adaptation and saved attempts retain the exact assessed scope. The first reviewer-only codegen hash lookup treated stored relative filenames as absolute; the second attempt resolves them relative to the codegen study and passes. Producer evidence is unchanged. Final attachment/index readback is separate.
'''
(p/'README.md').write_text(readme)
receipt={'status':'sealed','date_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'review_sha256':sha((p/'review.json').read_bytes()),'inputs_unchanged':True,'reviewer_only_correction':{'initial_exit':1,'cause':'Codegen evidence_sha256 keys are relative to codegen-study, not absolute paths.','correction':'Resolve within exact original codegen-study; all assertions retained.','final_exit':0},'final_pointer_approved_subject_to_readback':'[Independent package review](package-independent-review/README.md) and [publication-text review](prose-independent-review/review.json) record the final review scope.'}
for name,h in json.loads((p/'checked-files.json').read_text()).items():assert sha(Path(name).read_bytes())==h,name
(p/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
files={str(f.relative_to(p)):{'bytes':f.stat().st_size,'sha256':sha(f.read_bytes())} for f in sorted(p.rglob('*')) if f.is_file() and f.name!='files.json'}
(p/'files.json').write_text(json.dumps(files,indent=2)+'\n')
for name,row in files.items():assert sha((p/name).read_bytes())==row['sha256']
print(json.dumps({'directory':str(p),'review_sha256':receipt['review_sha256'],'receipt_sha256':sha((p/'receipt.json').read_bytes()),'files_sha256':sha((p/'files.json').read_bytes()),'indexed_files':len(files)}))
