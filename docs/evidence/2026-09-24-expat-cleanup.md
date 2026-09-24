# Expat 2.8.5 follow-up cleanup

These changes follow the [2.8.5 compatibility update](2026-09-24-expat-2.8.5.md).

## API failure baseline

The API baseline now allows 495 failing configurations, down from 509. All twelve
configurations of `test_nsalloc_long_element` and the two whole-buffer
configurations of `test_alloc_realloc_subst_public_entity_value` must pass.
The remaining allowances and their assertions are unchanged.

Both recorded 2.8.5 API matrices pass the tighter gate with 495 known failures
and no remaining improvements. The earlier 2.8.4 run also passed these fourteen
configurations; this cleanup records an existing improvement.

For each removed allowance, a validation probe changed the recorded successful
row back into its former assertion failure, including the log and result counts.
The old baseline accepted all fourteen injected regressions. The new baseline
rejected every one. The local report is
`.cache/review-stack/cleanup-baseline-check.json`.
