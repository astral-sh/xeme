# Improve parser performance

Use `hillclimb.py` for new optimization work on **x86-64 Linux**. It compares the
candidate, its baseline and Expat in each randomized round, using the existing
native C callback driver and unmodified CPython consumers. It does not use PGO or
change allocators.

The real corpus is the six pinned, original XML files in
[projects/corpus-manifest.json](projects/corpus-manifest.json). Native conditions
cover namespaces on/off and 4/64 KiB feeds; Python covers ElementTree and pyexpat at
both feed sizes. Each consumer has **24 real conditions**. The four deterministic
inputs from `tools/corpus.py` add **16 separate native generated conditions**.
These generated controls differ from some historical studies' smaller subsets.
This recipe also fixes one iteration count per consumer; historical studies used
other schedules, including per-condition counts. Compare new baseline and candidate
runs together; this entrypoint does not reproduce historical timings exactly.

## 1. Freeze the baseline and candidate

Prerequisites: the Ohm Rust toolchain, Python, uv, a C compiler, CMake, Git, `taskset`
and `readelf`. Run from the repository root. Keep one stable Cargo target directory
per worktree and the existing shared Ohm build directory separate. Choose a new
study directory; build and measurement commands refuse to overwrite existing
outputs.

```sh
benchmark_root="$PWD/../oriole-bench"
study="$benchmark_root/study-001"
mkdir -p "$study"
export CARGO_BUILD_BUILD_DIR="$benchmark_root/shared-build"

cargo +ohm worktree add -b bench-baseline "$benchmark_root/baseline" main \
  --target-dir "$benchmark_root/baseline-target"

python3 -I -S benchmarks/hillclimb.py build \
  --checkout "$benchmark_root/baseline" \
  --target-dir "$benchmark_root/baseline-target" --output "$study/baseline"
python3 -I -S benchmarks/hillclimb.py build \
  --checkout "$PWD" --target-dir "$benchmark_root/candidate-target" \
  --output "$study/candidate"
```

The builder uses `cargo +ohm -Zohm-defaults=no rustc`, ordinary O3, ThinLTO, one
codegen unit and generic x86-64. It selects only the C library crate types so
ThinLTO takes effect. Each output contains a frozen shared library, compiler/source
hashes and verbose compiler commands in `build.log`. Build both revisions with the
same compiler.

Controlled benchmark builds use a **fresh `output/intermediates` directory** for
the subprocess's `CARGO_BUILD_BUILD_DIR`, while keeping each worktree's stable
target directory. A shared intermediate cache incorrectly returned a candidate
artifact as fresh for a different baseline checkout during harness validation.
The builder therefore requires actual compiler records for the storage, parser
and C interface from the requested checkout, writing into the fresh directory,
before accepting the library. This override applies only to these benchmark
builds; the normal development shared cache and environment remain unchanged.
It adds no compiler metadata or optimization flags. Commit the candidate before publishing results so others can recover
its exact source; hashes also identify uncommitted experiments.

## 2. Build the pinned Expat control once

Keep this normal GCC O3 build unchanged across candidates. Expat uses its own
normal build configuration, without LTO; Oriole's recipe is the one above. Use the
same GCC version throughout a study and retain its version and CMake cache.

```sh
git clone https://github.com/libexpat/libexpat.git "$benchmark_root/expat-source"
git -C "$benchmark_root/expat-source" checkout --detach \
  12cf0b1f25f026a022fe728ad8f7e3d017285b80
cmake -S "$benchmark_root/expat-source/expat" -B "$benchmark_root/expat-build" \
  -DCMAKE_C_COMPILER=gcc -DCMAKE_BUILD_TYPE=Release \
  '-DCMAKE_C_FLAGS_RELEASE=-O3 -DNDEBUG' \
  -DCMAKE_INTERPROCEDURAL_OPTIMIZATION=OFF -DCMAKE_INSTALL_LIBDIR=lib \
  -DEXPAT_SHARED_LIBS=ON -DEXPAT_BUILD_TESTS=OFF \
  -DEXPAT_BUILD_EXAMPLES=OFF -DEXPAT_BUILD_TOOLS=OFF
cmake --build "$benchmark_root/expat-build" --verbose \
  > "$benchmark_root/expat-build/build.log" 2>&1
cmake --install "$benchmark_root/expat-build" --prefix "$benchmark_root/expat"
gcc --version > "$benchmark_root/expat-build/compiler.txt"
expat="$benchmark_root/expat/lib/libexpat.so"
```

