import ctypes as c,itertools,json,gzip,hashlib,sys
from pathlib import Path
P=c.c_void_p; S=c.c_char_p; I=c.c_int; B=c.c_ubyte
EXT=c.CFUNCTYPE(I,P,S,S,S,S);NOT=c.CFUNCTYPE(I,P);VOID=c.CFUNCTYPE(None,P);START=c.CFUNCTYPE(None,P,S,P);TEXT=c.CFUNCTYPE(None,P,P,I);DTD=c.CFUNCTYPE(None,P,S,S,S,I)
REF='/home/dev-user/.cache/oriole/expat-build/libexpat.so';CAND=sys.argv[1] if len(sys.argv)>1 else '/home/dev-user/.cache/oriole/foreign-dtd-target/debug/liboriole_expat.so';OUT=Path(sys.argv[2] if len(sys.argv)>2 else '/tmp/oriole-foreign-dtd/matrix-first')
def setup(path):
 l=c.CDLL(path)
 for name,result,args in [('XML_ParserCreate',P,[S]),('XML_ExternalEntityParserCreate',P,[P,S,S]),('XML_ParserFree',None,[P]),('XML_Parse',I,[P,S,I,I]),('XML_GetErrorCode',I,[P]),('XML_SetParamEntityParsing',I,[P,I]),('XML_UseForeignDTD',I,[P,B]),('XML_SetExternalEntityRefHandler',None,[P,EXT]),('XML_SetNotStandaloneHandler',None,[P,NOT]),('XML_SetStartElementHandler',None,[P,START]),('XML_SetCharacterDataHandler',None,[P,TEXT]),('XML_SetDefaultHandlerExpand',None,[P,TEXT]),('XML_SetDoctypeDeclHandler',None,[P,DTD,VOID])]:
  f=getattr(l,name);f.restype=result;f.argtypes=args
 return l
libs=[setup(REF),setup(CAND)]
def probe(l,doc,mode,action,reject,width,encoding,handlers):
 events=[];children=[];actor=['root'];root=l.XML_ParserCreate(None)
 def event(kind,*args):
  if kind in ('default','text') and events and events[-1][:2]==[actor[0],kind]:events[-1][2]+=args[0]
  else:events.append([actor[0],kind,*args])
 def parse(parser,data,final=True):
  status=1
  for offset in range(0,max(len(data),1),width or max(len(data),1)):
   part=data[offset:offset+(width or max(len(data),1))];status=l.XML_Parse(parser,part,len(part),int(final and offset+len(part)>=len(data)))
   if status==0:break
  return status
 def external(parser,context,base,system,public):
  name=system.decode() if system else 'foreign';event('external',name)
  if action=='skip' or (name=='q' and action=='nested-skip'):return 1
  if action=='reject':return 0
  child=l.XML_ExternalEntityParserCreate(parser,context,b'unknown-protocol' if action=='unknown' else None)
  if not child:event('null-child');return 0
  if action=='create':l.XML_ParserFree(child);return 1
  old=actor[0];actor[0]=name
  body=''
  if action in ('load','nested-read','nested-skip'):
   body='<!ENTITY supplied "X">'
   if name=='foreign' and action.startswith('nested-'):body='<!ENTITY % q SYSTEM "q">%q;'+body
  elif action=='standalone':body='<?xml version="1.0" encoding="UTF-8" standalone="yes"?><!ENTITY supplied "X">'
  elif action=='invalid':body='!'
  data=body.encode(encoding)
  if encoding=='utf-16-le':data=b'\xff\xfe'+data
  if encoding=='utf-16-be':data=b'\xfe\xff'+data
  status=parse(child,data,action!='nonfinal-empty');children.append([name,status,l.XML_GetErrorCode(child)])
  actor[0]=old;l.XML_ParserFree(child)
  return 1 if action in ('unknown','invalid') else int(status!=0)
 callbacks=[EXT(external),NOT(lambda _: (event('not-standalone'),int(not reject))[1]),START(lambda _,name,attrs:event('start',name.decode())),TEXT(lambda _,p,n:event('text',c.string_at(p,n).decode())),TEXT(lambda _,p,n:event('default',c.string_at(p,n).decode())),DTD(lambda _,name,system,public,internal:event('doctype',name.decode())),VOID(lambda _:event('end-doctype'))]
 l.XML_SetParamEntityParsing(root,mode);l.XML_UseForeignDTD(root,1)
 if action!='absent':l.XML_SetExternalEntityRefHandler(root,callbacks[0])
 l.XML_SetNotStandaloneHandler(root,callbacks[1]);l.XML_SetStartElementHandler(root,callbacks[2]);l.XML_SetCharacterDataHandler(root,callbacks[3])
 if handlers:l.XML_SetDefaultHandlerExpand(root,callbacks[4]);l.XML_SetDoctypeDeclHandler(root,callbacks[5],callbacks[6])
 status=parse(root,doc.encode());result=dict(status=status,error=l.XML_GetErrorCode(root),events=events,children=children);l.XML_ParserFree(root);return result
cases=[]
for prefix,standalone,mode,action,reject,width,handlers in itertools.product(['','<!DOCTYPE r>','<!DOCTYPE r []>','<!DOCTYPE r [<!ENTITY % e "">%e;]>'],['','yes','no'],range(3),['absent','skip','create','nonfinal-empty','load','reject','unknown','invalid','nested-skip','nested-read'],[False,True],[0,1,3],[False,True]):
 doc=('<?xml version="1.0" standalone="'+standalone+'"?>' if standalone else '')+prefix+'<r>&supplied;</r>';cases.append((doc,mode,action,reject,width,'utf-8',handlers))
for prefix,enc,width,reject in itertools.product(['','<!DOCTYPE r []>'],['utf-8','utf-16-le','utf-16-be'],range(0,26),[False,True]):cases.append((prefix+'<r>&supplied;</r>',2,'load',reject,width,enc,True))
diffs=[]
with gzip.open(str(OUT)+'.cases.json.gz','wt')as f:
 f.write('[')
 for index,args in enumerate(cases):
  reference,candidate=[probe(l,*args)for l in libs];row=dict(case=args,reference=reference,candidate=candidate)
  if index:f.write(',\n')
  json.dump(row,f)
  if reference!=candidate:diffs.append(row)
 f.write(']\n')
summary=dict(cases=len(cases),differences=len(diffs),outcome_differences=sum((r['reference']['status'],r['reference']['error'])!=(r['candidate']['status'],r['candidate']['error']) for r in diffs),libraries={p:hashlib.sha256(Path(p).read_bytes()).hexdigest()for p in (REF,CAND)},diffs=diffs)
Path(str(OUT)+'.json').write_text(json.dumps(summary,indent=2)+'\n');print({k:v for k,v in summary.items()if k!='diffs'})
