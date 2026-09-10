from pathlib import Path
import json,hashlib,sys,gzip,ast
HERE=Path(__file__).resolve().parent
ns={};exec((HERE/'reference.py').read_text().split('rows=[]')[0],ns)
normalize={};exec(Path('/tmp/oriole-default-declarations/compare.py').read_text().split('rows=[]')[0],normalize)
tree=ast.parse((HERE/'merge_matrix.py').read_text())
bodies=next(ast.literal_eval(n.value)for n in tree.body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='bodies'for t in n.targets))
REF='/home/dev-user/.cache/oriole/expat-build/libexpat.so';lib=sys.argv[1];out=Path(sys.argv[2]);rows=0;diffs=[]
with gzip.open(str(out)+'.cases.json.gz','wt')as stream:
 for default in ('absent','present','install','remove'):
  for handler in (False,True):
   scopes=[]
   for library in (REF,lib):
    code=ns['source'].replace(REF,library)
    if default in ('absent','install'):code=code.replace('l.XML_SetDefaultHandlerExpand(parent, callbacks[0])','pass')
    if not handler:code=code.replace('l.XML_SetEntityDeclHandler(parent, callbacks[3])','pass')
    if default in ('install','remove'):
     mutation='DEFAULT()'if default=='remove'else'callbacks[0]'
     code=code.replace("name = decode(system)\n        event",f"name = decode(system)\n        if name == 'p': l.XML_SetDefaultHandlerExpand(parser,{mutation})\n        event")
    scope={};exec(code,scope);scopes.append(scope)
   for bi,body in enumerate(bodies):
    for standalone in (None,'yes'):
     for mode in (0,1,2):
      for action in ('parse','skip','empty','create-only'):
       for chunk in (0,1,3):
        data='<!ENTITY % p SYSTEM "p"><!ENTITY % i "I">'+body+'<!ENTITY after "A"><!ATTLIST r a CDATA "yes">'
        a,b=[normalize['norm'](s['probe'](standalone,mode,data.encode(),b'X',action,chunk))for s in scopes]
        row={'body':bi,'standalone':standalone,'mode':mode,'action':action,'chunk':chunk,'handler':handler,'default':default,'reference':a,'candidate':b};stream.write(json.dumps(row,separators=(',',':'))+'\n');rows+=1
        if a!=b:diffs.append(row)
report={'library':lib,'sha256':hashlib.sha256(Path(lib).read_bytes()).hexdigest(),'reference_sha256':hashlib.sha256(Path(REF).read_bytes()).hexdigest(),'cases':rows,'differences':len(diffs),'outcome_differences':sum(any(r['reference'][k]!=r['candidate'][k]for k in ('status','error','children'))for r in diffs),'successful_differences':sum(r['reference']['status']==1 for r in diffs),'bodies':bodies,'rows':diffs}
out.with_suffix('.json').write_text(json.dumps(report,separators=(',',':'))+'\n')
print(json.dumps({k:v for k,v in report.items()if k not in ('rows','bodies')}))
for row in [r for r in diffs if r['reference']['status']==1][:8]:print(json.dumps(row))
