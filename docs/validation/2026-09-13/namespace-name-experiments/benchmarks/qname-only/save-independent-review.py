"""Use the reviewed saved-evidence summarizer for the lean QName-only candidate."""
from pathlib import Path

original = Path('/tmp/oriole-save-namespace-independent-review.py')
source = original.read_text()
assert source.count('CONFIG = {') == 1
source = source.replace(
    'CONFIG = {',
    "CONFIG = {\n    'qname-only': ('namespace-qname-only', '735948097c48a2a30cdc88be49632c1410b4edc8', 484, True),",
)
exec(compile(source, str(original), 'exec'))
