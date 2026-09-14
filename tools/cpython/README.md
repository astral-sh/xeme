# CPython XML consumers

`run.py` builds CPython 3.12.13's XML extensions against a supplied Xeme shared
library or static archive and runs six unchanged upstream XML test modules. The
source checkout must match the pinned revision and remain clean. The output
directory contains build commands, source hashes, logs, and exit codes.

An import finder loads the built `pyexpat` and `_elementtree` extensions, including
when tests reimport them. Before running the suite, the harness checks module
paths, import specs, and binary hashes, then verifies ElementTree's accelerator
and pure-Python import paths. These checks require Python without `-O`. Results
are recorded in `origin.log`.

## Test requirements

All six upstream XML suites must pass in CI, including the line-break callback
checks in `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. Other
callback boundaries can still differ from Expat's.

All six modules and the complete pinned 803-case discovery inventory are required.
Every case must execute or have an explicit class-setup skip. Discovery is
recorded in `test-inventory.log`, and the callback fixture files must match their
pinned hashes. Two additional checks verify element/CDATA order, text preservation,
and callback-controlled buffering with the same extensions. `tests.log` and
`summary.json` retain the upstream results and exit codes.

`--consumer-fix` applies the [CPython allocation-failure backport](consumer-fix/)
to a temporary source copy. `--system-allocator` selects the system allocator. The
summary records both options.

## Installed distributions

To test a CPython 3.12.13 distribution built with Xeme, run its installed
interpreter in isolated mode:

```console
/absolute/python/install/bin/python3.12 -I tools/cpython/installed.py --output /tmp/installed-xml
```

The output directory must be new. This runner requires the same six upstream
suites, fixture hashes, test inventory, and semantic checks to pass with the
installed modules. It retains raw logs and exit codes. Packaging, platform
compatibility, and threaded parsing need separate validation.
