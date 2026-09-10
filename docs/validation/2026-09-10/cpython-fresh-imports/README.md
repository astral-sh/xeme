# CPython fresh-import harness correction

The harness now keeps initial and fresh `pyexpat` and `_elementtree` imports on
the selected extension files. Previously, CPython's fresh-import tests could load
the managed interpreter's incompatible built-in accelerator and skip 19
accelerator-specific tests. A persistent finder handles only these two module
names. Deliberately blocked imports still select the pure-Python implementation.

The mandatory preflight verifies initial and fresh file/spec origins and records
binary hashes. It runs both unchanged ElementTree accelerator import paths,
completes a parse with each, and checks pure-Python and explicit blocked imports.
It rejects optimized Python so assertions cannot be disabled. The runner retains
the helper hashes, command, exit code and `origin.log`.

| Frozen library | Reported tests | Reported skips | Unexpected failures | Exit |
| --- | ---: | ---: | ---: | ---: |
| Oriole `4b11ace`, `ac6a0a6a…` | 802 | 14 | 2 | 2 |
| Expat 2.8.4, `7a333bc8…` | 802 | 14 | 0 | 0 |

Both rows use the unchanged six CPython 3.12.13 XML test modules. Each reports
three expected failures. The 14 skips comprise five whole-method skips, eight
subtest skips and one class-setup skip. They are not 14 skipped methods.
The remaining Oriole failures are `BufferTextTest.test1` and
`CDATAHandlerTest.test_handlers`; this harness correction preserves those results.

Eighteen previously skipped accelerator methods now execute successfully. One
retains its actual 2 GiB memory skip. The working accelerator causes
`NoAcceleratorTest` to skip during class setup, explaining 802 rather than the
historical 803 methods. The local managed interpreter lacks `_testcapi`, which
accounts for one additional skip compared with the default-resource installed
[PBS suites](../version-consistent-pbs/).

## Evidence and review

The [summary](summary.json) and [independent review](independent-review.json)
retain the exact library/helper identities and count reconciliation. Negative
checks confirm that the old startup-only loader and optimized Python both fail
the new preflight. The existing two fragmentation-gate unit tests, Ruff and ty
pass. The initial unit-test launch omitted its local module path; that failed
invocation and its corrected invocation remain in the evidence.

The review also records four configurations of a separately tested newline
prototype. That runtime is outside this harness change. The two baseline
failures above remain visible, and earlier 803-test/31-skip results retain their
original records and coverage limitation.

[evidence.tar.gz](evidence.tar.gz) preserves the immutable handoff, final helper
sources, baseline/reference commands and logs, origins, negative checks and
independent review. Its SHA-256 is
`99d0ce8ef750d8b631ca63ce43f0ddc57cbbc739e3674c29c5436569daa44659`.
[files.json](files.json) records 13 outer members; the embedded member manifest
records 25 files in the nested evidence archive. Root packaging rehashed all
members. Executable libraries are identified by hash and excluded from the archive.

The executable helper files in the root checkout match the reviewed source
hashes exactly. The root harness guide expands the handoff's documentation with
the PBS coverage findings. Packaging does not rerun a compiler or parser.
