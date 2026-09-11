"""Portable saved-artifact verification; never load parser or compiler binaries."""
from pathlib import Path
import hashlib
import io
import json
import math
import statistics
import tarfile

def verify(directory):
    directory = Path(directory)
    sha = lambda data: hashlib.sha256(data).hexdigest()
    inventory = json.loads((directory / 'inventory.json').read_text())
    payloads = {}
    with tarfile.open(directory / 'evidence.tar.gz') as archive:
        for member in archive:
            assert member.isfile() and not member.name.startswith('/') and '..' not in Path(member.name).parts
            assert member.name not in payloads
            payloads[member.name] = archive.extractfile(member).read()
    assert set(payloads) == {row['archive_member'] for row in inventory['files'].values()}
    files = {}
    for logical, row in inventory['files'].items():
        data = payloads[row['archive_member']]
        assert len(data) == row['bytes'] and sha(data) == row['sha256'], logical
        files[logical] = data
    manifest = json.loads(files['inputs/corpus-manifest.json'])
    expected_notices = []
    for project in manifest['projects']:
        fixture = files.get('inputs/' + project['name'] + '.xml')
        if fixture is None:
            continue
        inputs = [entry for entry in project['files'] if entry['role'] == 'input']
        assert len(inputs) == 1 and sha(fixture) == inputs[0]['sha256']
        for entry in project['files']:
            if entry['role'] in ['license', 'notice']:
                logical = 'inputs/' + entry['path']
                assert len(files[logical]) == entry['bytes'] and sha(files[logical]) == entry['sha256']
                expected_notices.append({'project': project['name'], 'role': entry['role'],
                    'manifest_relative_path': entry['path'], 'logical_path': logical,
                    'bytes': entry['bytes'], 'sha256': entry['sha256']})
    assert len(expected_notices) == 9
    assert inventory['redistribution_notices']['corpus'] == expected_notices
    oriole_notices = inventory['redistribution_notices']['oriole']
    assert {entry['source_relative_path'] for entry in oriole_notices} == {
        'LICENSE-APACHE', 'LICENSE-MIT', 'tests/c/UPSTREAM-NOTICES.txt', 'THIRD_PARTY_LICENSES.md'}
    for entry in oriole_notices:
        assert entry['logical_path'] == 'source-notices/oriole/' + entry['source_relative_path']
        assert len(files[entry['logical_path']]) == entry['bytes']
        assert sha(files[entry['logical_path']]) == entry['sha256']
    for row in inventory['raw_worker_aliases']:
        value = json.loads(files[row['container']])
        for component in row['json_pointer_components']:
            value = value[component]
        raw = {key: value for key, value in value.items() if key not in ['observations', 'median_seconds']}
        data = (json.dumps(raw, indent=2) + '\n').encode()
        assert len(data) == row['bytes'] and sha(data) == row['sha256']
        assert row['byte_equivalence'] == 'verified_exact'
    assert len(inventory['raw_worker_aliases']) == 1344
    source = json.loads(files['build/source.json'])['source_sha256']
    with tarfile.open(fileobj=io.BytesIO(files['build/source.tar.gz'])) as archive:
        archived = {member.name: sha(archive.extractfile(member).read()) for member in archive if member.isfile()}
    assert len(source) == 70 and archived == source
    assert source == json.loads(files['control/source.json'])['source_sha256']
    audit = json.loads(files['audits/native.json'])
    assert sha(files['audits/native.json']) == 'd18e234595fa4dde63ddb86daa1a4fe9f7e5289cf5a902c00abe462a11d37617'
    assert sha(files['audits/build.json']) == '7c89c6a103f60d7f50d05eb42c1d2aedb82fda45a05d7fe069b1d0c66c80d696'
    assert sha(files['build/proof-attempt01.log']) == '0b69d0afbf02e54034af4825df46c596c5414257fa4cb88672abb69120f73e53'
    samples = 0
    for mode in ['normal', 'pgo']:
        root = 'native/' + mode + '/native-screen/'
        preflight, results = (json.loads(files[root + name]) for name in ['preflight.json', 'results.json'])
        assert preflight['status'] == results['status'] == 'passed'
        assert len(preflight['rows']) == 84 and len(results['rows']) == 196 and len(results['summary']) == 28
        rows = preflight['rows'] + [process for cohort in results['rows'] for process in cohort['processes']]
        assert len(rows) == 672
        for row in rows:
            assert row['returncode'] == 0 and row['stderr'] == ''
            raw_samples = json.loads(row['stdout'])['samples']
            samples += len(raw_samples)
            assert all(math.isfinite(s['seconds']) and s['seconds'] > 0 for s in raw_samples)
            assert [s['warmup'] for s in raw_samples] == [True] + [False] * (len(raw_samples) - 1)
            assert statistics.median(s['seconds'] for s in raw_samples[1:]) == row['median_seconds']
        ratios = [{'condition': row['condition'], 'median_ratios': row['median_ratios']} for row in results['summary']]
        assert ratios == audit['native'][mode]['all_conditions']
    assert samples == 212352
    return {'status': 'passed_saved_artifact_readback', 'source_files': 70,
        'unique_archive_members': len(payloads), 'logical_files': len(files),
        'corpus_license_notice_files': 9, 'oriole_notice_files': 4,
        'worker_aliases_reconstructed': 1344, 'raw_samples': samples, 'conditions': 56,
        'archive_sha256': sha((directory / 'evidence.tar.gz').read_bytes()),
        'inventory_sha256': sha((directory / 'inventory.json').read_bytes()),
        'limits': 'Transport/source/alias/sample readback only. Original full protocol/build audits remain separate unchanged evidence. No parser, compiler, profiler or timing execution.'}

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    print(json.dumps(verify(parser.parse_args().directory), indent=2))
