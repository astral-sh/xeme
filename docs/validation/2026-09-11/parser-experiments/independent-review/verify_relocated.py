"""Independently inspect sealed packages and run their saved-records replayers.

No package builder, compiler, parser, profiler, or elapsed worker is executed.
"""
import argparse
import csv
import difflib
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parent
PINS = {
    'default-namespace-guard': {
        'manifest':'7f020c48d005383e26ba5fea9ed66bab788d1b4d22c2759f95b83c2c3c0f0d96',
        'native':'df0c80bac70689b2c4fcd906d4e37eeabc13f868f509eda3bfa12ac94c0f1a19',
        'build':'fdc59331aff03afa911db6fd83fec5e3d20cd0dbb7d6984c064050735c96def5',
        'source':'0823604d724fd28d3f5cc1fb50358f859266ed25e53b6eb4346a4efcf4e5a8e0',
        'tests':438,
        'changed':['crates/oriole/src/lib.rs','crates/oriole/tests/adapter_frame.rs'],
    },
    'cdata-prefilter': {
        'manifest':'a902c4dca74608eca5731f1ff864d9ff069dc7804e55429eb26b5d351a7e9dfe',
        'native':'a158aa14709a035cbabfa76fc0e5ac4e459e467403b183a7aa1a9f8ecf762edc',
        'build':'92c456b019f1ba4331b7345695d661ec16ec79a500419948dbb2fa3d81785486',
        'source':'0a41677d3a99d3be014be60c3f5dd19d000ca4d368f6f72a9d5ca8989ac6839a',
        'tests':439,
        'changed':['crates/oriole/src/lib.rs'],
    },
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--released', action='store_true')
    args = parser.parse_args()
    assert args.released and __debug__ and os.sched_getaffinity(0) == {6}
    assert sha(Path(sys.executable).read_bytes()) == '021044895e95be79dc2f110367607e684119afbc8ce75f6f0eec94844e0acec7'
    attempt = ROOT / 'attempt01'
    attempt.mkdir(exist_ok=False)
    results = {}
    for name, pins in PINS.items():
        origin = Path('/tmp/oriole-' + name + '-publication')
        destination = attempt / name
        destination.mkdir()
        manifest_data = (origin / 'files.json').read_bytes()
        assert sha(manifest_data) == pins['manifest']
        manifest = json.loads(manifest_data)
        (destination / 'files.json').write_bytes(manifest_data)
        copied = 0
        for path, identity in manifest['files'].items():
            assert Path(path).name == path
            data = (origin / path).read_bytes()
            assert (sha(data),len(data)) == (identity['sha256'],identity['bytes'])
            copied += len(data)
            assert copied < 20 * 1024**2
            (destination / path).write_bytes(data)
        outcome = subprocess.run(
            [sys.executable, '-I', '-S', str(destination / 'verify.py'),
             '--package', str(destination), '--output', str(destination / 'relocated-readback.json')],
            cwd=destination, capture_output=True, timeout=180, check=False,
        )
        (destination / 'verify.stdout').write_bytes(outcome.stdout)
        (destination / 'verify.stderr').write_bytes(outcome.stderr)
        process = {'returncode':outcome.returncode,'reaped':True,'stdout_sha256':sha(outcome.stdout),'stderr_sha256':sha(outcome.stderr)}
        (destination / 'process.json').write_text(json.dumps(process,indent=2)+'\n')
        assert outcome.returncode == 0 and outcome.stderr == b''
        receipt = json.loads((destination / 'relocated-readback.json').read_bytes())
        assert receipt['status'] == 'passed'
        index = json.loads((destination / 'archive-index.json').read_bytes())
        objects = {}
        with tarfile.open(destination / 'evidence.tar.gz', 'r:gz') as archive:
            for member in archive:
                if member.name.startswith('objects/'):
                    data = archive.extractfile(member).read()
                    assert sha(data) == member.name.removeprefix('objects/')
                    objects[sha(data)] = data
        files = {row['path']:row for row in index['files']}
        def get(path):
            return objects[files[path]['sha256']]
        source_data = get(name+'-pgo-study/source.json')
        assert sha(source_data) == pins['source']
        source = json.loads(source_data)['source_sha256']
        control = json.loads(get('selected-control/source.json'))['source_sha256']
        assert len(source) == len(control) == 72 and set(source) == set(control)
        assert sorted(p for p in source if source[p] != control[p]) == pins['changed']
        allocator = 'crates/oriole_storage/src/allocator.rs'
        assert source[allocator] == control[allocator] == '910ce7ead099bd6a306a037c77fa276f746ac0eac59f14d8bda9b9b1c2deeac7'
        native_bytes = get(name+'-independent-review/native/review.json')
        build_bytes = get(name+'-independent-review/build/review.json')
        assert sha(native_bytes) == pins['native'] and sha(build_bytes) == pins['build']
        native, build = json.loads(native_bytes), json.loads(build_bytes)
        assert native['status'] == build['status'] == 'passed'
        assert build['focused_checks']['tests'] == pins['tests']
        assert build['source_sha256'] == pins['source']
        csv_bytes = (destination / 'conditions.csv').read_bytes()
        assert b'\r' not in csv_bytes
        rows = list(csv.DictReader(io.StringIO(csv_bytes.decode())))
        assert len(rows) == 56
        expected = [(mode,row) for mode in ('normal','pgo') for row in native['native'][mode]['all_conditions']]
        for row, (mode, reference) in zip(rows, expected, strict=True):
            condition = reference['condition']
            assert row['mode'] == mode and row['name'] == condition['name']
            assert int(row['chunk']) == condition['chunk']
            assert row['namespaces'] == str(condition['namespaces'])
            for key, value in reference['median_ratios'].items():
                assert float(row[key]) == value
        corpus = json.loads(get('reproduce/benchmarks/projects/corpus-manifest.json'))
        fixture_count = notice_count = 0
        for project in corpus['projects']:
            for item in project['files']:
                if item['role'] in ('input','license','notice'):
                    assert sha(get('reproduce/benchmarks/projects/'+item['path'])) == item['sha256']
                    fixture_count += item['role'] == 'input'
                    notice_count += item['role'] != 'input'
        assert fixture_count == 6 and notice_count == 9
        legal = [p for p in files if p.startswith('notices/')]
        assert len(legal) == 5
        for path in legal:
            assert get(path) and sha(Path(files[path]['origin']).read_bytes()) == files[path]['sha256']
        # Independently establish every retained object still matches its recorded
        # source. Excluded binaries/caches remain identities only in this review.
        for path, row in files.items():
            original = Path(row['origin']).read_bytes()
            assert original == get(path) and len(original) == row['bytes'], path
        readme = (destination / 'README.md').read_text()
        for mode in ('normal','pgo'):
            groups = native['native'][mode]['groups']
            for kind in ('real','generated'):
                assert format(groups[kind]['ratios']['candidate_over_control'],'.6f') in readme
                adverse = groups[kind]['conditions'] - groups[kind]['candidate_faster_than_control']
                assert f'{adverse}/{groups[kind]["conditions"]}' in readme
        assert format(native['native']['pgo']['groups']['real']['ratios']['candidate_over_expat'],'.6f') in readme
        results[name] = {'status':'passed','manifest_sha256':pins['manifest'],'archive_sha256':sha((destination/'evidence.tar.gz').read_bytes()),'readme_sha256':sha(readme.encode()),'relocated_receipt_sha256':sha((destination/'relocated-readback.json').read_bytes()),'process':process,'logical_files_origin_exact':len(files),'unique_objects':len(objects),'source_files':72,'exact_changed_files':pins['changed'],'allocator_e1_unchanged':True,'tests':pins['tests'],'csv_rows':56,'csv_lf_only_and_all_ratios_exact':True,'project_fixtures':fixture_count,'project_licenses_notices':notice_count,'oriole_expat_notices':len(legal),'worker_aliases':receipt['worker_aliases'],'replayed_totals':receipt['native']['totals']}
    old = attempt/'default-namespace-guard'
    new = attempt/'cdata-prefilter'
    assert (new/'replay_native.py').read_text() == (old/'replay_native.py').read_text().replace('default-namespace-guard','cdata-prefilter')
    assert (new/'verify.py').read_text() == (old/'verify.py').read_text().replace('namespace-guard evidence','CDATA-prefilter evidence').replace('default-namespace-guard','cdata-prefilter')
    for filename in ('package.py','verify.py','replay_native.py'):
        patch = ''.join(difflib.unified_diff((old/filename).read_text().splitlines(True),(new/filename).read_text().splitlines(True),fromfile=str(Path('/tmp/oriole-default-namespace-guard-publication')/filename),tofile=str(Path('/tmp/oriole-cdata-prefilter-publication')/filename)))
        patch_name = {'package.py':'package-adaptation.patch','verify.py':'verify-adaptation.patch','replay_native.py':'replay-adaptation.patch'}[filename]
        assert patch == (new/patch_name).read_text()
    review = {'status':'passed','role':'Independent readback of root-adapted CDATA packaging, plus repeat relocated verification of namespace packaging previously authored by this reviewer. Native/source/compiler arithmetic reviews have their separately disclosed roles; this is not an independent-author review of every inherited script.','packages':results,'adapters':'Exact reviewed patches; numerical replay changes only logical root; verification changes only caption/logical archive paths.','source_claims':'Both remain rejected and selected runtime unchanged. CDATA source-assessment/patch preparation is historical source-only evidence, distinct from later439-test source72 and elapsed records.','no_targets_or_package_builder_executed':True,'excluded_binary_bytes_checked':False,'reader_sha256':sha(Path(__file__).read_bytes())}
    with (ROOT/'review.json').open('x') as file:
        file.write(json.dumps(review,indent=2)+'\n')
    print(json.dumps({'status':'passed','review_sha256':sha((ROOT/'review.json').read_bytes()),'packages':results},indent=2))


if __name__ == '__main__':
    main()
