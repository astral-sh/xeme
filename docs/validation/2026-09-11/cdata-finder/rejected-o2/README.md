# Rejected experiment: Cargo O2 versus O3

**Keep O3 with ThinLTO.** O2 improved the normal real-project aggregate by
1.16%, but regressed the selected PGO aggregate by 3.07%; 19 of its 24 real
conditions were slower. No Python or full compatibility gates followed rejection.

Packaging status: **assembled and verified; see [readback.json](readback.json)**.

| Build | Real O2/O3 ratio | Real conditions lower | Generated O2/O3 ratio | Generated conditions lower |
|---|---:|---:|---:|---:|
| Normal | 0.988425684 | 20/24 | 1.007805764 | 1/4 |
| Fresh PGO | 1.030655382 | 5/24 | 0.986041214 | 3/4 |

Ratios are geometric means of per-condition median paired ratios; lower is
faster. The normal generated aggregate regressed 0.78%; generated PGO improved
1.40%. [All 56 conditions](conditions.csv) include every O2/O3 and Expat ratio.
[All 27 adverse conditions](adverse-conditions.csv) remain visible: seven normal
and twenty PGO, including generated cases. [summary.json](summary.json) retains
the exact decision, counts and condition values.

## What changed

The source stayed byte-identical across the selected streaming baseline and
O2 treatment: 70 pinned source/test/header files, including the then-current C
fixture. The explicit supported Cargo configuration
`--config=profile.release.opt-level=2` applied to normal, profile-generation
and profile-use builds. Both configurations kept explicit-host C-only ThinLTO,
one codegen unit and the same tools and generated-only training protocol.

**This compares complete Cargo O2 and O3 configurations**, including dependency
rebuilds and Cargo metadata. It does not isolate one LLVM flag as the cause.
The original strict vector proof rejected the metadata differences and stopped.
Its script, first failure log and all preliminary vectors remain unchanged.
A separately reviewed proof then accepted only these exact pairs in all three
phases, preserving every raw argument and rejecting other unexpected differences:

| Crate | O3 metadata | O2 metadata |
|---|---|---|
| `oriole_storage` | `8ae6edb74b4afbee` | `940fdde942b9ced9` |
| `oriole` | `e41f1ff64bd2ed1c` | `3ff054c7a2303f29` |
| `oriole_expat` | `f10a79f8fbc60887` | `e21525dd7ff3033d` |

The build audit covers all 63 original frozen entries, nine actual workspace
compiler vectors, six unchanged pipeline files and 864 candidate generated
parse records. All three candidate training phases match the three control
phases; the raw profile and merged profile are retained. Six compiled candidate
artifacts are excluded by recorded identity, with every other frozen entry
retained. Compiler/tool and benchmark executable identities are recorded without
copying their binaries. Existing source checks are historical O3 checks, not
fresh O2 compatibility results.

## Measurement and complete raw evidence

Each mode ran the fixed native lifecycle protocol once: six real XML projects
at 4 KiB and 64 KiB with namespaces off/on (24 conditions), plus four generated
conditions. Seven seeded paired cohorts compare O2, frozen O3 and the same
mode-matched Expat. Each mode retains 84 preflight and 588 timed workers,
105,420 measured samples, 588 timed warmups and 168 preflight samples: **106,176
raw samples per mode; 212,352 total**. Seed `2026091003`, 90-second worker bounds,
600-second preflight and 1,200-second timing bounds are unchanged. Timings ran
on reserved CPU0; preflights ran on CPU5. All first campaign commands passed.

The archive retains complete original `results.json` and `preflight.json`,
including each worker's exact stdout/stderr and every warmup and measured
sample. The 1,344 separate `worker-*.json` files duplicate records already in
those full reports. They are represented by **explicit, per-file aliases**:
source path, byte length, SHA-256, container and JSON pointer, plus the exact
serialization rule. Assembly compared every reconstructed file byte-for-byte
with its original before omitting it. Identical remaining payloads are stored
once; the inventory maps every logical path to its canonical archive member.

The portable package contains `evidence.tar.gz`, `inventory.json`, these
summary tables, and [verify.py](verify.py). Verification uses Python's standard
library, requires no original absolute paths, and never loads a parser or
compiler. Original absolute paths remain provenance labels inside untouched
reports. Run `python3 -I -S verify.py .` from the assembled bundle. The new
readback validates transport, source bytes, worker aliases and sample records;
it is separate from the original full independent audits.

## Source redistribution notices

All nine license/notice entries for the six included projects are retained from
the pinned corpus manifest (`inputs/corpus-manifest.json` inside the archive). Their full relative
paths are preserved as `inputs/corpus/<project>/<file>` in the logical inventory,
with exact byte counts and SHA-256 values. `redistribution_notices` records the
manifest-to-archive mapping; the portable verifier checks complete coverage and
fixture identity against that manifest.

Oriole's `LICENSE-APACHE`, `LICENSE-MIT`, `THIRD_PARTY_LICENSES.md`, and
`tests/c/UPSTREAM-NOTICES.txt` come from the unchanged O2 worktree and retain
those relative paths under `source-notices/oriole/`. They accompany the retained
Oriole source archive, native driver and C fixture. These notice additions do
not alter any study source or measured result.

## Independent audits and limits

The unchanged build audit is
`7c89c6a103f60d7f50d05eb42c1d2aedb82fda45a05d7fe069b1d0c66c80d696`;
the full native protocol audit is
`d18e234595fa4dde63ddb86daa1a4fe9f7e5289cf5a902c00abe462a11d37617`.
Both reports and their audit scripts are included. The archive preserves the
original frozen captions; this README states their complete Cargo-config scope.

These are original XML fixtures processed by the native callback driver, not
full project executions. Shared-host frequency/cache/memory effects remain
uncontrolled. The legacy driver used explicit library paths and file hashes;
this study did not add same-process `XML_Parse` symbol-origin verification.
The source of the available driver is included; its binary identity is pinned
separately by the original protocol. Local tools were Ohm; this is no production
stable-toolchain result, full API/ASan certification, or compiler-mechanism proof.
No later Finder or PBS results are transferred to O2.

## Package verification

Assembly verified every retained payload against its recorded SHA-256 and byte count, and every reconstructed worker file against its original bytes. The portable readback then checked the archive, all 70 source files, all 1,344 worker aliases, all 56 condition summaries and 212,352 raw samples. It also checked all nine corpus license/notice entries against the pinned manifest and retained Oriole notices. See [readback.json](readback.json) and [package-files.json](package-files.json) for the completed receipt and file hashes.
