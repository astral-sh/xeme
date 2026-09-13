"""Seal the completed local bundle and add two standalone audit sources."""
from pathlib import Path
import hashlib
import json

root=Path(__file__).resolve().parent
bundle=root/'bundle'
def h(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
readers={
    '/tmp/oriole-real-pgo-corpus-independent-audit.py':'audit-real-corpus.py',
    '/tmp/oriole-deferred-c-text-raw-build-independent-audit.py':'audit-text-build.py',
}
for src,name in readers.items():(bundle/name).write_bytes(Path(src).read_bytes())
(bundle/'seal.py').write_bytes(Path(__file__).read_bytes())
(bundle/'supplemental-readers.json').write_text(json.dumps({name:{'source':src,'sha256':h(src)} for src,name in readers.items()},indent=2)+'\n')
readback=json.loads((root/'readback-attempt01.stdout').read_bytes())
assert readback['status']=='passed_portable_saved_data_readback'
assert not (root/'readback-attempt01.stderr').read_bytes()
readback.update(verifier_sha256=h(bundle/'verify.py'),first_attempt_stderr_sha256=h(root/'readback-attempt01.stderr'),first_attempt_exit_code=0)
(bundle/'readback.json').write_text(json.dumps(readback,indent=2)+'\n')
files={p.name:{'bytes':p.stat().st_size,'sha256':h(p)} for p in sorted(bundle.iterdir()) if p.is_file() and p.name!='package-files.json'}
(bundle/'package-files.json').write_text(json.dumps({'status':'sealed_local_package','files':files,'scope':'Original evidence unchanged; portable verifier never executes targets. supplemental-readers.json binds two additional standalone source/build readers outside the archive.'},indent=2)+'\n')
receipt={
    'status':'sealed_local_package_no_repository_edits','bundle':str(bundle),
    'archive':files['evidence.tar.gz'],'inventory':files['inventory.json'],'readback':files['readback.json'],
    'package_files_sha256':h(bundle/'package-files.json'),
    'total_package_bytes':sum(p.stat().st_size for p in bundle.iterdir() if p.is_file()),
    'review_points':[
        'All three experiments unselected; B modest and DocBook adverse, no recipe adoption or full compatibility claim.',
        '136 condition rows/54 primary adverse/3904 workers/530640 samples; aliases omit only verified byte-exact native duplicates.',
        'Selected-source70 and Text candidate70 remain distinct; all source attempts and failed checks retained.',
        'No compiled library/extension/tool/object payload. Full assembly/DWARF and unused prospective artwork bodies excluded with identities.',
        'Original absolute paths are provenance; portable verification uses logical archive names only.',
        'Corpus boundary excludes8 unused Inkscape base64 blob responses and2 unused raw artworks; selected files/licenses/metadata retained. Corpus author agreed the content-identity boundary; no new author package scan.'],
    'sessions_all_reaped':True}
(root/'package-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({'receipt_sha256':h(root/'package-receipt.json'),**receipt},indent=2))
