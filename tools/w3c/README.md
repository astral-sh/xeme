# W3C XML acceptance corpus

These Unix harnesses check nonvalidating XML 1.0 Fifth Edition and Namespaces 1.0
acceptance against the original catalog in the W3C XML test suite. They record
valid and invalid documents as required accepts, not-well-formed documents as
required rejects, and `error` cases as optional observations. The native gate
applies the Fifth Edition expectation correction documented below. XML 1.1 and
older-edition-only cases remain listed with their selection reasons.

Use the [2013 suite](https://www.w3.org/XML/Test/) from the
[pinned mirror](https://github.com/lddubeau/xml-conformance-suite/tree/3fb4516daedd0ac736249a383733883b95fcdbb2).
Fetch that revision and pass `packages/test-data/xmlconf`, including its original
`xmlconf.xml`. The harness records the original catalog's known `xml:base` typo
and correction; it does not consume the mirror's cleaned catalog. Mirror data
carries MIT AND W3C-19980720 licensing; see the mirror's license.
We have not verified the mirror against the official release tarball.

## Native Rust conformance gate

`native.py` runs the safe Rust parser directly through the `w3c` example, with
Fifth Edition names, the namespace mode specified by each catalog case, and
default native resource limits. It judges acceptance against the specification's
Fifth Edition expectations. Test documents are parsed only by the native runner;
no reference parser library is required. Python reads the catalog metadata using
its standard XML parser, whose outcomes do not determine test-document acceptance.

```console
cargo build --release --locked -p xeme --example w3c
python3 -I -S tools/w3c/native.py --suite /path/to/xmlconf \
  --runner target/release/examples/w3c --output /tmp/xeme-w3c-native
```

Use a new output directory. CI runs this command as a separate CI job and
retains its reports even when it fails. The pinned selection contains 6,003 rows
across chunks of 1, 7 and 4,096 bytes, including 81 optional `error` observations.
Both `valid` and `invalid` cases must be accepted: `invalid` means a DTD validity
error in an otherwise well-formed document, which a nonvalidating parser does
not reject. Every `not-wf` case after the edition correction below must be
rejected. Optional `error` cases allow either outcome, but still must run
successfully.

There is no accepted-failure baseline. The checked-in `native-corpus.json` binds
the selected cases and suite bytes; missing rows or modified inputs fail the
gate. Resolver failures, worker failures and timeouts also fail it, including in
optional cases. Resource-limit and allocation failures are inconclusive and
fail the gate, including for cases expected to be rejected. The worker has a
1 GiB address-space ceiling and a 120-second deadline. The resolver only reads
files inside the suite, with 2 MiB per file, depth 32 and at most 1,024 requests.
The parser itself performs no I/O.

This gate checks standard acceptance, including external entities and incremental
input. It does not validate DTD content models, compare canonical output or
establish complete XML conformance. XML 1.1 and older-edition-only cases remain
excluded with recorded reasons. The C interface's Fourth Edition name rules and
its known W3C failures are evaluated separately below.

### Fifth Edition expectation correction

The catalog marks `rmt-e2e-38` as `not-wf` because an external entity declares
version `1.1`. Its content uses only XML 1.0 features.
[W3C erratum E10](https://www.w3.org/XML/xml-V10-4e-errata#E10) reverses the older
E38 rule and permits processing such entities as XML 1.0. The native gate
therefore requires this case to be accepted at every chunk size.

`native-corpus.json` binds this explicit correction and the corpus inventory.
The original catalog descriptor and raw-catalog failures remain in the reports.
The corrected expectation is mandatory: the case is neither excluded nor
allowed to fail, and all 6,003 rows still run. The C-interface regression gate
continues to use the original catalog and its separate reviewed baseline.

## C-interface acceptance comparison

```console
python3 tools/w3c/run.py --suite /path/to/xmlconf \
  --library /path/to/libxeme_expat.so \
  --reference /path/to/libexpat.so --output /tmp/xeme-w3c
```

Use a new output directory. Each library runs in a separate process with a 1 GiB
address-space ceiling and a 120-second deadline. The local-only resolver stays
inside the suite, limits file size to 2 MiB, depth to 32, and requests to 1,024.
Use the pinned, trusted corpus: catalog loading happens before the workers and
file-size checking follows the read. The parser itself performs no I/O.

All source/library hashes, callbacks' child outcomes, chunk sizes, final status,
error code and byte index are preserved. Resolver failures are inconclusive.
Worker failures and timeouts persist a summary and last-case progress. The command
exits nonzero for any required mismatch, inconclusive case, or worker failure in
either library.
This checks acceptance only. It does not compare canonical output or establish
complete XML or Expat conformance.

## C-interface regression gate

CI uses `tools/compatibility.py w3c` with the pinned suite and reviewed baseline.
It requires the complete selection and recorded source bytes, compares acceptance
and loaded-child outcomes, and rejects missing rows, resolver failures, or new
differences. The raw catalog failures remain in its reports; baseline agreement is
not a passing conformance suite. See the [compatibility guide](../../docs/compatibility.md)
for the current boundary. Historical reports apply only to their recorded source
and artifact hashes.
