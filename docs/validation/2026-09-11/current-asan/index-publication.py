"""Index the completed publication, excluding the index itself."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).parent
review = json.loads((root / 'publication-review.json').read_text())
assert str(review['status']).startswith('passed')
files = {
    str(path.relative_to(root)): {
        'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
        'size': path.stat().st_size,
    }
    for path in sorted(root.rglob('*'))
    if path.is_file() and path.name != 'files.json'
}
(root / 'files.json').write_text(json.dumps(files, indent=2) + '\n')
assert all(
    hashlib.sha256((root / name).read_bytes()).hexdigest() == item['sha256']
    for name, item in files.items()
)
print(f'Indexed {len(files)} publication files.')
