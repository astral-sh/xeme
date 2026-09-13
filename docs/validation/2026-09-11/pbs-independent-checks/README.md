# PR124 PBS distribution review

The selected streaming runtime built and ran on **glibc 2.17**, including **1,024 threaded XML parses**. The distribution validator passed. The workflow remains failed because both installed XML runs preserve the same two callback assertions; the glibc step failed when its subsequent XML subprocess returned exit 2.

[Run 34582636093](https://github.com/astral-sh/oriole/actions/runs/34582636093) built PR124 head `4d603c7487ca93cc73dbd4acfe6fa5714ffc2151` through merge checkout `a1068cba2ff0ecbd5718e7e5d704e1e7ebfb329a`, against PBS `a4553880293fe9d1bb62747d34ab0e5121d3554f`. This was the default **normal ThinLTO build**, using stable Rust 1.98.1 / LLVM 22.1.8, PIC, unwinding and one codegen unit. It was not a PGO distribution.

| Gate | Result |
| --- | --- |
| Build and archive validator | Passed |
| PBS custom suite | 22 run: 17 passed, 5 skipped |
| Installed parser identity | `oriole_compat_2.8.4` |
| Host XML suites | Initial 802 tests, 2 failures, 12 skips; built-in retries add 4 methods and repeat the 2 failures |
| glibc 2.17 identity and threaded parsing | Passed; 1,024 parses |
| glibc 2.17 XML suites | 802 tests, 2 failures, 13 skips |

The failures are `test.test_pyexpat.BufferTextTest.test1` (the `2\n3` character data arrives together) and `test.test_sax.CDATAHandlerTest.test_handlers` (the callback includes surrounding newlines). Raw assertions are retained in `validation.zip` and `report.json`. The host harness enables more test resources and automatic retries; the old-glibc script uses the default resources. The aggregate logs do not identify every skipped or expected-failure method, so this report does not infer a per-method 802-test map.

All 69 bundle source pins match PR124; 64 overlap and exactly match the selected streaming benchmark source inventory. The remaining bundle inputs are licenses and the CPython cleanup backport. The downloaded distribution contains the exact recorded `libexpat.a` (`b0a01875…`), matching notices and byte-identical bundle manifest. Its executable and `libpython` have maximum GLIBC symbol version 2.17 and only an unversioned weak undefined `__wrap___cxa_thread_atexit_impl`; neither dynamically depends on system libexpat. The installed consumer includes the checked CPython child-parser cleanup backport.

Distribution SHA-256: `0afb79be550e2a2a49e9bd02de0f090c26ba8f6919065303a51e802bec289551`. Both original GitHub artifact ZIP hashes were independently checked against the API digests, then their payloads were checked. `report.json` records full hashes, compiler vectors, linkage and artifact origins; `audit.py` reproduces the saved-data assertions.

This review downloaded and inspected saved bytes only. It did not rerun a compiler, parser, installed interpreter, container or workflow. The original failures remain intact. Compact publication inputs are listed in `compact-evidence-files.json`; distribution and executable binaries are retained locally and explicitly excluded from that list. Normal CI recorded the compiler version and actual invocations, but did not retain a compiler-executable hash.

## Saved evidence

The [report](report.json), [compact evidence archive](evidence.tar.xz), and [member hashes](archive-members.json) retain this normal-build baseline separately from the [fresh local PGO bundle](../pbs-pgo-bundle/). The archive includes the original 13-member GitHub validation ZIP, complete workflow log, exact metadata and ELF inspections, fixture scripts, and the executed saved-data auditor. Full audit replay requires the pinned Git checkout and the original distribution ZIP/extracted binary inputs described in `report.json`; those binaries are excluded from publication. The auditor never executes them.
