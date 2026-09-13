# Explicit PBS target bundle validation

The normal generic, normal x86-64-v3 and PGO x86-64-v3 local bundles passed. The
generic target remains the default. The v3 product uses the generic Rust ABI
target with an explicit `target-cpu=x86-64-v3` requirement.

The saved build logs reproduce all 12 workspace compiler vectors: three per
normal bundle and three in each PGO phase. They retain O3, ThinLTO, one codegen
unit, PIC and unwind behavior, with no hidden CPU or experimental rustc options.
The PGO bundle retains both sets of 288 generated training observations and the
exact selected static archive identity. These observations are not benchmarks.

The packet includes 12 focused Python tests, pinned-overlay fixtures, Ruff, ty,
workflow checks and the generic-ISA GCC CPU/OS guard. It preserves failed setup
attempts, including the initial sibling-import error before compilation.

These are local `cargo +ohm -Zohm-defaults=no` builds. Production CI uses plain
stable Cargo. The new explicit v3 PBS distribution and installed target/manifest/
archive checks remain pending, along with its complete XML and glibc 2.17 gates.
Existing API ceiling differences and strict callback failures remain; this
packet makes no new performance or production-readiness claim.

[Report](report.json) lists each bundle and its limitations. The
[index](index.json) binds every member of [the saved records](evidence.tar.gz)
to its original byte count and SHA-256; all members were read back after packing.
`original_path` is the historical machine location. Archive member paths are
relative. Libraries, compilers, profile blobs and machine Cargo configuration
are excluded with their byte identities retained. The saved assembler reuses
the repository's bundle verifier and the existing deterministic packet method;
it does not execute targets.
