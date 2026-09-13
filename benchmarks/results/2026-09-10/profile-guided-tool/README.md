# Optional PGO tooling validation

`pgo-tool.patch` adds five files under `tools/pgo/`; it changes no parser runtime, allocator, limit, or CI configuration. The tool implements an offline generate/train/merge/use build with fresh profiles per invocation, LLVM version matching, input/tool/source hashes, raw command logs, an exclusive output lock, and generated replay verification.

Twelve tool tests, Ruff and ty pass. Four actual x86_64 Linux build runs are retained separately: the first on frozen8d, the isolated-training followup on final9cc, the path-with-spaces run on9cc, and the final reviewed tool on9cc after compiler-wrapper and failed-process-group cleanup corrections. The last run also proves that an explicitly empty CARGO_ENCODED_RUSTFLAGS overrides inherited RUSTFLAGS. Each run completed 288 instrumented and 288 optimized parses with identical selected callback records; final child interpreters use -I -S and verify XML_Parse's actual loaded library with dladdr.

The 12 generated fixture hashes exactly match the earlier study; no held-out project inputs enter training. Element model trees are freed but not serialized in these digests. This build check does not replace full consumer, compatibility or security validation, and no new performance/adoption claim is made here. macOS has not been executed.

Each compressed evidence archive preserves manifests, logs, input bytes, training records and raw/merged profiles. Binaries remain at the local paths in summary.json with hashes; they are intentionally not duplicated. The initial safe-path import failure is preserved as a raw launch log. The original `/tmp/oriole-pgo-study-handoff` remains unchanged.

Independent final source review is included and reports no outstanding source blockers. It does not claim to have rerun the tool. The final build uses empty wrapper environment overrides so configured Cargo wrappers cannot substitute a compiler, and a failed command terminates any remaining process group before releasing its lock.

## Repository integration

The root's repository-wide ty check caught collisions between the generic `build`/`corpus` imports and existing modules. The integrated tool uses a `pgo` package, qualified imports and an explicit path bootstrap for direct and isolated execution. The generator and parser runtime are unchanged. All 12 tests and the canonical full-repository Ruff/ty checks pass. The integrated trainer reproduces all 288 optimized-build records exactly and verifies the same loaded library origin. `integration.json` records the changed script hashes; the original four build manifests remain unchanged and do not claim to use the new import paths. The CI job performs a fresh full build with the integrated scripts.
