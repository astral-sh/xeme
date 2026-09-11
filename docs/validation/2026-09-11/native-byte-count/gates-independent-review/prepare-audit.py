from pathlib import Path
import difflib,hashlib,json
O=Path(__file__).resolve().parent
base=Path('/tmp/oriole-end-cell-gates-independent-review/audit.py');old=base.read_text()
head=old[:old.index('\n\nS=L')]
head=head.replace("R = Path('/tmp/oriole-end-frame-cells-thinlto-gates')","R = Path('/tmp/oriole-native-byte-count-gates')").replace("W = Path('/home/dev-user/code/oss/oriole-end-frame-cells')","W = Path('/home/dev-user/code/oss/oriole-native-byte-count')").replace("L = Path('/tmp/oriole-end-frame-cells-thinlto-study')","L = Path('/tmp/oriole-native-byte-count-thinlto-study')").replace("B = Path('/tmp/oriole-ffi-family-cell-thinlto-study')","B = Path('/tmp/oriole-end-frame-cells-thinlto-study')").replace('== [6]','== [4]').replace('cpu=4','cpu=1')
start=head.index('    # Source paths');end=head.index('    data = p.read_bytes()',start);head=head[:start]+head[end:]
core=old[old.index("A=R/'api-native'"):old.index("ck('saved files unchanged across audit'")].replace('allow_api=True,cpu=4','allow_api=True,cpu=1')
suppbase=Path('/tmp/oriole-end-cell-gates-independent-review/handoff-audit.py');supp=suppbase.read_text();endcore=supp[supp.index("LC=R/'end-lifecycle'"):supp.index('historical_helper=')]
endcore=endcore.replace("LC=R/'end-lifecycle'","LC=R/'end-lifecycle-final'").replace("data['affinity']==[4]","data['affinity']==[1]").replace("['taskset','-c','4']","['taskset','-c','1']")
new=head+(O/'audit-setup.py').read_text()+core+"\nOLD=prior_root\ngate={'source':{'direct_control_shared_sha256':normal_libraries['liboriole_expat.so']},'libraries':expected_libraries}\n"+endcore+(O/'audit-tail.py').read_text()
(O/'generator.py').write_text(new)
(O/'adaptation.patch').write_text(''.join(difflib.unified_diff(old.splitlines(True),new.splitlines(True),fromfile=str(base),tofile='generator.py')))
(O/'audit-preparation.json').write_text(json.dumps({'prior_core':str(base),'prior_core_sha256':hashlib.sha256(base.read_bytes()).hexdigest(),'prior_end_supplement':str(suppbase),'prior_end_supplement_sha256':hashlib.sha256(suppbase.read_bytes()).hexdigest(),'generator_sha256':hashlib.sha256(new.encode()).hexdigest(),'changes':'Reuse complete saved API/native/strict/alias/malformed/publication and End raw audits. Set exact integrated source/libraries, CPU1 commands, preserved CPU4-preparation/failed-End/final-End lineage; skip prior archive handoff and source overlay redirection. No target execution.'},indent=2)+'\n')
