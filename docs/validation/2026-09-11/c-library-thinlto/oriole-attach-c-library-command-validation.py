from pathlib import Path
import hashlib
import json
import shutil

repo = Path('/home/dev-user/code/oss/oriole-c-library-lto')
out = repo / 'docs/validation/2026-09-11/c-library-thinlto'
original = Path('/tmp/oriole-c-library-lto-command-handoff')
destination = out / 'commands'
assert not destination.exists()
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
summary = json.loads((original / 'summary.json').read_text())
assert summary['status'] == 'passed_with_initial_attempts_retained'
assert summary['pgo']['rows_exact'] and summary['pbs']['explicit_pic_and_unwind']
assert summary['pbs']['empty_encoded_flags_tested'] and summary['pbs']['nonempty_encoded_flags_exit'] == 2
shutil.copytree(original, destination)
for p in original.rglob('*'):
    if p.is_file():
        assert sha(p) == sha(destination / p.relative_to(original))
(out / 'command-validation.md').write_text('''# Production command validation

The C consumer, CPython and normal PGO CI builds now select only `cdylib,staticlib` through Cargo. Both PGO phases and the PBS archive helper use the same C-only selection. The manifest retains `rlib` for Rust consumers. CI also builds an actual PBS archive bundle with stable Rust and the `rust-docs` component. The [uv PR #21572 runner pattern](https://github.com/astral-sh/uv/pull/21572) is preserved; the separate PBS validation executable keeps its existing build command.

PGO accepts recorded global Cargo arguments so local Ohm runs can disable experimental defaults. The options precede Cargo's operation and are not passed to rustc or llvm-profdata. Directory-changing arguments and external Cargo configuration files are rejected; inline TOML configurations are recorded and cannot include additional files. Actual compiler invocations are retained through single-verbose output.

The PBS helper continues to reject nonempty `CARGO_ENCODED_RUSTFLAGS` before creating output. It now removes an accepted empty value from the child environment: Cargo otherwise gives that value precedence over the required PIC/unwind `RUSTFLAGS`.

## Completed checks

Thirteen PGO unit tests, Ruff, formatting, ty and the PBS recipe/backport validation pass. The source change contains no parser, storage, header, Cargo manifest, dependency, or resource-limit changes.

An actual PGO run completes **288 training parses and 288 optimized replay parses with identical rows**. Both phases record three fresh workspace compiler invocations and effective ThinLTO. The strict warning check remains unchanged. The optimized shared artifact is `c2d677b7789717169900f4de348ebf1b22988d37d0885c629f0ca2bf6228fa58`; its manifest is `8a7aeed73432e48d7138330faf9bf7e22baac8cbb7c441b81d627238e8bd9d0a`. This validates the revised build process, with generated training inputs and Ohm defaults disabled. No new throughput or full compatibility result is claimed for this PGO artifact. The final subsequent source edit affects only the PBS helper, so the PGO inputs remain identical.

An actual PBS bundle build starts with an intentionally empty encoded-flags variable. Three fresh workspace compiler commands contain explicit PIC/unwind flags; the final C library has ThinLTO. Native static linker libraries are extracted and the optional TLS destructor hook remains weak (`w`). A nonempty encoded-flags value exits with status 2 before output or compilation. The successful bundle manifest is `b3c33f33414372bf36a2679081d2d9f4612418d51d18c0b53a1eba32f9492a56`. This local smoke uses Ohm's default experimental options; it is distinct from the normal benchmark compiler configuration. The stable CI job and a complete PBS distribution have not been executed by this local smoke.

## Preserved failed and limited attempts

The first double-verbose PGO attempt compiles both phases and completes training, then the unchanged warning check rejects a real `allocator-api2` implicit-autoref warning before optimized replay. Cargo double verbosity enables dependency warnings; single verbosity records compiler commands using Cargo's default dependency lint policy. The dependency bytes and warning check were not changed. The package retains the original diagnostic, the current source/caller investigation, an upstream fix reference and the actual compiler arguments from both verbosity modes.

The PBS attempts retain an invalid empty `RUSTC` controller value, a completed bundle whose empty encoded flags prevented proof of explicit PIC/unwind, a successful clean-environment control, and a completed hardened-helper build rejected by the evidence check for cached rather than fresh compilation. The final fresh attempt establishes the required flags with the problematic empty environment value present. Source/check-script and diagnostic-postprocessing failures remain recorded separately. None of these earlier attempts supplies the final fresh-build claim.

## Reproduction and evidence

[commands/summary.json](commands/summary.json) records exact outcomes, identities and scope. [commands/candidate.patch](commands/candidate.patch) contains the nine-file command layer against `1cb326c6dcd3aaebf37e555826b515a0b53b6641`; the surrounding README and benchmark reports are separate. [commands/source.json](commands/source.json) and its source archive preserve all 81 tracked inputs. The [command evidence archive](commands/evidence.tar.gz) has 198 direct members plus 243 nested source members, all read back against their hashes. Its SHA-256 is `20d1ad701fbfec33c9cdba3c0a037eb7d73fb88526d52369520e00526da4afe6`. Twelve generated library files are omitted with their hashes in [commands/excluded-binaries.json](commands/excluded-binaries.json).

The separate [matched ThinLTO report](README.md) owns the `a55` C-library benchmarks and canonical compatibility results. Those results are not reassigned to these PGO or PBS artifacts.
''')
for p in ['/tmp/oriole-write-c-library-thinlto-docs.py', __file__]:
    shutil.copy2(p, out / Path(p).name)
index = {'files': {str(p.relative_to(out)): {'bytes': p.stat().st_size, 'sha256': sha(p)}
                   for p in sorted(out.rglob('*')) if p.is_file()}}
(out / 'files.json').write_text(json.dumps(index, indent=2) + '\n')
print(json.dumps({'copied_command_files': len(list(destination.iterdir())), 'indexed_files': len(index['files'])}))
