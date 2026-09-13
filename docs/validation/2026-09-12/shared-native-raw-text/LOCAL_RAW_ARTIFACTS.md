# Retained local raw artifacts

These directories are retained on the development host. They are **local evidence locations, not public download URLs**. The compact report copies summaries, complete condition tables and readbacks; full worker output stays here. Public raw hosting has not been assigned.

## initial native complete raw worker JSON, stdout/stderr, schedule and samples

Local directory: `/tmp/oriole-core-owned-text-raw-study/native/native-screen`.

Copied binding: [measurements/initial/native/review.json](measurements/initial/native/review.json).

- Local file `/tmp/oriole-core-owned-text-raw-study/native/native-screen/preflight.json`; SHA-256 `ff92aa6a82eb556bbfec3dfb5c33d7de5bb6ccbcc949faebac5ee0d9457379d4` (from its completed readback).

- Local file `/tmp/oriole-core-owned-text-raw-study/native/native-screen/results.json`; SHA-256 `9500ec83f83c8e3cfe4df8b05f6ea9223b27e5b771afff5b290be89396c0a610` (from its completed readback).

## initial python complete raw worker JSON, stdout/stderr, schedule and samples

Local directory: `/tmp/oriole-core-owned-text-raw-study/python/screen`.

Copied binding: [measurements/initial/python/normal-review.json](measurements/initial/python/normal-review.json).

- Local file `/tmp/oriole-core-owned-text-raw-study/python/screen/preflight.json`; SHA-256 `1b1e1bf1512254f1317669d53277d31afc19f365c525bb615ed53d23127b0cb9` (from its completed readback).

- Local file `/tmp/oriole-core-owned-text-raw-study/python/screen/results.json`; SHA-256 `dec6ec4b94a4e49d5c2f3bf0016bf44a35b902367624e18218279f151ca591a4` (from its completed readback).

## confirmation native complete raw worker JSON, stdout/stderr, schedule and samples

Local directory: `/tmp/oriole-core-owned-text-raw-confirmation/native/native-screen`.

Copied binding: [measurements/confirmation/native/review.json](measurements/confirmation/native/review.json).

- Local file `/tmp/oriole-core-owned-text-raw-confirmation/native/native-screen/preflight.json`; SHA-256 `082baf6ec7ac0c5eca0ebb03060cf332142febb854f020d7ea3c801d470e2eca` (from its completed readback).

- Local file `/tmp/oriole-core-owned-text-raw-confirmation/native/native-screen/results.json`; SHA-256 `022f487ce0253e1f1cfd77ad7f76c231c42cbca6820db0a71f29836018ecd3b5` (from its completed readback).

## confirmation python complete raw worker JSON, stdout/stderr, schedule and samples

Local directory: `/tmp/oriole-core-owned-text-raw-confirmation/python/screen`.

Copied binding: [measurements/confirmation/python/normal-review.json](measurements/confirmation/python/normal-review.json).

- Local file `/tmp/oriole-core-owned-text-raw-confirmation/python/screen/preflight.json`; SHA-256 `0978776556a0050507144d44a8b6c9361fd54c50a0b592f5480d025c70be7081` (from its completed readback).

- Local file `/tmp/oriole-core-owned-text-raw-confirmation/python/screen/results.json`; SHA-256 `48bd52e48545743698ec5cb12340a73c51534451968768d41aefbfa3e88e906e` (from its completed readback).

## original API raw tests, C consumer outputs, origins and original manifests

Local directory: `/tmp/oriole-core-owned-text-raw-correctness/api-native`.

Copied binding: [qualification/api-c-readback.json](qualification/api-c-readback.json).

## full strict rendered outputs, consumer origins and build commands

Local directory: `/tmp/oriole-core-owned-text-raw-correctness/strict-cpython`.

Copied binding: [qualification/strict-semantic-readback.json](qualification/strict-semantic-readback.json).

## both complete engine rows, child outcomes, catalog and progress logs

Local directory: `/tmp/oriole-core-owned-text-raw-correctness/w3c`.

Copied binding: [qualification/w3c-saved-readback.json](qualification/w3c-saved-readback.json).

## preserved failed test fixture attempt 1, source/preparation and raw build logs

Local directory: `/tmp/oriole-core-owned-text-raw-attempt01`.

Copied binding: [build/fixture-corrections/test-fix.json](build/fixture-corrections/test-fix.json).

## preserved failed test fixture attempt 2, source/preparation and raw build logs

Local directory: `/tmp/oriole-core-owned-text-raw-attempt02`.

Copied binding: [build/fixture-corrections/allocation-test-fix.json](build/fixture-corrections/allocation-test-fix.json).

## allocation-retry original/adapted test sources and compiled diagnostic provenance

Local directory: `/tmp/oriole-core-owned-text-raw-allocation-retry`.

Copied binding: [qualification/allocation-retry/report.json](qualification/allocation-retry/report.json).

## buffer-tail original/adapted test sources and compiled diagnostic provenance

Local directory: `/tmp/oriole-core-owned-text-raw-buffer-tail`.

Copied binding: [qualification/buffer-tail/report.json](qualification/buffer-tail/report.json).

The full raw directories for the additional experiment readbacks are also retained beside their original `/tmp/oriole-<candidate>-study` or `-confirmation` reports. Their copied readbacks preserve the original artifact paths and hashes. No raw archive or compiler binary was copied.

## Current-source ASan raw artifacts

`/tmp/oriole-core-owned-text-raw-asan-study` is a retained local path, not a public download URL. Sanitizer binaries and initial/final corpus contents remain there; compact reports, compiler proof and all 22 logs are copied in [qualification/asan/](qualification/asan/).
