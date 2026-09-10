# Allocation assertion audit

The original Expat suite failures remain failures. This separate diagnostic raises
only numeric allocation retry ceilings to 512, preserving every assertion. The
frozen consumer-contracts library completed all 720 contexts: 644 passed, 76
failed, and 51 of 60 tests passed all twelve contexts.

- Fifty tests need more allocation attempts than their original ceilings permit.
- One also exposed missing DTD default whitespace, fixed in consumer-contracts.
- Six assert Expat-specific realloc, string-pool, or reset-cache schedules.
- One exposed a recoverable NO_BUFFER error, fixed in the later API recovery layer.
- One requires unsupported inline parameter entities.
- One injects failure at allocation 12 assuming child creation has completed;
  Oriole exhausts it during construction. Its no-fault input also uses inline
  parameter entities.

The compressed audit contains all original source locations, ceilings, reasoning,
and per-context outcomes. The exact overlay, source/library hashes, compiler
command, complete results and assertion logs are retained. Separate allocator
counter probes explain the realloc/cache expectations; their counters include
setup and retries. These diagnostic passes do not replace the unchanged suite
results or claim compatible allocation schedules.
