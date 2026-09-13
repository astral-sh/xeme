from pathlib import Path
import json,hashlib
base=Path('/tmp/oriole-external-pe-review/probe.py').read_text().split("if __name__ == '__main__':")[0]
base=base.replace("('XML_ParserCreate', P, [S]),","('XML_UseForeignDTD',I,[P,c.c_ubyte]),\n    ('XML_ParserCreate', P, [S]),").replace('name = decode(system)','name = decode(system) or "d"').replace('status = l.XML_Parse(parent, document, len(document), 1)','l.XML_UseForeignDTD(parent,1)\n    status = l.XML_Parse(parent, document, len(document), 1)')
libs=['/home/dev-user/.cache/oriole/expat-build/libexpat.so','/tmp/oriole-values-final-source/liboriole_expat.so'];rows=[]
for body in ('<r/>','<r>&entity;</r>'):
 for standalone in (None,'no','yes'):
  for mode in (0,1,2):
   for reject in (False,True):
    for action in ('skip','create','nonfinal-empty','parse','absent'):
     scopes=[]
     for lib in libs:
      code=base.replace(libs[0],lib).replace('b"<!DOCTYPE r SYSTEM \'d\'><r/>"',repr(body.encode())).replace('l.XML_SetParamEntityParsing(parent, 2)','l.XML_SetParamEntityParsing(parent, mode)')
      if reject:code=code.replace("NOT(lambda _: (event('not-standalone'), 1)[1])","NOT(lambda _: (event('not-standalone'), 0)[1])")
      if action=='skip':code=code.replace("if name != 'd' and action == 'skip':","if True:")
      if action=='create':code=code.replace("saved = active.copy()","l.XML_ParserFree(child)\n        return 1\n        saved = active.copy()")
      if action=='nonfinal-empty':code=code.replace('int(start+width>=len(content))','0')
      if action=='absent':code=code.replace('l.XML_SetExternalEntityRefHandler(parent, callbacks[5])','pass')
      scope={};exec(code,scope);scopes.append(scope)
     for content in (b'',b'<!ENTITY entity "X">'):
      if action=='nonfinal-empty' and content:continue
      result=[s['probe'](standalone,mode,content,b'')for s in scopes];rows.append({'body':body,'standalone':standalone,'mode':mode,'reject':reject,'action':action,'dtd':content.decode(),'reference':result[0],'candidate':result[1]})
Path('/tmp/oriole-upstream-final-values/foreign-expanded.json').write_text(json.dumps(rows,indent=2)+'\n')
for r in rows:
 if r['dtd']=='' and r['body']=='<r>&entity;</r>' and r['standalone'] is None and not r['reject']:
  print(r['mode'],r['action'],[(r[k]['status'],r[k]['error'],[e[1] for e in r[k]['events']]) for k in ('reference','candidate')])
print('cases',len(rows))
