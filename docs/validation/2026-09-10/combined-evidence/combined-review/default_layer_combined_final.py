from pathlib import Path
import hashlib
import json

BASE=Path('/tmp/oriole-external-pe-review/probe.py').read_text().split("if __name__ == '__main__':")[0]
LIBRARIES=['/home/dev-user/.cache/oriole/expat-build/libexpat.so','/tmp/oriole-values-combined-fixed2.so']
BODIES=[
 '<!ENTITY e %missing;"X">', '<!ATTLIST r %missing;a CDATA "X">',
 '<!ATTLIST r a CDATA "X" %missing; b CDATA "Y">',
 '<!ENTITY e "L%missing;R"><!ENTITY after "A">',
 '<!ENTITY e SYSTEM %missing;"sys">',
 '<!ATTLIST r a %missing;CDATA "X">',
 '<!ATTLIST r a CDATA %missing;"X">',
 '<!ENTITY e %missing;%other;"X">',
 '<!ATTLIST r a CDATA "X" %missing; b CDATA "Y" %other; c CDATA "Z">',
 '<!ATTLIST r>', '<!ATTLIST r a CDATA "X" b CDATA "Y">',
 '<!ENTITY % t "CDATA"><!ATTLIST r a %t; "X" %missing; b CDATA "Y">',
 '<!ENTITY e "FIRST"><!ENTITY e "SECOND">',
 '<!ENTITY e "FIRST"><!ENTITY e SYSTEM "sys">',
 '<!ENTITY e "FIRST"><!ENTITY e PUBLIC "pub" "sys">',
 '<!ENTITY e "FIRST"><!ENTITY e SYSTEM "sys" NDATA n>',
 '<!ENTITY % e "FIRST"><!ENTITY % e "SECOND">',
 '<!ENTITY e "FIRST"><!ENTITY e %missing;"SECOND">',
 '<!ENTITY e "FIRST"><!ENTITY e "L%missing;R">',
 '<!ENTITY e "FIRST"><!ENTITY e SYSTEM %missing;"sys">',
]

def norm(result):
    events=[]
    for event in result['events']:
        if event[:2]==['parent','not-standalone']:
            continue
        event=event[:2]+event[3:]
        if event[1]=='default' and events and events[-1][:2]==event[:2]:
            events[-1][2]+=event[2]
        else:
            events.append(event)
    return {'status':result['status'],'error':result['error'],'children':[(x['name'],x['status'],x['error'])for x in result['children']], 'events':events}

rows=[]
for entity in (False,True):
    for attlist in (False,True):
        for expand in (False,True):
            scopes=[]
            for library in LIBRARIES:
                source=BASE.replace(LIBRARIES[0],library)
                if not entity:source=source.replace('l.XML_SetEntityDeclHandler(parent, callbacks[3])','pass')
                if not attlist:source=source.replace('l.XML_SetAttlistDeclHandler(parent, callbacks[4])','pass')
                if not expand:source=source.replace('l.XML_SetDefaultHandlerExpand(parent, callbacks[0])','l.XML_SetDefaultHandler(parent, callbacks[0])')
                scope={};exec(source,scope);scopes.append(scope)
            for body in BODIES:
                for standalone in (None,'yes'):
                    for mode in (0,1,2):
                        for chunk in (0,1,3):
                            a,b=[norm(s['probe'](standalone,mode,body.encode(),b'',chunk=chunk))for s in scopes]
                            rows.append({'body':body,'standalone':standalone,'mode':mode,'chunk':chunk,'entity':entity,'attlist':attlist,'expand':expand,'match':a==b,'reference':a,'candidate':b})
report={'cases':len(rows),'differences':sum(not row['match']for row in rows),'library':LIBRARIES[1],'sha256':hashlib.sha256(Path(LIBRARIES[1]).read_bytes()).hexdigest(),'rows':rows}
Path('/tmp/oriole-external-value-review/default-layer-combined-final.json').write_text(json.dumps(report,separators=(',',':'))+'\n')
print('cases',report['cases'],'differences',report['differences'])
for row in rows:
    if not row['match'] and row['chunk']==0 and row['expand'] and row['mode']==2:print(json.dumps(row))
