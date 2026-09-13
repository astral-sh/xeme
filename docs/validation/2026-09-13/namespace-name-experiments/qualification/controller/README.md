# Held namespace compatibility comparison

Preparation only; no compiler, parser, consumer, or supervisor was run.

After root releases the final normal source, run on CPU 3 with a fresh output:

```sh
taskset -c 3 /home/dev-user/.local/share/uv/python/cpython-3.12.13-linux-x86_64-gnu/bin/python3.12 -I -S \
  /tmp/oriole-namespace-name-storage-correctness-preparation/run.py \
  --build-json /tmp/oriole-namespace-name-proof-study/build.json \
  --output /tmp/oriole-namespace-name-storage-correctness-run01
```

Choose the actual final reviewed `build.json`; the example does not select the current timing candidate. Its source map must equal the worktree's tracked Cargo/crates files (currently 69), and every source and normal library hash must match. The five unchanged ABI/C files are pinned separately, together with tools and baseline evidence. No historical 74-file count or predicted library hash is used.

The runner re-execs through the unchanged reviewed supervisor, with a 2,400-second whole-run bound covering the existing API 300-second wrapper and two 1,000-second strict wrappers. A clean environment and CPU 3 apply throughout. Supervisor output is the chosen output path plus `-supervision`; both directories must be absent. Its retained receipt includes raw exit, timeout/cancellation and adopted-descendant cleanup.

Existing tools remain byte-exact to the completed combined baseline:

- API: baseline `api-native/run_clean.py` launches `tools/upstream-expat/run.py`; all 4,740 ordered rows, BEGIN/result records, assertion/error lines, adapted inputs and compilation vector are compared. Only output paths and candidate library/binary identities differ. Original limits remain 3 seconds per case, 1 GiB address space, 768 MiB RSS and 240 seconds for the test process.
- CPython: `tools/cpython/run.py` runs shared then static with the existing cleanup backport and static native-library order. Original 120-second module and 900-second suite bounds remain. Fresh module origins, exact O2 compile vectors, 802 ordered methods, 809 rendered outcomes, skips/subtests and both failure names are checked against the completed combined baseline.
- `methods.py` is an unchanged copy of the existing multiline/subtest reader. The existing supervisor handles nested process sessions; no new cleanup implementation is introduced.

Successful comparison means the original 4,349 API passes, 391 API failures, zero timeouts and two strict grouping failures per linkage are preserved. Raw API exit 1 and strict exits 2 remain recorded. It is not a clean upstream-test pass. Any changed row/assertion/outcome is saved before failing for review, including potential improvements. This runner does not launch C sanitizer integrations, W3C, fuzzing, benchmarks or a normal Rust build.

`report.json`, `api-comparison.json`, strict per-linkage comparisons and unchanged tool-generated logs retain results. `preparation.json` and `pins.json` identify the small source-only preparation; final build identity is supplied at release.
