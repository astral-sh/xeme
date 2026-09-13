# Converter and parser-family sanitizer campaigns

Both campaigns completed without a sanitizer finding, assertion failure, timeout,
or saved failure input. Each replayed its saved initial corpus before a ten-minute
campaign with seed `20260910` and a maximum input size of 65,536 bytes.

| Target | Initial inputs | Campaign executions | Elapsed seconds | Peak RSS |
| --- | ---: | ---: | ---: | ---: |
| `ffi_family` | 1,774 | 1,165,436 | 601.146 | 456 MiB |
| `multibyte` | 1,963 | 2,564,903 | 601.118 | 476 MiB |

The family target exercises parent/child construction, parsing, reset and free
order, custom allocation failures, DTD children, and content-model ownership. The
converter target exercises two-, three-, and four-byte conversion, incremental
input, callback re-entry guards, suspension, abort, allocation failures, and
release ownership. Both use valid parser handles and assert that their custom
allocator has no live allocations when an input finishes.

## Reproduction and provenance

[manifest.json](manifest.json) records exact build and run commands, CPU affinity,
elapsed times, final libFuzzer statistics, and SHA-256 hashes. `source.tar.gz` is
the source snapshot captured before either build; `source.json` hashes every
captured file. Source hashes matched after the builds and campaigns. Binary hashes
also matched after both campaigns. Later packaging or documentation changes are
outside this snapshot.

Each target directory contains compressed, unedited build/replay/campaign logs,
the initial corpus and its hash manifest, and hashes of the resulting corpus.
The complete final corpora and compressed executables remain at the local paths
recorded in the manifest. Executables are not included in this repository.

The builds used AddressSanitizer with the installed Clang 18 runtime and Ohm's
experimental defaults disabled. `ASAN_OPTIONS=detect_leaks=0` disabled
LeakSanitizer because it cannot operate under this environment's tracing setup;
the allocation-balance and callback-release assertions remained enabled.

These bounded runs provide evidence about the recorded inputs and execution
paths. They do not establish that the implementation is free of defects.
