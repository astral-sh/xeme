# Consumer evidence before the version correction

This archive preserves the complete normal `9cc87e9b` and profile-guided `bb43c2fc`
consumer handoffs. Both were built before the version-consistency change at
`4b11ace`. Their original manifests, raw reports, test output, comparison scripts,
source identities and independent review remain intact inside `evidence.tar.gz`.
`files.json` records every archived member and both original handoff manifests.

Both libraries record 4,323 passing and 417 failing original API configurations.
All six native C callback/allocation processes pass, including 327 selected
allocation scenarios per linkage. Four CPython configurations per build each
report 803 tests, 31 skips and two text-fragmentation failures with strict exit 2;
the separate two-test semantic checks pass. These are distinct gates.

The normal C interface's W3C scan records 4,962 mandatory passes, 960 mandatory
failures and 81 optional observations, matching reference acceptance. Of the
failures, 954 reflect Fifth Edition name fixtures rejected under the deliberate
Fourth Edition C policy. Earlier 5,916/6 results describe the earlier naming policy.
The older optimized handoff did not rerun W3C; its normal counterpart identifies
the runtime and policy used by that scan.

The detailed API classification distinguishes actual allocation costs from
Expat-specific allocation schedules, numeric version consistency, diagnostic and callback
differences, and resource limits that also prevent the reference from passing
under the unchanged harness bounds. All 417 original failures remain visible.
The final version-consistent builds have separate validation evidence.
