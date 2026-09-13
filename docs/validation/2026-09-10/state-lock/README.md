# Allocation-free shared-state lock

The external-value channel needs exclusive access to shared output across related
parsers. A platform mutex may allocate outside the selected C memory suite. TryLock
makes one atomic acquisition attempt and returns immediately on contention, without
an OS mutex, allocation, spinning, or poisoning. Guard destruction releases access,
including during unwinding. The safe parser can use this primitive without unsafe code.

Independent and root unsafe-code reviews confirm the unique-guard, acquire/release,
value-lifetime, and Send/Sync invariants. The guard's exclusive-borrow marker prevents
sharing a guard for a non-Sync value. A forgotten guard only makes the lock unavailable.
The primitive permits transferring a guard across threads, since it has no OS mutex
owner-thread requirement.

All 26 storage tests, one negative trait doctest, formatting, and strict Clippy pass.
Focused tests check cross-thread contention and visibility, guard transfer, unwinding,
exactly-once value destruction, and selected-allocation failure followed by reacquisition.
The combined external-value channel remains a separately reviewed integration layer.
The independent review file covers that larger isolated candidate too; this PR adopts
only the lock primitive, with its exact integrated source hashes recorded separately.
