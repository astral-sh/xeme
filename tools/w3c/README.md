# W3C XML acceptance corpus

These Unix harnesses test XML 1.0 Fifth Edition and Namespaces 1.0 acceptance using
the W3C XML test suite. For a nonvalidating parser, they treat valid and invalid
documents as required accepts, not-well-formed documents as required rejects,
and `error` cases as optional observations. The native gate applies the Fifth
Edition correction described below. XML 1.1 and
older-edition-only cases are excluded, with the reasons recorded in the results.

Use the [2013 suite](https://www.w3.org/XML/Test/) from the
[pinned mirror](https://github.com/lddubeau/xml-conformance-suite/tree/3fb4516daedd0ac736249a383733883b95fcdbb2).
Fetch that revision and pass `packages/test-data/xmlconf`, including its original
`xmlconf.xml`. The harness corrects and records a known `xml:base` typo in that
catalog. Mirror data carries MIT AND W3C-19980720 licensing; see the mirror's license.
We have not verified the mirror against the official release tarball.

## Native Rust conformance gate

`native.py` runs the safe Rust parser directly through the `w3c` example, with
Fifth Edition names, the namespace mode specified by each catalog case, and
default native resource limits. Python reads the catalog metadata; the Rust runner
parses the test documents. No reference parser library is required.

```console
cargo build --release --locked -p xeme --example w3c
python3 -I -S tools/w3c/native.py --suite /path/to/xmlconf \
  --runner target/release/examples/w3c --output /tmp/xeme-w3c-native
```

Use a new output directory. CI runs this command in a separate job and retains
reports even when it fails. The pinned selection contains 6,003 rows
across chunks of 1, 7 and 4,096 bytes, including 81 optional `error` observations.
Both `valid` and `invalid` cases must be accepted: `invalid` means a DTD validity
error in an otherwise well-formed document, which a nonvalidating parser does
not reject. Every `not-wf` case after the edition correction below must be
rejected. Optional `error` cases allow either outcome, but still must run
successfully.

Every required outcome must match. `native-corpus.json` pins the selected cases
and suite bytes; missing rows or modified inputs fail the gate. Resolver failures,
worker failures, and timeouts also fail it, including in optional cases.
Resource-limit and allocation failures are inconclusive and fail the gate, even
for cases expected to be rejected. The worker has a
1 GiB address-space ceiling and a 120-second deadline. The resolver only reads
files inside the suite, with 2 MiB per file, depth 32 and at most 1,024 requests.
The parser itself performs no I/O.

This gate checks acceptance, including external entities and incremental input.
It does not validate DTD content models or compare canonical output. The C
interface uses Fourth Edition name rules and has separate W3C checks below.

### Fifth Edition expectation correction

The catalog marks `rmt-e2e-38` as `not-wf` because an external entity declares
version `1.1`. Its content uses only XML 1.0 features.
[W3C erratum E10](https://www.w3.org/XML/xml-V10-4e-errata#E10) reverses the older
E38 rule and permits processing such entities as XML 1.0. The native gate
therefore requires this case to be accepted at every chunk size.

`native-corpus.json` records this correction. Reports retain the original catalog
descriptor and results against its expectations. All 6,003 rows must still run.
The C-interface regression gate uses the original catalog and its own baseline.

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

Reports include source and library hashes, external-entity outcomes, chunk sizes,
status, error code, and byte index. Worker failures and timeouts also save the
last case reached. The command exits nonzero for any required mismatch, resolver
failure, or worker failure in either library. It compares acceptance, not
canonical output.

## C-interface regression gate

CI uses `tools/compatibility.py w3c` with the pinned suite and reviewed baseline.
It checks every selected case and its source bytes, compares acceptance and
external-entity outcomes, and rejects missing results, resolver failures, or new
differences. Existing failures remain in the reports, so passing CI does not mean
passing the conformance suite. See the [compatibility guide](../../docs/compatibility.md)
for known differences.
