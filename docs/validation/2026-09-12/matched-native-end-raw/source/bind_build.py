"""Compare the completed candidate build to the selected normal build."""
from pathlib import Path
import hashlib
import json
import os
import re
import shlex
import subprocess

assert __debug__ and os.sched_getaffinity(0) == {6}
root = Path('/home/dev-user/code/oss/oriole-matched-native-end-raw')
PREP = Path(__file__).parent
out = Path('/tmp/oriole-matched-native-end-raw-study')
control = Path('/tmp/oriole-core-owned-text-raw-study')
base = '008d818237fe0d6f8c04a000a98ad69ab5780ceb'
read = lambda path: json.loads(Path(path).read_text())
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
report_path = out / 'build-binding.json'
assert not report_path.exists()
report = {'status': 'incomplete', 'scope': 'Saved normal source/compiler/artifact comparison only. No target execution or compiler-setting changes.'}

def normalize(vector):
    return [str(Path(vector[0]).resolve(strict=True))] + [
        re.sub(r'/home/dev-user/\.cache/ohm/verified-build/[0-9a-f]{2}/[0-9a-f]{14}(?=/)', '@WORKSPACE_BUILD@', arg)
        for arg in vector[1:]
    ]

def vectors(values):
    result = {v[v.index('--crate-name') + 1]: normalize(v) for v in values}
    assert len(values) == len(result) == 3
    assert set(result) == {'oriole', 'oriole_expat', 'oriole_storage'}
    return result


def policy_vector(vector):
    """Retain every flag/extern and order; normalize only Cargo identities and known dependency fingerprints."""
    result=[];i=0
    while i<len(vector):
        value=vector[i]
        if value=='-C' and vector[i+1].startswith(('metadata=','extra-filename=')):
            result.extend([value,vector[i+1].split('=',1)[0]+'=@CARGO_ID@']);i+=2;continue
        value=re.sub(r'(lib(?:allocator_api2|hashbrown|memchr|oriole|oriole_storage|wide|safe_arch|bytemuck))-[0-9a-f]{16}(?=\.r(?:lib|meta)$)',r'\1-@CARGO_ID@',value)
        result.append(value);i+=1
    return result

