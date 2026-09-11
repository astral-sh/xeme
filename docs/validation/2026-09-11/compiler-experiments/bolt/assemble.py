#!/usr/bin/env python3
"""Create a deterministic saved-evidence package. Never builds or runs targets."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import tarfile

ROOT = Path(__file__).resolve().parent
STUDY = Path('/tmp/oriole-allocator-bolt-study')
BUILD = Path('/tmp/oriole-allocator-provenance-pgo-study')
TOOLS = Path('/home/dev-user/.cache/oriole/bolt/22.1.8')
AUDIT = Path('/tmp/oriole-allocator-bolt-native-independent-audit')


def load(path):
    return json.loads(Path(path).read_text())


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--released', action='store_true')
    args = parser.parse_args()
    assert args.released and os.sched_getaffinity(0) == {6}
    assert not (ROOT / 'evidence.tar.gz').exists()
    plan = load(ROOT / 'package-plan.json')
    records, omitted, payloads, skipped = {}, {}, {}, []
    known = dict(plan['required_pins'])
    prep = load(STUDY / 'preparation.json')
    known.update(prep['pins'])
    known.update(load(AUDIT / 'details.json')['raw_files_sha256'])
    freeze = load(BUILD / 'pair-freeze.json')
    known.update({str(BUILD / name): row['sha256'] for name, row in freeze['files'].items()})
    tool_files = load(TOOLS / 'files.json')['files']
    known.update({str(TOOLS / row['path']): row['sha256'] for row in tool_files})
    for path in [STUDY / 'attempt01/link/report.json', STUDY / 'attempt02/profile/report.json',
                 STUDY / 'attempt02/optimize/report.json']:
        known.update(load(path)['artifacts'])

    def omit(path, reason, expected=None):
        path = Path(path)
        key = str(path)
        expected = expected or known.get(key)
        assert expected is not None, ('Unpinned excluded payload', key)
        row = {'path': key, 'sha256': expected, 'bytes': path.stat().st_size,
               'reason': reason, 'verification': 'recorded producer hash; payload excluded'}
        if key in omitted:
            assert omitted[key]['sha256'] == expected
        else:
            omitted[key] = row

    def add_bytes(key, data, reason, expected=None, origin=None):
        actual = sha(data)
        assert expected is None or actual == expected, ('Hash mismatch', key, expected, actual)
        assert not data.startswith((b'\x7fELF', b'!<arch>\n')), key
        row = {'path': key, 'origin': origin or key, 'sha256': actual,
               'bytes': len(data), 'member': 'payload/' + actual, 'reason': reason}
        if key in records:
            assert records[key]['sha256'] == actual
            return
        assert key not in omitted
        records[key] = row
        assert actual not in payloads or payloads[actual] == data
        payloads[actual] = data

    def add(path, reason, expected=None):
        path = Path(path)
        key = str(path)
        expected = expected or known.get(key)
        if key in records:
            assert expected is None or records[key]['sha256'] == expected
            return
        assert path.is_file(), key
        if path.name.endswith(('.tar.gz', '.tar.xz', '.tar.zst', '.a', '.so')) or '.so.' in path.name:
            # Source tar capsules are expanded separately, never nested. Large
            # tool releases are represented by their verified acquisition pins.
            if expected is None and path.stat().st_size < 4 * 1024 * 1024:
                expected = sha(path.read_bytes())
            omit(path, 'Binary/archive payload omitted; source capsules separately expanded', expected)
            return
        with path.open('rb') as stream:
            prefix = stream.read(8)
        if prefix.startswith((b'\x7fELF', b'!<arch>\n')):
            omit(path, 'Compiled binary/tool payload omitted', expected)
            return
        add_bytes(key, path.read_bytes(), reason, expected)

    def tree(root, reason):
        for path in sorted(Path(root).rglob('*')):
            if not path.is_file():
                continue
            if '__pycache__' in path.parts or path.suffix == '.pyc':
                skipped.append({'path': str(path), 'reason': 'Python bytecode cache'})
                continue
            add(path, reason)

    # Preserve staged wording and record its historical status; the top-level
    # final report is updated only after portable verification succeeds.
    (ROOT / 'staging').mkdir(exist_ok=True)
    for name in ['README.md', 'report.json', 'package-plan.json', 'staging.json']:
        snapshot = ROOT / 'staging' / name
        data = (ROOT / name).read_bytes()
        if snapshot.exists():
            assert snapshot.read_bytes() == data
        else:
            snapshot.write_bytes(data)
        add(snapshot, 'Pre-assembly text staging snapshot; not the final completion report')
    for row in plan['trees']:
        tree(row['path'], row['reason'])
    for path in sorted(Path('/tmp').glob('oriole-allocator-bolt*')):
        if path.is_file():
            add(path, 'Independent reviewer source, result, and all retained reviewer attempts')

    source = load(STUDY / 'selected-source.json')['source_sha256']
    assert source == prep['source_sha256'] and len(source) == 70
    with tarfile.open(STUDY / 'selected-source.tar.gz') as stream:
        members = stream.getmembers()
        assert len(members) == 70 and {m.name for m in members} == set(source)
        for member in members:
            assert member.isfile() and not PurePosixPath(member.name).is_absolute()
            assert '..' not in PurePosixPath(member.name).parts
            add_bytes(str(Path(prep['workspace']) / member.name), stream.extractfile(member).read(),
                      'Exact selected 70-file source reconstructed from frozen source capsule', source[member.name],
                      str(STUDY / 'selected-source.tar.gz') + '#' + member.name)

    add(BUILD / 'pair-freeze.json', 'Original complete matched build freeze, including omitted binary hashes')
    for name, row in freeze['files'].items():
        add(BUILD / name, 'Frozen original-G build/compiler/profile/training evidence', row['sha256'])
    # Add every original native auditor input, retaining recorded-only binary
    # pins. This includes the exact inherited controller source and all fixtures.
    for path, expected in load(AUDIT / 'details.json')['raw_files_sha256'].items():
        add(path, 'Exact dependency of the independently executed native auditor', expected)
    for path, expected in plan['required_pins'].items():
        add(path, 'Required sealed review', expected)

    for path in sorted(TOOLS.iterdir()):
        if path.is_file():
            add(path, 'LLVM release, acquisition, attestation, warning-source license and tool metadata')
    for row in tool_files:
        path = TOOLS / row['path']
        if not row['path'].startswith('selected/'):
            continue
        if 'LICENSE' in path.name:
            add(path, 'LLVM distribution source notice', row['sha256'])
        else:
            omit(path, 'Extracted tool/library binary or linker alias, excluded from payload', row['sha256'])

    expat = Path(plan['expat_provenance']['root'])
    for name in plan['expat_provenance']['files']:
        add(expat / name, 'Pinned Expat source/compiler/training provenance and source notice')
    for mode in ['control', 'generate', 'use']:
        for kind in ['configure', 'compile']:
            add(expat / f'expat-{mode}-{kind}.log', 'Original Expat compiler/configuration log')
    for name, expected in load(expat / 'expat-profile.json')['files'].items():
        add(expat / name, 'Exact GCC profile consumed by the unchanged Expat PGO control', expected)

    manifest = Path(plan['benchmark_inputs']['manifest'])
    add(manifest, 'Fixed held-out input provenance')
    for project in load(manifest)['projects']:
        for entry in project['files']:
            add(manifest.parent / entry['path'], 'Held-out original XML or required license/notice', entry['sha256'])
    for name in ['native_driver.c', 'projects/README.md', 'projects/RERUN.md']:
        add(manifest.parent.parent / name, 'Unchanged benchmark driver and protocol documentation')
    for path in plan['benchmark_inputs']['generated']:
        add(path, 'Original generated native fixture, separate from real aggregate and training')
    for name in plan['notices']['files']:
        add(Path(plan['notices']['oriole_root']) / name, 'Oriole source/derived fixture notice')
    for path in plan['notices']['llvm']:
        add(path, 'LLVM/BOLT license and saved canonical acquisition receipt')
    add(ROOT / 'assemble.py', 'Deterministic package producer; no target execution')
    add(ROOT / 'verify.py', 'Portable member verification and original numerical replay adapter')

    archive = ROOT / 'evidence.tar.gz'
    with archive.open('xb') as output:
        with gzip.GzipFile(filename='', mode='wb', fileobj=output, mtime=0, compresslevel=9) as compressed:
            with tarfile.open(fileobj=compressed, mode='w', format=tarfile.PAX_FORMAT) as stream:
                for digest, data in sorted(payloads.items()):
                    member = tarfile.TarInfo('payload/' + digest)
                    member.size = len(data)
                    member.mode = 0o644
                    member.uid = member.gid = member.mtime = 0
                    stream.addfile(member, io.BytesIO(data))
    index = {'schema_version': 1, 'archive_sha256': sha(archive.read_bytes()),
             'archive_bytes': archive.stat().st_size, 'stored_members': len(payloads),
             'origins': [records[key] for key in sorted(records)],
             'omitted': [omitted[key] for key in sorted(omitted)], 'skipped_caches': skipped,
             'scope': 'Saved BOLT evidence. Omitted ELF/tool hashes are recorded-only; all stored bytes are read back by verify.py.'}
    encoded = (json.dumps(index, indent=2) + '\n').encode()
    (ROOT / 'members.json.gz').write_bytes(gzip.compress(encoded, compresslevel=9, mtime=0))
    # Re-read included source origins once after archive writing. Frozen tar
    # children are verified via their archived capsule read and source manifest.
    for row in records.values():
        if '#' not in row['origin']:
            assert sha(Path(row['origin']).read_bytes()) == row['sha256'], row['origin']
    print(json.dumps({'status': 'assembled_origins_read_back_portable_verifier_pending',
                      'archive_sha256': index['archive_sha256'], 'archive_bytes': index['archive_bytes'],
                      'stored_members': len(payloads), 'origins': len(records),
                      'aliases': len(records) - len(payloads), 'omitted': len(omitted),
                      'members_sha256': sha((ROOT / 'members.json.gz').read_bytes())}, indent=2))


if __name__ == '__main__':
    main()
