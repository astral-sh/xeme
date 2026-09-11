# Literal-attribute eligibility check

The start-tag scanner now checks whether a literal attribute value can use the existing frame path in one pass. The change gives a small improvement with the measured ThinLTO PGO build: **2.14% less native time and 1.23% less CPython consumer time** on the measured real-project XML. The normal build is flat overall. Its generated native controls regress by 2.12%, and all adverse conditions remain in the evidence.

This is a follow-up to the [current PGO study](../pgo-lto/). It does not complete the faster-than-Expat goal: the candidate still takes **1.375× Expat PGO's native time and 1.132× its CPython time** in these separate cohorts. These are fixed XML workloads, not complete application timings or a claim of statistical significance.

## Source and matched builds

The frozen candidate changes only `crates/oriole/src/tag.rs` among 70 source files, against accepted `be22a271` runtime bytes at documentation commit `c1776c92`. [The source patch](candidate.patch) replaces two eligibility scans—special attribute bytes and invalid XML characters—with a single predicate. ASCII words can advance together; non-ASCII input uses the existing scalar XML-character rule. Ampersands and XML attribute whitespace still take the original fallback. Attribute parsing, error publication, allocation ownership, resource limits and callback behavior are unchanged.

Independent source review verifies the word masks, UTF-8 boundaries and equivalence to the old two-check predicate. The focused oracle covers 1,243,809 cases; all 58 core tests, strict core Clippy and formatting pass. The source manifest is `1120c74e96c7…`, with exact source archives and full hashes retained.

Fresh normal, profile-generation and profile-use builds contain nine actual workspace compiler invocations. They use the same explicit host target, Ohm 1.98.1/LLVM 22.1.8 with experimental defaults disabled, private build directories, and effective C-only ThinLTO. Matched compiler vectors preserve the prior control settings. PGO uses a fresh generated-only profile; 288 records in each of the three phases match the prior corresponding control. The six real projects are held out of training. PGO pipeline code is unchanged; the separately updated guide's hash is recorded.

| Build | Control shared-library hash prefix | Candidate shared-library hash prefix |
| --- | --- | --- |
| Normal ThinLTO | `0c8c58750c02` | `8ad88e13e07c` |
| ThinLTO PGO | `4f1518fc819b` | `8cc1c4144427` |

The candidate static hashes start `cbd6e2ec48d2` and `a294cbd13adb`, respectively. Expat controls remain pinned normal `7a333bc8ac92` and PGO `12d33ad26315`. Rust/LLVM and GCC differ; matching applies within each parser's build pair. The explicit-host normal control is distinct from the older implicit-host `ccfb2295` artifact.

## Measured results

Each native cohort retains six real projects, both namespace modes, 4 KiB and 64 KiB feeds, and two generated controls at both feed widths. Each Python cohort retains those six projects and feed widths with unmodified CPython 3.12.13 ElementTree and pyexpat consumers. Both Python consumers enable namespace processing. All cohorts use seven seeded paired rounds; normal and PGO are separate experiments, with no cross-cohort or additive speedup calculation.

Ratios below one mean less candidate time than the matched control.

| Workload | Normal ratio | Lower conditions | PGO ratio | Lower conditions |
| --- | ---: | ---: | ---: | ---: |
| Native real projects | 1.002103 | 12/24 | 0.978634 | 23/24 |
| Native generated controls | 1.021219 | 1/4 | 0.981787 | 4/4 |
| Python, both consumers | 1.000027 | 11/24 | 0.987735 | 22/24 |
| ElementTree | 0.995051 | 8/12 | 0.985853 | 11/12 |
| Pyexpat events | 1.005029 | 3/12 | 0.989620 | 11/12 |

The one PGO native regression is Maven with namespaces at 4 KiB (+0.594%). PGO Python retains Batik pyexpat at 4 KiB (+0.612%) and GTK ElementTree at 64 KiB (+0.072%). Normal Python has 13 adverse conditions; normal native generated controls have three. The full condition tables and raw samples remain archived.

Each native cohort contains 84 preflights, 588 timed workers, 196 paired groups, 105,420 measured parses and 588 warmups. Each Python cohort contains six extension builds, 72 preflights, 504 timed workers, 168 paired groups, 25,788 measured samples and 504 warmups. Every worker's callback observation and library identity is checked. Independent numerical reconstruction verifies every seeded order, sample median, paired ratio and aggregate.

