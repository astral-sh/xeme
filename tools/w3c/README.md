# W3C XML acceptance corpus

This Unix harness compares nonvalidating XML 1.0 Fifth Edition and Namespaces 1.0
acceptance against the original catalog in the W3C XML test suite. It records
valid and invalid documents as required accepts, not-well-formed documents as
required rejects, and `error` cases as optional observations. XML 1.1 and
older-edition-only cases remain listed with their selection reasons.

The [recorded run](https://github.com/astral-sh/oriole/tree/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/validation/2026-09-10/w3c) uses the
[2013 suite](https://www.w3.org/XML/Test/) from the
[pinned mirror](https://github.com/lddubeau/xml-conformance-suite/tree/3fb4516daedd0ac736249a383733883b95fcdbb2).
Fetch that revision and pass `packages/test-data/xmlconf`, including its original
`xmlconf.xml`. The harness records the original catalog's known `xml:base` typo
and correction; it does not consume the mirror's cleaned catalog. Mirror data
carries MIT AND W3C-19980720 licensing; see its license and the retained source
archive. We have not verified the mirror against the official release tarball.

```console
python3 tools/w3c/run.py --suite /path/to/xmlconf \
  --library /path/to/liboriole_expat.so \
  --reference /path/to/libexpat.so --output /tmp/oriole-w3c
```

Use a new output directory. Each library runs in a separate process with a 1 GiB
address-space ceiling and a 120-second deadline. The local-only resolver stays
inside the suite, limits file size to 2 MiB, depth to 32, and requests to 1,024.
Use the pinned, trusted corpus: catalog loading happens before the workers and
file-size checking follows the read. The parser itself performs no I/O.

All source/library hashes, callbacks' child outcomes, chunk sizes, final status,
error code and byte index are preserved. Resolver failures are inconclusive and
receive no conformance credit. Worker failures and timeouts persist a summary
and last-case progress. The command exits nonzero for any required mismatch,
inconclusive case, or worker failure in either library; there are no waivers.
This checks acceptance only. It does not compare canonical output or establish
complete XML or Expat conformance.
