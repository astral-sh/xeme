from pathlib import Path
import gzip,hashlib,json
R=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert not (R/'verify-attempt01.py').exists()
(R/'verify-attempt01.py').write_bytes((R/'verify.py').read_bytes())
(R/'members-attempt01.json.gz').write_bytes((R/'members.json.gz').read_bytes())
audit=Path('/tmp/oriole-x86-64-v3-native-independent-review/audit.py');posthoc=audit.with_name('posthoc.py')
pinned={'audit.py':sha(audit),'posthoc.py':sha(posthoc)}
assert pinned['audit.py']=='dc66c465a3948db1c00aeb5853bde8b302276ae988b16ce8ad3915e5d429e6ea'
s=(R/'verify.py').read_text();old="  original=N+'/'+name;code=data(original).decode();old='from pathlib import Path';assert code.count(old)==1;code=code.replace(old,'Path = archive_path')"
new="  original=N+'/'+name\n  expected_original="+repr(pinned)+"\n  assert digest(data(original))==expected_original[name], 'Original arithmetic auditor identity mismatch'\n  code=data(original).decode();old='from pathlib import Path';assert code.count(old)==1;code=code.replace(old,'Path = archive_path')"
assert s.count(old)==1;(R/'verify.py').write_text(s.replace(old,new))
idx=json.loads(gzip.decompress((R/'members.json.gz').read_bytes()));rows=[x for x in idx['origins'] if x['path']==str(R/'verify.py')];assert len(rows)==1
row=rows[0];assert row['sha256']==sha(R/'verify-attempt01.py')
row['path']=row['origin']=str(R/'verify-attempt01.py');row['reason']='Retained initial verifier source; final top-level verifier adds immutable original-auditor digest guards'
idx['origins'].sort(key=lambda x:x['path']);idx['verifier_amendment']='Only the initial-verifier source origin is relabeled to its retained snapshot; archive payload bytes unchanged. Final top-level verify.py adds exact original-auditor digest guards and is bound by files.json.'
(R/'members.json.gz').write_bytes(gzip.compress((json.dumps(idx,indent=2)+'\n').encode(),compresslevel=9,mtime=0))
report={'scope':'Portable-verifier hardening only; no producer data or archive payload change.','initial_verifier_sha256':sha(R/'verify-attempt01.py'),'final_verifier_sha256':sha(R/'verify.py'),'initial_members_sha256':sha(R/'members-attempt01.json.gz'),'final_members_sha256':sha(R/'members.json.gz'),'archive_sha256':sha(R/'evidence.tar.gz'),'pinned_original_auditors':pinned,'relabeled_origin':str(R/'verify-attempt01.py'),'archive_payload_changed':False}
(R/'verifier-amendment.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
