# CPython XML consumers

`run.py` builds the pinned CPython 3.12.13 XML extensions against a frozen Xeme shared library or static archive, verifies their loaded paths, and runs six unchanged upstream XML test modules. The source checkout must match the pinned revision and remain clean. Build commands, source hashes, logs and original exit codes are retained in the output directory.

A persistent import finder routes only `pyexpat` and `_elementtree` to those built extensions, including fresh imports made by the upstream tests. The mandatory preflight checks initial and fresh module paths, import specs and binary hashes, exercises both ElementTree accelerator import paths, and verifies that deliberately blocked imports still select the pure-Python implementation. Optimized Python is rejected so it cannot disable the preflight's assertions. The helper sources and `origin.log` are recorded with each run.

Xeme can combine character-data callbacks across line breaks. Two CPython tests
assert Expat's particular boundaries: `BufferTextTest.test1` and
`CDATAHandlerTest.test_handlers`. The optional `--allow-text-fragmentation` gate
accepts only the exact recorded assertion messages and final traceback locations
from the pinned fixture files. An unrelated failure or changed payload in either
method fails the gate.

All six modules and the complete pinned 803-case discovery inventory are required.
Every case must execute or have an explicit class-setup skip, with consistent
test/file counts and no errors. The runner retains discovery in
`test-inventory.log`. Two additional checks using the same compiled extensions
must preserve element/CDATA order, every text character, and callback-controlled
buffering. Original failures and exit codes remain in `tests.log` and
`summary.json`; an accepted gate does not mean the upstream suite passed.

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
