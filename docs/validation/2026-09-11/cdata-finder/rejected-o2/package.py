"""Plan, then explicitly assemble saved O2 evidence; never execute target code."""
from pathlib import Path
import argparse
import csv
import gzip
import hashlib
import io
import json
import os
import tarfile

ROOT = Path(__file__).resolve().parent
BUILD = Path('/tmp/oriole-opt2-pgo-study')
CONTROL = Path('/tmp/oriole-streaming-work-pgo-study-attempt02')
AUDIT = Path('/tmp/oriole-opt2-native-independent-review.json')
BUILD_AUDIT = Path('/tmp/oriole-opt2-build-independent-review.json')
DECISION = Path('/tmp/oriole-opt2-native-decision/decision.json')

def digest(data):
    return hashlib.sha256(data).hexdigest()

def load(path):
    return json.loads(Path(path).read_text())

def save(path, value):
    with path.open('x') as output:
        output.write(json.dumps(value, indent=2) + '\n')

def plan():
    """Read small existing receipts and stat payloads; defer raw payload reads."""
    assert os.sched_getaffinity(0) == {6}
    assert digest(AUDIT.read_bytes()) == 'd18e234595fa4dde63ddb86daa1a4fe9f7e5289cf5a902c00abe462a11d37617'
    assert digest(BUILD_AUDIT.read_bytes()) == '7c89c6a103f60d7f50d05eb42c1d2aedb82fda45a05d7fe069b1d0c66c80d696'
    audit, build_audit, decision = load(AUDIT), load(BUILD_AUDIT), load(DECISION)
    frozen, control_frozen = load(BUILD / 'pair-freeze.json'), load(CONTROL / 'pair-freeze.json')
    assert frozen['count'] == len(frozen['files']) == 63
    files, excluded, aliases = {}, {}, []

    def include(path, logical, expected=None):
        path = Path(path)
        size = path.stat().st_size
        assert not logical.startswith('/') and '..' not in Path(logical).parts
        assert path.suffix not in {'.a', '.so', '.o', '.rlib', '.rmeta'}
        if expected is None:
            assert size <= 131072, ('unsealed large read forbidden in plan', path, size)
            expected = digest(path.read_bytes())
        assert logical not in files
        files[logical] = {'source': str(path), 'bytes': size, 'sha256': expected}

    def exclude(path, expected, reason):
        path = Path(path)
        excluded[str(path)] = {'sha256': expected, 'bytes': path.stat().st_size,
                               'reason': reason, 'payload_read_or_copied': False}

    for relative, entry in frozen['files'].items():
        path = BUILD / relative
        assert path.stat().st_size == entry['bytes']
        if path.suffix in {'.a', '.so'}:
            exclude(path, entry['sha256'], 'Compiled candidate library; original frozen identity retained only.')
        else:
            include(path, 'build/' + relative, entry['sha256'])
    include(BUILD / 'pair-freeze.json', 'build/pair-freeze.json', build_audit['freeze_sha256'])
    for relative in ['pair-freeze.json', 'source.json', 'tool-source.json', 'pair-report.json',
                     'normal-build.log', 'normal-training.json',
                     'pgo/runs/run-psjiwn9z/manifest.json',
                     'pgo/runs/run-psjiwn9z/04-build-generate.log',
                     'pgo/runs/run-psjiwn9z/08-build-use.log',
                     'pgo/runs/run-psjiwn9z/generate-training.json',
                     'pgo/runs/run-psjiwn9z/use-training.json']:
        entry = control_frozen['files'].get(relative)
        include(CONTROL / relative, 'control/' + relative, entry['sha256'] if entry else None)
    for relative, expected in load(BUILD / 'tool-source.json').items():
        include(Path('/home/dev-user/code/oss/oriole-opt2-study') / relative, 'pipeline/' + relative, expected)
    for path, logical in [(AUDIT, 'audits/native.json'), (BUILD_AUDIT, 'audits/build.json'),
                          (DECISION, 'decision.json'),
                          (Path('/tmp/oriole-review-opt2-native.py'), 'audits/native-audit.py'),
                          (Path('/tmp/oriole-opt2-build-independent-audit.py'), 'audits/build-audit.py'),
                          (Path('/tmp/oriole-review-opt2-native.log'), 'audits/native-audit.log')]:
        include(path, logical)
    for mode in ['normal', 'pgo']:
        study = Path('/tmp/oriole-opt2-' + mode + '-native-study')
        for path_string, expected in audit['input_sha256'].items():
            path = Path(path_string)
            if path.is_relative_to(study):
                include(path, 'native/' + mode + '/' + str(path.relative_to(study)), expected)
        for relative in ['run.py.in', 'preflight-controller.log', 'time-controller.log']:
            include(study / relative, 'native/' + mode + '/' + relative)
        template = Path(load(study / 'template-preparation.json')['source']) / 'native_screen.py'
        include(template, 'native/' + mode + '/original-template.py')
        protocol = load(study / 'native-screen/protocol.json')
        for engine, entry in protocol['libraries'].items():
            exclude(entry['path'], entry['sha256'], 'Compiled parser library identity only (' + mode + '/' + engine + ').')
        driver = '/tmp/oriole-grammar-bench-plain/native-driver'
        exclude(driver, protocol['hashes'][driver], 'Compiled timing driver identity only; source is included separately.')
        for condition in protocol['conditions']:
            logical = 'inputs/' + condition['name'] + '.xml'
            if logical not in files:
                include(condition['input'], logical, protocol['hashes'][condition['input']])
        for ordinal in range(672):
            relative = 'worker-' + str(ordinal).zfill(4) + '.json'
            path = study / 'native-screen' / relative
            pointer = ['rows', ordinal] if ordinal < 84 else ['rows', (ordinal - 84) // 3, 'processes', (ordinal - 84) % 3]
            aliases.append({'source': str(path), 'bytes': path.stat().st_size,
                'logical_path': 'native/' + mode + '/native-screen/' + relative,
                'container': 'native/' + mode + '/native-screen/' + ('preflight.json' if ordinal < 84 else 'results.json'),
                'json_pointer_components': pointer,
                'transform': 'Drop observations and median_seconds; preserve field order; json.dumps(indent=2, ensure_ascii=True) plus newline.',
                'sha256': None, 'byte_equivalence': 'pending post-hold readback'})
    manifest_path = Path('/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json')
    manifest_sha256 = 'c725b079ba4f0b19912880f53c6f0ac83758711071151abc8765d1e06491ff42'
    assert digest(manifest_path.read_bytes()) == manifest_sha256
    include(manifest_path, 'inputs/corpus-manifest.json', manifest_sha256)
    notices = {'corpus': [], 'oriole': []}
    for project in load(manifest_path)['projects']:
        fixture = files.get('inputs/' + project['name'] + '.xml')
        if fixture is None:
            continue
        inputs = [entry for entry in project['files'] if entry['role'] == 'input']
        assert len(inputs) == 1 and fixture['sha256'] == inputs[0]['sha256']
        for entry in project['files']:
            if entry['role'] not in ['license', 'notice']:
                continue
            path = manifest_path.parent / entry['path']
            assert path.stat().st_size == entry['bytes'] <= 131072
            assert digest(path.read_bytes()) == entry['sha256'], path
            logical = 'inputs/' + entry['path']
            include(path, logical, entry['sha256'])
            notices['corpus'].append({'project': project['name'], 'role': entry['role'],
                'manifest_relative_path': entry['path'], 'logical_path': logical,
                'bytes': entry['bytes'], 'sha256': entry['sha256']})
    assert len(notices['corpus']) == 9
    for relative in ['LICENSE-APACHE', 'LICENSE-MIT', 'tests/c/UPSTREAM-NOTICES.txt', 'THIRD_PARTY_LICENSES.md']:
        path = Path('/home/dev-user/code/oss/oriole-opt2-study') / relative
        logical = 'source-notices/oriole/' + relative
        include(path, logical)
        notices['oriole'].append({'source_relative_path': relative, 'logical_path': logical,
            'bytes': files[logical]['bytes'], 'sha256': files[logical]['sha256']})
    include('/home/dev-user/code/oss/oriole/benchmarks/projects/README.md', 'inputs/corpus-README.md')
    include('/home/dev-user/code/oss/oriole/benchmarks/native_driver.c', 'inputs/native_driver.c')
    for path, expected in load(BUILD / 'pair-report.json')['tools_before'].items():
        exclude(path, expected, 'Compiler/tool binary identity only; versions and actual invocation logs retained.')
    rows = []
    for mode, data in audit['native'].items():
        for row in data['all_conditions']:
            condition = row['condition']
            ratios = row['median_ratios']
            rows.append({'mode': mode, 'name': condition['name'], 'chunk': condition['chunk'],
                'namespaces': condition['namespaces'], 'iterations': condition['iterations'],
                **ratios, 'adverse_vs_O3': ratios['candidate_over_control'] > 1})
    assert len(rows) == 56 and sum(row['adverse_vs_O3'] for row in rows) == 27
    for name, selected in [('conditions.csv', rows), ('adverse-conditions.csv', [row for row in rows if row['adverse_vs_O3']])]:
        with (ROOT / name).open('x', newline='') as output:
            writer = csv.DictWriter(output, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(selected)
    save(ROOT / 'summary.json', {'status': 'rejected_saved_result_packaging_pending',
        'decision': decision, 'counts_per_mode': {mode: {k: v for k, v in row.items() if k not in ['groups', 'all_conditions']} for mode, row in audit['native'].items()},
        'all_conditions': rows, 'build_audit_sha256': digest(BUILD_AUDIT.read_bytes()),
        'native_audit_sha256': digest(AUDIT.read_bytes()), 'scope': decision['scope']})
    inventory = {'status': 'draft_no_payload_read_or_compression', 'files': files,
        'compiled_artifact_exclusions': excluded, 'raw_worker_aliases': aliases,
        'redistribution_notices': notices,
        'frozen_63_accounted': {'retained': 57, 'compiled_excluded': 6},
        'raw_worker_alias_count': len(aliases), 'payload_bytes_before_deduplication': sum(x['bytes'] for x in files.values()),
        'worker_bytes_planned_to_reconstruct': sum(x['bytes'] for x in aliases),
        'planned_archive': 'evidence.tar.gz',
        'notes': ['No large raw result, worker, XML fixture, source archive or profile payload has been read by --plan.',
                  'Existing recorded hashes are expectations, not new readback claims. Alias digests/equality remain pending until --assemble.',
                  'All original paths remain provenance labels; archive logical names and the portable verifier do not require those paths.',
                  'Identical retained payloads are stored once under their first sorted logical member; explicit inventory aliases map every logical path.']}
    save(ROOT / 'inventory-plan.json', inventory)
    print(json.dumps({'status': inventory['status'], 'files': len(files), 'aliases': len(aliases),
                      'compiled_exclusions': len(excluded), 'payload_bytes': inventory['payload_bytes_before_deduplication']}))

def worker_bytes(container, pointer):
    row = container
    for component in pointer:
        row = row[component]
    raw = {key: value for key, value in row.items() if key not in ['observations', 'median_seconds']}
    return (json.dumps(raw, indent=2) + '\n').encode()

def completed_readme(draft):
    """Render the publication README only after the portable readback passes."""
    replacements = {
        'Packaging status: **draft; raw payload reads, archive creation and readback are held**.':
            'Packaging status: **assembled and verified; see [readback.json](readback.json)**.',
        'Assembly must compare every reconstructed file byte-for-byte\nwith its original before omitting it.':
            'Assembly compared every reconstructed file byte-for-byte\nwith its original before omitting it.',
        'The portable package is planned as': 'The portable package contains',
    }
    for old, new in replacements.items():
        assert draft.count(old) == 1, old
        draft = draft.replace(old, new)
    marker = '## Packaging hold\n'
    assert draft.count(marker) == 1
    draft = draft.split(marker)[0] + (
        '## Package verification\n\n'
        'Assembly verified every retained payload against its recorded SHA-256 and byte count, '
        'and every reconstructed worker file against its original bytes. The portable readback '
        'then checked the archive, all 70 source files, all 1,344 worker aliases, all 56 condition '
        'summaries and 212,352 raw samples. It also checked all nine corpus license/notice entries '
        'against the pinned manifest and retained Oriole notices. See [readback.json](readback.json) '
        'and [package-files.json](package-files.json) for the completed receipt and file hashes.\n')
    assert all(text not in draft for text in ['Only `plan` is authorized', 'Packaging hold',
        'Assembly must', 'planned as', 'Root must release', 'packaging_pending'])
    return draft

def assemble():
    """Only after root releases saved-data I/O: verify, deduplicate and archive."""
    assert os.sched_getaffinity(0) == {6}
    plan_data = load(ROOT / 'inventory-plan.json')
    assert plan_data['status'] == 'draft_no_payload_read_or_compression'
    output = ROOT / 'bundle'
    output.mkdir(exist_ok=False)
    payloads, files, by_digest = {}, {}, {}
    for logical, row in sorted(plan_data['files'].items()):
        data = Path(row['source']).read_bytes()
        assert len(data) == row['bytes'] and digest(data) == row['sha256'], logical
        assert not data.startswith(b'\x7fELF') and not data.startswith(b'!<arch>\n'), logical
        member = by_digest.setdefault(row['sha256'], logical)
        if member in payloads:
            assert payloads[member] == data
        else:
            payloads[member] = data
        files[logical] = row | {'archive_member': member}
    containers = {}
    aliases = []
    for row in plan_data['raw_worker_aliases']:
        logical = row['container']
        if logical not in containers:
            containers[logical] = json.loads(payloads[files[logical]['archive_member']])
        recreated = worker_bytes(containers[logical], row['json_pointer_components'])
        original = Path(row['source']).read_bytes()
        assert recreated == original and len(original) == row['bytes'], row['source']
        aliases.append(row | {'sha256': digest(original), 'byte_equivalence': 'verified_exact'})
    assert len(aliases) == 1344
    # Original independent reports are retained unchanged. Portable verification
    # below checks transport, source payload and worker aliases without targets.
    inventory = plan_data | {'status': 'assembled_pending_portable_readback', 'files': files,
        'raw_worker_aliases': aliases, 'unique_payload_members': len(payloads),
        'unique_payload_bytes': sum(map(len, payloads.values())),
        'plan_sha256': digest((ROOT / 'inventory-plan.json').read_bytes()),
        'packager_sha256': digest(Path(__file__).read_bytes())}
    save(output / 'inventory.json', inventory)
    for name in ['summary.json', 'conditions.csv', 'adverse-conditions.csv', 'verify.py', 'package.py']:
        (output / name).write_bytes((ROOT / name).read_bytes())
    archive = output / 'evidence.tar.gz'
    with archive.open('xb') as raw:
        with gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode='w') as tar:
                for name, data in sorted(payloads.items()):
                    info = tarfile.TarInfo(name)
                    info.size, info.mode, info.mtime = len(data), 0o644, 0
                    tar.addfile(info, io.BytesIO(data))
    import importlib.util
    spec = importlib.util.spec_from_file_location('portable_saved_data_verify', ROOT / 'verify.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    receipt = module.verify(output)
    save(output / 'readback.json', receipt)
    (output / 'README.md').write_text(completed_readme((ROOT / 'README.md').read_text()))
    save(output / 'package-files.json', {p.name: {'sha256': digest(p.read_bytes()), 'bytes': p.stat().st_size}
                                        for p in sorted(output.iterdir()) if p.is_file()})
    print(json.dumps(receipt))

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=['plan', 'assemble'])
    args = parser.parse_args()
    (plan if args.phase == 'plan' else assemble)()
