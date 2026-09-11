"""Preserve unselected namespace/name experiments without editing their studies."""
from pathlib import Path
import hashlib, io, json, shutil, tarfile
O=Path('/tmp/oriole-name-experiments-handoff');O.mkdir(exist_ok=False)
D={
'b1':'/tmp/oriole-expanded-name-frames-b1-handoff',
'b2':'/tmp/oriole-expanded-name-frames-equal-owner-handoff',
'b2-thinlto':'/tmp/oriole-expanded-name-frames-equal-owner-thinlto-study',
'b2-native':'/tmp/oriole-expanded-name-frames-equal-owner-thinlto-timing-study',
'b3':'/tmp/oriole-expanded-name-frames-outlined-handoff',
'b4':'/tmp/oriole-expanded-name-frames-attribute-inline-handoff',
'b5':'/tmp/oriole-expanded-name-frames-attribute-always-handoff',
'b5-native':'/tmp/oriole-expanded-name-frames-attribute-always-thinlto-timing-study',
'b5-python':'/tmp/oriole-expanded-name-frames-attribute-always-thinlto-python-study',
'b5-chain-review':'/tmp/oriole-expanded-name-frames-b5-chain-independent-review',
'b5-native-review':'/tmp/oriole-expanded-name-frames-b5-native-independent-review',
'b5-python-review':'/tmp/oriole-expanded-name-frames-b5-python-independent-review',
'b2-source-review':'/tmp/oriole-expanded-name-frames-equal-owner-root-review',
'ascii-source':'/tmp/oriole-ascii-name-table-study',
'ascii-build-profile':'/tmp/oriole-ascii-name-table-thinlto-study',
'ascii-native':'/tmp/oriole-ascii-name-table-thinlto-timing-study',
'ascii-python':'/tmp/oriole-ascii-name-table-thinlto-python-study',
'ascii-source-review':'/tmp/oriole-ascii-name-table-independent-review',
'ascii-build-review':'/tmp/oriole-ascii-name-table-build-independent-review',
'ascii-profile-review':'/tmp/oriole-ascii-name-table-profile-independent-review',
'ascii-static-review':'/tmp/oriole-ascii-name-table-static-independent-review',
'ascii-native-review':'/tmp/oriole-ascii-name-table-native-independent-review',
'ascii-python-review':'/tmp/oriole-ascii-name-table-python-independent-review',
}
sha=lambda b:hashlib.sha256(b).hexdigest()
paths={}
for prefix,directory in D.items():
 root=Path(directory);assert root.is_dir(),root
 for p in sorted(root.rglob('*')):
  if p.is_file() and not p.is_symlink() and '__pycache__' not in p.parts:paths[prefix+'/'+str(p.relative_to(root))]=p
paths['controllers/'+Path(__file__).name]=Path(__file__)
for name in ['prepare-expanded-name-frames-attribute-always-thinlto-timing','time-expanded-name-frames-attribute-always-thinlto-native','prepare-expanded-name-frames-attribute-always-thinlto-python','time-expanded-name-frames-attribute-always-thinlto-python','prepare-ascii-name-table-thinlto-timing','time-ascii-name-table-thinlto-native','prepare-ascii-name-table-thinlto-python','time-ascii-name-table-thinlto-python']:
 p=Path('/tmp/oriole-'+name+'.py');assert p.is_file();paths['controllers/'+p.name]=p
members=[];excluded=[]
with tarfile.open(O/'evidence.tar.gz','w:gz',compresslevel=9) as a:
 for name,p in sorted(paths.items()):
  b=p.read_bytes();entry={'path':name,'source':str(p),'bytes':len(b),'sha256':sha(b)}
  if b.startswith((b'\x7fELF',b'!<arch>\n')):excluded.append(entry);continue
  info=tarfile.TarInfo(name);info.size=len(b);info.mode=0o644;info.mtime=0;a.addfile(info,io.BytesIO(b));members.append(entry)
with tarfile.open(O/'evidence.tar.gz','r:gz') as a:
 am=a.getmembers();assert len(am)==len(members)
 for m,e in zip(am,members):
  assert m.isfile() and m.name==e['path'] and m.size==e['bytes'];assert sha(a.extractfile(m).read())==e['sha256']==sha(Path(e['source']).read_bytes())
for e in excluded:assert sha(Path(e['source']).read_bytes())==e['sha256']
(O/'members.json').write_text(json.dumps({'members':members,'excluded_binaries':excluded},indent=2)+'\n')
summary={'status':'unselected_experiments_preserved','scope':'B1-B5 expanded namespace frame lineage and ASCII NameChar table are unselected. The passing raw preflights/reviews do not imply full compatibility gates or adoption. No source or raw result modified by packaging. All measured conditions and regressions retained. B5 uses a55 control; ASCII uses selected Cell0cfd. No composition with End or with one another was built.','decisions':{'b1':'Reject allocation amplification on equal raw/expanded names.','b2':'Equality ownership fixed; real native ratio1.00637964 with20/24 regressions; no Python/full gates.','b3':'Outlined expanded helper fails to restore identity attribute inlining; no profiling/timing.','b4':'Ordinary inline hint generates byte-identical .text to B3; no profiling/timing.','b5':'Forced tiny append inlining restores targeted static criterion. Native .9851897854/a55 with15/24 regressions; Python .9825116994 with12/24 regressions. Namespace-heavy gains but common-path/generated regressions; hold without new full gates.','ascii':'Table reduces real instructions2.58%, but native only0.80% with6/24 regressions, Python0.29% with9/24 regressions. Both native entity controls slower. Hold without full gates.'},'archive':{'sha256':sha((O/'evidence.tar.gz').read_bytes()),'members':len(members),'direct_binary_exclusions':len(excluded),'bytes':(O/'evidence.tar.gz').stat().st_size,'raw_bytes':sum(e['bytes'] for e in members),'all_members_and_origins_read_back':True},'directories':D,'limitations':['Nested handoffs retain their original manifests and scoped reviews; this outer packager rechecks their bytes but does not re-audit every nested archive member.','B5 failed native preparation CPU5-versusCPU3 stopped before parser dispatch and is retained with the corrected successful run.','ASCII first worktree destination existed; refusal and corrected distinct setup preserved.','No new full API, strict802, complete PBS or sustained sanitizer campaign is claimed for these unselected candidates.']}
(O/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');shutil.copyfile(__file__,O/'package.py')
manifest={'files':{p.name:sha(p.read_bytes()) for p in O.iterdir() if p.is_file()},'scope':'No compiled binaries; original source/build/raw results and review identity retained.'};(O/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(summary['archive']),flush=True)
