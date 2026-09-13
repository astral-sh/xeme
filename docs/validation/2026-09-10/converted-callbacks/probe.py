from pathlib import Path
import json,hashlib,argparse
arguments=argparse.ArgumentParser()
arguments.add_argument("--candidate",type=Path,required=True)
args=arguments.parse_args()
source=Path(__file__).with_name('probe-support.py').read_text().replace("p=lib.XML_ParserCreate(b'alias');assert p", "p=lib.XML_ParserCreate(None);assert p")
reference='/home/dev-user/.cache/oriole/expat-build/libexpat.so'
libraries={'reference':reference,'baseline':'/tmp/oriole-api-amplification/reviewed/liboriole_expat.so','candidate':str(args.candidate)}
loaded={}
for name,path in libraries.items():
 state={};exec(compile(source.replace(reference,path),name,'exec'),state);loaded[name]=state
cases={f'version-{index}':f"<?xml version='{version}'?><r/>".encode() for index,version in enumerate(['1.0','1.1','1.7','1.01','1.000','1.','1.x','2.0','01.0','A_-9','','1.\u0661','1. 0'])}
cases['replacement-cr']=b"<!DOCTYPE r [<!ENTITY e 'a&#13;b<![CDATA[c&#13;d]]><!--e&#13;f--><?pi g&#13;h?>'><!ENTITY nl 'a\r\nb'>]><r>&e;&nl;</r>"
rows=[]
for name,data in cases.items():
 for chunk in [0,1,7]:
  rows.append({'name':name,'hex':data.hex(),'chunk':chunk,'results':{name:state['parse'](data,chunk,False) for name,state in loaded.items()}})
report={'libraries':{name:{'path':path,'sha256':hashlib.sha256(Path(path).read_bytes()).hexdigest()} for name,path in libraries.items()},'cases':rows}
Path('/tmp/oriole-ordinary-payload-reproduced.json').write_text(json.dumps(report,indent=2)+'\n')
print('cases',len(rows),'candidate acceptance mismatches',sum(row['results']['reference']['status']!=row['results']['candidate']['status'] for row in rows),'candidate successful callback mismatches',sum(row['results']['reference']['status']==row['results']['candidate']['status']==1 and row['results']['reference']['events']!=row['results']['candidate']['events'] for row in rows))
