"""Assemble saved Context Text evidence, excluding binaries and duplicate workers.

Run only after the parent releases saved-data assembly. This does not execute a
compiler, parser or timing controller. Original producer evidence is read-only.
"""
from pathlib import Path
import gzip
import hashlib
import io
import json
import tarfile

ROOT = Path(__file__).resolve().parent
WORK = Path('/home/dev-user/code/oss/oriole-context-text-frame')
NATIVE = 'context-text-frame-fixed-native-study'
AUDIT = 'context-text-frame-fixed-independent-review'
PYTHON = 'context-text-frame-fixed-python-study'
PY_AUDIT = 'context-text-frame-fixed-python-elapsed-independent-review'
STUDIES = {name: Path('/tmp/oriole-' + name) for name in (
 'context-text-frame-fixed-pgo-study', AUDIT, NATIVE,
 'context-text-frame-fixed-thin-pgo-gates',
 'context-text-frame-fixed-thin-pgo-cpython-gates',
 'context-text-frame-fixed-thin-pgo-cpython-preparation',
 'context-text-frame-fixed-strict-outcome-review', PYTHON, PY_AUDIT,
)}
STUDIES.update({'combined-ci':Path('/tmp/oriole-context-text-frame-ci-combined-attempt01'),
 'historical-local':Path('/tmp/oriole-context-text-frame-local-publication'),
 'historical-notices':Path('/tmp/oriole-context-text-frame-local-publication-notices'),
 'historical-reviewer-attempts':Path('/tmp/oriole-context-text-frame-local-publication-reviewer-attempts')})

def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2) + '\n').encode()


def read(path):
    return json.loads(path.read_bytes())


