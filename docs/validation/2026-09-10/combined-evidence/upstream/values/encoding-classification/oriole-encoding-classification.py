import ctypes as c,hashlib,json
from pathlib import Path
P=c.c_void_p;S=c.c_char_p;I=c.c_int
CHAR=c.CFUNCTYPE(None,P,P,I);START=c.CFUNCTYPE(None,P,S,P);CONVERT=c.CFUNCTYPE(I,P,P)
class Encoding(c.Structure):_fields_=[('map',I*256),('data',P),('convert',P),('release',P)]
UNKNOWN=c.CFUNCTYPE(I,P,S,c.POINTER(Encoding))
paths=['/home/dev-user/.cache/oriole/expat-build/libexpat.so','/tmp/oriole-values-final-source/liboriole_expat.so'];libs=[]
for path in paths:
 l=c.CDLL(path)
 for name,result,args in [('XML_ParserCreate',P,[S]),('XML_ExternalEntityParserCreate',P,[P,S,S]),('XML_ParserFree',None,[P]),('XML_Parse',I,[P,P,I,I]),('XML_GetErrorCode',I,[P]),('XML_SetCharacterDataHandler',None,[P,CHAR]),('XML_SetStartElementHandler',None,[P,START]),('XML_SetUnknownEncodingHandler',None,[P,UNKNOWN,P]),('XML_SetReparseDeferralEnabled',c.c_ubyte,[P,c.c_ubyte])]:
  f=getattr(l,name);f.restype=result;f.argtypes=args
 libs.append(l)
def probe(l,body,protocol,width,child,unknown=False,final=True):
 events=[];converted=[];parent=l.XML_ParserCreate(None);parser=l.XML_ExternalEntityParserCreate(parent,b'',protocol) if child else parent
 assert parser
 text=CHAR(lambda _,p,n:events.append(['text',c.string_at(p,n).decode('utf8')]))
 start=START(lambda _,name,atts:events.append(['start',name.decode('utf8')]))
 @CONVERT
 def convert(_,p):
  v=c.cast(p,c.POINTER(c.c_byte));result=-1 if v[0]==-1 else (v[1]+(v[0]&127))&511;converted.append(result);return result
 @UNKNOWN
 def handler(_,name,out):
  for i in range(256):out.contents.map[i]=i if i<128 else -2
  out.contents.data=None;out.contents.convert=c.cast(convert,P).value;out.contents.release=None;return 1
 l.XML_SetCharacterDataHandler(parser,text);l.XML_SetStartElementHandler(parser,start);l.XML_SetReparseDeferralEnabled(parser,0)
 if unknown:l.XML_SetUnknownEncodingHandler(parser,handler,None)
 parts=[body] if not width else [body[i:i+width] for i in range(0,len(body),width)]
 for index,part in enumerate(parts):
  status=l.XML_Parse(parser,part,len(part),int(final and index==len(parts)-1))
  if status==0:break
 error=l.XML_GetErrorCode(parser)
 if child:l.XML_ParserFree(parser)
 l.XML_ParserFree(parent)
 text=''.join(e[1] for e in events if e[0]=='text');starts=[e[1]for e in events if e[0]=='start']
 return {'status':status,'error':error,'text':text,'starts':starts,'converter_results':converted}
cases=[]
for name,body,protocol in [('latin1-le-bom',b'\xff\xfeL ',b'iso-8859-1'),('latin1-be-bom',b'\xfe\xff L',b'iso-8859-1'),('latin1-utf8-bom',b'\xef\xbb\xbfX',b'iso-8859-1'),('auto-le-ascii',b'a\0b\0c\0',None),('auto-be-ascii',b'\0a\0b\0c',None),('auto-le-markup',b'<\0r\0/\0>\0',None),('auto-be-markup',b'\0<\0r\0/\0>',None),('auto-le-bom',b'\xff\xfea\0b\0c\0',None),('explicit-le-ascii',b'a\0b\0c\0',b'UTF-16LE'),('explicit-utf8-ascii-nul',b'a\0b\0c\0',b'UTF-8')]:
 for width in [0,1,2,3]:cases.append((name,body,protocol,width,True,False,True))
prefix=b"<?xml version='1.0' encoding='prefix-conv'?>\n"
for name,body in [('prefix-success',b'<\x81\x64\x80oc>Hello, world</\x81\x64\x80oc>'),('prefix-long-name-1',b'<abcdefghabcdefghabcdefghijkl\x80m\x80n\x80o\x80p>Hi</abcdefghabcdefghabcdefghijkl\x80m\x80n\x80o\x80p>'),('prefix-long-name-2',b'<abcdefghabcdefghabcdefghijklmnop>Hi</abcdefghabcdefghabcdefghijklmnop>'),('prefix-nonascii',b'<\xc1\x7f/>')]:
 for width in [0,1,3]:cases.append((name,prefix+body,None,width,False,True,True))
for name,body,final in [('supplementary-name','<do\U00010000/>'.encode(),True),('name-case19-start','<\u0901><!--'.encode(),False),('name-case19-mid','<a\u0901><!--'.encode(),False)]:
 for width in [0,1,3]:cases.append((name,body,None,width,False,False,final))
rows=[]
for name,body,protocol,width,child,unknown,final in cases:
 row={'name':name,'input_hex':body.hex(),'protocol':protocol.decode() if protocol else None,'width':width,'child':child,'unknown_handler':unknown,'final':final}
 for key,l in zip(['reference','candidate'],libs):row[key]=probe(l,body,protocol,width,child,unknown,final)
 rows.append(row)
result={'libraries':{p:hashlib.sha256(Path(p).read_bytes()).hexdigest()for p in paths},'cases':len(rows),'rows':rows}
Path('/tmp/oriole-encoding-classification.json').write_text(json.dumps(result,indent=2)+'\n')
for row in rows:
 if row['width']==0:print(row['name'],row['reference'],row['candidate'])
