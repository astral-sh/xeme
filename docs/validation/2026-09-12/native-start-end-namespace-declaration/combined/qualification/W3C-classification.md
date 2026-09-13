# Shared W3C catalog mismatches

Both implementations have identical acceptance on all 6,003 rows and the same 960 mandatory catalog mismatches. These are **shared failures, not 960 Oriole-only acceptance failures**.

- **954 rows** are explicitly Fifth Edition name-profile fixtures (318 files across three chunk sizes). Both C interfaces select Fourth Edition name rules. The catalog/name-profile classification is preserved; it is not a separate causal proof for every rejection.
- **Six rows** are two files across three chunk sizes: `rmt-e2e-38` (`eduni/errata-2e/E38.xml`) references an external entity with an XML 1.1 text declaration, and `hst-lhs-007` (`eduni/misc/007.xml`) combines a UTF-8 BOM with an ISO-8859-1 declaration. Both implementations accept these catalog not-well-formed cases.

The [current readback](w3c/readback.json) verifies the unchanged catalog and all qualified declaration rows. The reference has zero changes from raw-view and current acceptance is identical, preserving the [earlier detailed catalog/source classification](https://github.com/astral-sh/oriole/blob/5d983f7e09a095fc6c9a94c6ce05ef5a8ec95c8d/docs/validation/2026-09-12/shared-native-raw-text/qualification/W3C-classification.md). The 48 Oriole diagnostic corrections change no acceptance outcomes. Remaining Oriole-versus-Expat differences are 198 error codes and 1,695 byte indices. The raw W3C exit remains 1; no conformance failure is converted to a pass.
