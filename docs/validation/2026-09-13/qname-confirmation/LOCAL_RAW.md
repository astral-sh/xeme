# Local raw evidence

The full second epoch is preserved at:

```text
/tmp/oriole-namespace-qname-only-confirmation
```

This checked-in directory contains exact copies of 45 files (1,329,522 bytes).
`COPY_INDEX.json` records each original path, destination path, size, and SHA-256.
`README.md`, this file, and `COPY_INDEX.json` are generated documentation rather
than copied experiment outputs.

## Raw workers and output

The following remain local and are not included in the checked-in report:

- `native/native-screen/worker-*.json`: all 672 native process outputs.
- `native/native-screen/preflight.json` and `results.json`: complete native
  preflight and timing records, including every sample.
- `python/screen/`: all 576 CPython worker specifications and compressed stdout
  and stderr files, plus full `preflight.json` and `results.json`.

The copied readers and summary retain SHA-256 references to these files. The
native and CPython readbacks reconstruct every worker, canonical output,
process median, paired ratio, and reported aggregate. They execute no parser
targets. Raw files may be unavailable after this devbox or `/tmp` is removed;
the checked-in summary and per-condition reports alone cannot reconstruct them.

## Frozen inputs and original epoch

The first epoch remains unchanged at:

```text
/tmp/oriole-namespace-qname-only-benchmark
```

The candidate's build record and normal library are under:

```text
/tmp/oriole-namespace-qname-only-study
```

The candidate library hash is
`24a7d8262675977fd358c20fde32b346d248e0495d59124bbe39cc74717b352e`.
The qualified library hash is
`c3e6533900cf0b1ec6b127fd173f7e25be210cb5afe23ad9963c84873f6ea025`.
The normal Expat control hash is
`7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.
Its historical `/tmp/oriole-pgo-study/` directory name does not indicate PGO:
this is the normal control.

`binding.json`, the original preparation records, and the CPython build manifest
identify all library, extension, source, compiler, driver, and corpus paths.
The confirmation reuses all six existing CPython extensions without rebuilding
them; their original build manifest still describes the original compilation.
Fresh preflights verify the actual loaded extensions and parser library origins.

## Controllers and exact copies

The copied controllers contain absolute local paths. They document the executed
commands and measurement rules; they are not a relocatable benchmark package.
The copied patch files show the changes from the first epoch: output paths,
reuse of the previously built candidate extensions, and the corresponding
readback checks. The measurement loops, seeds, rounds, and iteration counts are
unchanged. The original `preparation.json` and `binding.json` were copied exactly;
`confirmation-preparation.json` records the new preparation separately.

Native and CPython elapsed work ran sequentially on CPU 0. Preflights used CPUs
5 and 3, respectively; controllers, readbacks, and host monitoring used CPU 6.
The host counter files record shared-host activity and do not prove isolation.
All controller and monitor tool sessions were reaped after completion.

Raw CSV files preserve their original CRLF line endings, and logs preserve their
original trailing newlines. Do not normalize copied evidence: doing so changes
its hash and breaks the exact-copy provenance.
