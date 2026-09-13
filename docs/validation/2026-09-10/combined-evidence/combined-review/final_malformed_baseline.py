from pathlib import Path
import json,hashlib
HERE=Path(__file__).resolve().parent
base={};exec((HERE/'reference.py').read_text().split('rows=[]')[0],base)
normalize={};exec(Path('/tmp/oriole-default-declarations/compare.py').read_text().split('rows=[]')[0],normalize)
lib='/home/dev-user/.cache/oriole/default-declarations-target/debug/liboriole_expat.so';scopes={};results=[]
for report_name in ('merge-final','default-switch-final'):
 report=json.loads((HERE/(report_name+'.json')).read_text());rows=[]
 for row in report['rows']:
  assert row['reference']['status']==0
  assert all(child[0]!='p' for child in row['candidate']['children'])
  enabled=row.get('default','present') in ('present','remove');key=(row['handler'],enabled)
  if key not in scopes:
   source=base['source'].replace('/home/dev-user/.cache/oriole/expat-build/libexpat.so',lib)
   if not key[0]:source=source.replace('l.XML_SetEntityDeclHandler(parent, callbacks[3])','pass')
   if not key[1]:source=source.replace('l.XML_SetDefaultHandlerExpand(parent, callbacks[0])','pass')
   scopes[key]={};exec(source,scopes[key])
  data='<!ENTITY % p SYSTEM "p"><!ENTITY % i "I">'+report['bodies'][row['body']]+'<!ENTITY after "A"><!ATTLIST r a CDATA "yes">'
  got=scopes[key]['probe'](row['standalone'],row['mode'],data.encode(),b'X',row['action'],row['chunk'])
  got=json.loads(json.dumps(normalize['norm'](got)))
  rows.append({k:row[k]for k in ('body','standalone','mode','action','chunk','handler')}|{'matches_baseline':got==row['candidate']})
 results.append({'report':report_name,'cases':len(rows),'matches_baseline':sum(x['matches_baseline']for x in rows),'rows':rows})
output={'library':lib,'sha256':hashlib.sha256(Path(lib).read_bytes()).hexdigest(),'results':results}
(HERE/'final-malformed-baseline.json').write_text(json.dumps(output,separators=(',',':'))+'\n')
for r in results:print(r['report'],r['cases'],r['matches_baseline'])
