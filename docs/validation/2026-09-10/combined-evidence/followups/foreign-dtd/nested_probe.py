from pathlib import Path
import json,itertools,hashlib,sys
p=Path('/tmp/oriole-foreign-dtd')
s=(p/'matrix.py').read_text().split('cases=[]')[0]
s=s.replace("if action=='create':l.XML_ParserFree(child);return 1","if action=='create' or (name=='q' and action=='nested-create'):l.XML_ParserFree(child);return 1")
s=s.replace("if action in ('load','nested-read','nested-skip'):","if action in ('load','nested-read','nested-skip','nested-create','nested-empty','nested-missing'):")
s=s.replace("if name=='foreign' and action.startswith('nested-'):body='<!ENTITY % q SYSTEM \"q\">%q;'+body", "if name=='foreign' and action.startswith('nested-'):\n    nested='%missing;' if action=='nested-missing' else '<!ENTITY % q SYSTEM \"q\">%q;'\n    body=body+nested if BEFORE else nested+body\n   if name=='q' and action=='nested-empty':body=''")
s=s.replace("action!='nonfinal-empty'","action!='nonfinal-empty' and not (name=='q' and action=='nested-empty')")
scope={};exec(s,scope);rows=[]
for before,prefix,mode,action,reject,width,entity in itertools.product([False,True],['','<!DOCTYPE r []>','<!DOCTYPE r [<!ENTITY % prior "">%prior;]>'],[1,2],['nested-skip','nested-create','nested-empty','nested-read','nested-missing'],[False,True],[0,1],['supplied','missing']):
 scope['BEFORE']=before;args=(prefix+'<r>&'+entity+';</r>',mode,action,reject,width,'utf-8',True)
 ref,cand=[scope['probe'](l,*args)for l in scope['libs']];rows.append(dict(before=before,case=args,reference=ref,candidate=cand))
report={'candidate_sha256':hashlib.sha256(Path(scope['CAND']).read_bytes()).hexdigest(),'cases':len(rows),'differences':sum(r['reference']!=r['candidate']for r in rows),'outcome_differences':sum((r['reference']['status'],r['reference']['error'])!=(r['candidate']['status'],r['candidate']['error'])for r in rows),'rows':rows}
(p/'nested-probe.json').write_text(json.dumps(report,indent=2)+'\n')
print({k:v for k,v in report.items()if k!='rows'})
for r in rows:
 if not r['before'] and r['case'][0]=='<r>&missing;</r>' and r['case'][1]==2 and not r['case'][3] and r['case'][4]==0:print(r['case'][2],r['reference'],r['candidate'])
