import ctypes as c
import hashlib
import json
from pathlib import Path

LIBRARY = '/home/dev-user/.cache/oriole/expat-build/libexpat.so'
lib = c.CDLL(LIBRARY)
CONVERT = c.CFUNCTYPE(c.c_int, c.c_void_p, c.c_void_p)
class Encoding(c.Structure):
    _fields_ = [('map', c.c_int * 256), ('data', c.c_void_p), ('convert', c.c_void_p), ('release', c.c_void_p)]
UNKNOWN = c.CFUNCTYPE(c.c_int, c.c_void_p, c.c_char_p, c.POINTER(Encoding))
START = c.CFUNCTYPE(None, c.c_void_p, c.c_char_p, c.POINTER(c.c_char_p))
END = c.CFUNCTYPE(None, c.c_void_p, c.c_char_p)
TEXT = c.CFUNCTYPE(None, c.c_void_p, c.c_void_p, c.c_int)
PI = c.CFUNCTYPE(None, c.c_void_p, c.c_char_p, c.c_char_p)
ENTITY = c.CFUNCTYPE(None, c.c_void_p, c.c_char_p, c.c_int, c.c_void_p, c.c_int, c.c_char_p, c.c_char_p, c.c_char_p, c.c_char_p)
for name,args,result in [
    ('XML_ParserCreate',[c.c_char_p],c.c_void_p),
    ('XML_ParserFree',[c.c_void_p],None),
    ('XML_Parse',[c.c_void_p,c.c_char_p,c.c_int,c.c_int],c.c_int),
    ('XML_GetErrorCode',[c.c_void_p],c.c_int),
    ('XML_GetCurrentByteIndex',[c.c_void_p],c.c_long),
    ('XML_SetUnknownEncodingHandler',[c.c_void_p,UNKNOWN,c.c_void_p],None),
    ('XML_SetElementHandler',[c.c_void_p,START,END],None),
    ('XML_SetCharacterDataHandler',[c.c_void_p,TEXT],None),
    ('XML_SetDefaultHandlerExpand',[c.c_void_p,TEXT],None),
    ('XML_SetProcessingInstructionHandler',[c.c_void_p,PI],None),
    ('XML_SetEntityDeclHandler',[c.c_void_p,ENTITY],None),
]:
    fn=getattr(lib,name);fn.argtypes=args;fn.restype=result

@CONVERT
def convert(_, pointer):
    return c.string_at(pointer,2)[1]

@UNKNOWN
def unknown(_, name, output):
    for index in range(256):output.contents.map[index]=index
    output.contents.map[128]=-2
    output.contents.convert=c.cast(convert,c.c_void_p).value
    return 1


COMMENT=c.CFUNCTYPE(None,c.c_void_p,c.c_char_p)
DOCTYPE=c.CFUNCTYPE(None,c.c_void_p,c.c_char_p,c.c_char_p,c.c_char_p,c.c_int)
NOTATION=c.CFUNCTYPE(None,c.c_void_p,c.c_char_p,c.c_char_p,c.c_char_p,c.c_char_p)
ATTLIST=c.CFUNCTYPE(None,c.c_void_p,c.c_char_p,c.c_char_p,c.c_char_p,c.c_char_p,c.c_int)
DECL=c.CFUNCTYPE(None,c.c_void_p,c.c_char_p,c.c_char_p,c.c_int)
for name,kind in [('XML_SetCommentHandler',COMMENT),('XML_SetStartDoctypeDeclHandler',DOCTYPE),('XML_SetNotationDeclHandler',NOTATION),('XML_SetAttlistDeclHandler',ATTLIST),('XML_SetXmlDeclHandler',DECL)]:
 getattr(lib,name).argtypes=[c.c_void_p,kind]
def decoded(value):return value.decode() if value is not None else None

