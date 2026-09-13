#!/usr/bin/env python3
"""Portable saved-evidence verification and original arithmetic replay; no targets."""
import argparse,contextlib,fnmatch,gzip,hashlib,io,json,os,re,shlex,sys,tarfile
from pathlib import Path

def digest(data):return hashlib.sha256(data).hexdigest()
def verify(root):
 assert __debug__, 'Python -O disables required assertions'
 index=json.loads(gzip.decompress((root/'members.json.gz').read_bytes()))
 archive=root/'evidence.tar.gz'
 with archive.open('rb') as f:archive_hash=hashlib.file_digest(f,'sha256').hexdigest()
 assert archive_hash==index['archive_sha256'] and archive.stat().st_size==index['archive_bytes']
 payload={}
 with tarfile.open(archive,'r:gz') as t:
  for m in t:
   assert m.isfile() and m.name not in payload and re.fullmatch(r'payload/[0-9a-f]{64}',m.name)
   assert m.uid==m.gid==m.mtime==0
   data=t.extractfile(m).read();assert len(data)==m.size and digest(data)==m.name[8:]
   assert not data.startswith((b'\x7fELF',b'!<arch>\n'));payload[m.name]=data
 entries={r['path']:r for r in index['origins']};omitted={r['path']:r for r in index['omitted']}
 assert len(entries)==len(index['origins']) and len(omitted)==len(index['omitted']) and not entries.keys()&omitted.keys()
 assert len(payload)==index['stored_members'] and set(payload)=={r['member'] for r in entries.values()}
 for r in entries.values():assert len(payload[r['member']])==r['bytes'] and digest(payload[r['member']])==r['sha256']
 used_omitted=set();writes={}
 S='/tmp/oriole-x86-64-v3-pgo-study';G='/tmp/oriole-allocator-provenance-pgo-study';E='/tmp/oriole-pgo-study';N='/tmp/oriole-x86-64-v3-native-independent-review';W='/home/dev-user/code/oss/oriole-allocator-x86-64-v3'
 def data(path):
  key=os.path.normpath(str(path))
  return writes[key] if key in writes else payload[entries[key]['member']]
 def h(path):
  key=str(path)
  if key in omitted:used_omitted.add(key);return omitted[key]['sha256']
  return digest(data(key))
 def load(path):return json.loads(data(path))
 # All prepared/frozen members are covered; omitted payloads are identities only.
 for directory in (S,G):
  frozen=load(directory+'/pair-freeze.json');assert frozen['status']=='frozen'
  for name,r in frozen['files'].items():assert h(directory+'/'+name)==r['sha256']
 assert len(load(S+'/pair-freeze.json')['files'])==108
 source=load(S+'/source.json')['source_sha256'];assert len(source)==70 and source==load(G+'/source.json')['source_sha256']
 for name,expected in source.items():assert h(W+'/'+name)==expected
 scripts=load(S+'/tool-source.json');assert len(scripts)==6 and scripts==load(G+'/tool-source.json')
 for directory in (S,G):
  for name,expected in scripts.items():assert h(directory+'/pipeline/'+name)==expected
 expat_source=load(E+'/expat-source.json')['files'];assert len(expat_source)==167
 for name,expected in expat_source.items():assert h(E+'/expat-source/'+name)==expected
 ap,cp=load(S+'/pair-report.json'),load(G+'/pair-report.json');am,cm=[load(p['pgo_manifest']['path']) for p in (ap,cp)];ar,cr=am['run'],cm['run']
 assert ap['status']==cp['status']==am['status']==cm['status']=='passed'
 assert am['source_sha256']==cm['source_sha256'] and len(am['source_sha256'])==67
 assert all(source[n]==v for n,v in am['source_sha256'].items())
 for k in ('tools_sha256','rustc_version','cargo_version','profdata_version','cargo_config_sha256','cargo_args'):assert am[k]==cm[k]
 assert am['base_rustflags']==['-Ctarget-cpu=x86-64-v3'] and cm['base_rustflags']==[]
 assert am['script_sha256']==cm['script_sha256']=={n.rsplit('/',1)[1]:v for n,v in scripts.items()}
 metadata={'oriole_storage':'8ae6edb74b4afbee','oriole':'e41f1ff64bd2ed1c','oriole_expat':'f10a79f8fbc60887'}
 def vectors(directory,pair,m,phase):
  path=directory+'/normal-build.log' if phase=='normal' else m['run']+'/'+next(c['log'] for c in m['commands'] if c['label']=='build-'+phase)
  result={}
  for line in data(path).decode().splitlines():
   if 'Running `' not in line or '--crate-name ' not in line:continue
   args=shlex.split(line.split('Running `',1)[1].rsplit('`',1)[0]);crate=args[args.index('--crate-name')+1]
   if crate in metadata:assert crate not in result;result[crate]=args
  expected=pair['normal_vectors'] if phase=='normal' else pair['pgo_vectors'][phase]
  assert result=={x['crate']:x['argv'] for x in expected} and set(result)==set(metadata)
  return result
 def norm(argv,run,name):
  assert argv.count('opt-level=3')==argv.count('codegen-units=1')==1
  assert [x for x in argv if x.startswith('metadata=')]==['metadata='+metadata[name]]
  assert argv[argv.index('--target')+1]=='x86_64-unknown-linux-gnu'
  assert ('lto=thin' if name=='oriole_expat' else 'linker-plugin-lto') in argv
  result=[]
  for arg in argv:
   arg=arg.replace(run,'<RUN>');arg=re.sub(r'/home/dev-user/\.cache/ohm/verified-build/[0-9a-f]{2}/[0-9a-f]{14}(?=/)','<BUILD>',arg)
   arg=re.sub(r'(?<=extra-filename=-)[0-9a-f]{16}$','<ARTIFACT>',arg);arg=re.sub(r'(lib\w+)-[0-9a-f]{16}(\.(?:rlib|rmeta))$',r'\1-<ARTIFACT>\2',arg);result.append(arg)
  return result
 for phase in ('normal','generate','use'):
  left,right=vectors(G,cp,cm,phase),vectors(S,ap,am,phase)
  for name in metadata:
   assert right[name].count('-Ctarget-cpu=x86-64-v3')==1
   assert norm(left[name],cr,name)==norm([x for x in right[name] if x!='-Ctarget-cpu=x86-64-v3'],ar,name)
 for directory,pair,m in ((S,ap,am),(G,cp,cm)):
  assert h(pair['pgo_manifest']['path'])==pair['pgo_manifest']['sha256']
  for c in pair['commands']:assert c['exit']==0 and h(directory+'/'+c['label']+'.log')==c['log_sha256']
  for c in m['commands']:assert c['status']=='passed' and c['returncode']==0 and h(m['run']+'/'+c['log'])==c['log_sha256']
  for n,v in m['profiles_sha256'].items():assert h(m['run']+('/' if n=='merged.profdata' else '/raw-profiles/')+n)==v
 assert am['profiles_sha256']['merged.profdata']!=cm['profiles_sha256']['merged.profdata']
 im=load(ar+'/inputs/manifest.json');oldim=load(E+'/training-inputs.json')
 assert len(im['rows'])==12 and len(oldim['rows'])==12
 expected=[(f['name'],ns,ch,handler,it) for f in im['rows'] for ns in f['namespaces'] for ch in f['chunks'] for handler in ['minimal','full'] for it in range(2)];assert len(expected)==288
 for new,old in zip(im['rows'],oldim['rows'],strict=True):
  for k in ('name','encoding','bytes','sha256','chunks','namespaces'):assert new[k]==old[k]
  assert h(ar+'/inputs/'+new['file'])==h(old['path'])==new['sha256']
 for phase in ('normal','generate','use'):
  a=load(S+'/normal-training.json' if phase=='normal' else ar+'/'+phase+'-training.json')
  c=load(G+'/normal-training.json' if phase=='normal' else cr+'/'+phase+'-training.json')
  assert a['rows']==c['rows']==load(S+'/normal-training.json')['rows']
  assert [(x['fixture'],x['namespaces'],x['chunk'],x['handlers'],x['iteration']) for x in a['rows']]==expected
  assert all(x['status']==1 and x['error']==0 and not x['callback_exceptions'] for x in a['rows'])
  assert h(a['xml_parse_origin']['path'])==a['library_sha256']==a['xml_parse_origin']['sha256']
 er=load(S+'/expat/report.json');assert er['status']=='passed' and len(er['profiles_sha256'])==7
 for n,v in er['profiles_sha256'].items():assert h(S+'/expat/raw-profiles/'+n)==v
 for phase,oldphase in [('normal','control'),('generate','generate'),('use','use')]:
  old=load(E+'/expat-'+oldphase+'-build.json');newbuild=S+'/expat/'+phase+'-build';oldbuild=E+'/expat-'+oldphase+'-build'
  assert h(newbuild+'/expat_config.h')==old['generated_config_sha256']
  vv=[[shlex.split(line) for line in data(p).decode().splitlines() if line.startswith('/usr/bin/cc ')] for p in (E+'/expat-'+oldphase+'-compile.log',S+'/expat/'+phase+'-compile.log')]
  assert len(vv[0])==len(vv[1])==8
  for left,right in zip(*vv,strict=True):
   assert right.count('-march=x86-64-v3')==1
   a=[x.replace(newbuild,'<BUILD>').replace(S+'/expat/raw-profiles','<RAW>') for x in right if x!='-march=x86-64-v3']
   b=[x.replace(oldbuild,'<BUILD>').replace(E+'/expat-raw-profiles','<RAW>') for x in left];assert a==b
  t=load(S+'/expat/'+phase+'-training.json');prior=load(E+'/expat-'+oldphase+'-training.json')
  assert t['rows']==prior['rows']==load(S+'/expat/normal-training.json')['rows']
  assert [(x['fixture'],x['namespaces'],x['chunk'],x['handlers'],x['iteration']) for x in t['rows']]==expected
  assert t['inputs_sha256']==h(ar+'/inputs/manifest.json') and prior['inputs_sha256']==h(E+'/training-inputs.json')
  assert all(x['status']==1 and x['error']==0 and not x['callback_exceptions'] for x in t['rows'])
  assert h(t['xml_parse_origin']['path'])==t['library_sha256']==t['xml_parse_origin']['sha256']
 # Replay the exact original native and post-hoc auditors with in-memory Path
 # I/O. Only hash reads for excluded binaries return recorded identities.
 outputs={N+'/normal.json',N+'/pgo.json',N+'/posthoc.json'}
 class WriteBuffer(io.StringIO):
  def __init__(self,key):super().__init__();self.key=key
  def __exit__(self,*exc):
   if exc[0] is None:writes[self.key]=self.getvalue().encode()
   self.close();return False
 class ArchivePath:
  def __init__(self,path):self.path=os.path.normpath(str(path))
  def __str__(self):return self.path
  def __truediv__(self,other):return ArchivePath(os.path.join(self.path,str(other)))
  @property
  def name(self):return os.path.basename(self.path)
  @property
  def parent(self):return ArchivePath(os.path.dirname(self.path))
  def resolve(self):assert self.path.startswith('/');return self
  def mkdir(self,**kwargs):assert self.path==N
  def read_bytes(self):return data(self.path)
  def read_text(self):return self.read_bytes().decode()
  def glob(self,pattern):
   prefix=self.path+'/'
   return [ArchivePath(k) for k in sorted(entries) if k.startswith(prefix) and '/' not in k[len(prefix):] and fnmatch.fnmatchcase(k[len(prefix):],pattern)]
  def open(self,mode):
   assert mode=='x' and self.path in outputs and self.path not in writes
   return WriteBuffer(self.path)
 def replay(name,arguments,replace_sha):
  original=N+'/'+name;code=data(original).decode();old='from pathlib import Path';assert code.count(old)==1;code=code.replace(old,'Path = archive_path')
  if replace_sha:
   old="def sha(p):\n with Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()";assert code.count(old)==1;code=code.replace(old,'def sha(p):\n return archive_sha(p)')
  argv=sys.argv;sys.argv=[original,*arguments]
  try:
   with contextlib.redirect_stdout(io.StringIO()):exec(compile(code,original,'exec'),{'__file__':original,'__name__':'__main__','archive_path':ArchivePath,'archive_sha':h})
  finally:sys.argv=argv
 for mode in ('normal','pgo'):
  replay('audit.py',['--mode',mode,'--completed-release'],True)
  path=N+'/'+mode+'.json';assert writes[path]==payload[entries[path]['member']],path
 replay('posthoc.py',[],False);assert writes[N+'/posthoc.json']==payload[entries[N+'/posthoc.json']['member']]
 normal,pgo,posthoc=[json.loads(writes[N+'/'+n+'.json']) for n in ('normal','pgo','posthoc')]
 assert sum(x['all_workers'] for x in (normal,pgo))==1792 and sum(x['total_samples'] for x in (normal,pgo))==283136
 for n in ('LICENSE-APACHE','LICENSE-MIT','tests/c/UPSTREAM-NOTICES.txt'):assert W+'/'+n in entries
 assert E+'/expat-source/COPYING' in entries
 return {'status':'passed_portable_bytes_build_records_and_original_native_replay','archive_sha256':archive_hash,'stored_members':len(payload),'origins':len(entries),'content_aliases':len(entries)-len(payload),'omitted_payloads':len(omitted),'source_files':70,'pipeline_source_files':67,'pipeline_helpers':6,'expat_source_files':167,'actual_compiler_vectors_rederived':{'rust':9,'expat':24},'v3_generated_records_recompared':1728,'native_workers':1792,'native_samples':283136,'conditions':56,'original_normal_pgo_posthoc_audits_reconstructed_byte_equal':True,'groups':{'normal':normal['groups'],'pgo':pgo['groups']},'posthoc':{k:v['groups'] for k,v in posthoc['modes'].items()},'recorded_only_omitted_hashes_used':sorted(used_omitted),
 'limitations':['No original absolute filesystem reads occur during replay; absolute paths in reports are logical archive keys.','Excluded compiler/library/archive bytes are not verified by this portable replay. Their hashes are recorded identities, separate from included source/log/profile/sample byte verification.','Build replay rederives raw vectors and compares archived training/profile/source bindings; it does not rebuild binaries or validate newly executed compiler behavior.','Native replay preserves all original planned and post-hoc arithmetic. It adds no parser runs, observations, full compatibility, Python or PBS validation.','The ISA study remains experimental/unselected and uses allocator e1 without ContextText composition.']}
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('package',type=Path,nargs='?',default=Path(__file__).resolve().parent);a=p.parse_args();print(json.dumps(verify(a.package),indent=2))
