# Large-input reference controls

We ran the original large-buffer and stream tests against pinned Expat under the same 3-second, 1 GiB address-space and 768 MiB RSS settings used by the adapted API harness. All 12 one-gigabyte GetBuffer configurations fail their required-allocation assertion; both active two-gigabyte stream configurations time out. These controls do not turn Oriole's corresponding failures into passes. Oriole also retains its documented 512 MiB allocation and 256 MiB input ceilings.

Forty bounded-prefix runs show constant streaming storage through approximately 256 MiB: Oriole uses 29 selected allocation calls and 90,198 peak requested bytes; Expat uses 10 calls and 15,984 peak bytes. Both release all blocks. Oriole rejects the next feed when cumulative input crosses its ceiling. Shared-host timing is noisy and does not establish two-gigabyte throughput within the original timeout for either implementation.

`evidence.tar.gz` preserves the original test bodies, reference outcomes, source/library identities, every prefix measurement and the allocation-schedule review. `files.json` verifies the archive and each member. No runtime code, limits or original test assertions change in this package.
