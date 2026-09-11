"""Publish the exact current-source PBS handoff with a short integration report."""
from pathlib import Path
import hashlib
import json
import shutil
import tarfile

worktree = Path('/home/dev-user/code/oss/oriole-current-pbs')
source = Path('/tmp/oriole-be22-pbs-followup-handoff')
destination = worktree / 'docs/validation/2026-09-11/current-pbs'
destination.mkdir()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(source / 'report.json') == '78196e28e8df72e8e4331092adec1edd73fb297288ed51a77dcab861aac26261'
members = {str(p.relative_to(source)): {'sha256': sha(p), 'size': p.stat().st_size} for p in sorted(source.rglob('*')) if p.is_file()}
with tarfile.open(destination / 'producer-handoff.tar.gz', 'w:gz') as archive:
    for name in members:
        archive.add(source / name, arcname=name, recursive=False)
with tarfile.open(destination / 'producer-handoff.tar.gz') as archive:
    actual = {p.name: {'sha256': hashlib.sha256(archive.extractfile(p).read()).hexdigest(), 'size': p.size} for p in archive if p.isfile()}
assert members == actual
assert all(sha(source / name) == item['sha256'] for name, item in members.items())
(destination / 'archive-members.json').write_text(json.dumps(members, indent=2) + '\n')
shutil.copyfile(source / 'report.json', destination / 'report.json')
(destination / 'README.md').write_text('''# Current parser PBS integration

The [full PBS workflow](https://github.com/astral-sh/oriole/actions/runs/34563056904) built CPython 3.12.13 with the accepted `be22a27` runtime from commit `c1776c9`. The distribution loads on glibc 2.17, including both built-in XML accelerators. **The workflow and XML suites remain failed on two character-data callback-grouping assertions.**

## Results

| Check | Result |
| --- | --- |
| Full distribution build and archive validator | Passed |
| Two custom-suite invocations in CI | Each: 17 successes, five skips |
| Installed identity and fresh accelerator import, local | Passed; Oriole 2.8.4; both XML accelerators built in |
| Threaded loading on glibc 2.17, local | 1,024 parses passed |
| Host and glibc 2.17 verbose XML suites, local | Each: 802 tests, two failures, 12 skipped outcomes, three expected failures |
| Original glibc 2.17 script, local | Threaded check passed; XML failed: 802 tests, two failures, 13 skips |

All 802 method outcomes, fixture outcomes and subtest outcomes match between the host and container verbose runs. Both fail `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`, with the same grouping assertions found in this workflow's raw log. CI ran 802 initial tests and four automatically selected reruns, producing four failure occurrences across those two methods.

Collection finds 803 methods; the no-accelerator class skips with accelerators present. The verbose runs' 12 skipped outcomes comprise three methods, eight subtests and one class. The original `-m test` script adds a CPU-resource skip; the corresponding Minidom test passes under both verbose `unittest` runs. No test or assertion was changed.

## Build and scope

The 69 bundle source hashes and 97 launch source hashes match the Git revision; their 101-file union is retained. Three actual compiler invocations record stable Rust 1.98.1, C-only ThinLTO, one codegen unit, PIC and unwind. These sysroot-built binaries have no PGO or sanitizer instrumentation and are distinct from the local Ohm benchmark binaries. PBS retains the upstream bounded-allocation cleanup backport in `pyexpat.c`; its XML tests are unchanged.

The distribution archive is SHA-256 `d7033afb5a01b88737c2e57500269f0078d9237af01455506e5343f46cba539e`. Its static Expat library matches the Oriole bundle. All 6,546 archive members were inspected and all 5,497 original regular members rehashed after local checks. Source, test and native binary bytes remained exact; interpreter startup rewrote three existing encoding bytecode caches. The downloaded archive is unchanged.

CI skipped installed identity and glibc validation after XML validation failed. The five local diagnostics supplement those skipped steps. They do not turn the workflow or original glibc script green. The pinned CentOS image was already present; both containers disabled networking, mounted the distribution and checks read-only, and were cleaned up by their owned IDs. A Docker socket permission denial occurred before container execution; its receipt is preserved, and authorized access resumed only the two unexecuted container diagnostics. There were no target retries or timeouts.

## Evidence

The [report](report.json) and [complete producer handoff](producer-handoff.tar.gz) retain raw CI and local logs, reviewed command arrays, source snapshots, compiler vectors, archive/source/binary hashes, every method outcome, cleanup receipts and the failed Docker prerequisite. Every handoff file is archived unchanged; its pre-execution plan retains its historical held status. Compiled distribution files are excluded from the compact package with exact hashes, and the complete original downloads remain in the recorded local study directory.

The [archive member index](archive-members.json) records the producer wrapper's exact members. The [independent review](independent-review.json) and [publication index](files.json) provide the final review and file identities. This report supplies current-source integration evidence; it makes no speed, exhaustive compatibility or production-readiness claim.
''')
readme = worktree / 'README.md'
old = 'The [PBS distribution report](docs/validation/2026-09-10/version-consistent-pbs/) applies to the earlier `4b11ace` runtime.'
new = 'The [current PBS distribution](docs/validation/2026-09-11/current-pbs/) loads both XML accelerators on glibc 2.17 and passes 1,024 threaded parses; its XML suite retains the same two grouping failures.'
text = readme.read_text()
assert text.count(old) == 1
readme.write_text(text.replace(old, new))
review = worktree / 'docs/review.md'
review.write_text(review.read_text() + '\nThe [current PBS follow-up](validation/2026-09-11/current-pbs/) builds the accepted `be22a27` runtime with stable Rust and verifies installed identity, fresh XML accelerators and 1,024 threaded parses on glibc 2.17. All 802 host/container XML method outcomes agree, including the two known grouping failures. The full workflow and XML gates remain failed; skipped CI steps and separate local diagnostics retain their actual outcomes.\n')
shutil.copyfile(__file__, destination / 'package-publication.py')
print(json.dumps({'members': len(members), 'archive_sha256': sha(destination / 'producer-handoff.tar.gz')}))
