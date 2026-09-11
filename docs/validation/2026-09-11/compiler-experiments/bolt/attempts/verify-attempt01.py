#!/usr/bin/env python3
"""Portable archive integrity and original BOLT native-audit replay; no targets."""
import argparse
import contextlib
import fnmatch
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import tarfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def verify(root):
    index = json.loads(gzip.decompress((root / 'members.json.gz').read_bytes()))
    archive = root / 'evidence.tar.gz'
    with archive.open('rb') as stream:
        archive_hash = hashlib.file_digest(stream, 'sha256').hexdigest()
    assert archive_hash == index['archive_sha256']
    assert archive.stat().st_size == index['archive_bytes']
    payloads = {}
    with tarfile.open(archive, 'r:gz') as stream:
        for member in stream:
            assert member.isfile() and member.name not in payloads
            assert member.name.startswith('payload/')
            assert member.uid == member.gid == member.mtime == 0
            data = stream.extractfile(member).read()
            assert digest(data) == member.name.removeprefix('payload/')
            assert len(data) == member.size
            assert not data.startswith((b'\x7fELF', b'!<arch>\n'))
            payloads[member.name] = data
    entries = {entry['path']: entry for entry in index['origins']}
    assert len(entries) == len(index['origins'])
    assert set(payloads) == {entry['member'] for entry in entries.values()}
    for entry in entries.values():
        data = payloads[entry['member']]
        assert len(data) == entry['bytes'] and digest(data) == entry['sha256']
    omitted = {entry['path']: entry for entry in index['omitted']}
    assert len(omitted) == len(index['omitted']) and not (entries.keys() & omitted.keys())
    writes = {}
    output_root = '/tmp/oriole-allocator-bolt-native-independent-audit'
    output_paths = {output_root + '/details.json', output_root + '/review.json'}
    omitted_used = set()

    class ArchivePath:
        """Only the Path operations used by the unchanged saved-data auditor."""
        def __init__(self, path):
            self.path = os.path.normpath(str(path))

        def __str__(self):
            return self.path

        def __truediv__(self, suffix):
            return ArchivePath(os.path.join(self.path, str(suffix)))

        @property
        def name(self):
            return os.path.basename(self.path)

        @property
        def parent(self):
            return ArchivePath(os.path.dirname(self.path))

        def resolve(self):
            assert self.path.startswith('/')
            return self

        def read_bytes(self):
            if self.path in writes:
                return writes[self.path]
            return payloads[entries[self.path]['member']]

        def read_text(self):
            return self.read_bytes().decode()

        def exists(self):
            if self.path in output_paths:
                return self.path in writes
            return self.path in entries

        def write_text(self, text):
            assert self.path in output_paths and self.path not in writes
            writes[self.path] = text.encode()
            return len(text)

        def glob(self, pattern):
            prefix = self.path + '/'
            return [ArchivePath(path) for path in sorted(entries)
                    if path.startswith(prefix)
                    and '/' not in path[len(prefix):]
                    and fnmatch.fnmatchcase(path[len(prefix):], pattern)]

    def archive_sha(path):
        key = str(path)
        if key in omitted:
            # No payload bytes exist in this package. This is a recorded identity,
            # explicitly excluded from the package's byte-readback claim.
            omitted_used.add(key)
            return omitted[key]['sha256']
        return digest(ArchivePath(path).read_bytes())

    original = output_root + '/audit.py'
    code = ArchivePath(original).read_text()
    edits = [
        ('from pathlib import Path', 'Path = archive_path'),
        ('assert os.sched_getaffinity(0)=={6}', '# Portable saved-data replay; no CPU affinity dependency.'),
        ("def sha(p):\n with Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()",
         'def sha(p):\n return archive_sha(p)'),
    ]
    for old, new in edits:
        assert code.count(old) == 1
        code = code.replace(old, new)
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        exec(compile(code, original, 'exec'),
             {'__file__': original, '__name__': '__main__',
              'archive_path': ArchivePath, 'archive_sha': archive_sha})
    for path in output_paths:
        assert writes[path] == payloads[entries[path]['member']], path
    replay = json.loads(writes[output_root + '/review.json'])
    assert replay['all_workers'] == 896 and replay['total_samples'] == 141568
    assert replay['all_raw_callbacks_and_order_exact'] is True

    # Bind archived source and pipeline bytes to the exact producer preparation.
    study = '/tmp/oriole-allocator-bolt-study'
    preparation = json.loads(ArchivePath(study + '/preparation.json').read_text())
    source = json.loads(ArchivePath(study + '/selected-source.json').read_text())
    assert source['source_sha256'] == preparation['source_sha256']
    assert len(source['source_sha256']) == 70
    for name, expected in source['source_sha256'].items():
        assert archive_sha(preparation['workspace'] + '/' + name) == expected
    assert len(preparation['pipeline_source_sha256']) == 67
    for name, expected in preparation['pipeline_source_sha256'].items():
        assert source['source_sha256'][name] == expected
    helpers = {path: expected for path, expected in preparation['pins'].items()
               if path.startswith(preparation['pipeline'] + '/')}
    assert len(helpers) == 6
    for path, expected in helpers.items():
        assert archive_sha(path) == expected
    fdata = ArchivePath(study + '/attempt01/profile/generated.fdata').read_bytes()
    assert digest(fdata) == '5ef151d81b721eb2515b63cc07a02df678ff484355f836e16464e19e5c9ddf81'
    assert len(fdata.splitlines()) == 8423
    return {'status': 'passed_portable_archive_and_original_numerical_replay',
            'archive_sha256': archive_hash, 'stored_members': len(payloads),
            'origins': len(entries), 'aliases': len(entries) - len(payloads),
            'source_files': 70, 'cargo_inputs': 67, 'pipeline_helpers': len(helpers),
            'workers': replay['all_workers'], 'samples': replay['total_samples'],
            'groups': replay['groups'],
            'reconstructed_details_and_review_byte_equal': True,
            'recorded_only_binary_hashes_used_by_replay': sorted(omitted_used),
            'limitations': ['Omitted binary/tool bytes are not revalidated by portable replay.',
                           'The original auditor is archived unchanged. The adapter changes only Path I/O, hash I/O for omitted files, and local CPU affinity.',
                           'No parser, compiler, profiler, benchmark, or original filesystem read is executed.',
                           'This reproduces saved evidence; it does not add BOLT compatibility coverage.']}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('package', nargs='?', type=Path, default=Path(__file__).resolve().parent)
    arguments = parser.parse_args()
    print(json.dumps(verify(arguments.package), indent=2))
