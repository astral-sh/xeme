# Local Ohm PBS PGO bundle

The first local build and both existing C integration consumers passed. This is an **Ohm** result, not a production stable build or an installed PBS distribution result.

The bridge used fresh generated-only profiles with PIC, unwinding and ThinLTO. Each phase recorded three workspace compiler invocations and 288 generated parses; all generated callbacks agreed. The bundled static archive is byte-identical to the final use archive. Its native dependencies came from that exact compile, and the archive retained only weak references to the optional TLS destructor hook. The existing shared and exact-static C consumers passed without retries. The final PBS linker TLS treatment and glibc/thread compatibility still need installed-distribution validation.

[Summary](summary.json) records exact identities, scope, exclusions and independent reviews. [Evidence](evidence.tar.gz) retains frozen source, all earlier Python-check attempts, first build/training/profile records, manifests, raw C checks and both reviews. [Member index](members.json.gz) records every member hash and origin; all were read back. Compiled ELFs/static archives and target caches are excluded; selected artifact hashes are retained. Raw and merged profiles and generated input bytes are included.

The worktree was advanced from PR124 to PR125 only after all target sessions ended. The compiled C consumer is the earlier `tests/c/integration.c` at commit `4d603c7487ca93cc73dbd4acfe6fa5714ffc2151`, reconstructed from that exact Git blob. Historical before/after source pins and raw outcomes are unchanged. The newer nested-entity fixture is not part of these two runs.
