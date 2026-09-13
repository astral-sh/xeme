# CPython XML consumers

`run.py` builds the pinned CPython 3.12.13 XML extensions against a frozen Oriole shared library or static archive, verifies their loaded paths, and runs six unchanged upstream XML test modules. The source checkout must match the pinned revision and remain clean. Build commands, source hashes, logs and original exit codes are retained in the output directory.

A persistent import finder routes only `pyexpat` and `_elementtree` to those built extensions, including fresh imports made by the upstream tests. The mandatory preflight checks initial and fresh module paths, import specs and binary hashes, exercises both ElementTree accelerator import paths, and verifies that deliberately blocked imports still select the pure-Python implementation. Optimized Python is rejected so it cannot disable the preflight's assertions. The helper sources and `origin.log` are recorded with each run.

The earlier startup-only loader allowed fresh imports to select the managed interpreter's incompatible built-in accelerator. That produced 19 accelerator-specific skips in the historical 803-test/31-skip results. Correct routing executes 18 of those successfully; one keeps its actual 2 GiB memory skip. Corrected local suites report 802 tests and 14 skips, including eight subtest skips and one class-setup skip. See the [PBS comparison](https://github.com/astral-sh/oriole/tree/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/validation/2026-09-10/version-consistent-pbs) for the source and environment differences behind these counts.

Oriole can combine character-data callbacks across line breaks. Expat permits arbitrary character-data fragmentation, but two CPython tests assert its particular boundaries: `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. The optional `--allow-text-fragmentation` CI gate accepts only the exact recorded assertion messages and final traceback locations from those two pinned source files. An unrelated assertion or changed payload in the same method fails the gate. All six modules and the complete pinned 803-case discovery inventory are required; every case must execute or have an explicit class-setup skip, with consistent test/file counts and no errors. The discovery output is retained in `test-inventory.log`.

Two additional semantic checks must also pass. They preserve ordered element/CDATA boundaries, every text character and callback-controlled buffering, using the same compiled extensions. All original failures and exit codes remain in `tests.log` and `summary.json`; an accepted gate does not mean the unchanged upstream suite passed. A new fragmentation shape requires an explicit reviewed exception instead of being accepted by test name.

The default runner remains strict. `--consumer-fix` separately applies the pinned upstream allocation-failure backport to a temporary C source copy. `--system-allocator` separately selects the system allocator. Each adaptation is recorded explicitly.

For an installed PBS distribution, run its actual interpreter in isolated mode:

```console
/absolute/python/install/bin/python3.12 -I tools/cpython/installed.py --output /tmp/installed-xml
```

The output directory must be new. This runner applies the same fixture hashes,
complete inventory, exact assertion exceptions and semantic checks to the
installed modules, retaining raw logs and exit codes. It does not load replacement
extensions or modify CPython sources. Distribution structure, parser identity,
and the old-glibc threaded checks remain separate requirements.
