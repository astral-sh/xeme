from pathlib import Path
import json,hashlib
base=Path('/tmp/oriole-external-pe-review/probe.py').read_text().split("if __name__ == '__main__':")[0]
base=base.replace("('XML_ParserCreate', P, [S]),","('XML_UseForeignDTD',I,[P,c.c_ubyte]),\n    ('XML_ParserCreate', P, [S]),").replace('name = decode(system)','name = decode(system) or "d"').replace('status = l.XML_Parse(parent, document, len(document), 1)','l.XML_UseForeignDTD(parent,1)\n    status = l.XML_Parse(parent, document, len(document), 1)')
libs=['/home/dev-user/.cache/oriole/expat-build/libexpat.so','/tmp/oriole-values-final-source/liboriole_expat.so'];rows=[]
for body in ('<r/>','<r>&entity;</r>'):
 for reject in (False,True):
  scopes=[]
  for lib in libs:
   code=base.replace(libs[0],lib).replace('b"<!DOCTYPE r SYSTEM \'d\'><r/>"',repr(body.encode()))
   if reject:code=code.replace("NOT(lambda _: (event('not-standalone'), 1)[1])","NOT(lambda _: (event('not-standalone'), 0)[1])")
   scope={};exec(code,scope);scopes.append(scope)
  for content in (b'',b'<!ELEMENT r ANY>',b'<!ENTITY entity "X">'):
   result=[s['probe'](None,2,content,b'')for s in scopes];rows.append({'body':body,'reject':reject,'dtd':content.decode(),'reference':result[0],'candidate':result[1]})
for r in rows:print(r['body'],r['reject'],r['dtd'],[(r[k]['status'],r[k]['error'],[e[1]for e in r[k]['events']])for k in ('reference','candidate')])
Path('/tmp/oriole-upstream-final-values/foreign-probe.json').write_text(json.dumps(rows,indent=2)+'\n')
