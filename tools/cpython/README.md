# CPython XML consumers

`run.py` builds the pinned CPython 3.12.13 XML extensions against a frozen Xeme shared library or static archive, verifies their loaded paths, and runs six unchanged upstream XML test modules. The source checkout must match the pinned revision and remain clean. Build commands, source hashes, logs and original exit codes are retained in the output directory.

A persistent import finder routes only `pyexpat` and `_elementtree` to those built extensions, including fresh imports made by the upstream tests. The mandatory preflight checks initial and fresh module paths, import specs and binary hashes, exercises both ElementTree accelerator import paths, and verifies that deliberately blocked imports still select the pure-Python implementation. Optimized Python is rejected so it cannot disable the preflight's assertions. The helper sources and `origin.log` are recorded with each run.

The C interface preserves the line-break character-data boundaries required by
`BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. All six unchanged
upstream XML suites must pass in CI. This resolves the two historical assertions;
other exact callback-fragmentation differences remain outside this guarantee.

All six modules and the complete pinned 803-case discovery inventory are required.
Every case must execute or have an explicit class-setup skip, with consistent
test/file counts and no errors. The runner retains discovery in
`test-inventory.log`; the formerly failing fixture files must match their pinned
hashes. Two additional checks using the same compiled extensions
must preserve element/CDATA order, every text character, and callback-controlled
buffering. Original failures and exit codes remain in `tests.log` and
`summary.json`.

The optional `--allow-text-fragmentation` reproduces the historical exception for
the two exact recorded assertion messages and traceback locations. It is retained
for old-source diagnostics and is not a current CI or release gate. An unrelated
failure or changed payload in either method fails even that historical gate.

`--consumer-fix` separately applies the pinned upstream [allocation-failure backport](consumer-fix/) to a temporary C source copy. `--system-allocator` separately selects the system allocator. Each adaptation is recorded explicitly.

## Installed distributions

To check a CPython 3.12.13 distribution built with Xeme, run its installed interpreter in isolated mode:

```console
/absolute/python/install/bin/python3.12 -I tools/cpython/installed.py --output /tmp/installed-xml
```

The output directory must be new. This runner requires all six upstream suites,
the same fixture hashes, complete inventory and semantic checks to pass with the
installed modules, retaining raw logs and exit codes. It does not substitute
extensions or modify CPython sources. Packaging, platform compatibility, and
threaded parsing need separate validation. Results identify the installed artifact
they tested; earlier runs do not qualify a later build.
