# Current raw-view W3C result: compatibility match, failed catalog run

The unchanged Fifth Edition acceptance harness returned **1**. Each engine produced 6,003 rows: 4,962 mandatory passes, **960 mandatory mismatches**, 81 optional observations, and zero resolver-inconclusive rows. Both workers exited 0, and the supervisor reaped everything in 26.70 seconds. All raw rows, child outcomes, skips and mismatch lists remain intact in [the completed results](w3c/summary.json). The [saved readback](w3c-saved-readback.json) independently reconstructs the ordered rows and classifications.

The run compares current raw-view `e59d89d6` with pinned normal Expat `7a333bc8`. Acceptance agrees on all 6,003 rows. This is narrower than exact API agreement: 225 error-code rows, 1,695 byte-index rows and 21 child-outcome rows differ. It is not a successful conformance run or a canonical callback-output comparison. Accepted `invalid` cases follow the nonvalidating harness's acceptance rule; they do not establish validating conformance.

## The main scope mismatch is the XML name edition

The runner selects Fifth Edition catalog cases and skips earlier-edition-only cases ([selection and stated scope](../source/references/w3c.py#L238)). The current C constructor explicitly selects `NameRules::FourthEdition` ([raw source](../source/references/oriole_expat/lib.rs#L301)); the core documents that C policy and the separate broader Rust default ([NameRules](../source/references/oriole/names.rs#L39)). Local Expat likewise describes itself as Fourth Edition ([README](../source/references/expat/README.md.txt#L23)); its tokenizer uses the historical name tables and rejects four-byte UTF-8 name characters ([xmltok.c](../source/references/expat/xmltok.c#L147), [name checks](../source/references/expat/xmltok_impl.c#L72)).

| Original catalog category | Distinct files | Rows per engine | Recorded result |
| --- | ---: | ---: | --- |
| Explicit `EDITION="5"`, `eduni/errata-4e`, `valid` | 306 | 918 | Rejected, error 4 |
| Explicit `EDITION="5"`, `eduni/errata-4e`, `invalid` | 12 | 36 | Rejected, error 4 |
| `not-wf`, no explicit edition flag: two other cases | 2 | 6 | Accepted, error 0 |

Thus 954 failures are explicitly Fifth Edition fixtures. Their section metadata names the character/name productions: 307 files cite Appendix B, nine cite section 2.3, and one each cites productions [4] and [5]. The edition-policy mismatch is concrete; this grouping does not claim a separately proven causal trace for every rejection.

Representative fixture/source checks demonstrate the distinction:

- `eduni/errata-4e/ibm04v01.xml` uses U+02FE at a name start. Fifth Edition's broad range accepts it; the Fourth Edition start table does not.
- `140.xml` puts U+309A at a name start through an internal entity. Fourth Edition allows it only as a continuation, whereas Fifth Edition accepts it as a start.
- `141.xml` uses U+0E5C in a name continuation; `014.xml` uses U+017F; `ibm89n06.xml` uses U+0EC7. All are accepted by the Fifth Edition ranges and excluded by the relevant Fourth Edition table. The inherited comment in `014.xml` refers to older XML 1.0 wording; the catalog explicitly marks this copy Edition 5.

## Six remaining accepted `not-wf` rows stay separate

- `rmt-e2e-38` (`eduni/errata-2e/E38.xml`) references `E38.ent`, whose text declaration says XML 1.1. Both engines accept the parent and child in each of the three chunk sizes.
- `hst-lhs-007` (`eduni/misc/007.xml`) starts with a UTF-8 BOM but declares `encoding='iso-8859-1'`. Both engines accept it at all three chunk sizes. The existing catalog URI correction is recorded unchanged.

These are not name-edition failures and remain six mandatory mismatches. The 584 skipped descriptors also remain explicit: 310 earlier-edition-only and 274 XML 1.1. No cases were reclassified, waived or rerun. Source inspection and saved-data classification added no parser execution. The current source is uncommitted on base `2c4d216`; the readback binds its exact source, build, patch and library hashes.
