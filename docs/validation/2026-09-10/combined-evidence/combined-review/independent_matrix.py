from pathlib import Path
import sys,json,hashlib,gzip
ns={}
exec(Path('/tmp/oriole-external-value-review/reference.py').read_text().split('rows=[]')[0],ns)
source=ns['source'].replace("'data':data.decode()", "'data':data.hex()")
reference='/home/dev-user/.cache/oriole/expat-build/libexpat.so'
candidate=sys.argv[1]
output=Path(sys.argv[2])

def normalize(row):
    events=[]
    for item in row['events']:
        if item[:2]==['parent','not-standalone']:continue
        item=item[:2]+item[3:]
        if item[1]=='default' and events and events[-1][:2]==item[:2]:events[-1][2]+=item[2]
        else:events.append(item)
    return {'status':row['status'],'error':row['error'],'children':[(c['name'],c['status'],c['error']) for c in row['children']], 'events':events}

bodies=[
 '<!ENTITY % p SYSTEM "p"><!ENTITY % q SYSTEM "q"><!ENTITY e "L%p;R"><!ENTITY after "A">',
 '<!ENTITY % p SYSTEM "p"><!ENTITY % q SYSTEM "q"><!ENTITY e "L%missing;%p;R"><!ENTITY after "A">',
 '<!ENTITY % p SYSTEM "p"><!ENTITY % q SYSTEM "q"><!ENTITY e "FIRST"><!ENTITY e "L%p;R"><!ENTITY after "A">',
]
values=[('unicode','名 X'),('quoted_refs','"&#65;&amp;\r\n名"'),('nested','X%q;Y'),('standalone','<?xml version="1.0" standalone="yes"?>名'),('prefix_decl','<!--old--><?xml version="1.0"?>名')]
rows=0;diffs=[];status_diffs=0
with gzip.open(str(output)+'.cases.json.gz','wt') as stream:
 for handler in (False,True):
    scopes=[]
    for lib in (reference,candidate):
        code=source.replace(reference,lib)
        if not handler:code=code.replace('l.XML_SetEntityDeclHandler(parent, callbacks[3])','pass')
        scope={};exec(code,scope);scopes.append(scope)
    for bi,body in enumerate(bodies):
     for vn,value in values:
      for enc in ('utf-8','utf-16-le','utf-16-be'):
       bom={'utf-8':b'','utf-16-le':b'\xff\xfe','utf-16-be':b'\xfe\xff'}[enc]
       data=body.encode()
       external={'p':bom+value.encode(enc),'q':bom+'"Q&#13;名"'.encode(enc)}
       for standalone in (None,'yes'):
        for mode in (0,1,2):
         for chunk in range(0,len(external['p'])+1):
          results=[normalize(s['probe'](standalone,mode,data,external,'parse',chunk)) for s in scopes]
          row={'body':bi,'value':vn,'encoding':enc,'standalone':standalone,'mode':mode,'chunk':chunk,'handler':handler,'reference':results[0],'candidate':results[1]}
          stream.write(json.dumps(row,separators=(',',':'))+'\n');rows+=1
          if results[0]!=results[1]:
           diffs.append(row)
           if any(results[0][k]!=results[1][k] for k in ('status','error','children')):status_diffs+=1
report={'library':candidate,'sha256':hashlib.sha256(Path(candidate).read_bytes()).hexdigest(),'reference_sha256':hashlib.sha256(Path(reference).read_bytes()).hexdigest(),'cases':rows,'differences':len(diffs),'status_error_differences':status_diffs,'bodies':bodies,'values':values,'rows':diffs,'normalization':'Compare outcomes and event payloads with byte positions and parent NotStandalone removed; coalesce consecutive Default fragments per parser.'}
output.with_suffix('.json').write_text(json.dumps(report,separators=(',',':'))+'\n')
print(json.dumps({k:v for k,v in report.items() if k not in ('rows','bodies','values')}))
for r in diffs[:6]:print(json.dumps(r))