def main():
    if not __debug__:
        raise RuntimeError('Assertions must remain enabled')
    assert not (ROOT / 'evidence.tar.gz').exists()
    pgo=STUDIES['context-text-frame-fixed-pgo-study']
    source=read(pgo/'source.json');checks=read(pgo/'source-checks.json')
    assert source['candidate_commit']==checks['head']=='0f28139f83b04290e62301c38d8ed489f183a88d'
    assert source['base_runtime']=='e1d263a711e862e4f0f0018b15de9ac83a87a342'
    assert checks['status']=='passed' and len(checks['jobs'])==16
    assert all(x['conclusion']=='success' for x in checks['jobs'])
    assert read(pgo/'pair-report.json')['status']=='passed'
    assert digest((pgo/'source.json').read_bytes())=='8a7da2759b67ca85f82fb29bd775392e532b3ff355340eb10f92ea7764bbe642'
    ci=read(pgo/'combined-ci-independent-review.json')
    assert ci['head']==checks['head'] and ci['all_ci_jobs_successful']==16
    restack=read(pgo/'allocator-docs-restack.json')
    assert restack['old_head']==checks['head'] and restack['source_files_byte_exact']==72
    assert restack['new_head']=='20eda6ccaa22bd65b0ef1a4b3cf0b15d382a5f33'
    gates=read(STUDIES['context-text-frame-fixed-thin-pgo-gates']/'summary.json')
    assert gates['api']['all4740rows_exact'] and (gates['api']['passed'],gates['api']['failed'])==(4347,393)
    compatibility=read(STUDIES[AUDIT]/'compatibility/review.json')
    for row in compatibility['CPython'].values():
        assert row['methods']==802 and row['raw_exit']==2 and row['baseline_equal']
    native=read(STUDIES[AUDIT]/'native/review.json')
    assert native['status']=='passed'
    py=read(STUDIES[PY_AUDIT]/'review.json')
    assert digest((STUDIES[PY_AUDIT]/'review.json').read_bytes())=='2448a869cf73fcc7861b3ab5919df91fde65b3ee2591c546fa1ecfae0087d581'
    assert py['status']=='passed_independent_both_Python_campaigns'
    assert py['counts']['all_workers']==1152 and py['counts']['all_samples']==52728
    for mode in ('normal','pgo'):
        assert read(STUDIES[NATIVE]/mode/'controller.json')['status']=='passed'
        assert read(STUDIES[PYTHON]/mode/'time-controller.json')['status']=='passed'
        assert read(STUDIES[PY_AUDIT]/(mode+'-review.json'))['status']=='passed_independent_raw_elapsed_reconstruction'
    sources = {}
    for prefix, folder in STUDIES.items():
        assert folder.is_dir(), folder
        for path in sorted(folder.rglob('*')):
            if path.is_file():
                sources[prefix + '/' + path.relative_to(folder).as_posix()] = path
    sources['reviews/cgate-outcome.json']=Path('/tmp/oriole-context-text-frame-fixed-cgate-outcome-review.json')
    for path in sorted(Path('/tmp').glob('oriole-context-text-frame-fixed-python-independent-preflight-review*')):
        if path.is_file(): sources['reviews/python-preflight/'+path.name]=path
    for name in ('COPYING', 'expat/COPYING'):
        sources['notices/expat-2.8.4/' + name] = Path('/home/dev-user/.cache/oriole/upstream/expat-2.8.4') / name
    sources['notices/cpython-3.12.13/LICENSE'] = Path('/home/dev-user/.cache/oriole/upstream/cpython-3.12.13/LICENSE')
    for name in ('LICENSE-APACHE', 'LICENSE-MIT', 'tests/c/UPSTREAM-NOTICES.txt'):
        sources['notices/oriole/' + name] = WORK / name
    # These source helpers are outside the measured 70-file runtime map.
    for path in sorted((WORK / 'tools/cpython').glob('*')):
        if path.is_file():
            sources['reproduce/oriole/tools/cpython/' + path.name] = path
    for name in ('provenance.json', 'cpython-3.12.13-external-parser.patch'):
        sources['reproduce/oriole/consumer-fix/' + name] = WORK / 'integration/python-build-standalone/consumer-fix' / name
    for name in ('PLAN.json','README.draft.md','allocator-reference-package.py','allocator-reference-verify.py','prepare-final.py','package.py','verify.py','replay_native.py','replay_python.py','recompute-native-readme.py','native-readme-table.json','native-readme-table.md'):
        sources['reproduce/' + name] = ROOT / name

    alias_rows = read(STUDIES[AUDIT] / 'native/worker-aliases.json')['aliases']
    assert len(alias_rows) == 1344
    alias_map = {}
    for mode, ordinal, container, row, process, size, sha in alias_rows:
        logical = f'{NATIVE}/{mode}/native-screen/worker-{ordinal:04d}.json'
        assert logical not in alias_map
        alias_map[logical] = {'container': f'{NATIVE}/{mode}/native-screen/{container}.json',
                              'row_index': row, 'process_index': process,
                              'sha256': sha, 'bytes': size}

    files, excluded, aliases, objects = [], [], [], {}
    total_read = 0
    for logical, path in sorted(sources.items()):
        data = path.read_bytes()
        total_read += len(data)
        assert total_read <= 2 * 1024**3
        item = {'path': logical, 'origin': str(path), 'sha256': digest(data), 'bytes': len(data)}
        if logical in alias_map:
            target = alias_map[logical]
            assert (item['sha256'], item['bytes']) == (target['sha256'], target['bytes'])
            aliases.append(item | {'reconstruct': target})
            continue
        if ('__pycache__' in path.parts or 'targets' in path.parts
                or data.startswith((b'\x7fELF', b'!<arch>\n'))
                or path.suffix in {'.pyc', '.o', '.rlib', '.rmeta'}):
            excluded.append(item | {'reason': 'Compiled artifact or reusable build cache; identity only.'})
            continue
        objects.setdefault(item['sha256'], data)
        files.append(item)
    assert len(aliases) == 1344
    assert sum(len(x) for x in objects.values()) <= 256 * 1024**2
    index = {'format': 'sha256-object-archive-v1', 'files': files,
             'excluded_binaries_and_caches': excluded, 'native_worker_aliases': aliases,
             'alias_encoding': {'remove_fields': ['observations', 'median_seconds'],
                                'serialization': 'json.dumps(record, indent=2) + newline, UTF-8; insertion order and default ensure_ascii=True'},
             'objects': len(objects), 'unique_bytes': sum(map(len, objects.values()))}
    index_bytes = encoded(index)
    with (ROOT / 'evidence.tar.gz').open('xb') as raw:
        with gzip.GzipFile(fileobj=raw, mode='wb', mtime=0, filename='') as gz:
            with tarfile.open(fileobj=gz, mode='w') as bundle:
                for name, data in [('index.json', index_bytes), *[(f'objects/{h}', d) for h, d in sorted(objects.items())]]:
                    member = tarfile.TarInfo(name)
                    member.size = len(data)
                    member.mode = 0o644
                    bundle.addfile(member, io.BytesIO(data))
    assert (ROOT / 'evidence.tar.gz').stat().st_size <= 128 * 1024**2
    (ROOT / 'archive-index.json').write_bytes(index_bytes)
    (ROOT / 'conditions.csv').write_bytes((STUDIES[AUDIT] / 'native/conditions.csv').read_bytes())
    report={
        'status':'selected_context_text_frame_evidence',
        'runtime_commit':checks['head'],'publication_restack':restack,
        'source_manifest_sha256':digest((pgo/'source.json').read_bytes()),
        'source_files':72,'runtime_delta':source['runtime_changed_files'],'all_changed_paths':source['changed_files'],
        'combined_CI':{'all_jobs_successful':16,'Miri_tests_per_model':45,'head':checks['head'],'warning_retained':True},
        'historical_local_scope':'Unmodified source06 packet retains226 focused local tests and original warm-test/empty-raw/Clippy/Miri failures. Current combined0f results are separate.',
        'api':{'rows':4740,'passed':4347,'assertion_failures':391,'timeouts':2,'raw_exit':1,'all_outcomes_match_corrected_allocator':True},
        'cpython_compatibility':compatibility['CPython'],
        'native':{mode:native['native'][mode]['groups'] for mode in ('normal','pgo')},
        'native_totals':{'workers':1344,'samples':212352,'measured_timed_samples':210840,'timed_warmups':1176,'preflight_samples':336},
        'python':py['modes'],'python_totals':py['counts'],
        'selection':'Keep Context Text on the corrected allocator baseline. Native normal/PGO and Python PGO improve; normal Python and generated native PGO regress. All adverse cases retained. No new PBS or BOLT claim.',
        'roles':'Runtime/source and CI reviewers are separate. Build readback uses a separate established reader but shares candidate build author; compatibility/native/Python raw audits are independent of their collectors.',
        'archive_sha256':digest((ROOT/'evidence.tar.gz').read_bytes()),
        'archive_index_sha256':digest(index_bytes),'archive_bytes':(ROOT/'evidence.tar.gz').stat().st_size,
        'logical_files':len(files),'unique_files':len(objects),'excluded_binaries_and_caches':len(excluded),'native_worker_aliases':len(aliases),
        'python_alias_scope':'JSON-value aliases for reconstructed rows only. Full original Python report JSON, raw stdout/stderr gzip files, specs and process records are retained. No Python whole-file byte alias or omission is asserted.',
    }
    (ROOT/'report.json').write_bytes(encoded(report))
    names=('README.md','package.py','verify.py','replay_native.py','replay_python.py','evidence.tar.gz','archive-index.json','report.json','conditions.csv','native-readme-table.json','native-readme-table.md')
    seal={name:{'sha256':digest((ROOT/name).read_bytes()),'bytes':(ROOT/name).stat().st_size} for name in names}
    (ROOT/'files.json').write_bytes(encoded({'format':'sha256-file-manifest-v1','files':seal}))
    print(json.dumps({k:report[k] for k in ('status','archive_sha256','archive_bytes','logical_files','unique_files','excluded_binaries_and_caches','native_worker_aliases')}))

if __name__=='__main__':
    main()
