from pathlib import Path
import ctypes as c,json,hashlib
P=c.c_void_p;S=c.c_char_p;I=c.c_int;EXT=c.CFUNCTYPE(I,P,P,S,S,S);DECL=c.CFUNCTYPE(None,P,S,S,I);ENT=c.CFUNCTYPE(None,P,S,I,P,I,S,S,S,S);START=c.CFUNCTYPE(None,P,S,P)
paths=['/home/dev-user/.cache/oriole/expat-build/libexpat.so','/tmp/oriole-xml-versions.so'];libs=[]
for path in paths:
 l=c.CDLL(path)
 for name,result,args in [('XML_ParserCreate',P,[S]),('XML_ParserCreateNS',P,[S,c.c_char]),('XML_ExternalEntityParserCreate',P,[P,P,S]),('XML_ParserFree',None,[P]),('XML_Parse',I,[P,P,I,I]),('XML_GetErrorCode',I,[P]),('XML_SetXmlDeclHandler',None,[P,DECL]),('XML_SetEntityDeclHandler',None,[P,ENT]),('XML_SetStartElementHandler',None,[P,START]),('XML_SetExternalEntityRefHandler',None,[P,EXT]),('XML_SetParamEntityParsing',I,[P,I]),('XML_SetReparseDeferralEnabled',c.c_ubyte,[P,c.c_ubyte])]:
  f=getattr(l,name);f.restype=result;f.argtypes=args
 libs.append(l)
def parse(l,doc,width,ns,kind='root',replacement=b''):
 events=[];children=[];callbacks=[]
 root=l.XML_ParserCreateNS(None,b'|') if ns else l.XML_ParserCreate(None);assert root
 def feed(parser,body,label):
  dec=DECL(lambda _,v,e,s:events.append([label,'declaration',v.decode()if v else None,e.decode()if e else None,s]))
  ent=ENT(lambda _,name,param,value,length,*args:events.append([label,'entity',name.decode(),param,c.string_at(value,length).decode()if value else None]))
  start=START(lambda _,name,atts:events.append([label,'start',name.decode()]))
  callbacks.extend([dec,ent,start]);l.XML_SetXmlDeclHandler(parser,dec);l.XML_SetEntityDeclHandler(parser,ent);l.XML_SetStartElementHandler(parser,start);l.XML_SetExternalEntityRefHandler(parser,external);l.XML_SetParamEntityParsing(parser,2);l.XML_SetReparseDeferralEnabled(parser,0)
  parts=[body] if width==0 else [body[i:i+width] for i in range(0,len(body),width)]
  for index,part in enumerate(parts):
   status=l.XML_Parse(parser,part,len(part),int(index==len(parts)-1))
   if not status:break
  return {'status':status,'error':l.XML_GetErrorCode(parser)}
 @EXT
 def external(parent,context,base,system,public):
  child=l.XML_ExternalEntityParserCreate(parent,context,None);assert child
  label=system.decode();content=b'<!ENTITY % p SYSTEM "p"><!ENTITY e "L%p;R">' if label=='d' else replacement
  try:result=feed(child,content,label);children.append({'label':label,**result});return int(result['status']==1)
  finally:l.XML_ParserFree(child)
 try:
  if kind in ('general','dtd'):
   child=l.XML_ExternalEntityParserCreate(root,c.cast(c.c_char_p(b''),P)if kind=='general' else None,None);assert child
   try:result=feed(child,doc,kind)
   finally:l.XML_ParserFree(child)
  else:result=feed(root,doc,'root')
  return {**result,'children':children,'events':events}
 finally:l.XML_ParserFree(root)
namespace_rows=[]
names=['xmlns','xmlns:','xmlns::','xmlns:p','xmlns:p:q','xmlns:xml','xmlns:xmlns','xmlns:XML','xmlns:1','xmlns:名','xmlnsfoo']
uris=['','urn:x','http://www.w3.org/XML/1998/namespace','http://www.w3.org/2000/xmlns/']
for name in names:
 for uri in uris:
  for form in ('explicit','defaulted'):
   doc=(f'<r {name}="{uri}"/>' if form=='explicit' else f'<!DOCTYPE r [<!ATTLIST r {name} CDATA "{uri}">]><r/>').encode()
   for ns in [False,True]:
    for width in [0,1,2,7]:
     row={'name':name,'uri':uri,'form':form,'namespace':ns,'width':width};row.update({key:parse(l,doc,width,ns)for key,l in zip(['reference','candidate'],libs)});namespace_rows.append(row)
versions=['1.0','1.1','1.7','1.01','1.000','1.x','1.','2.0','1.10','1.01x','01.0','1.٠','1.²','1.\t0','']
version_rows=[]
for version in versions:
 for encoding in [False,True]:
  decl=f'<?xml version="{version}"'+(' encoding="UTF-8"'if encoding else '')+'?>'
  for kind in ['root','general','dtd','value']:
   body=(decl+('<r/>'if kind=='root'else '<!ELEMENT r ANY>'if kind=='dtd'else 'X')).encode()
   document=b'<!DOCTYPE r SYSTEM "d"><r/>' if kind=='value'else body
   for width in [0,1,3]:
    row={'version':version,'encoding':encoding,'kind':kind,'width':width};row.update({key:parse(l,document,width,False,kind,body)for key,l in zip(['reference','candidate'],libs)});version_rows.append(row)
for stem,rows in [('namespace',namespace_rows),('version',version_rows)]:
 out={'libraries':{p:hashlib.sha256(Path(p).read_bytes()).hexdigest()for p in paths},'cases':len(rows),'rows':rows};Path('/tmp/oriole-xml-versions-'+stem+'-review.json').write_text(json.dumps(out,indent=2)+'\n')
 print(stem,len(rows),'outcome_differences',sum((r['reference']['status'],r['reference']['error'])!=(r['candidate']['status'],r['candidate']['error'])for r in rows))
