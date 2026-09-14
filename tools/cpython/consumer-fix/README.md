# CPython external parser cleanup

This directory backports CPython's fix for allocation failures in
`pyexpat.ExternalEntityParserCreate` to CPython 3.12.13.
The original cleanup can dereference an uninitialized handler array and decrement
the parent parser's reference count twice. The same failure occurs when using
reference Expat.

The patch comes from [CPython PR #144992](https://github.com/python/cpython/pull/144992),
commit `e6b9a1406980fbb1d4032eca9cc0b4f8f252b716`, which fixes
[issue #144984](https://github.com/python/cpython/issues/144984). It includes only
the `Modules/pyexpat.c` change. The patch is
derived from CPython under its [license](https://github.com/python/cpython/blob/e6b9a1406980fbb1d4032eca9cc0b4f8f252b716/LICENSE).
`provenance.json` records the original, patched-source, and patch hashes.

## Consumer tests

Enable the backport in the CPython harness with `--consumer-fix`:

```shell
python3 tools/cpython/run.py \
  --source /absolute/cpython-3.12.13 \
  --library /absolute/libxeme_expat.so \
  --output /absolute/consumer-fix-shared \
  --consumer-fix
```

The harness verifies the pinned source and patch hashes, applies the patch without
fuzzy matching to a temporary copy, and verifies the resulting source hash.

## Fault injection

The Linux diagnostic below forces `XML_ExternalEntityParserCreate` to return NULL.
Each extension runs in a separate process with a 15-second wall timeout, a 10-second
CPU limit, and a 1 GiB address-space limit. A direct extension import checks the
tested module's origin, even when the interpreter has a builtin `pyexpat`.

```shell
python3 tools/cpython/consumer-fix/run.py \
  --original-extension /absolute/original-reference/pyexpat.cpython-312-x86_64-linux-gnu.so \
  --original-extension /absolute/original-xeme/pyexpat.cpython-312-x86_64-linux-gnu.so \
  --fixed-extension /absolute/fixed-reference/pyexpat.cpython-312-x86_64-linux-gnu.so \
  --fixed-extension /absolute/fixed-xeme/pyexpat.cpython-312-x86_64-linux-gnu.so \
  --output /absolute/fault-probe
```

The probe checks that the original extensions crash during cleanup and the fixed
extensions raise `MemoryError` while preserving the parent reference count. It
retains extra parent references and skips interpreter teardown after reporting,
so a refcount mismatch cannot cause a second crash during shutdown.

This probe isolates NULL-parser cleanup. It does not inject the separate
Python-side buffer or handler-array allocation failures. The backport fixes all
three ownership paths from the upstream change.
