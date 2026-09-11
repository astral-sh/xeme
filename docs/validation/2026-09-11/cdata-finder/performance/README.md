# Reusable CDATA Finder: measured build and performance evidence

This package preserves the first normal and fresh-PGO native/CPython campaigns for the one-hunk Finder change. The decision is **retain for publication** after the separate compatibility gates. All 420 workspace tests and one doc test, strict workspace all-target Clippy and formatting passed on their first full-workspace attempt; the prior 114 focused tests are also retained. No target was rerun during packaging.

| Consumer | Inputs | Build | Candidate / streaming control | Candidate / Expat | Faster than control |
| --- | --- | --- | ---: | ---: | ---: |
| Native | real | normal | 0.989832 | 1.587298 | 20/24 |
| Native | generated | normal | 0.967396 | 4.912457 | 4/4 |
| Native | real | pgo | 0.994922 | 1.369664 | 16/24 |
| Native | generated | pgo | 0.976596 | 4.921072 | 4/4 |
| CPython | real | normal | 1.003800 | 1.269216 | 9/24 |
| CPython | real | pgo | 0.994929 | 1.131862 | 16/24 |

Ratios are geometric means of per-condition medians of seven paired ratios. Values below one mean less elapsed time. Normal CPython regresses by 0.38%; PGO native and CPython each improve by about 0.51%. Every adverse condition remains in the raw campaigns and independent audits. This still does not meet the goal of beating Expat: the PGO real-project aggregates take 1.37× its native time and 1.13× its CPython time.

## Exact scope

- Native: 28 conditions per mode, 84 preflights and 588 timed workers; 24 real-project conditions and four generated controls remain separate.
- CPython: 24 real-project conditions per mode, 72 preflights and 504 timed workers. Consumers use unmodified CPython 3.12.13; the separate strict suite uses the cleanup backport. Canonical adjacent-text coalescing is not exact callback equivalence.
- Oriole uses Ohm 1.98.1-1 / LLVM 22.1.8, O3, ThinLTO and one codegen unit, with experimental defaults disabled. Expat 2.8.4 uses GCC 13.3, O3 and no LTO. Each engine has its own generated-only PGO; this is not a comparison using matched compilers.
- The three bounded PGO instruction profiles each collected one warmup and one measured parse inside `XML_Parse`, including callbacks and first lazy Finder initialization. Inherited Vulkan `iterations=7` and Wayland `iterations=64` fields describe the earlier elapsed matrix, not the executed diagnostic commands. The final diagnostic receipt corrects the preserved earlier prose. Instruction counts and code-layout changes do not predict elapsed speedup.
- The measured source base is `144e69e08c5462217d54791374e579e3ef390a87`; the publication base is `ee73ccdeefa9572666c753776ed309860a1057e4`. All 70 runtime/test source hashes remain unchanged by that restack. All six measured pipeline files are recovered from the former Git revision and verified against both measured manifests; newer PGO/PBS tooling is not substituted into this evidence.

## Archive and verification

The [summary](summary.json) records the source and build boundaries, actual aggregates and archive identity. [members.json.gz](members.json.gz) maps 5,424 original sources to 4,388 stored payloads and 1,036 aliases with identical bytes. Each alias names its canonical stored member and retains its original path and hash. All members were read back after creation.

The [archive](evidence.tar.gz) retains complete raw workers, output, specs, controller adaptations, independent audits and their correction attempts, source/pipeline pins, actual compiler logs, generated training inputs/records and profile data, selected-control and candidate instruction diagnostics, and source/corpus notices. Historical diagnostic “unselected” labels are preserved; the later decision is authoritative. Original compiled libraries, extensions, interpreters, tools, Cargo caches, Python bytecode and nested source archives are excluded; their recorded hashes and origins remain explicit.

Run the portable verifier using only these package files:

```sh
python3 -I -S verify.py .
```

This verifies every stored payload and alias without accessing original absolute paths or executing a compiler/parser. The retained numerical readers keep their original machine paths as provenance; replaying them requires resolving those paths through the member/alias map. The full strict C/API and patched-consumer evidence is supplied separately by the publication report. No benchmark or compatibility result is inferred from the archive check.

The [inherited C-fixture notices](tests-c-UPSTREAM-NOTICES.txt) accompany the archived `tests/c/integration.c` source. Independent review found this notice missing from the original archive selection; the exact Git144 notice is attached as a hash-pinned sidecar without changing the archive or rerunning any study.
