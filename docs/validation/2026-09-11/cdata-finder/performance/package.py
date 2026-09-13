"""Package only saved Finder evidence on CPU6; never execute target code."""
from pathlib import Path
import gzip, hashlib, io, json, os, subprocess, tarfile

REPO = Path('/home/dev-user/code/oss/oriole-cdata-finder')
STUDY = Path('/tmp/oriole-cdata-finder-study')
OUT = Path('/tmp/oriole-cdata-finder-perf-handoff')
SCRATCH = Path('/tmp/oriole-cdata-finder-perf-package-source')
MEASURED = '144e69e08c5462217d54791374e579e3ef390a87'
PUBLICATION = 'ee73ccdeefa9572666c753776ed309860a1057e4'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
load = lambda p: json.loads(Path(p).read_text())


def main():
    assert os.sched_getaffinity(0) == {6}
    assert not OUT.exists()
    SCRATCH.mkdir(exist_ok=True)
    assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip() == PUBLICATION
    source = load(STUDY / 'source.json')['source_sha256']
    assert sha(STUDY / 'source.json') == '69b96121a090467b3cea81eb25a4e627976f1a18c4cab628855c9880dd25f982'
    assert all(sha(REPO / n) == h for n, h in source.items())
    checks = load(STUDY / 'workspace-checks-attempt01/summary.json')
    assert checks['status'] == 'passed' and checks['workspace_non_doc_tests_passed'] == 420
    files = []
    names = set()
    omitted = []

    def add(path, dest, purpose, expected=None, origin=None):
        path = Path(path)
        assert path.is_file() and not path.is_symlink(), path
        assert dest not in names, dest
        data = path.read_bytes()
        assert not data.startswith((b'\x7fELF', b'!<arch>\n')), path
        actual = hashlib.sha256(data).hexdigest()
        if expected:
            assert actual == expected, path
        names.add(dest)
        files.append(dict(source=str(path), destination=dest, bytes=len(data), sha256=actual, purpose=purpose, **({'origin': origin} if origin else {})))

    def tree(root, dest, purpose, consumer=False):
        root = Path(root)
        for p in sorted(root.rglob('*')):
            if not p.is_file():
                continue
            rel = p.relative_to(root)
            if p.is_symlink() or '__pycache__' in rel.parts or p.suffix in ('.so', '.a', '.pyc') or p.name.endswith(('.tar.gz', '.tar.xz', '.zip')):
                omitted.append(str(p)); continue
            if consumer and 'consumers' in rel.parts and not (p.name == 'build.json' or p.name.endswith('-build.log')):
                omitted.append(str(p)); continue
            add(p, dest + '/' + str(rel), purpose)

    for mode in ('normal', 'pgo'):
        for kind in ('native', 'python'):
            root = Path(f'/tmp/oriole-cdata-finder-{mode}-{kind}-study')
            tree(root, f'{kind}-{mode}', 'Entire immediate campaign: raw workers, stdout/stderr, specs, controller and origin/build records', consumer=True)
            adaptation = load(root / 'adaptation.json')
            if kind == 'native':
                for name in ('run.py', 'native_screen.py'):
                    add(Path(adaptation['source']) / name, f'templates/{kind}-{mode}/{name}', 'Exact inherited controller source for path-only adaptation proof')
            else:
                for name, row in adaptation['parent_files'].items():
                    p = Path(adaptation['parent_adaptation']['path']).parent / name
                    add(p, f'templates/{kind}-{mode}/{p.name}', 'Exact inherited Python controller source', row)

    # Preserve complete raw candidate and control build evidence without caches/libraries.
    pair_roots = {'candidate': Path('/tmp/oriole-cdata-finder-pgo-study'), 'control': Path('/tmp/oriole-streaming-work-pgo-study-attempt02')}
    manifests = {}
    for label, root in pair_roots.items():
        freeze = load(root / 'pair-freeze.json')
        add(root / 'pair-freeze.json', f'build/{label}/pair-freeze.json', 'Original complete producer hash inventory, including excluded binaries')
        for name, row in freeze['files'].items():
            path = root / name
            if name.endswith(('.a', '.so', '.tar.gz', '.tar.xz')):
                omitted.append(str(path)); continue
            add(path, f'build/{label}/{name}', 'Exact frozen build/compiler/profile/training input or output', row['sha256'])
        pair = load(root / 'pair-report.json')
        manifest = load(pair['pgo_manifest']['path'])
        manifests[label] = manifest
        assert manifest['script_sha256'] == manifests['candidate']['script_sha256']
    for name, expected in manifests['candidate']['script_sha256'].items():
        data = subprocess.check_output(['git', 'show', f'{MEASURED}:tools/pgo/{name}'], cwd=REPO)
        p = SCRATCH / 'pipeline' / name
        p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(data)
        add(p, 'source/measured-pipeline/' + name, 'Measured pipeline recovered from pre-restack Git, not newer publication files', expected, f'git:{MEASURED}:tools/pgo/{name}')
    for name, expected in source.items():
        add(REPO / name, 'source/candidate/' + name, 'Exact measured 70-file runtime/test source inventory', expected)
    for name in ('LICENSE-APACHE', 'LICENSE-MIT'):
        add(REPO / name, 'source/' + name, 'Oriole source license')
    control_lib = subprocess.check_output(['git', 'show', f'{MEASURED}:crates/oriole/src/lib.rs'], cwd=REPO)
    p = SCRATCH / 'control-lib.rs'; p.write_bytes(control_lib)
    add(p, 'source/control/crates/oriole/src/lib.rs', 'Only Rust source delta from accepted streaming control', manifests['control']['source_sha256']['crates/oriole/src/lib.rs'], f'git:{MEASURED}:crates/oriole/src/lib.rs')
    tree(STUDY, 'checks-and-preparation', 'Original focused and full workspace commands/raw outputs, source/adaptation provenance')

    # Raw bounded profiles include their first-use warmup; this is not elapsed evidence.
    tree('/tmp/oriole-cdata-finder-codegen-study', 'codegen/candidate', 'Exact assembly, three raw Callgrind profiles, output and corrected one-warmup/one-measure scope')
    tree('/tmp/oriole-streaming-current-profile', 'codegen/control', 'Exact three retained control Callgrind profiles and raw output')
    prior_asm = Path('/tmp/oriole-native-tag-execution-codegen-study/assembly')
    add(prior_asm / 'collection.json', 'codegen/control-assembly/collection.json', 'Original collection receipt for reused selected-control assembly; older candidate entries are historical context only')
    for mode in ('normal', 'pgo'):
        for suffix in ('asm', 'nm', 'sections'):
            for ending in ('', '.stderr'):
                name = f'control-{mode}.{suffix}{ending}'
                add(prior_asm / name, 'codegen/control-assembly/' + name, 'Exact saved selected-control assembly/symbol/section output reused by Finder diagnostic')
    tree('/tmp/oriole-cdata-finder-python-elapsed-independent-review', 'reviews/python-elapsed', 'Independent raw Python numerical reconstruction')
    for p in sorted(Path('/tmp').glob('oriole-cdata-finder*')):
        if p.is_file() and p.suffix in ('.py', '.json', '.log', '.patch'):
            add(p, 'reviews/' + p.name, 'Independent source/build/protocol/numerical audit or retained reviewer correction')
    for name in ('oriole-review-cdata-finder-native.py', 'oriole-review-cdata-finder-python-elapsed.py', 'oriole-prepare-cdata-finder-native-audit.py'):
        add(Path('/tmp') / name, 'reviews/' + name, 'Executed numerical reader or exact reader adaptation')
    tree('/tmp/oriole-cdata-finder-native-decision', 'decision', 'Final selection and preceding decision with every adverse condition')

    # Cross-engine comparison is explicitly GCC13 O3/no-LTO versus Ohm O3/ThinLTO.
    expat = Path('/tmp/oriole-pgo-study')
    for name in ('expat-source.json', 'expat-control-build.json', 'expat-generate-build.json', 'expat-use-build.json', 'expat-profile.json', 'expat-control-training.json', 'expat-generate-training.json', 'expat-use-training.json', 'build_expat.py', 'profile_expat.py', 'training-inputs.json', 'generate_training.py', 'train.py'):
        add(expat / name, 'expat/' + name, 'Pinned Expat compiler/configuration/native-profile/training provenance')
    for mode in ('control', 'generate', 'use'):
        for kind in ('configure', 'compile'):
            add(expat / f'expat-{mode}-{kind}.log', f'expat/expat-{mode}-{kind}.log', 'Raw Expat GCC13 compiler/configuration output')
    for name, expected in load(expat / 'expat-profile.json')['files'].items():
        add(expat / name, 'expat/' + name, 'Exact native GCC training profile', expected)
    add(expat / 'expat-source/COPYING', 'expat/COPYING', 'Expat source notice')

    for name in ('native_driver.c', 'projects/corpus-manifest.json', 'projects/README.md', 'projects/RERUN.md'):
        add(REPO / 'benchmarks' / name, 'source/benchmarks/' + name, 'Native driver and original pinned real-project provenance')
    tree(REPO / 'benchmarks/projects/corpus', 'source/benchmarks/projects/corpus', 'Original held-out XML fixtures and all accompanying notices')
    for path, dest in (('/tmp/oriole-text-frame-prototype/text/screen/rare-declarations.xml', 'inputs/rare-declarations.xml'), ('/tmp/oriole-grammar-bench-plain/entities.xml', 'inputs/entities.xml')):
        add(path, dest, 'Generated native controls, never part of held-out project aggregate')
    cpython = Path('/run/user/1000/oriole-artifacts/upstream/cpython-3.12.13')
    for name in ('Modules/pyexpat.c', 'Modules/_elementtree.c', 'LICENSE'):
        add(cpython / name, 'source/cpython/' + name, 'Unmodified CPython3.12.13 benchmark consumer source/notice; strict cleanup-patched consumer is separate')
    dependency = load('/tmp/oriole-cdata-finder-independent-review.json')['dependency_source_sha256']
    for root in Path('/home/dev-user/.cache/toucan/cargo/registry/src').glob('*/memchr-*'):
        if sha(root / 'Cargo.toml') == dependency['Cargo.toml']:
            for name, expected in dependency.items():
                add(root / name, 'source/memchr/' + name, 'Pinned memchr source behind immutable borrowed Finder review', expected)
            for name in ('LICENSE-MIT', 'UNLICENSE', 'COPYING'):
                add(root / name, 'source/memchr/' + name, 'Pinned memchr notice')
            break
    else:
        raise AssertionError('Pinned memchr source missing')
    add(__file__, 'package.py', 'Deterministic packager and complete readback source')
    add('/tmp/oriole-verify-cdata-finder-perf.py', 'verify.py', 'Portable archive/member/alias verification; no original machine paths needed')

    # Same-byte sources keep every original path but store the payload only once.
    stored, aliases, seen = [], [], {}
    for row in files:
        if row['sha256'] in seen:
            aliases.append(dict(row, same_bytes_as_destination=seen[row['sha256']]))
        else:
            seen[row['sha256']] = row['destination']; stored.append(row)
    stored.sort(key=lambda row: row['destination'])
    OUT.mkdir()
    archive = OUT / 'evidence.tar.gz'
    with archive.open('wb') as output:
        with gzip.GzipFile(filename='', mode='wb', fileobj=output, mtime=0, compresslevel=3) as gz:
            with tarfile.open(fileobj=gz, mode='w|', format=tarfile.PAX_FORMAT) as tar:
                for row in stored:
                    data = Path(row['source']).read_bytes()
                    assert hashlib.sha256(data).hexdigest() == row['sha256']
                    info = tarfile.TarInfo(row['destination']); info.size = len(data); info.mode = 0o644; info.mtime = info.uid = info.gid = 0
                    tar.addfile(info, io.BytesIO(data))
    actual = []
    with tarfile.open(archive, 'r|gz') as tar:
        for member in tar:
            data = tar.extractfile(member).read()
            actual.append((member.name, len(data), hashlib.sha256(data).hexdigest()))
    assert actual == [(row['destination'], row['bytes'], row['sha256']) for row in stored]
    index = {'archive': archive.name, 'archive_sha256': sha(archive), 'members': stored, 'aliases': aliases, 'excluded_paths': omitted}
    (OUT / 'members.json.gz').write_bytes(gzip.compress(json.dumps(index, separators=(',', ':'), sort_keys=True).encode(), mtime=0))
    decision = load('/tmp/oriole-cdata-finder-native-decision/decision.json')
    summary = {
        'status': 'packaged and every member read back; independent numerical/source audits retained separately',
        'measured_source_base': MEASURED, 'publication_base': PUBLICATION,
        'measured_source_manifest_sha256': sha(STUDY / 'source.json'), 'measured_pipeline_git_base': MEASURED,
        'runtime_delta': ['crates/oriole/src/lib.rs'], 'inherited_C_fixture_is_separate': True,
        'four_campaigns': {'native_each': {'conditions': 28, 'preflight_workers': 84, 'timed_workers': 588}, 'python_each': {'conditions': 24, 'preflight_workers': 72, 'timed_workers': 504}},
        'decision': decision['decision'], 'decision_origin_sha256': sha('/tmp/oriole-cdata-finder-native-decision/decision.json'),
        'workspace': checks,
        'scope': ['Actual normal and PGO campaigns stay separate; every adverse condition retained.', 'Oriole uses Ohm1.98.1-1 LLVM22.1.8 O3/ThinLTO/cgu1 with defaults disabled. Expat2.8.4 uses GCC13.3 O3/noLTO; each uses its own generated-only PGO. Cross-engine toolchains are not matched.', 'Python benchmark source is unmodified CPython3.12.13; cleanup-patched strict compatibility is separate. Canonical text coalescing is not exact callback equivalence.', 'Three instruction diagnostics each collect one warmup plus one measured parse, including first lazy Finder initialization. Inherited Vulkan iterations7/Wayland64 metadata is not the executed diagnostic count. Instructions do not imply elapsed speedup.', 'Publication restack changes PGO/PBS tooling, not measured parser bytes. All six measured pipeline files are recovered from exact earlier Git and verified against both measured manifests.', 'Excluded ELF/static/compiler/interpreter/extension binaries, Cargo caches, Python bytecode and nested source archives retain source/hash/origin records. Profile data is included.'],
        'archive': archive.name, 'archive_sha256': sha(archive), 'compressed_bytes': archive.stat().st_size,
        'stored_members': len(stored), 'aliases': len(aliases), 'selected_sources': len(files),
        'member_index_sha256': sha(OUT / 'members.json.gz'), 'packager_sha256': sha(__file__),
    }
    (OUT / 'summary.json').write_text(json.dumps(summary, indent=2, sort_keys=True) + '\n')
    for source_path, target in ((__file__, 'package.py'), ('/tmp/oriole-verify-cdata-finder-perf.py', 'verify.py')):
        (OUT / target).write_bytes(Path(source_path).read_bytes())
    print(json.dumps({k: summary[k] for k in ('archive_sha256', 'compressed_bytes', 'stored_members', 'aliases', 'selected_sources')}))


if __name__ == '__main__':
    main()
