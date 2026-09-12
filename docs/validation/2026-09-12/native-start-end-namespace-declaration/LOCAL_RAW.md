# Retained raw evidence

These are local machine artifacts, not public download links. Exact copied reports and their source hashes appear in [COPY_INDEX.json](COPY_INDEX.json); complete reader inputs remain bound in those reports.

- Initial native and Python: `/tmp/oriole-native-start-end-namespace-declaration-study/{native,python}/`.
- Separate confirmation: `/tmp/oriole-native-start-end-namespace-declaration-confirmation/{native,python}/`. Native epochs each retain 672 workers; Python epochs each retain 576, including fresh preflights. No epoch is pooled. The [six-row README reconstruction](combined/native-confirmation/readme-benchmarks.json) additionally pins its 126 exact raw workers.
- Full API/C/strict/semantic records: `/tmp/oriole-native-start-end-namespace-declaration-correctness/`. Original API rows, strict method maps and readbacks are copied; full stdout/stderr and all build receipts remain local.
- Supplementals and diagnostics: `/tmp/oriole-native-start-end-namespace-declaration-{allocation-retry,buffer-tail,diagnostics}/`. All W3C/callback rows, both supplemental reports/logs and exact control edits are copied. Failed original outcomes stay failed.
- Existing harnesses remain in the pinned checkout and `/tmp/oriole-final-consumer-validation/tools/w3c.py`. Sources, input paths/hashes, compiler identities and worker commands are recorded by the copied provenance. No corpus, extension, library or compiler cache is bundled.
- Standalone raw roots remain `/tmp/oriole-direct-source-start-{study,confirmation,correctness}/`, `/tmp/oriole-default-namespace-slot-study/` and `/tmp/oriole-declaration-bootstrap-grammar-{study,diagnostics}/`; [STANDALONE.md](STANDALONE.md) keeps their original source/result scopes separate.

This staging step executes no parser, compiler, benchmark or qualification target.

- Completed ASan raw corpora, binaries and instrumentation symbol logs remain in `/tmp/oriole-native-start-end-namespace-declaration-asan-study/`; the [readback](combined/qualification/asan/readback.json) pins them. Compact compiler evidence, all replay/exploration logs and the complete report are copied.
- Current CI raw job logs remain in `/tmp/oriole-native-composition-ci/ci/`; [the copied CI receipt](combined/qualification/ci/readback.json) includes their hashes and the terminal run/jobs snapshots. Only the two focused Miri logs are copied. [CI run](https://github.com/astral-sh/oriole/actions/runs/34722470510).
- PBS full build log (`pbs/validation/pbs-build.log`, SHA-256 `474be3852bd7a1a3fc90be6759dcc61052d94deb2eca1bbc66174a75e6359705`) and raw job log (`pbs/job-103630784165.log`, SHA-256 `7411e0e2be75ec365a466d3159aa6b034c574c66f96bb6afbe6faddfd4fdfa73`) remain below `/tmp/oriole-native-composition-ci/`. Compact installed manifests, provenance, XML/glibc/custom/structure/TLS logs, artifact metadata and the preserved saved-reader first failure are copied. [PBS run](https://github.com/astral-sh/oriole/actions/runs/34722487535), validation artifact ID `10306863717`, ZIP SHA-256 `e133b26022858678c957e024dbe9eef4ed70c6ead02dfa8d0865f22db3c5446b`. The distribution was not downloaded or executed locally; installed checks ran in CI.

Earlier preparation/performance snapshots may say pending or unadopted. They are preserved historical records; [final-adoption.json](final-adoption.json) records the terminal decision without rewriting them.
