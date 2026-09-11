#!/usr/bin/env python3
"""Finalize the publication caption after successful saved-only readback."""
import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def load(path):
    return json.loads(path.read_text())


verified = load(root / 'verification-attempt03.stdout')
assert verified['status'] == 'passed_portable_archive_and_original_numerical_replay'
assert (root / 'verification-attempt03.stderr').read_bytes() == b''
index = json.loads(gzip.decompress((root / 'members.json.gz').read_bytes()))
assert verified['archive_sha256'] == index['archive_sha256'] == digest(root / 'evidence.tar.gz')
report = load(root / 'staging/report.json')
assert report['native']['groups'] == verified['groups']
report['status'] = 'complete_saved_evidence_package_experimental_unselected'
report['publication'] = {
    'archive': {'file': 'evidence.tar.gz', 'sha256': index['archive_sha256'],
                'bytes': index['archive_bytes'], 'stored_members': verified['stored_members']},
    'member_index': {'file': 'members.json.gz', 'sha256': digest(root / 'members.json.gz'),
                     'origins': verified['origins'], 'aliases': verified['aliases']},
    'portable_replay': {'file': 'verification-attempt03.stdout',
                        'sha256': digest(root / 'verification-attempt03.stdout'),
                        'verifier': 'verify.py', 'verifier_sha256': digest(root / 'verify.py'),
                        'workers': 896, 'samples': 141568,
                        'reconstructed_details_and_review_byte_equal': True},
    'attempts': [
        {'phase': 'assembly', 'session': 19111, 'exit': 0},
        {'phase': 'portable-replay-attempt01', 'session': 55330, 'exit': 1,
         'reason': 'Missing inherited run.py alias, whose existing bytes were already in archive'},
        {'phase': 'alias-index-correction', 'session': 73818, 'exit': 0},
        {'phase': 'portable-replay-attempt02', 'session': 27902, 'exit': 0},
        {'phase': 'historical-verifier-origin-caption-correction', 'exit': 0},
        {'phase': 'portable-replay-attempt03-final-index', 'session': 5799, 'exit': 0}],
    'archive_unchanged_after_first_assembly': True,
    'corrections': ['index-correction.json', 'verifier-correction.json', 'origin-correction.json'],
    'independent_adapter_source_review': {
        'file': 'independent-review/oriole-allocator-bolt-portable-replay-source-review.json',
        'sha256': '0a5b73c70ffa0d662a76d2ca1c6fd9ad1a333cbfebe039a853b80f25a02ff96b',
        'scope': 'Source/AST review, not independent archive execution; its original index input is preserved under attempts'},
    'binary_payloads': 'Excluded. Recorded hashes and sizes remain; portable replay explicitly reports five binary identities it cannot rehash.',
    'profiling_payloads': 'Included: exact Rust raw/merged profile, GCC raw profiles, BOLT fdata and all generated replay records.',
    'origin_scope': 'All original stored source files were read back at assembly; the corrected wrapper alias was independently read and hash-checked. Frozen source capsule members were verified against all 70 source hashes.',
    'no_new_target_execution': True}
(root / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
readme = (root / 'staging/README.md').read_text()
prefix = readme.split('## Publication preparation\n', 1)[0]
tail = '''## Evidence package and replay

The [archive](evidence.tar.gz) is 4,432,145 bytes with 1,205 stored payloads.
The [member index](members.json.gz) maps all 1,274 origins, including 69 explicit
same-byte aliases. It retains source, compiler/profile provenance, original
G inputs and replay, fdata, smoke/ELF metadata, native raw records, independent
reviews, and the required Oriole, LLVM, Expat, and held-out corpus notices.
There are no target caches, ELF/static-library bytes, or nested source archives.

Run this saved-data check from any location with Python 3.11 or newer:

```sh
python3 /path/to/package/verify.py /path/to/package
```

The [final readback](verification-attempt03.stdout) validates every stored and
alias hash, the 70-file source/67 Cargo inputs/six helper bindings, and replays
the original native auditor through archive-backed paths. It reproduces the
original detailed and compact numerical audit bytes exactly: 896 workers,
141,568 samples, all orders, callbacks, medians, and five ratio maps. It performs
no target execution and cannot revalidate omitted library/tool bytes. Its
output explicitly lists the five recorded-only binary hashes used by replay.

The [independent adapter review](independent-review/oriole-allocator-bolt-portable-replay-source-review.json)
checks source and AST preservation, not archive execution. The archived auditor
remains unchanged; the adapter changes only path I/O, recorded hash handling for
omitted binaries, and CPU affinity, with a fixed auditor SHA256 guard.

### Retained publication corrections

Assembly passed first attempt. The first portable replay failed because an
inherited `run.py` byte-comparison dependency was missing from the original
auditor's hash map. Its identical bytes were already stored. A
[one-alias index correction](index-correction.json) fixed that omission; the
archive never changed. Both corrected replays passed. The original index and
first failure logs remain, as does a separate review-helper AST-count mistake.

The final verifier adds a [one-line source identity guard](verifier-correction.json).
Its archived first version is retained, and an
[origin caption correction](origin-correction.json) points that historical
version to its exact saved copy. This changes no archived payload, path key,
hash, or numerical assertion. The independent source review's earlier index is
also retained under `attempts/`.

[report.json](report.json) records scope, all adverse conditions, command
outcomes, and final hashes. [files.json](files.json) hashes the complete published
directory except itself. `staging/` preserves the original pre-assembly report;
the archive contains that historical snapshot, while this top-level report
records completion. `assemble.py`, `repair-index.py`, `retarget-origin.py`, and
`finalize.py` retain the saved-only packaging steps. Reconstructing the first
archive uses its staged report and original archived verifier version; no new
measurement is implied by packaging reproduction.

The report author independently audited the root-collected native samples and
previously authored the allocator correctness fix. The author did not collect
or transform the BOLT measurements. This package is experimental and unselected;
the current ContextText candidate was not measured with BOLT, and no default
build recipe changes follow from this report.
'''
(root / 'README.md').write_text(prefix + tail)
files = {}
for path in sorted(root.rglob('*')):
    if path.is_file() and path.name != 'files.json':
        files[str(path.relative_to(root))] = {'sha256': digest(path), 'bytes': path.stat().st_size}
(root / 'files.json').write_text(json.dumps({'scope': 'All published files except this index itself; raw archive member/origin index is separate.',
                                           'files': files, 'count': len(files)}, indent=2) + '\n')
print(json.dumps({'status': report['status'], 'archive': report['publication']['archive'],
                  'members': report['publication']['member_index'], 'published_files': len(files) + 1,
                  'report_sha256': digest(root / 'report.json'),
                  'files_sha256': digest(root / 'files.json')}, indent=2))
