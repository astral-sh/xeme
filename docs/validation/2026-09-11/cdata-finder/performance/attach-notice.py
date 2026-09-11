"""Attach the missing inherited C-fixture notice without rebuilding evidence."""
from pathlib import Path
import hashlib, json, subprocess

root = Path('/tmp/oriole-cdata-finder-perf-handoff')
repo = Path('/home/dev-user/code/oss/oriole-cdata-finder')
name = 'tests-c-UPSTREAM-NOTICES.txt'
source = repo / 'tests/c/UPSTREAM-NOTICES.txt'
data = source.read_bytes()
assert data == subprocess.check_output(['git', 'show', '144e69e08c5462217d54791374e579e3ef390a87:tests/c/UPSTREAM-NOTICES.txt'], cwd=repo)
assert not (root / name).exists()
(root / name).write_bytes(data)
summary_path = root / 'summary.json'
summary = json.loads(summary_path.read_text())
summary['notice_sidecar'] = {
    'path': name, 'source': str(source), 'bytes': len(data),
    'sha256': hashlib.sha256(data).hexdigest(),
    'git_origin': '144e69e08c5462217d54791374e579e3ef390a87:tests/c/UPSTREAM-NOTICES.txt',
    'reason': 'Independent package review identified the inherited C-fixture notice absent from the first archive selection. Exact sidecar added; archive and all raw evidence remain unchanged.',
}
summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + '\n')
readme = root / 'README.md'
text = readme.read_text()
text += '\nThe [inherited C-fixture notices](tests-c-UPSTREAM-NOTICES.txt) accompany the archived `tests/c/integration.c` source. Independent review found this notice missing from the original archive selection; the exact Git144 notice is attached as a hash-pinned sidecar without changing the archive or rerunning any study.\n'
readme.write_text(text)
(root / 'attach-notice.py').write_bytes(Path(__file__).read_bytes())
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
index = {'scope': 'All top-level publication files except this index; archive member and alias maps are separately sealed.', 'files': [
    {'path': p.name, 'bytes': p.stat().st_size, 'sha256': sha(p)}
    for p in sorted(root.iterdir()) if p.name != 'files.json'
]}
(root / 'files.json').write_text(json.dumps(index, indent=2, sort_keys=True) + '\n')
print(json.dumps({'notice': summary['notice_sidecar'], 'summary_sha256': sha(summary_path), 'files_sha256': sha(root / 'files.json')}))