def parse(data, chunk=0, default=False):
    events=[]
    @START
    def start(_,name,attrs):
        values=[];index=0
        while attrs[index]:
            values.append(attrs[index].decode());index+=1
        events.append(['start',name.decode(),values])
    @END
    def end(_,name):events.append(['end',name.decode()])
    @TEXT
    def text(_,pointer,length):
        value=c.string_at(pointer,length).decode()
        if events and events[-1][0]=='text':events[-1][1]+=value
        else:events.append(['text',value])
    @TEXT
    def raw(_,pointer,length):
        value=c.string_at(pointer,length).decode()
        if events and events[-1][0]=='default':events[-1][1]+=value
        else:events.append(['default',value])
    @PI
    def pi(_,name,value):events.append(['pi',name.decode(),value.decode()])
    @ENTITY
    def entity(_,name,parameter,value,length,base,system,public,notation):
        events.append(['entity',name.decode(),parameter,c.string_at(value,length).decode() if value else None,decoded(system),decoded(public),decoded(notation)])

    @COMMENT
    def comment(_,text):events.append(['comment',decoded(text)])
    @DOCTYPE
    def doctype(_,name,system,public,subset):events.append(['doctype',decoded(name),decoded(system),decoded(public),subset])
    @NOTATION
    def notation(_,name,base,system,public):events.append(['notation',decoded(name),decoded(system),decoded(public)])
    @ATTLIST
    def attlist(_,element,name,kind,value,required):events.append(['attlist',decoded(element),decoded(name),decoded(kind),decoded(value),required])
    @DECL
    def declaration(_,version,encoding,standalone):events.append(['declaration',decoded(version),decoded(encoding),standalone])

    p=lib.XML_ParserCreate(b'alias');assert p
    lib.XML_SetUnknownEncodingHandler(p,unknown,None)
    lib.XML_SetCommentHandler(p,comment)
    lib.XML_SetStartDoctypeDeclHandler(p,doctype)
    lib.XML_SetNotationDeclHandler(p,notation)
    lib.XML_SetAttlistDeclHandler(p,attlist)
    lib.XML_SetXmlDeclHandler(p,declaration)
    lib.XML_SetElementHandler(p,start,end)
    lib.XML_SetCharacterDataHandler(p,text)
    lib.XML_SetProcessingInstructionHandler(p,pi)
    lib.XML_SetEntityDeclHandler(p,entity)
    if default:lib.XML_SetDefaultHandlerExpand(p,raw)
    width=chunk or len(data)
    status=1
    for offset in range(0,len(data),width):
        piece=data[offset:offset+width];status=lib.XML_Parse(p,piece,len(piece),int(offset+width>=len(data)))
        if status!=1:break
    row={'status':status,'error':lib.XML_GetErrorCode(p),'byte_index':lib.XML_GetCurrentByteIndex(p),'events':events}
    lib.XML_ParserFree(p)
    return row

CASES = {
    'content': b'<r>@@</r>',
    'cdata': b'<r><![CDATA[@@]]></r>',
    'attribute': b"<r a='@@'/>",
    'pi-data': b'<?p @@?><r/>',
    'comment': b'<!--@@--><r/>',
    'name-matched': b'<@@oc></@@oc>',
    'name-start-alias': b'<@@oc></Aoc>',
    'name-end-alias': b'<Aoc></@@oc>',
    'name-trailing': b'<a@@></a@@>',
    'attribute-name': b"<r @@='1'/>",
    'attribute-duplicate': b"<r A='1' @@='2'/>",
    'pi-target': b'<?@@ml?><r/>',
    'pi-target-middle': b'<?x@@l?><r/>',
    'doctype-keyword': b'<!@@OCTYPE r><r/>',
    'doctype-name': b'<!DOCTYPE @@><r/>',
    'entity-keyword': b'<!DOCTYPE r [<!@@NTITY e "A">]><r>&e;</r>',
    'entity-name': b'<!DOCTYPE r [<!ENTITY @@ "A">]><r>&@@;</r>',
    'entity-value': b'<!DOCTYPE r [<!ENTITY e "@@">]><r>&e;</r>',
    'entity-value-preserved': b'<!DOCTYPE r [<!ENTITY e "@@">]><r/>',
    'default-value': b'<!DOCTYPE r [<!ATTLIST r a CDATA "@@">]><r/>',
    'attribute-type': b'<!DOCTYPE r [<!ATTLIST r a @@DATA "v">]><r/>',
    'element-keyword': b'<!DOCTYPE r [<!ELEMENT r @@MPTY>]><r/>',
    'numeric-reference': b'<r>&#@@65;</r>',
    'numeric-digits': b'<r>&#x@@1;</r>',
    'predefined-reference': b'<r>&@@mp;</r>',
    'reference-end': b'<!DOCTYPE r [<!ENTITY e "a">]><r>&e@@</r>',
    'xml-decl-version': b"<?xml @@ersion='1.0'?><r/>",
    'xml-decl-version-value': b"<?xml version='@@.0'?><r/>",
    'namespace-prefix': b'<@@:r xmlns:A="u"/>',
    'public-id': b'<!DOCTYPE r PUBLIC "@@" "s"><r/>',
    'system-id': b'<!DOCTYPE r SYSTEM "@@"><r/>',
}

