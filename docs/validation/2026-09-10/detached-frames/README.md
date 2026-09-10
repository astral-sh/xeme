# Detached start-tag and character-data storage

The three parser layers share complete/incremental tag grammar, deliver eligible
start tags from detached storage, and extend that storage to plain text. The
combined runtime is `5bc806e5fc2a545f22c75f2a1bcbef45cfa3632b`. All **392 workspace
tests**, formatting, and strict Clippy pass.

The normal release improves every measured real-project condition against the
earlier `4b11ace` runtime: **18.3% less native elapsed time** and **8.2% less time
through actual CPython consumers**. It still takes **2.13× Expat's time natively**
and **1.48× through CPython**. Only the two Wayland pyexpat conditions are faster
than Expat. See the [complete benchmark report](../../../../benchmarks/results/2026-09-10/detached-frames/).

## Compatibility and safety checks

| Check | Result |
| --- | --- |
| Final workspace, all targets | 392 pass in 33 executables |
| Formatting and strict Clippy | Pass |
| Original upstream public API matrix | 4,347 pass / 393 fail; all 4,740 outcomes byte-identical to `db5a867` |
| Strict callback/status/error/position comparison | 3,318 cases, no differences from `db5a867` |
| Malformed character data, UTF-8/UTF-16 and selected chunk boundaries | 2,392 cases, no differences from `db5a867` |
| Callback coordinates, raw context, DefaultCurrent, replacement handlers and suspend/resume | 48 conditions; 16 candidate traces exactly match `db5a867` |
| Unchanged CPython 3.12.13 XML tests, shared and static | Each runs 802 tests; two failures, 14 skips and three expected failures |
| Separate CPython text/CDATA semantic checks | Both checks pass for each linkage |
| Native shared-library C consumers | Integration, adversarial lifecycle and 327 allocation-failure scenarios pass |
| C consumers with ASan, UBSan and LSan | All three pass against the normal Rust release library |

The two CPython failures remain `BufferTextTest.test1` and
`CDATAHandlerTest.test_handlers`. They assert Expat's text callback grouping.
The separate semantic checks preserve every text character, ordered element and
CDATA boundaries, and callback-controlled buffering. Neither original suite was
modified or converted to a passing result. The corrected import finder verifies
both initial and fresh extension imports.

The coordinate probe retains 11 discrepancies per engine against its idealized
raw-prefix coordinate calculation. The candidate and published parser agree in
all 16 corresponding full traces. These measurements do not claim exact Expat
callback or position equivalence.

The upstream matrix retains its original three-second per-configuration timeout,
1 GiB address-space bound and 768 MiB RSS bound. Its existing exclusions concern
Expat implementation-private tests; no public assertion was changed. Of the
remaining 393 failures, 298 exceed allocation retry ceilings, 44 require a
particular reallocation, 12 expect allocation during an empty ParseBuffer,
12 reach child allocation failure earlier, 12 require Expat's literal version
identity, 14 concern large-input/buffer policy or harness bounds, and one checks
input-buffer allocation cost. They remain failures, including tests whose
allocation schedule is not a portable parser contract. The per-test mapping and
classification script are retained in [api-classification](api-classification/).

The preceding grammar layer separately passed 377 workspace tests, 1,518 strict
cases and 186,348 cold/warm per-feed tag cases. The start-frame layer passed 386
workspace tests, the same two oracles, and shared/static C ASan/UBSan consumers.
Both preserve all original API outcomes. Their source freezes and original
results remain separate from the final combined measurements.

## Source and build identity

The measured and behavior-tested library is `9277b71c85f3c0345697010eb282a3be18a0144f25bc2f78e2a0cee20c175dcd`.
The final stack library is `5af2406b17092dad35aa61c086cdfe39e24f51e653441fd9baf50cd12da3b2fe`.
The static CPython suite used archive `773d48c79dd4b9d0f47327e103ae0daf706d4d23fa36239fe046f69798756e79`;
the assembled archive is `303988d9a1f6abe34e5aec54a682b6140a69d2a170f0b312e324a88e7208b8f8`.
The static archives have verified build provenance; the section-by-section
equivalence comparison below covers the shared libraries.
Its source retains one additional Rustdoc line and a `cfg(test)` fallback
regression from the first stack layer. All other 67 frozen source entries agree.
Independent comparison finds identical executable code and every other ELF
section except the GNU build ID and 18 panic-location line numbers in
`encoding.rs`. Those line numbers each increase by one; filenames, pointers and
columns are unchanged. The evidence preserves both libraries' hashes and source
archives instead of presenting them as the same binary.

Normal builds used Ohm with its experimental defaults disabled, explicit empty
compiler-wrapper/Rust-flag overrides, distinct worktree targets, and private
intermediates under a stable `{workspace-path-hash}` build-root template. Source
mtimes were advanced without changing bytes, and the logs require actual rustc
invocations for all three workspace library crates. Baseline and candidate use
matching compiler settings. No PGO, alternate library allocator or LTO override
was introduced.

Earlier ordinary-Cargo invocations had reused stale outputs through a shared
intermediate directory across worktrees. Those comparisons were discarded as
source-effect evidence. The four fresh private builds and their independent
provenance review establish the controls used here. A separate read-only-intended
objcopy command had rewritten metadata in two old diagnostic binaries; both were
restored byte-for-byte from independently verified copies. Neither incident
changed the final source, measured libraries or recorded samples in this report.

## Retained failures and scope

Two new author tests initially assumed a discarded newline-policy hook; their
original failures remain recorded. The hook was removed, and the replacement
tests exercise the existing coalesced policy and callback lifetimes. A subsequent
unused-mut lint failure was corrected without changing assertions or runtime
behavior. The first final-build controller assumed a nonexistent toolchain file
and failed before compilation; the corrected build is recorded separately.

LeakSanitizer initially failed inside the ptrace sandbox. The three sanitizer
consumers were then run outside that sandbox with leak detection enabled and
passed. Their original error and the complete retry are retained. These checks
instrument the C consumers; they are not an ASan/UBSan instrumentation claim for
the Rust release library. The earlier 14.50-million-execution Rust ASan campaigns
and full PBS distribution trial apply to `4b11ace`, not this new runtime.

The unique-owner accounting experiment was not selected: its matched native
screen took 1.02% longer across the real-project conditions and 1.24% longer on
generated inputs. Its complete separate handoff is retained. The original API
ceilings are unchanged; the reports retain all failures and benchmark conditions.

## Evidence

`evidence.tar.gz` retains source snapshots, build commands and logs, API and
CPython outcomes, supplemental probes, raw timing samples, benchmark controllers
and independent reviews. The Python results retain a stale prose `method` label
from the prior prototype; their recorded loaded-library hashes, source manifests
and independent audit identify the final `9277b71c` measurements. `archive-members.json` records every included member's
size and SHA-256. Compiled ELF files and static archives are excluded; their
recorded hashes and build provenance remain available. `files.json` indexes the
outer report files. The separate atomic experiment has its own archive and
readback record.
