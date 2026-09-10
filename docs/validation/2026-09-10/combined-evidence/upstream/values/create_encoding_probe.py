import ctypes as c,json
from pathlib import Path
P=c.c_void_p;I=c.c_int;S=c.c_char_p;Z=c.c_size_t
M=c.CFUNCTYPE(P,Z);R=c.CFUNCTYPE(P,P,Z);F=c.CFUNCTYPE(None,P)
class Suite(c.Structure):_fields_=[('malloc',M),('realloc',R),('free',F)]
libc=c.CDLL(None);libc.malloc.argtypes=[Z];libc.malloc.restype=P;libc.realloc.argtypes=[P,Z];libc.realloc.restype=P;libc.free.argtypes=[P]
rows=[]
for path in ['/home/dev-user/.cache/oriole/expat-build/libexpat.so','/tmp/oriole-values-final-source/liboriole_expat.so']:
 l=c.CDLL(path);l.XML_ParserCreate_MM.argtypes=[S,c.POINTER(Suite),S];l.XML_ParserCreate_MM.restype=P;l.XML_ParserFree.argtypes=[P]
 for limit in range(32):
  state={'calls':0,'live':set()}
  def malloc(size):
   state['calls']+=1
   if state['calls']>limit:return None
   p=libc.malloc(size)
   if p:state['live'].add(p)
   return p
  def realloc(p,size):
   q=libc.realloc(p,size)
   if q:state['live'].discard(p);state['live'].add(q)
   return q
  def free(p):state['live'].discard(p);libc.free(p)
  suite=Suite(M(malloc),R(realloc),F(free));p=l.XML_ParserCreate_MM(b'us-ascii',c.byref(suite),None);created=bool(p);l.XML_ParserFree(p)
  assert not state['live']
  rows.append(dict(library=path,limit=limit,created=created,calls=state['calls'],live=len(state['live'])))
  if created:break
Path('/tmp/oriole-upstream-final-values/create-encoding-probe.json').write_text(json.dumps(rows,indent=2)+'\n');print([r for r in rows if r['created']])
