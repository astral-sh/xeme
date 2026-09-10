# First input-buffer reservation study

We reserve a bounded first block on the first nonempty append to UTF-8 decoded storage and raw callback context. The buffers still allocate lazily, respect their configured reservation bounds and use the selected allocator. Existing capacity keeps ordinary geometric growth.

On the independently frozen event-output source, the combined change resolves 210 original allocation-retry configurations and introduces 12 failures that require an incidental buffer reallocation. All original assertions remain: 4,311 passing and 429 failing. The independent reset probe confirms exact semantic metadata, zero failed realloc calls on successful parses and zero selected live allocations after free. These counts belong to this isolated source; later ATTLIST composition is measured separately.

The native project screen is mixed and near parity overall: 1.00505× geometric speedup, with 14 of 24 condition medians improving. No general throughput gain is claimed. Tiny UTF-8 input retains 2,032 additional selected bytes, and all component memory tradeoffs, negative timing conditions and original failed assertions remain explicit.

The archive contains all final source hashes, separate component builds, 328 core/adapter Rust checks, strict Clippy/formatting, complete API observations, custom-encoding and streaming probes, six native C sanitizer runs and independent source/allocation review. Runtime binaries are represented by hashes. Every included author-manifest entry is verified before packaging.
