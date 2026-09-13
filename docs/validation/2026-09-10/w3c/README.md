# W3C XML acceptance checkpoint

The [portable runner](../../../../tools/w3c/) reads the original catalog from a
[pinned mirror](https://github.com/lddubeau/xml-conformance-suite/tree/3fb4516daedd0ac736249a383733883b95fcdbb2)
of the [2013 W3C suite](https://www.w3.org/XML/Test/). The source archive, license,
exact revision, every input hash, and known catalog URI correction are retained.
The mirror has not been cryptographically compared with the official archive.

Of 2,585 catalog descriptors, 2,001 apply to XML 1.0 Fifth Edition/Namespaces 1.0.
Each runs with one-, seven-, and 4,096-byte chunks. Required checks comprise 957
valid/invalid accepts and 1,017 not-well-formed rejects, or 5,922 observations.
The other 27 `error` descriptors produce 81 optional observations without pass
credit. The 584 version/edition exclusions remain in the catalog report.

| Implementation | Required passes | Required failures | Inconclusive |
| --- | ---: | ---: | ---: |
| Oriole after namespace, XML version, and foreign-DTD fixes | 5,901 | 21 | 0 |
| Expat 2.8.4 | 4,962 | 960 | 0 |

The command exits 1 because mismatches remain. Neither library has a resolver
error, worker signal, or timeout. Most reference failures concern the older XML
name repertoire; Oriole implements the Fifth Edition's broader ranges. These
are corpus acceptance checks, without canonical-output comparison.

Oriole's seven remaining failed documents (three chunk sizes each) are:

- `invalid--005`, `invalid--006`, `invalid-not-sa-022`, `valid-not-sa-003`, and
  `rmt-e2e-14`: unsupported parameter-entity declaration/delimiter composition.
- `rmt-e2e-38` and `hst-lhs-007`: reference Expat also accepts these corpus rejects.

This checkpoint predates the later external-content decoder change. The results
must not be described as a complete conformance pass. Independent review checks
catalog selection, namespaces, opaque child contexts, local-only resolution,
source identities, limits, and failure reporting. Synthetic resolver/timeout
fixtures are in the separate combined validation evidence package.
