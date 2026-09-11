"""Write the selected End frame report from retained, complete benchmark evidence."""
from pathlib import Path
import json

repo = Path('/home/dev-user/code/oss/oriole-end-frame-cells')
out = repo / 'docs/validation/2026-09-11/detached-end-frames'
bench = repo / 'benchmarks/results/2026-09-11/detached-end-frames'
summary = json.loads((out / 'summary.json').read_text())
assert summary['workspace']['all_target_passed'] == 407
table = (out / 'table.md').read_text().strip()
report = '''# Detached closing-tag callbacks

Matched native closing tags can now transfer the opening element's name directly into an owned callback frame. This avoids constructing, queueing and dispatching an owned `Event` for eligible closing tags. The name remains independently owned while C callbacks execute, and the existing start-tag arena stays available for reuse.

The combined change reduces elapsed time by **4.0% across 24 native project conditions** and **2.1% across 24 actual CPython conditions**, relative to the published Cell-counter runtime. Oriole still takes **1.64× Expat's time natively** and **1.29× through CPython**. Generated controls regress by **2.5%** overall, including **6.2% and 3.3%** for entity input at 4/64 KiB. The broader performance goal remains unmet.

## Eligibility and ownership

The existing scanner must have matched a complete closing name against the already validated opening name in the sole native UTF-8 root source. The frame path also requires no DTD, foreign DTD, shared DTD tables or namespace bindings to restore. Fragment and converted-source parsing retain their existing paths. Empty tags retain their queued End event. Public owned Rust events keep their behavior.

We charge and copy the raw token before removing the opening element, transfer its expanded name or original name, restore token scratch, consume the already accounted bytes, and publish the frame at the existing position. The End name does not replace the reusable start/text byte arena. Taking the name deactivates that End delivery. Clearing or finishing an unconsumed frame drops its owner before the generation is released; oversized names retain the existing recycling limits.

The C adapter snapshots the callback and argument, charges the callback name length before its terminator before checking for a handler, and adds a terminator only for an End callback. The local name survives callback setters, child parsing and stop/resume behavior without a borrowed parser field crossing the callback. Recycling precedes the default-handler fallback when there is no End handler. Existing allocation failures, error codes, raw publication and callback-byte limits remain in the same phases. No new unsafe block or trait implementation is introduced by the runtime change.

The frame grows from 208 to 256 bytes on this x86_64 compiler. Owning a String in its payload also adds drop checks on common Start/Text assignments; these costs are included in the measurements, not treated as free.

## Source and build identity

Runtime `RUNTIME` is stacked on PR #114 (`5f20adf`). It changes nine source/test/example files. The first combined source manifest was `e7eb4e525a8ae2e06889888aa4c855bc41ad830d08fd716296a1ccd0d4b9154f`. C-only builds and all elapsed/C compatibility studies used that source. The first full Rust build then found two test-only atomic accessor calls left over from the standalone prototype: `.store` and `.load` were invalid after the parent PR changed the private counters to `Cell`.

The final source manifest is `344730269a59a8402d3e18e5eed170982d2c44acfe6fc3d869f2acc166f149df`. Its only difference is `.set` and `.get` in that test; the assertions are unchanged. A fresh three-crate C-only compile produces **byte-identical shared and static libraries**. [assembly.json](assembly.json) and the [final build review](final-build-review.json) tie all 70 source inputs and the committed patch to the measured libraries. The failed initial Rust attempt remains archived separately from the successful final run.

Both elapsed Oriole builds use effective C-only ThinLTO, without PGO. Local compilation uses `cargo +ohm -Zohm-defaults=no`, one job, no incremental compilation, empty compiler wrappers and Rust flags, distinct worktree targets and workspace-specific shared intermediates. Three actual workspace compiler invocations are recorded for each fresh release. Their vectors match after private paths are normalized. The local compiler is Ohm Rust 1.98.1-dev (`f62703110`), LLVM 22.1.8; CI uses the repository's normal toolchain.

```console
cargo rustc --release --locked -p oriole_expat --lib --crate-type cdylib,staticlib
```

| Artifact | Published Cell control | End + Cell candidate |
| --- | --- | --- |
| Shared SHA-256 | `0cfd610876f1bfd26a4d37a7f86adc9961896dfe24177c180ec7d15e0bae04e9` | `02fcab59f6e3818c128da6706d67987ae7450dd8b59e97cd28ce497a547f28f5` |
| Static SHA-256 | `c86ad85e232afe4f7947053e522dbc8517cdca3694740994c3d1da64540f0610` | `69ebed414946cbaef1a940c052a636be277c809902943d676e824deab45d22d5` |

The unchanged Expat 2.8.4 reference is commit `12cf0b1f25f026a022fe728ad8f7e3d017285b80`, shared SHA-256 `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`. Normal Rust workspace builds are separate from these C-only artifacts.

## Native project measurements

The display below selects 4 KiB chunks with namespaces disabled from the complete suite.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
TABLE

The fixed suite parses six pinned project XML inputs with namespaces enabled/disabled and 4/64 KiB chunks, plus four generated conditions. Seven seeded shuffled rounds include every condition. Timing includes parser creation, incremental feeding, finalization, callbacks and destruction after one excluded warmup. Input preparation and output checks are outside the timed interval. Per-condition ratios are medians of paired ratios; aggregate ratios are their geometric means. Displayed times are medians of process medians.

| Group | Conditions | Candidate / Cell | Candidate / Expat | Lower than Cell | Lower than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| Project XML | 24 | 0.959761 | 1.644336 | 20 | 0 |
| Generated controls | 4 | 1.024736 | 5.204509 | 1 | 0 |

All four Batik conditions regress against Cell, by 0.41–0.84%. Generated entities regress by 6.24% at 4 KiB and 3.34% at 64 KiB; rare declarations regress by 0.80% at 64 KiB. Every regression remains in [native-summary.json](native-summary.json). The run retains 84 passing preflights, 588 timed workers, 196 complete comparison groups, 105,420 measured parses and 588 warmups. No native condition beats Expat in this run.

## Actual CPython measurements

Unmodified CPython 3.12.13 extension sources (`3bb231a6a5dc02b95658877318bf61501a7209e9`) are compiled against each frozen library. Same-process origin checks verify the loaded `XML_Parse`; canonical outputs agree before timing. ElementTree and pyexpat event collection each parse six inputs at 4/64 KiB, with namespaces enabled. Imports, input preparation and output canonicalization are outside timing. Parser creation, feeding, finalization and explicit result destruction are timed; automatic GC stays enabled and explicit `gc.collect` calls are outside timing.

| Consumer | Conditions | Candidate / Cell | Candidate / Expat | Lower than Cell | Lower than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| ElementTree | 12 | 0.975746 | 1.371776 | 11 | 1 |
| pyexpat events | 12 | 0.982336 | 1.210315 | 11 | 2 |
| Combined | 24 | 0.979036 | 1.288519 | 22 | 3 |

Vulkan pyexpat at 64 KiB regresses by 0.44%; Batik ElementTree at 4 KiB regresses by 0.15%. Three Wayland conditions beat Expat: both pyexpat chunks and ElementTree at 64 KiB. All 72 preflights, 504 timed workers, 168 complete groups, 25,788 measured parses and 504 warmups are retained.

Both elapsed studies run serially on CPU 0 of the shared Linux AMD EPYC-Milan host. Process affinity does not imply exclusive host use. These measurements parse project XML, not complete project builds; Batik's external DTD is not loaded. No condition or sample is discarded.

## Instruction evidence and generated regressions

A separate Callgrind study retains 32 preflights and 32 profiles over 12 project conditions and four generated controls, with both parser samples collected per process. All 12 project instruction counts improve; the geometric ratio is 0.969907. Generated instructions increase by 0.230% overall, with three regressions. Entity input increases by 0.459% at 4 KiB and 0.454% at 64 KiB.

Static disassembly and dynamic self-instruction attribution identify added common-path work: frame clear adds four selected-path instructions, short text preparation adds three, and the selected inline-text C dispatch loop adds six. The successful scoped-frame path removes one instruction. The End String move is skipped for ordinary text. A whole-frame copy grows from 208 to 256 bytes at C event-loop cleanup, not on every text event. These facts explain measured extra instructions; they do **not** establish the full cause of the 6.24% elapsed regression. Cache, branch and scheduling effects were not separately measured.

## Compatibility and adversarial checks

The final normal workspace passes **407 tests in 33 binaries**, one doc test, strict workspace Clippy and formatting. Seven new tests cover detached End ownership, retained arena capacity, abandoned/oversized names, callback-budget order, stop/resume, selected allocator failures and callback reentry. The same 900-second per-command bounds are retained. The initial test and Clippy commands failed to compile the two Cell accessor calls; zero all-target tests ran then, while one doc test and formatting passed. The corrected run and original failures are both retained.

The C-only artifact has **4,347 passing / 393 failing configurations** in the original 4,740-case Expat API matrix, with every raw result row unchanged. These configurations repeat 395 test bodies over chunking and deferral modes. The remaining 393 failures span 38 bodies: 298 concern allocation retry ceilings, 44 a particular realloc schedule, 12 allocations after empty ParseBuffer, 12 fixed-budget child creation, 12 literal version identity, 14 resource/harness bounds and one buffer-growth heuristic. The [classification](../../2026-09-10/detached-frames/api-classification/) preserves the details. We retain the 3-second, 1 GiB address-space, 768 MiB RSS and 240-second overall bounds; no allocator calls, limits, version strings or expectations are adjusted to improve the score.

Six C ASan/UBSan runs cover three consumers under shared/static linkage, including 327 selected-allocation scenarios per linkage. Rust release libraries are uninstrumented and leak checking is disabled. Baseline observations remain exact across 3,318 strict traces, 36,456 custom-encoding comparisons, 2,392 malformed-input comparisons and 1,304 publication cases with 3,912 executions. The custom-encoding harness records commands, hashes, counts and difference summaries, but does not archive all successful raw pairs.

All 38 End lifecycle cases preserve their baseline behavior and existing Expat differences, including 12 text-content leaves as well as raw/context/position differences. Six End allocation scenarios retain four forced failures, two successes and zero live blocks; these fixtures exercise namespace-restoration fallback. The new focused Rust tests separately exercise eligible detached End paths.

Strict shared/static CPython each execute **802 tests with the same two failures**, `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`, plus 14 reported skips and three expected failures. Raw exits remain 2; there is no callback-fragmentation waiver. Skips include five methods, eight subtests and one class setup.

## Earlier experiments and review

The standalone End prototype predates composition with Cell. Against the earlier `a55` ThinLTO library it measured native ratio 0.964212 and CPython ratio 0.993321, with four and eleven project regressions respectively. Its focused tests, build, profiles, lifecycle checks, elapsed samples and independent reviews remain archived under `standalone-*`. Complete API and final workspace/strict CPython gates apply to the combined candidate described above. Historical handoff summaries retain their original scope before later timing was added.

The separate [unselected name experiments](unselected-names/README.md) retain the B1–B5 expanded-name frame chain and the ASCII name table, with all adverse results. They are not part of this runtime. B5 helped namespace-heavy inputs but regressed 15 of 24 native conditions; the ASCII table's native benefit was only 0.8%, with generated-entity regressions. Neither received full compatibility gates or was composed with End + Cell.

[summary.json](summary.json), [assembly.json](assembly.json), [build.json](build.json), [workspace.json](workspace.json), [gates.json](gates.json), and the native/Python summaries provide the complete identities and results. [evidence.tar.gz](evidence.tar.gz) preserves source, build logs, raw samples, controllers and review handoffs. [archive-members.json](archive-members.json) records all ARCHIVE_MEMBERS direct members and ARCHIVE_EXCLUDED omitted binaries with original paths and hashes; archive SHA-256 is `ARCHIVE_SHA`. Every direct member and original file was read back. Nested handoffs preserve their own scoped manifests. An initial packaging attempt expected the previous workspace summary key names and stopped before creating output; its script and error are retained. The corrected packager reads the actual unchanged 407/33/1 fields.

The source/ownership, composition/compiler, instruction/disassembly, elapsed, C-gate and strict CPython evidence received separate reviews. The End author also audited the saved C-gate package and final workspace; the root source review independently covers the implementation and focused tests. Final corrected-source/build identity has a separate independent review. Final package and prose reviews are attached alongside this report.

This runtime has no new sustained Rust sanitizer campaign or full PBS distribution run. Earlier campaigns and PBS evidence retain their original source versions in the main README. Oriole remains experimental; representative speed, API configurations and strict CPython text grouping still fall short of the complete goal.
'''
report = report.replace('RUNTIME', summary['runtime_commit']).replace('TABLE', table).replace('ARCHIVE_MEMBERS',str(summary['archive']['members'])).replace('ARCHIVE_EXCLUDED',str(summary['archive']['direct_binary_exclusions'])).replace('ARCHIVE_SHA',summary['archive']['sha256'])
(out/'README.md').write_text(report)
(out/'unselected-names'/'README.md').write_text('''# Unselected name optimizations

The expanded namespace frame chain and ASCII NameChar table remain experiments. None is included in the closing-tag runtime. All original preflights, measurements, regressions, source/build identities and independent reviews are preserved in [evidence.tar.gz](evidence.tar.gz), indexed by [members.json](members.json) and [manifest.json](manifest.json).

| Candidate | Result and decision |
| --- | --- |
| B1 expanded frames | Reject: equal raw/expanded names increased malloc/realloc calls from 0/1 to 129/129 in the NUL-separator case. |
| B2 equal-name owner | Ownership fix retained; native ratio 1.006380 with 20/24 regressions. No Python or full gates. |
| B3 outlined expansion | Targeted attribute inlining criterion failed. No profiles or elapsed study. |
| B4 ordinary inline hint | Generated `.text` is byte-identical to B3. No profiles or elapsed study. |
| B5 forced tiny append inlining | Native ratio 0.985190, 15/24 regressions; CPython 0.982512, 12/24 regressions. Namespace-heavy gains with common/generated costs; held. |
| ASCII NameChar table | Instructions improve 2.58%; native ratio 0.992047 with 6/24 regressions, CPython 0.997092 with 9/24 regressions. Both native entity conditions slower; held. |

B2/B5 use the earlier `a55` control; ASCII uses Cell `0cfd`. They were not composed with End or each other. B5's initial wrong-affinity preparation stopped before parser execution; the failure and corrected preparation remain. ASCII's first worktree destination refusal is also retained. No full API, strict 802-test CPython, PBS or sustained sanitizer claim applies to these candidates.

The outer archive has 6,002 direct members, 22 binary exclusions and SHA-256 `88572916fc7e7ae914ae8408c9612290f8aa74269ea09005d1833cc1c2b0be28`. Its packager rechecked every direct member and origin. Nested archive bytes were verified; their individual internal members retain the original scoped review receipts rather than a new complete outer audit.
''')
(bench/'README.md').write_text('''# Detached closing-tag callbacks

The selected End-frame change reduces native project elapsed time by 4.0% and actual CPython time by 2.1%, relative to the published Cell-counter runtime. Across all fixed project conditions, Oriole still takes 1.64× Expat's native time and 1.29× its CPython time. Generated controls regress by 2.5% overall; generated entities regress by 6.2% and 3.3% at 4/64 KiB.

See the [complete report](../../../../docs/validation/2026-09-11/detached-end-frames/) for every condition, raw sample, adverse result, source/library/compiler identity, compatibility gate and review. Both elapsed Oriole builds use C-only ThinLTO without PGO on the shared host.
''')
readme=(repo/'README.md').read_text();before=readme
start=readme.index('| Vulkan registry |');end=readme.index('\n\nThese [native measurements]',start)
readme=readme[:start]+table+readme[end:]
readme=readme.replace('These [native measurements](benchmarks/results/2026-09-11/family-budget-cells/)', 'These [native measurements](benchmarks/results/2026-09-11/detached-end-frames/)')
start=readme.index('Using cells for the C interface');end=readme.index('\n\nOptional [profile-guided builds]',start)
readme=readme[:start]+"Detaching matched closing-tag names into callback frames reduces elapsed time by 4.0% across the 24 native project conditions and 2.1% through CPython. Both Oriole builds use effective ThinLTO. Oriole still takes 1.64× Expat's time natively and 1.29× through CPython; three Wayland CPython conditions are faster than Expat in this measurement. Generated controls regress by 2.5% overall, with entity input regressing by 3.3–6.2%. The [full report](docs/validation/2026-09-11/detached-end-frames/) retains all conditions, samples and regressions."+readme[end:]
readme=readme.replace('The [latest compatibility report](docs/validation/2026-09-11/family-budget-cells/) records 400 workspace tests','The [latest compatibility report](docs/validation/2026-09-11/detached-end-frames/) records 407 workspace tests')
assert readme[readme.index('## License'):] == before[before.index('## License'):]
assert [l for l in readme.splitlines() if l.startswith(('#','>'))] == [l for l in before.splitlines() if l.startswith(('#','>'))]
(repo/'README.md').write_text(readme)
(out/Path(__file__).name).write_bytes(Path(__file__).read_bytes())
print('Wrote closing-tag report and README; warning, headings and license unchanged.')
