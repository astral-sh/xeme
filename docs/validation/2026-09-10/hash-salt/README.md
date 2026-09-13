# Caller hash salts

Both Expat salt setters now mix caller bytes into the randomized hash state. Reconfiguration prepares every replacement table before moving entries, preserving old tables and salt on allocation failure. Initial namespace bindings, declaration order, reset and newly created children retain the configured behavior. Existing children keep their independently owned table state.

The isolated draft passes 114 affected-package Rust checks, formatting, strict Clippy, and all 48 selected upstream configurations against pinned Expat 2.8.4. This fixes the 12 previously failing hash-salt configurations. Tests inject failure at all seven reconfiguration allocations and exercise allocator routing and reentry. The frozen debug library is `f8f4814065cca603430421b6f9dc06b7a1d9541f277f6775d72b7b9c25fe53f4`; this is not a release-performance artifact.

[evidence.tar.gz](evidence.tar.gz) retains exact source/patch hashes, commands, original observations, allocation tests, and independent review. [files.json](files.json) verifies the archive and every member. Integrated release validation is recorded separately with the combined security controls. No performance improvement is claimed for hash configuration.
