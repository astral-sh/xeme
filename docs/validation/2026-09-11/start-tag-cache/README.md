# Private start-tag structure cache — rejected

**Reject this candidate.** Its ordinary release build is 1.16% slower across real-project native conditions, with 17 of 24 conditions regressing. All four Maven and all four DocBook conditions regress. The checked parser and C handle layouts each grow by 48 bytes, and the source introduces a lazy heap cache whose allocation cost was not measured. Keep the suffix-sharing runtime selected.

Candidate: `d8c5e503a4a432fb79f9ce5c05f44baf3428e425`. Selected comparison runtime: `a0acaf1bf8c8ae3f55030332be794cb8b9b5bea9`. This packet publishes evidence only; the cache remains unselected.

## Normal native results

| Scope | Candidate / selected control | Candidate / Expat | Adverse conditions |
| --- | ---: | ---: | ---: |
| 24 real-project conditions | 1.011597× (+1.16%) | 1.596393× | 17 / 24 |
| 4 generated conditions | 1.002042× (+0.20%) | 4.859867× | 1 / 4 |

Maven regresses by 4.35–5.34% across its four conditions; DocBook regresses by 2.42–4.12%. Wayland is the only project with an aggregate gain, at 1.44% faster. The generated entity input at 64 KiB regresses by 2.94%; the other three generated conditions improve. All 28 conditions, including every regression, are retained in [conditions.csv](conditions.csv).

The campaign uses six original real-project XML inputs, 4 KiB and 64 KiB chunks, and namespaces off/on, plus four generated controls. It retains seven seeded, interleaved process pairs per condition. Ratios are medians of paired process-median ratios; aggregates are geometric means across conditions. The independent saved-data reader reconstructed all 672 workers and 106,176 samples, including canonical preflights, callback checks and library bindings.

Both normal Oriole libraries use O3, ThinLTO and one codegen unit on the same explicit x86_64 target. The Expat control uses its matching pinned normal O3 build. Compiler settings, selected allocator, driver, inputs, order, limits and timing arithmetic are unchanged. These are parser/consumer measurements on this machine, not whole-application timings or a demonstrated explanation of the regression.

## Build, source and review

Normal, instrumented and PGO builds completed. Each passed the original generated training/replay corpus of 288 parse records: 864 candidate records in total. PGO training uses the original generated inputs only. Nine actual workspace compiler vectors and six library artifacts were independently checked against the selected control. PGO elapsed performance was not measured.

Four Rust files differ: `lib.rs`, `recycling.rs`, `tag.rs` and `tests/adapter_frame.rs`. The 72-file source comparison also includes the inherited `tests/c/integration.c` regression from PR142; the 69-file PGO source comparison differs in only those four Rust files. Source snapshots and exact patches are archived.

Final checks report 443 passing Rust checks across 34 groups, formatting, all-target checking and Clippy. All 61 Rust source hashes match the committed candidate. Original receipts retain their precommit metadata; the earlier `check01` E0502 failure and its command, exit status and log are preserved separately from the passing final checks.

Independent source review found no actionable defect. The cache admits private lexical tokens and offset records after successful lowering. It has eight entries, bounded keys and attributes, an 8 KiB probe budget and a reservation inside the existing 64 KiB retained-storage ceiling. Values, current namespace bindings, expanded-name duplicates, resource limits and callback publication are still checked. Allocation failure, chunked input, changed values and namespaces, replacement and retained-budget behavior have focused coverage. This source review does not replace the unexecuted compatibility campaigns.

## Layout and earlier locality census

The checked debug-artifact probes report:

| Type | Selected | Candidate | Difference |
| --- | ---: | ---: | ---: |
| `XML_ParserStruct` | 3,160 bytes | 3,208 bytes | +48 bytes |
| Core `Parser` | 2,408 bytes | 2,456 bytes | +48 bytes |
| `AdapterFrame` | 256 bytes | 256 bytes | 0 |

Alignment remains eight bytes. These probes evaluate `size_of` and `align_of` using checked debug libraries; they create no parser. They do not measure private heap `ShapeCache` layout, allocator metadata, peak memory or RSS. The allocation diagnostic was prepared but never executed.

The earlier Expat-only diagnostic models lexical reuse in the six pinned inputs. It uses a token scanner cross-checked against Expat and models small LRU caches; it never loads the candidate. Its eight-entry exact-skeleton model reports substantial reuse in several documents, while excluding Batik because of its DOCTYPE. These are opportunity estimates, not actual candidate cache hits or executed hot-path coverage. Admission, split-token progress, decoding, available capacity and namespace-specific eligibility can differ. Namespace lexical hits do not establish that a start can use the identity-frame path; all Maven and DocBook starts are excluded from that subset.

The census measures no speed, instructions, heap, allocations or RSS, and byte coverage is not predicted savings. Its saved evidence contains aggregate reports and stdout, not a per-tag trace. Saved-only review checked provenance and aggregate arithmetic; it did not rerun the census. These estimates explain why the prototype was attempted and do not outweigh the later elapsed measurements.

## Campaigns not executed

The ordinary release native screen ran first and was sufficient to reject the candidate. **PGO native, allocation, upstream API/C sanitizer, strict CPython, and normal/PGO CPython campaigns were not executed.** Their controllers or preparation templates are retained as prepared, unexecuted artifacts. No compatibility improvement, sanitizer result, CPython result or measured private-heap benefit is claimed for this candidate. The selected runtime's previous results do not apply to it.

## Saved evidence

[report.json](report.json) records the decision, complete native summary, campaign status and evidence hashes. [evidence.tar.gz](evidence.tar.gz) retains source snapshots, checks including the first compiler failure, independent source review, compiler/training records, layout and diagnostic records, raw native workers, all condition rows and the original saved-data auditor. Input licenses and notices accompany copied corpus files. [index.json](index.json) lists each archive member, original path, size and SHA-256.

No executables, shared/static libraries or binary profiles are included. Their identities remain in the original receipts. Absolute paths describe the original machine; this is a saved-record bundle, not a new executable verification framework or a self-contained benchmark installation. Packaging performed one archive readback and executed no parser, compiler or benchmark.
