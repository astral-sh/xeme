from pathlib import Path
import hashlib,json
out=Path(__file__).parent
old=Path('/tmp/oriole-detached-end-frames-human-independent-review/audit.py').read_text()
s=old[:old.index('profile=js(')]
s=s[:s.index('assert f"{(1-ns[')]
changes={
"R=Path('/home/dev-user/code/oss/oriole-end-frame-cells')":"R=Path('/home/dev-user/code/oss/oriole-native-byte-count')",
"D=R/'docs/validation/2026-09-11/detached-end-frames'":"D=R/'docs/validation/2026-09-11/native-byte-count'",
",D/'unselected-names/README.md'":"",
"R/'benchmarks/results/2026-09-11/detached-end-frames/README.md'":"R/'benchmarks/results/2026-09-11/native-byte-count/README.md',R/'benchmarks/README.md'",
"Path('/tmp/oriole-pr-detached-end-frames.md')":"Path('/tmp/oriole-native-byte-count-pr-body.md')",
"5f20adf":"079fb66dc159cdc0e1c3ad16397f149751783014",
"1ff7b64":"be22a271f8a5c004d13e515cd4024a68afe57887",
"splitlines())==9":"splitlines())==1",
"/tmp/oriole-end-frame-cells-thinlto-timing-study/":"/tmp/oriole-raw-len-split-thinlto-timing-study/",
"/tmp/oriole-end-frame-cells-thinlto-python-study/":"/tmp/oriole-raw-len-split-thinlto-python-root-study/",
}
for a,b in changes.items():
 assert a in s,a
 s=s.replace(a,b)
(out/'adaptation.json').write_text(json.dumps({'base':'/tmp/oriole-detached-end-frames-human-independent-review/audit.py','base_sha256':hashlib.sha256(old.encode()).hexdigest(),'replacements':changes,'scope':'Reuse saved-sample table arithmetic and README/link invariants; new final receipt tail records current scopes.'},indent=2)+'\n')
(out/'audit.py').write_text(s+(out/'audit-tail.py').read_text())
