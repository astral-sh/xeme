from pathlib import Path
import hashlib,json
root=Path('/tmp/oriole-value-fuzz');out=root/'final-evidence/large-inputs';out.mkdir(exist_ok=True)
base=bytearray((root/'fuzz/seeds/value_family/quoted-numeric-cr').read_bytes()[:32])
payloads={
 'ascii':b'"'+b'X'*65502+b'"',
 'utf16':b'\xff\xfe'+('"'+'名'*32749+'"').encode('utf-16-le'),
 'references':b'"'+b'&#13;&#10;&amp;'*4300+b'"',
 'custom':b'"'+b'\x80v'*32751+b'"',
}
manifest={}
for name,p in payloads.items():
 for width in [0,63,255]:
  controls=bytearray(base);controls[3]=width;controls[13:15]=len(p).to_bytes(2,'little')
  if name=='custom':controls[0]|=64
  key=f'{name}-width{width+1}';data=controls+p;assert len(data)<=65536
  (out/key).write_bytes(data);manifest[key]={'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
for fail_at in [1,2,5,100,512,2048]:
 controls=bytearray(base);controls[3]=255;controls[0]|=1;controls[1:3]=(fail_at-1).to_bytes(2,'little');p=payloads['ascii'];controls[13:15]=len(p).to_bytes(2,'little')
 key=f'ascii-fail-at-{fail_at}';data=controls+p;(out/key).write_bytes(data);manifest[key]={'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
(root/'final-evidence/large-inputs-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(len(manifest))
