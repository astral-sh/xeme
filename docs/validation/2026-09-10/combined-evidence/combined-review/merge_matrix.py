from pathlib import Path
import sys,json,hashlib,gzip
ns={};exec(Path('/tmp/oriole-external-value-review/reference.py').read_text().split('rows=[]')[0],ns)
source=ns['source']
REF='/home/dev-user/.cache/oriole/expat-build/libexpat.so'
CAND=sys.argv[1]; out=Path(sys.argv[2])
normalize={};exec(Path('/tmp/oriole-default-declarations/compare.py').read_text().split('rows=[]')[0],normalize)
bodies=[
 '<!ENTITY e %missing;"L%p;R">',
 '<!ENTITY e %missing;%other;"L%p;R">',
 '<!ENTITY e %missing;"L%i;%p;R">',
 '<!ENTITY % quoted "\'L&#37;p;R\'"><!ENTITY e %quoted;>',
 '<!ENTITY % quoted "\'L&#37;p;R\'"><!ENTITY e %missing;%quoted;>',
 '<!ENTITY % quoted "\'L&#37;p;R\'"><!ENTITY e %quoted; %missing;>',
 '<!ENTITY % name "e"><!ENTITY %name; "L%p;R">',
 '<!ENTITY % name "e"><!ENTITY %missing;%name; "L%p;R">',
 '<!ENTITY % fields "e \'L&#37;p;R\'"><!ENTITY %fields;>',
 '<!ENTITY % fields "e \'L&#37;p;R\'"><!ENTITY %missing;%fields;>',
 '<!ENTITY e "FIRST"><!ENTITY e %missing;"L%p;R">',
 '<!ENTITY e "FIRST"><!ENTITY % quoted "\'L&#37;p;R\'"><!ENTITY e %missing;%quoted;>',
]
prefix='<!ENTITY % p SYSTEM "p"><!ENTITY % i "I">'
tail='<!ENTITY after "A"><!ATTLIST r a CDATA "yes">'
rows=0;diffs=[]
with gzip.open(str(out)+'.cases.json.gz','wt') as stream:
 for handler in (False,True):
  for change in ('none','remove','install'):
   if change=='remove' and not handler or change=='install' and handler:continue
   scopes=[]
   for lib in (REF,CAND):
    code=source.replace(REF,lib)
    if not handler:code=code.replace('l.XML_SetEntityDeclHandler(parent, callbacks[3])','pass')
    if change!='none':
     mutation='ENTITY()' if change=='remove' else 'callbacks[3]'
     code=code.replace("name = decode(system)\n        event",f"name = decode(system)\n        if name == 'p': l.XML_SetEntityDeclHandler(parser, {mutation})\n        event")
    scope={};exec(code,scope);scopes.append(scope)
   for bi,body in enumerate(bodies):
    for standalone in (None,'yes'):
     for mode in (0,1,2):
      for action in ('parse','skip','empty','create-only'):
       for chunk in (0,1,3):
        args=(standalone,mode,(prefix+body+tail).encode(),b'X',action,chunk)
        a,b=[normalize['norm'](s['probe'](*args))for s in scopes]
        row={'body':bi,'standalone':standalone,'mode':mode,'action':action,'chunk':chunk,'handler':handler,'change':change,'reference':a,'candidate':b}
        stream.write(json.dumps(row,separators=(',',':'))+'\n');rows+=1
        if a!=b:diffs.append(row)
report={'library':CAND,'sha256':hashlib.sha256(Path(CAND).read_bytes()).hexdigest(),'reference_sha256':hashlib.sha256(Path(REF).read_bytes()).hexdigest(),'cases':rows,'differences':len(diffs),'outcome_differences':sum(any(x['reference'][k]!=x['candidate'][k]for k in ('status','error','children'))for x in diffs),'bodies':bodies,'rows':diffs}
out.with_suffix('.json').write_text(json.dumps(report,separators=(',',':'))+'\n')
print(json.dumps({k:v for k,v in report.items()if k not in ('bodies','rows')}))
for row in diffs[:8]:print(json.dumps(row))