try:
    build, old = read(out / 'build.json'), read(control / 'build.json')
    source, old_source = read(out / 'source.json'), read(control / 'source.json')
    old_binding = read(control / 'build-binding.json')
    assert build['status'] == old['status'] == old_binding['status'] == 'passed'
    assert build['source_unchanged'] and old['source_unchanged']
    assert old_binding['build_sha256'] == sha(control / 'build.json')
    assert old_binding['source_manifest_sha256'] == sha(control / 'source.json')
    assert source['base'] == base
    assert sha(out / 'source.json') == build['source_sha256']
    assert sha(control / 'source.json') == old['source_sha256']
    assert set(source['sha256']) == set(old_source['sha256']) and len(source['sha256']) == 74
    prepared = read(PREP / 'source.json')
    build_inputs = read(PREP / 'build-inputs.json')
    inputs = read(PREP / 'inputs.json')
    assert sha(PREP / 'inputs.json') == build_inputs['inputs_sha256']
    assert sha(__file__) == build_inputs['binder_sha256']
    assert sha(control / 'source.json') == inputs['control_source_sha256']
    assert sha(control / 'build.json') == inputs['control_build_sha256']
    assert sha(control / 'build-binding.json') == inputs['control_binding_sha256']
    assert sha(PREP / 'source.json') == build_inputs['source_sha256']
    assert sha(PREP / 'build.py') == build_inputs['build_script_sha256']
    assert prepared['base'] == base and len(prepared['sha256']) == 74
    assert source['sha256'] == prepared['sha256']
    assert all(sha(root / p) == h for p, h in source['sha256'].items())
    assert prepared['control_source_manifest_sha256'] == sha(control / 'source.json')
    baseline = read(PREP / 'baseline-source.json')
    assert sha(PREP / 'baseline-source.json') == inputs['baseline_source_sha256']
    assert prepared['baseline_sha256'] == baseline['sha256'] == inputs['source74']
    fixture = 'crates/oriole_expat/src/context_text_tests.rs'
    correction = inputs['control_fixture_correction']
    assert {name for name in old_source['sha256'] if old_source['sha256'][name] != baseline['sha256'][name]} == {fixture}
    assert old_source['sha256'][fixture] == correction['original_sha256']
    assert baseline['sha256'][fixture] == source['sha256'][fixture] == correction['corrected_sha256']
    assert sha(correction['path']) == correction['sha256'] == sha(PREP / 'baseline-fixture-fix.patch')
    for name, digest in baseline['sha256'].items():
        assert hashlib.sha256(subprocess.check_output(['/usr/bin/git','--no-optional-locks','show',base+':'+name],cwd=root)).hexdigest() == digest
    delta = {name: {'control': old_source['sha256'][name], 'candidate': value}
             for name, value in source['sha256'].items() if value != baseline['sha256'][name]}
    assert set(delta) == set(inputs['source_delta'])
    control_delta = {name: {'control': old_source['sha256'][name], 'candidate': value}
                     for name, value in source['sha256'].items() if value != old_source['sha256'][name]}
    assert set(control_delta) == set(delta) | {fixture}
    report['control_fixture_correction'] = correction
    report['compiled_control_source_delta'] = control_delta
    report['baseline_source_manifest_sha256'] = sha(PREP / 'baseline-source.json')
    assert sha(out / 'candidate.patch') == build_inputs['git_patch_sha256']
    assert sha(PREP / 'change.patch') == prepared['patch_sha256'] == build_inputs['patch_sha256']
    assert (out / 'candidate.patch').read_bytes() == (PREP / 'candidate.patch').read_bytes()
    assert set(delta) == set(prepared['changed_files'])
    actual_patch = subprocess.check_output(['/usr/bin/git', '--no-optional-locks', 'diff', '--no-ext-diff', '--binary', base, '--', *sorted(delta)], cwd=root)
    assert actual_patch == (out / 'candidate.patch').read_bytes()
    preparation_root = Path('/tmp/oriole-text-lane-masks-preparation')
    auxiliary = prepared['auxiliary_files']
    assert build['auxiliary_files'] == auxiliary == old_binding['auxiliary_files'] and build['auxiliary_unchanged']
    assert set(auxiliary) == {'benchmarks/inprocess/Cargo.toml', 'benchmarks/inprocess/Cargo.lock', 'fuzz/Cargo.toml', 'fuzz/Cargo.lock'}
    assert all(sha(root / name) == digest for name,digest in auxiliary.items())
    for name in ['Cargo.toml','Cargo.lock','crates/oriole/Cargo.toml']:
        assert source['sha256'][name] == old_source['sha256'][name]
    metadata_receipt_path = preparation_root / 'format-metadata.json'
    metadata_receipt = read(metadata_receipt_path)
    assert sha(metadata_receipt_path) == old_binding['metadata_receipt_sha256'] == build['metadata_receipt_sha256']
    assert metadata_receipt['status'] == 'passed'
    assert {r['label'] for r in metadata_receipt['commands']} == {'format', 'main', 'benchmark', 'fuzz'}
    for command in metadata_receipt['commands']:
        assert command['exit'] == 0 and command['reaped']
        for stream,digest in command['output_sha256'].items():
            assert sha(preparation_root / (command['label']+'.'+stream)) == digest
    assert set(subprocess.check_output(['/usr/bin/git','--no-optional-locks','diff',base,'--name-only'],cwd=root,text=True).splitlines()) == set(delta)
    dependency_review_path = preparation_root / 'metadata-independent-review.json'
    assert build['dependency_review_sha256'] == old_binding['dependency_review_sha256'] == sha(dependency_review_path) == '45cd765fc1dd4759e2cc92dda846f5cdba4a51d1c978aa573c979888a235ba04'
    dependency_review = read(dependency_review_path)
    assert dependency_review['status'] == 'passed_saved_metadata_review'
    assert dependency_review['source_manifest_sha256'] == sha(preparation_root / 'source.json')
    commands = {row['label']: row for row in build['commands']}
    old_commands = {row['label']: row for row in old['commands']}
    assert set(old_commands) == {'rustc-version', 'fmt', 'tests', 'clippy', 'release', 'benchmark-fmt', 'benchmark-clippy'}
    assert set(commands) == set(old_commands)
    for label, row in commands.items():
        assert row['exit'] == 0 and row['reaped']
        assert row['argv'] == old_commands[label]['argv']
        assert row['pid'] > 0 and row['end'] >= row['start']
        assert sha(out / (label + '.log')) == row['log_sha256']
    assert build['rustc'] == old['rustc'] == (out / 'rustc-version.log').read_text()
    parsed_vectors = []
    dependency_vectors = []
    for line in (out / 'release.log').read_text().splitlines():
        if 'Running `' in line and '/rustc ' in line:
            vector = shlex.split(line.strip()[len('Running `'):-1])
            if vector[vector.index('--crate-name') + 1] in {'wide','safe_arch','bytemuck'}:
                dependency_vectors.append(vector)
            if vector[vector.index('--crate-name') + 1] in {'oriole', 'oriole_expat', 'oriole_storage'}:
                parsed_vectors.append(vector)
    assert parsed_vectors == build['compiler_vectors']
    assert dependency_vectors == build['dependency_compiler_vectors']
    assert len(dependency_vectors) == 3
    dependency_by_name = {v[v.index('--crate-name')+1]:v for v in dependency_vectors}
    assert set(dependency_by_name) == {'wide','safe_arch','bytemuck'}
    metadata = read(preparation_root / 'main.stdout')
    packages = {p['name']:p for p in metadata['packages']}
    for name, vector in dependency_by_name.items():
        lib_target, = [t for t in packages[name]['targets'] if t['kind'] == ['lib']]
        assert lib_target['src_path'] in vector
        assert vector[vector.index('--target')+1] == 'x86_64-unknown-linux-gnu'
        assert 'opt-level=3' in vector and 'codegen-units=1' in vector and 'linker-plugin-lto' in vector
        assert not any('profile-generate' in x or 'profile-use' in x or 'target-cpu' in x or 'target-feature' in x for x in vector)
        features = {vector[i+1] for i,x in enumerate(vector) if x == '--cfg'}
        assert features == ({'feature="default"','feature="bytemuck"'} if name == 'safe_arch' else set())
    current_vectors, old_vectors = vectors(parsed_vectors), vectors(old['compiler_vectors'])
    assert {name: policy_vector(v) for name,v in current_vectors.items()} == {name: policy_vector(v) for name,v in old_vectors.items()}
    assert {v[v.index('--crate-name')+1]:policy_vector(normalize(v)) for v in dependency_vectors} == {v[v.index('--crate-name')+1]:policy_vector(normalize(v)) for v in old['dependency_compiler_vectors']}
    wide_paths = [v[i+1].split('=',1)[1] for v in parsed_vectors for i,x in enumerate(v[:-1]) if x == '--extern' and v[i+1].startswith('wide=')]
    assert len(wide_paths) == 1 and Path(wide_paths[0]).is_file()

    assert all(sha(p) == h for p, h in old_binding['tool_hashes'].items())
    for study, data in [(out, build), (control, old)]:
        assert set(data['libraries']) == {'liboriole_expat.so', 'liboriole_expat.a'}
        assert all(sha(study / 'normal' / name) == digest for name, digest in data['libraries'].items())
    target = Path('/home/dev-user/.cache/oriole/matched-native-end-raw-target/x86_64-unknown-linux-gnu/release')
    assert all(sha(target / name) == digest for name, digest in build['libraries'].items())
    assert sha('/tmp/oriole-pgo-study/expat-control-liboriole_expat.so') == '7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478'
    assert old['libraries']['liboriole_expat.so'] == 'e59d89d6e21b92042b8358bd8ceef45b0a60404c61c9aa06ccf2e85a00f1c7f6'
    groups = re.findall(r'^test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;', (out / 'tests.log').read_text(), re.M)
    assert groups and all(int(ignored) == 0 for _, ignored in groups)
    assert {'passed':sum(int(n) for n,_ in groups),'groups':len(groups),'failures':0,'ignored':0} == build_inputs['expected_tests']
    report['binder_sha256'] = sha(__file__)
    report.update(status='passed', base=base, source_count=74, source_delta=delta,
                  source_manifest_sha256=sha(out / 'source.json'), prepared_source_sha256=sha(PREP / 'source.json'),
                  complete_patch_sha256=sha(out / 'candidate.patch'), check_scope='Full workspace tests/fmt/all-target strict Clippy; also locked inprocess benchmark fmt/Clippy. Same seven ordinary commands as the completed raw-view control.', comparison_scope='Matched native End raw projection versus exact completed raw-view e59d control (source0555/buildb202). Three source/test files change versus published base008d818; the separate three-line cfg(test) fixture correction is explicitly retained, not represented as a rebuilt control. No Start-token candidate is composed. Manifests, storage sources, dependencies, auxiliary locks and ordinary compiler policy are unchanged. No parser/compiler/benchmark target executed by this saved binder.', build_sha256=sha(out / 'build.json'),
                  control_source_manifest_sha256=sha(control / 'source.json'), control_build_sha256=sha(control / 'build.json'),
                  tool_hashes=old_binding['tool_hashes'], compiler_vectors_normalized=vectors(parsed_vectors),
                  normalization='Compiler policy comparison normalizes only resolved rustc/shared-build paths, Cargo metadata/extra-filename and known dependency fingerprints. Every flag and extern remains on both sides, including wide. Full actual core and dependency vectors are retained and compared; inherited dependency flags/features checked explicitly.',
                  candidate_libraries=build['libraries'], control_libraries=old['libraries'],
                  dependency_compiler_vectors=[normalize(v) for v in dependency_vectors], dependency_review_sha256=sha(dependency_review_path), wide_artifact_sha256={p:sha(p) for p in wide_paths},
                  auxiliary_files=auxiliary, metadata_receipt_sha256=sha(metadata_receipt_path),
                  expat_normal_control={'path':'/tmp/oriole-pgo-study/expat-control-liboriole_expat.so','sha256':sha('/tmp/oriole-pgo-study/expat-control-liboriole_expat.so')},
                  tests={'passed': sum(int(n) for n, _ in groups), 'groups': len(groups), 'failures': 0, 'ignored': 0})
except BaseException as exc:
    report.update(status='failed', error=repr(exc))
    raise
finally:
    report_path.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'status': report['status'], 'tests': report['tests'], 'libraries': report['candidate_libraries'], 'sha256': sha(report_path)}))
