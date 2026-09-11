# Checkpoint guards: rejected V2 experiment

**Experimental and rejected under the predeclared PGO-first gate. Every one of the 28 measured conditions is slower than the capacity-fixed PGO control.** No normal native or CPython timing campaign followed this failed gate.

Candidate: `421bc188622498fcdda60ac2b2a1461ee72b9245`. This adds guards and one regression test only in `encoding.rs` on rejected V1 `f766243bcc485ab305d339f5d37f095e0940eb49`. The selected comparison is mandatory capacity fix `de859c89577b2982d59b6923d682032f6a7c7800`. The archived Git/source identities preserve both rejected runtime experiments regardless of later PR documentation changes.

## PGO native results

| Group | Candidate/control | Candidate/Expat | Control/Expat | Adverse/control |
| --- | ---: | ---: | ---: | ---: |
| Real projects | 1.109723575 | 1.527622226 | 1.373331184 | 24/24 |
| Generated | 1.057408824 | 5.247647154 | 4.975911168 | 4/4 |

Ratios above 1 mean slower. Real projects regress 10.97%; generated cases regress 5.74%. `conditions.csv` is the original byte-exact 28-row audit CSV, including every control and Expat comparison. Each reported aggregate is the geometric mean of per-condition medians of seven paired time ratios.

The unchanged protocol contains 24 project and four generated conditions, interleaved, with seed 2026091003 and seven pairs. All 672 workers and 106,176 samples are retained: 105,420 measured samples, 588 timed warmups and 168 preflight samples. Every original worker JSON byte sequence was checked against exact reconstruction from the retained raw containers before omission. No selective reruns or performance-adoption claim.

## Source, build and checks

The 73-file source snapshot and full patch against the capacity-fixed control are retained. `guard.patch` preserves the exact one-file V1-to-V2 change reviewed independently. Both archived 73-file source snapshots are checked; only `encoding.rs` differs. V1 controller inputs are retained only where V2 actually depends on them.

The 90-file matched build inventory retains normal and fresh original-G PGO build provenance, compiler logs, profile identities, six Git-pinned pipeline helpers and training records. The separate build audit reconstructs nine complete compiler vectors and 864 original generated-training records per build against the capacity-fixed control. Builds use O3, ThinLTO and one codegen unit; normal build provenance does not imply a normal elapsed campaign.

After the final source edit, all 444 local tests, one compile-fail doctest and eight focused tests passed, alongside formatting and strict Clippy. All command receipts and raw streams remain. PGO matches the capacity-fixed parent on all 3,318 exact callback/status/error/location differential rows, without waivers; raw corpus/results, commands, combined log and parent-reported session completion are retained.

There is no V2 normal native timing, CPython timing, CI/Miri, full upstream API, full PBS, strict CPython or fuzz claim. This experiment does not establish production readiness. Build/source review, native collector preparation, root target collection and saved-result audit roles remain explicit. The native reviewer authored inherited V1 adapters and adapted the arithmetic reader; V2 runtime and collection were separate.

## Portable verification

Use Python 3.12 or later with assertions enabled:

```sh
python3 -I -S verify.py --package . --output /tmp/oriole-v2-rejected-readback.json
```

The records-only reader checks every archive object and the file manifest, both 73-file source archives, all 672 exact worker reconstructions, all sample/median/ratio calculations and 28 CSV rows. It also checks the final local command/stream outcomes and all 3,318 exact differential rows. It loads no parser, compiler or binary and requires no original absolute path. The full local build audit and compiler proofs are retained evidence; the portable reader does not rerun that audit.

This uses the same SHA256 object archive and verifier conventions as V1/PR134. `archive-index.json` maps logical files to deduplicated objects; `worker-aliases.json` defines exact worker reconstruction. Compiled libraries/executables and shared caches are excluded from archive bytes, with pinned identities retained. No archive is blindly extracted. Corpus and Oriole/Expat notices remain included. `files.json` binds the reviewable package files; verification receipts are separate outputs.
