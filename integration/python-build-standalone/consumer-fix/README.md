# CPython external parser cleanup

This directory backports CPython's existing fix for allocation failures in
`pyexpat.ExternalEntityParserCreate` to the pinned CPython 3.12.13 source.
The original cleanup can dereference an uninitialized handler array and decrement
the parent parser's reference count twice. The same failure occurs when using
reference Expat; this is a consumer ownership bug.

The patch comes from [CPython PR #144992](https://github.com/python/cpython/pull/144992),
commit `e6b9a1406980fbb1d4032eca9cc0b4f8f252b716`, which fixes
[issue #144984](https://github.com/python/cpython/issues/144984). It carries over only
the `Modules/pyexpat.c` change, preserving CPython's existing tests. The patch is
derived from CPython under its [license](https://github.com/python/cpython/blob/e6b9a1406980fbb1d4032eca9cc0b4f8f252b716/LICENSE).
`provenance.json` records the original, patched-source, and patch hashes.

## Consumer tests

The regular CPython harness leaves the consumer source unchanged by default.
Enable the backport explicitly:

```shell
python3 tools/cpython/run.py \
  --source /absolute/cpython-3.12.13 \
  --library /absolute/liboriole_expat.so \
  --output /absolute/consumer-fix-shared \
  --consumer-fix
```

The harness verifies the pinned source and patch hashes, applies the patch without
fuzzy matching to a temporary copy, and verifies the resulting source hash. Results
identify the consumer adaptation separately from Oriole's library hash.

## Fault injection

The Linux diagnostic below forces `XML_ExternalEntityParserCreate` to return NULL.
Each extension runs in a separate process with a 15-second wall timeout, a 10-second
CPU limit, and a 1 GiB address-space limit. A direct extension import checks the
tested module's origin, even when the interpreter has a builtin `pyexpat`.

```shell
python3 integration/python-build-standalone/consumer-fix/run.py \
  --original-extension /absolute/original-reference/pyexpat.cpython-312-x86_64-linux-gnu.so \
  --original-extension /absolute/original-oriole/pyexpat.cpython-312-x86_64-linux-gnu.so \
  --fixed-extension /absolute/fixed-reference/pyexpat.cpython-312-x86_64-linux-gnu.so \
  --fixed-extension /absolute/fixed-oriole/pyexpat.cpython-312-x86_64-linux-gnu.so \
  --output /absolute/fault-probe
```

The probe checks that the original consumers crash during cleanup and the fixed
consumers raise `MemoryError` while preserving the parent reference count. The
probe retains extra parent references and exits without interpreter teardown
after reporting, so a
refcount mismatch cannot cause an unrelated shutdown failure. Original crashes
remain recorded as crashes in the manifest.

This probe isolates NULL-parser cleanup. It does not inject the separate
Python-side buffer or handler-array allocation failures. The backport fixes all
three ownership paths from the upstream change.
