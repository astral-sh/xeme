import collections
import hashlib
import json
from pathlib import Path
import re

OUT = Path('/tmp/oriole-final-allocation-assessment')
OUT.mkdir(exist_ok=True)
API = Path('/tmp/oriole-frame-integrated-gates/upstream-api')
SOURCE = Path('/tmp/oriole-frame-integrated-final/source.json')
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
write = lambda p,d: p.write_text(json.dumps(d, indent=2, sort_keys=True) + '\n')
source = json.loads(SOURCE.read_text())
tree = Path(source['worktree'])
assert len(source['source_sha256']) == 69
for p,h in source['source_sha256'].items():
    assert sha(tree / p) == h
results = json.loads((API / 'results.json').read_text())
assert results['selection_complete'] and results['library_origin_verified']
assert len(results['results']) == 4740
assert sum(r['outcome']=='pass' for r in results['results']) == 4347
blocks = {}
for b in (API / 'tests.log').read_text().split('ORIOLE_BEGIN\t')[1:]:
    lines = b.splitlines()
    context,test = lines[0].split('\t')
    blocks[context,test] = [x for x in lines[1:] if x.startswith(('ERROR:', 'ASSERTION:'))]

def category(test, messages):
    if test == 'test_nsalloc_parse_buffer': return 'expects_allocation_during_empty_preinit_parsebuffer'
    if test == 'test_alloc_nested_entities': return 'fixed_injection_budget_fails_during_child_creation'
    if test == 'test_misc_version': return 'distinctive_implementation_version_string'
    if test == 'test_misc_input_2gb': return 'absolute_input_cap'
    if test == 'test_buffer_can_grow_to_max': return 'absolute_buffer_cap'
    if test == 'test_bypass_heuristic_when_close_to_bufsize': return 'buffer_growth_cost'
    if any('despite failing' in m or 'no reallocation' in m for m in messages):
        return 'expects_at_least_one_reallocation'
    return 'allocation_retry_ceiling'

rows = []
for row in results['results']:
    if row['outcome'] == 'pass': continue
    messages = blocks[row['context'], row['test']]
    cat = category(row['test'], messages)
    if cat == 'allocation_retry_ceiling':
        assert any('max' in m or 'full allocation count' in m or 'allocation count 10' in m for m in messages)
    rows.append(dict(row, category=cat, observed=messages))
assert len(rows) == 393
groups = {}
for r in rows:
    g=groups.setdefault(r['category'], {'failed_configurations': 0, 'tests': collections.Counter()})
    g['failed_configurations'] += 1
    g['tests'][r['test']] += 1
bodies = {}
for test in sorted({r['test'] for r in rows}):
    for p in sorted((API/'adapted').glob('*_tests.c')):
        text=p.read_text(); begin=text.find('START_TEST('+test+')')
        if begin < 0: continue
        body=text[begin:text.index('END_TEST', begin)+len('END_TEST')]
        bodies[test]={'file': str(p), 'file_sha256': sha(p), 'body': body,
                      'threshold_declarations': re.findall(r'[^\n]*(?:const int max_|const unsigned max_|g_allocation_count =|g_reallocation_count =)[^\n]*', body)}
        break
    assert test in bodies
write(OUT/'classification.json', {'scope':'Read-only reconstruction of actual latest failed rows; no assertion waiver or pass-count adjustment.', 'api_sha256':sha(API/'results.json'), 'source_manifest_sha256':sha(SOURCE), 'total':4740,'passes':4347,'failures':393,'failed_test_names':len(bodies),'groups':groups,'rows':rows})
write(OUT/'original-test-bodies.json', bodies)

corpus={}
base=Path('/home/dev-user/code/oss/oriole/benchmarks/projects/corpus')
for p in [base/'vulkan/vk.xml',base/'wayland/wayland.xml',base/'maven/pom.xml',base/'batik/batikLogo.svg',base/'gtk/gtkfilechooserwidget.ui',base/'docbook/docbook.xsl']:
    text=p.read_text()
    corpus[str(p)]={'sha256':sha(p),'bytes':p.stat().st_size,'literal_doctype_openers':text.count('<!DOCTYPE'),'literal_attlist_openers':text.count('<!ATTLIST'),'literal_entity_declaration_openers':text.count('<!ENTITY'),'named_reference_spellings':dict(collections.Counter(re.findall(r'&([A-Za-z_:][\w.:-]*);',text)))}
write(OUT/'corpus-coverage.json',{'scope':'Static source inventory only; not a parser run or an external-DTD-loaded claim. Native driver has no external loader.', 'files':corpus})
old=json.loads(Path('/tmp/oriole-dtd-allocation-cost/profiles.json').read_text())
profiles=[]
for r in old['observations']:
    if r['engine']=='candidate' and r['chunk']==0 and r['deferral']==0:
        profiles.append({k:r[k] for k in ['engine','test','chunk','deferral','malloc','realloc','peak_live','retained_live','after_free_live','raw_file','parse_allocations']})
write(OUT/'historical-profile-attribution.json',{'scope':'Historical ac6a/4b malloc traces, not measurements of final9277. Current source still has identified clone sites; exact new allocation counts remain unmeasured.','libraries':old['libraries'],'profiles_manifest_sha256':sha('/tmp/oriole-dtd-allocation-cost/profiles.json'),'observations':profiles})
write(OUT/'identity.json',{'scope':'No parser/build/profile/wall execution, no runtime edits.','source_manifest':str(SOURCE),'source_manifest_sha256':sha(SOURCE),'verified_source_files':source['source_sha256'],'api_results_sha256':sha(API/'results.json'),'api_manifest_sha256':sha(API/'manifest.json'),'api_log_sha256':sha(API/'tests.log'),'library_sha256':sha(API/'liboriole_expat.so'),'generator_sha256':sha(__file__)})
print(json.dumps({k:v['failed_configurations'] for k,v in groups.items()}))
