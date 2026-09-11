"""Independent single-test portability source review; no target executions."""
from pathlib import Path
import hashlib,json,subprocess,tarfile
S=Path('/tmp/oriole-pr115-windows-portability-study');sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
meta=json.loads((S/'source.json').read_text());W=Path(meta['worktree']);new=meta['source_sha256']
old=json.loads(Path('/tmp/oriole-end-frame-cells-test-fix-study/source.json').read_text())['source_sha256']
assert len(old)==len(new)==70 and set(old)==set(new)
changed=[n for n in new if new[n]!=old[n]];assert changed==['crates/oriole_expat/src/tests.rs']
for n,h in new.items():assert sha(W/n)==h
with tarfile.open(S/'source.tar.gz','r:gz') as t:
    mm=t.getmembers();assert len(mm)==70
    for m in mm:assert m.isfile() and hashlib.sha256(t.extractfile(m).read()).hexdigest()==new[m.name]
name=changed[0];before=subprocess.check_output(['git','-C',str(W),'show','1ff7b64:'+name]).decode();after=(W/name).read_text()
old_cast='opening.to_bytes().len() as i64';new_cast='opening.to_bytes().len() as c_long'
assert before.count(old_cast)==1 and after==before.replace(old_cast,new_cast)
lib=(W/'crates/oriole_expat/src/lib.rs').read_text()
assert 'pub unsafe extern "C" fn XML_GetCurrentByteIndex(parser: XML_Parser) -> c_long' in lib
assert 'use std::ffi::{CStr, c_char, c_int, c_long, c_ulong, c_void};' in lib
assert 'use super::*;' in after and '#[cfg(test)]\nmod tests;' in lib
assert 'let opening = c"<root><name a=\'v\'>";' in before
assert len(b"<root><name a='v'>")==18
patch=(S/'candidate.patch').read_bytes();assert hashlib.sha256(patch).hexdigest()=='8f240abe4d160b205fa88c7994abcfbef76022e9b4f5fedce417571e653c86ed'
failure=(S/'windows-failure.log').read_text();assert 'E0308' in failure and 'i32' in failure and 'i64' in failure
report={'status':'source_review_passed','scope':'Independent read-only source/lineage review of one cfg(test) expected-value cast; no build or test execution and no final CI claim.',
 'source_manifest_sha256':sha(S/'source.json'),'source_files':70,'changed_files':changed,'other_files_unchanged':69,
 'patch_sha256':sha(S/'candidate.patch'),'exact_change':'opening.to_bytes().len() as i64 -> as c_long',
 'reason':'XML_GetCurrentByteIndex returns the existing imported target C long type. The expectation must use that same type, including Windows where the saved diagnostic reports i32; the fixed literal byte index18 fits either target width.',
 'assertions_inputs_numeric_expectation_unchanged':True,'runtime_ABI_and_implementation_unchanged':True,
 'prior_407_and_measured_sources_preserved':'344730 and e7eb evidence stays historical. This source supplement does not rewrite those records.',
 'build_and_CI_scope':'Fresh C-only identity/focused checks and rerun CI are separate producer evidence, pending at this source review.',
 'blocking_findings':[],'generator_sha256':sha(__file__)}
out=Path('/tmp/oriole-pr115-windows-cast-independent-review.json');out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'sha256':sha(out),'status':report['status']}))
