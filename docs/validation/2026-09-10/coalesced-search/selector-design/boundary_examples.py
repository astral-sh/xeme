"""Record a small set of independently inspectable semantic-boundary examples."""
from pathlib import Path
import ast
import json
p = Path(__file__).with_name('oracle.py')
t = ast.parse(p.read_text())
module = ast.Module(body=[n for n in t.body if isinstance(n, (ast.FunctionDef, ast.Import, ast.ImportFrom)) or isinstance(n, ast.Assign) and any(isinstance(v, ast.Name) and v.id in ['CAP', 'counts', 'paths'] for v in n.targets)], type_ignores=[])
namespace = {}
exec(compile(module, str(p), 'exec'), namespace)
rows = []
cap = namespace['CAP']
for label, data in [('marker_at_cap', b'\n' + b'x'*(cap-1) + b'<'), ('crlf_cross_cap', b'x'*(cap-1) + b'\r\n<'), ('markup_after_old_boundary', b'\n' + b'x'*(cap-1) + b'x<'), ('cr_only_internal', b'\r'*cap + b'x<'), ('first_newline_after_cap', b'x'*(cap+1) + b'\n<')]:
    for internal in [False, True]:
        expected = namespace['old'](data, True, internal)
        actual = namespace['proposed'](data, True, internal)
        assert expected == actual
        rows.append({'case': label, 'internal': internal, 'length': len(data), 'end': expected})
p.with_name('boundary-examples.json').write_text(json.dumps(rows, indent=2) + '\n')
