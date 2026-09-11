"""Verify and replay a copied CPython evidence package using records only."""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import subprocess
import sys
import tarfile
import tempfile

cli = argparse.ArgumentParser(description=__doc__)
cli.add_argument('--package', type=Path, required=True)
cli.add_argument('--output', type=Path, required=True)
args = cli.parse_args()
assert sys.version_info[:2] == (3, 12)
assert not args.output.exists()
package = args.package.resolve()
sha = lambda data: hashlib.sha256(data).hexdigest()
load = lambda path: json.loads(path.read_text())
manifest = load(package / 'files.json')
for name, row in manifest.items():
    path = package / name
    assert path.stat().st_size == row['bytes'] and sha(path.read_bytes()) == row['sha256'], name
index = load(package / 'index.json')
report = load(package / 'report.json')
assert sha((package / 'index.json').read_bytes()) == report['index_sha256']
assert sha((package / 'evidence.tar.gz').read_bytes()) == report['evidence_sha256']
expected = {row['path']: row for row in index['files']}
assert len(expected) == len(index['files']) == report['archive_files']
assert len(index['excluded_binaries']) == report['excluded_binary_files']
results = {}
with tempfile.TemporaryDirectory(prefix='oriole-python-records-') as temp:
    root = Path(temp)
    with tarfile.open(package / 'evidence.tar.gz') as archive:
        seen = set()
        for member in archive:
            name = member.name
            path = PurePosixPath(name)
            assert member.isfile() and not path.is_absolute() and '..' not in path.parts
            assert name in expected and name not in seen
            seen.add(name)
            data = archive.extractfile(member).read()
            row = expected[name]
            assert len(data) == row['bytes'] == member.size and sha(data) == row['sha256']
            assert not data.startswith(b'\x7fELF')
            destination = root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
        assert seen == set(expected)
    preflight_path = root / 'review/preflight/oriole-capacity-python-independent-preflight-review.json'
    assert sha(preflight_path.read_bytes()) == '172385ca2d471dc21dce5f516e252206dac5d3f9548829f2781cd0afe26d3066'
    assert load(preflight_path)['status'] == 'passed_independent_source_build_preflight_review'
    protocol_path = root / 'review/protocol/review.json'
    assert sha(protocol_path.read_bytes()) == '4c59d062677fec3f7c839c8a3220f2981bb121ed6ae61a7f85e228a05cbc115b'
    assert load(protocol_path)['status'] == 'passed'
    for label in ('capacity', 'context'):
        directory = root / 'builds' / label
        source = load(directory / 'source.json')['source_sha256']
        assert len(source) == 72
        with tarfile.open(directory / 'source.tar.gz') as archive:
            members = archive.getmembers()
            assert len(members) == 72 and {m.name for m in members} == set(source)
            for member in members:
                assert member.isfile() and sha(archive.extractfile(member).read()) == source[member.name]
        pair = load(directory / 'pair-report.json')
        assert pair['status'] == 'passed'
        for mode in ('normal', 'pgo'):
            adaptation = load(root / 'study' / mode / 'adaptation.json')
            assert adaptation['source_runtime_candidate'] == report['candidate']
            assert adaptation['source_runtime_control'] == report['control']
            prefix = 'candidate' if label == 'capacity' else 'selected_control'
            assert adaptation[prefix + '_source_manifest']['sha256'] == sha((directory / 'source.json').read_bytes())
            assert adaptation[prefix + '_pair_freeze']['sha256'] == sha((directory / 'pair-freeze.json').read_bytes())
            proof = load(directory / 'vector-training-proof.json')
            assert proof['status'] == 'passed'
            assert proof['source_sha256'] == sha((directory / 'source.json').read_bytes())
            assert proof['pair_report_sha256'] == sha((directory / 'pair-report.json').read_bytes())
            field, key = ('normal_libraries', 'liboriole_expat.so') if mode == 'normal' else ('pgo_libraries', 'use/liboriole_expat.so')
            engine = 'candidate' if label == 'capacity' else 'control'
            assert pair[field][key] == adaptation['libraries'][engine]['sha256']
    assert (package / 'replay.py').read_bytes() == (root / 'review/elapsed/audit.py').read_bytes()
    for mode in ('normal', 'pgo'):
        destination = root / 'replayed' / mode
        command = [sys.executable, '-I', '-S', str(package / 'replay.py'), '--root',
                   str(root / 'study'), '--mode', mode, '--output', str(destination)]
        run = subprocess.run(command, capture_output=True, text=True, timeout=300)
        assert run.returncode == 0, (run.returncode, run.stdout, run.stderr)
        actual = load(destination / (mode + '-review.json'))
        original = load(root / 'review/elapsed' / (mode + '-review.json'))
        for key in ('counts', 'aggregates', 'summary', 'libraries', 'seed',
                    'pair_order_verified', 'raw_rows_exactly_reconstructed',
                    'source_results_sha256', 'source_preflight_sha256'):
            assert actual[key] == original[key], (mode, key)
        for key in ('counts', 'aggregates', 'libraries'):
            assert actual[key] == report['modes'][mode][key]
        aliases = mode + '-row-aliases.json'
        assert (destination / aliases).read_bytes() == (root / 'review/elapsed' / aliases).read_bytes()
        results[mode] = {'exit': run.returncode, 'counts': actual['counts'],
                         'aggregates': actual['aggregates'],
                         'stdout_sha256': sha(run.stdout.encode()),
                         'stderr_sha256': sha(run.stderr.encode())}
out = {'status': 'passed_portable_records_replay', 'archive_files': len(expected),
       'archive_sha256': report['evidence_sha256'], 'source_files': {'capacity': 72, 'context': 72},
       'modes': results, 'scope': 'Hashes, original records, both source archives and all Python '
                'sample/output/origin/order arithmetic replayed without original paths or binary imports. '
                'Actual compiler/library byte verification belongs to the included original reviews.'}
args.output.write_text(json.dumps(out, indent=2) + '\n')
print(json.dumps({'status': out['status'], 'archive_files': len(expected),
                  'workers': sum(v['counts']['all_workers'] for v in results.values()),
                  'samples': sum(v['counts']['all_samples'] for v in results.values())}, indent=2))