Native timing covers parser lifecycle and callbacks. Python timing includes construction, feeding, closing, Python callbacks/tree creation and explicit result destruction. Imports, input reads, canonical checks and explicit `gc.collect` are outside; automatic GC remains enabled. Canonical pyexpat comparison merges adjacent text callbacks and is separate from the strict compatibility suite. Other target workloads were held during the native cohorts; Python phases followed completion of ASan and candidate correctness gates. Workspace checks started after both Python timing phases. Read-only review work on CPU6 can share the host; affinity does not control cache, frequency or memory bandwidth.

## Correctness and remaining failures

The measured PGO library preserves all 4,740 original upstream API outcomes byte for byte: **4,347 pass and 393 fail**, with original exit 1 retained. Bounds remain 3 seconds per test, 1 GiB address space, 768 MiB RSS and 240 seconds overall. No retry ceiling, assertion or resource defense was raised. The existing [compatibility limitations](../../../compatibility.md) remain.

Six shared/static C consumer runs pass, covering the original 327 allocation scenarios per linkage. C code is checked with ASan/UBSan; the linked Rust PGO release library is not sanitizer-instrumented, and leak detection is disabled. These gates preserve 3,318 strict traces, 36,456 custom-encoding/external comparisons, 2,392 malformed cases, and 1,304 publication cases across 3,912 executions. The 38 closing-tag lifecycle cases and six selected allocation scopes match the control, including all 37 recorded allocation rows per engine.

Reference position differences remain: 10,416 Default and 864 external-entity events in the publication oracle, plus the retained closing-tag reference differences. The unchanged custom helper does not save every successful raw pair; commands, counts, hashes and difference summaries are retained.

Strict shared/static CPython runs each retain all 802 method outcomes, original exit 2, the same `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers` failures, 14 reported skip lines and three expected failures. No text-fragmentation adaptation is enabled.

Workspace checks in the debug profile pass **408 tests across 33 binaries plus one doctest**; workspace/all-targets Clippy passes with `-D warnings`. Each command retains its original 900-second bound. Independent saved-log readback verifies every named test and the unchanged 70 source files. Earlier same-source formatting remains valid. These workspace results are separate from the optimized C-library gates.

## Evidence and review roles

[The artifact index](artifacts.json) records full hashes, origins and member counts. Three existing archives are copied byte for byte; verbose worker records, compiler vectors, source snapshots and per-file maps stay compressed.

| Evidence | Archive hash prefix | Direct members |
| --- | --- | ---: |
| [Source, fresh builds and both native cohorts](native-evidence.tar.gz) | `0b124014d242` | 1,453 |
| [C/API/adversarial and strict CPython gates](correctness-evidence.tar.gz) | `ceb93ac3a758` | 368 |
| [Both Python cohorts and numerical reviews](python-evidence.tar.gz) | `ea88fe3aafe25` | 3,572 |
| [Later native review and original package indexes](supplementary-reviews.tar.gz) | See index | 39 |
| [Workspace raw checks and independent readback](workspace-evidence.tar.gz) | See index | See index |

[Native](native-report.json), [Python](python-report.json), [correctness](correctness-summary.json) and [workspace](workspace-review.json) summaries keep their individual scopes. Original native seal-time labels saying selection or later audits were pending remain unchanged; this report supplies the later results. Source/build reviews already included in the native archive are not duplicated as separate old evidence bundles.

The Python package excludes 18 compiled files and records two Expat soname aliases against their exact target hashes. Its initial packaging-only symlink guard failure is preserved; no parser, timing or numerical-audit rerun followed it. Native and correctness archives retain their own explicit binary/cache exclusions and original preparation records.

Source and build reviews, numerical reviews, and correctness reviews are distinct. Root authored the parser change and collected Python timings; the prepared-controller author independently reconstructed the saved Python numbers and discloses that role. Separate agents reviewed the protocol and build bindings. All reviews are by AI agents. No runtime or default-build setting was changed while preparing this documentation.

[Final independent Python package review](python-package-review.json) verifies the archive members, source snapshots, exclusions and preserved packaging attempt. Its [complete compressed receipt](python-package-review.tar.gz) supplements the original immutable handoff.

The [final publication review](publication-review.json) verifies the committed source identity, current README benchmark table, result claims, evidence links and preserved warning/license structure. Its [complete receipt](publication-review.tar.gz) retains the reviewed snapshots; the final attachment and file-index update are separate packaging steps.
