import ctypes as c,json,hashlib,itertools
from pathlib import Path
P=c.c_void_p;S=c.c_char_p;I=c.c_int;TEXT=c.CFUNCTYPE(None,P,P,I)
paths=['/home/dev-user/.cache/oriole/expat-build/libexpat.so','/tmp/oriole-xml-versions.so'];libs=[]
for path in paths:
 l=c.CDLL(path)
 for n,r,a in [('XML_ParserCreate',P,[S]),('XML_Parse',I,[P,S,I,I]),('XML_GetErrorCode',I,[P]),('XML_ParserFree',None,[P]),('XML_SetCharacterDataHandler',None,[P,TEXT])]:
  f=getattr(l,n);f.restype=r;f.argtypes=a
 libs.append(l)
def run(l,data,width):
 p=l.XML_ParserCreate(None);text=[];cb=TEXT(lambda _,s,n:text.append(c.string_at(s,n).decode()));l.XML_SetCharacterDataHandler(p,cb);status=1
 for off in range(0,len(data),width or len(data)):
  part=data[off:off+(width or len(data))];status=l.XML_Parse(p,part,len(part),int(off+len(part)==len(data)))
  if not status:break
 r=dict(status=status,error=l.XML_GetErrorCode(p),text=''.join(text));l.XML_ParserFree(p);return r
rows=[]
for version,body,encoding,width in itertools.product(['1.0','1.1','1.7','1.01','1.000'],['<r/>','<r>&#1;</r>','<r>\u0085</r>'],['utf-8','utf-16-le','utf-16-be'],[0,1,3]):
 data=("<?xml version='"+version+"'?>"+body).encode(encoding)
 if encoding=='utf-16-le':data=b'\xff\xfe'+data
 if encoding=='utf-16-be':data=b'\xfe\xff'+data
 a,b=[run(l,data,width)for l in libs];rows.append(dict(version=version,body=body,encoding=encoding,width=width,reference=a,candidate=b))
report={'libraries':{p:hashlib.sha256(Path(p).read_bytes()).hexdigest()for p in paths},'cases':len(rows),'differences':sum(r['reference']!=r['candidate']for r in rows),'rows':rows}
Path('/tmp/oriole-version-independent.json').write_text(json.dumps(report,indent=2)+'\n');print(report['cases'],report['differences'])
