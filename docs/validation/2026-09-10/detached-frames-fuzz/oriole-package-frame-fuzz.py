import hashlib
import io
import json
from pathlib import Path
import re
import shutil
import tarfile

root = Path('/tmp/oriole-frame-fuzz-study')
out = Path('/tmp/oriole-frame-fuzz-handoff')
out.mkdir(exist_ok=False)
def sha(data):
    return hashlib.sha256(data).hexdigest()
def digest(path):
    return sha(path.read_bytes())

source = json.loads((root / 'source.json').read_text())
for name, value in source['source_sha256'].items():
    assert digest(Path(source['worktree']) / name) == value, name
build = json.loads((root / 'build.json').read_text())
assert build['status'] == 'passed'
summary = json.loads((root / 'summary.json').read_text())
assert summary['status'] == 'passed' and len(summary['targets']) == 6
final_members = []
with tarfile.open(out / 'final-corpus.tar.gz', 'w:gz', compresslevel=9) as archive:
    for target in summary['targets']:
        directory = root / 'campaigns' / target['target']
        result = json.loads((directory / 'result.json').read_text())
        assert digest(directory / 'result.json') == target['result_sha256']
        assert result['passed'] and not result['artifacts']
        assert all(row['exit_code'] == 0 for row in result['runs'])
        assert result['runs'][1]['elapsed_seconds'] >= 600
        binary = root / 'binaries' / target['target']
        assert digest(binary) == build['binaries'][target['target']] == result['binary_sha256']
        initial = json.loads((directory / 'initial-manifest.json').read_text())['inputs']
        actual = {p.name: p.stat().st_size for p in (directory / 'initial-corpus').iterdir()}
        assert actual == initial
        assert all(digest(directory / 'initial-corpus' / name) == name for name in initial)
        stats = {key: int(value) for key, value in re.findall(r'stat::(\w+):\s+(\d+)', (directory / 'campaign.log').read_text(errors='replace'))}
        assert stats == result['stats'] and stats['number_of_executed_units'] == target['executed_units']
        corpus = directory / 'corpus'
        assert {p.name: digest(p) for p in corpus.iterdir()} == result['final_corpus']
        for path in sorted(corpus.iterdir()):
            data = path.read_bytes()
            assert hashlib.sha1(data).hexdigest() == path.name
            name = target['target'] + '/' + path.name
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = 0o644
            info.mtime = 0
            archive.addfile(info, io.BytesIO(data))
            final_members.append({'path': name, 'bytes': len(data), 'sha256': sha(data)})
(out / 'final-corpus-members.json').write_text(json.dumps(final_members, indent=2) + '\n')
with tarfile.open(out / 'final-corpus.tar.gz', 'r:gz') as archive:
    entries = archive.getmembers()
    assert len(entries) == len(final_members)
    for actual, expected in zip(entries, final_members):
        assert actual.isfile() and actual.name == expected['path'] and actual.size == expected['bytes']
        assert sha(archive.extractfile(actual).read()) == expected['sha256']

paths = {}
excluded = []
for path in sorted(root.rglob('*')):
    if not path.is_file() or path.is_symlink() or '__pycache__' in path.parts:
        continue
    relative = path.relative_to(root)
    if len(relative.parts) >= 3 and relative.parts[0] == 'campaigns' and relative.parts[2] in ['corpus', 'initial-corpus']:
        continue
    paths[str(relative)] = path
paths['controllers/oriole-build-frame-fuzz.py'] = Path('/tmp/oriole-build-frame-fuzz.py')
paths['controllers/oriole-package-frame-fuzz.py'] = Path(__file__)
members = []
with tarfile.open(out / 'evidence.tar.gz', 'w:gz', compresslevel=9) as archive:
    for name, path in sorted(paths.items()):
        data = path.read_bytes()
        entry = {'path': name, 'source': str(path), 'bytes': len(data), 'sha256': sha(data)}
        if data.startswith(b'\x7fELF') or data.startswith(b'!<arch>\n'):
            excluded.append(entry)
            continue
        info = tarfile.TarInfo(name)
        info.size = len(data)
        info.mode = 0o644
        info.mtime = 0
        archive.addfile(info, io.BytesIO(data))
        members.append(entry)
with tarfile.open(out / 'evidence.tar.gz', 'r:gz') as archive:
    entries = archive.getmembers()
    assert len(entries) == len(members)
    for actual, expected in zip(entries, members):
        assert actual.isfile() and actual.name == expected['path'] and actual.size == expected['bytes']
        assert sha(archive.extractfile(actual).read()) == expected['sha256']
        assert digest(Path(expected['source'])) == expected['sha256']
(out / 'members.json').write_text(json.dumps({'members': members, 'excluded_binaries': excluded}, indent=2) + '\n')
for name in ['source.json', 'build.json', 'summary.json', 'corpus-provenance.json']:
    shutil.copy2(root / name, out / name)
report = {'status': 'passed', 'source_files': len(source['source_sha256']), 'targets': 6,
          'executed_units': summary['total_executed_units'], 'initial_raw_files': summary['total_initial_inputs'],
          'initial_empty_files': 3, 'replay_runs': 26692,
          'replay_note': 'libFuzzer loads26686 nonempty files and separately tests empty input for each of six targets. All26689 retained raw files, including3 empty files, are preserved. Every initial file is below the configured64KiB bound.',
          'final_corpus_files': len(final_members), 'final_corpus_sha256': digest(out / 'final-corpus.tar.gz'),
          'evidence_members': len(members), 'excluded_binaries': len(excluded), 'evidence_sha256': digest(out / 'evidence.tar.gz'),
          'readback_all_members_verified': True,
          'scope': 'Measured source5bc806e only; ASan with external Clang runtime, leak detection disabled. Counts include initialization and repeated executions. Final corpus separately retained, never substituted for the original corpus.'}
(out / 'readback.json').write_text(json.dumps(report, indent=2) + '\n')
shutil.copy2(__file__, out / Path(__file__).name)
print(json.dumps(report, indent=2), flush=True)