## 3. Screen the native parser

Reserve a CPU and finish all builds before timing. Do not run another benchmark,
fuzzer or build concurrently. Affinity limits scheduling; it does not isolate CPU
frequency, caches or memory bandwidth from other users.

```sh
python3 -I -S benchmarks/hillclimb.py run --mode screen --cpu 0 \
  --baseline "$study/baseline/liboriole_expat.so" \
  --candidate "$study/candidate/liboriole_expat.so" --expat "$expat" \
  --baseline-build "$study/baseline/build.json" \
  --candidate-build "$study/candidate/build.json" \
  --build-manifest "$benchmark_root/expat-build/CMakeCache.txt" \
  --output "$study/screen"
```

A screen uses three rounds and three measured parses per process after a discarded
warmup. It covers every condition; it is a quick rejection signal, not evidence
for a small improvement. Every parser must pass the same complete normalized
callback checks, and every timed parse must match its expected output hash.
Both Oriole build records must report success and match their supplied library
SHA-256; failed, stale or swapped records stop the run. Python targets use isolated
mode with site initialization disabled, and target child processes clear loader
overrides (`LD_PRELOAD`, `LD_LIBRARY_PATH` and `LD_AUDIT`).

## 4. Confirm through CPython too

Use CPython **3.12.13** and its **unmodified** pinned source. Consumer compilation
uses the same O2 flags for all three libraries. Do this before starting timings.

```sh
uv python install 3.12.13
python312="$(uv python find 3.12.13)"
git clone https://github.com/python/cpython.git "$benchmark_root/cpython-source"
git -C "$benchmark_root/cpython-source" checkout --detach \
  3bb231a6a5dc02b95658877318bf61501a7209e9

"$python312" -I -S benchmarks/build_project_consumers.py \
  --library "$study/candidate/liboriole_expat.so" \
  --baseline "$study/baseline/liboriole_expat.so" --reference "$expat" \
  --source "$benchmark_root/cpython-source" --header include \
  --output "$study/consumers"

python3 -I -S benchmarks/hillclimb.py run --mode confirm --cpu 0 \
  --baseline "$study/baseline/liboriole_expat.so" \
  --candidate "$study/candidate/liboriole_expat.so" --expat "$expat" \
  --python "$python312" --consumers "$study/consumers/build.json" \
  --baseline-build "$study/baseline/build.json" \
  --candidate-build "$study/candidate/build.json" \
  --build-manifest "$benchmark_root/expat-build/CMakeCache.txt" \
  --output "$study/confirmation-1"
```

Confirmation uses seven rounds, twenty native parses and ten Python parses per
process, each after one discarded warmup. Use `--native-iterations 100` and
`--python-iterations 50`, for example, to increase sample work for small gains.
Keep these settings identical across confirmation epochs; actual counts are
recorded. The Python workers verify the loaded
extensions and parser library, and compare complete canonical trees/event lists.
Adjacent character callbacks are coalesced for these output comparisons; strict
callback grouping remains a separate compatibility test.

Repeat the last command later with `--output "$study/confirmation-2"`, retaining
both epochs separately; do not pool their samples or medians. A native-only confirmation omits `--consumers`; it makes
no Python performance claim. `--mode screen` also accepts `--consumers`.

## Read the results

- `conditions.csv` retains **every condition**, including regressions. Ratios are
  candidate time divided by baseline or Expat time; **lower is better**. They are
  medians of within-round ratios of process medians, not ratios of global medians.
- `report.json` contains equal-weight geometric means for `native-real`,
  `native-generated` and, when requested, `python-real`. Generated conditions never
  enter either real aggregate. Every slower-than-baseline condition is listed.
- The existing runners retain raw samples, canonical output hashes, process order,
  library/input hashes, and failure logs under each group's directory. A failed
  subprocess or incomplete result stops the campaign; earlier files remain.

Prefer an improvement that repeats in both real aggregates without substantial
individual regressions. Our Expat target is at most **1.20×** on each real aggregate;
meeting that target does not waive individual or generated outliers. Keep source,
build settings and corpus fixed between confirmation epochs. Benchmark success does
not replace compatibility, allocation, sanitizer or callback-lifetime review.

For lower-level controls and complete Wayland code generation, see
[projects/RERUN.md](projects/RERUN.md).
