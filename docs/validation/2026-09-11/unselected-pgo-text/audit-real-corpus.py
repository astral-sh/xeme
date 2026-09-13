"""Audit pinned saved corpus bytes and API evidence; never run an XML parser."""
from pathlib import Path
import base64
from collections import Counter
import hashlib
import json
import os
import re

assert os.sched_getaffinity(0) == {6}
R = Path('/tmp/oriole-real-pgo-training-inputs')
O = Path('/tmp/oriole-real-pgo-training-corpus-independent-review')
O.mkdir(exist_ok=False)
pins = {}


def bytes_at(path):
    path = Path(path)
    data = path.read_bytes()
    value = hashlib.sha256(data).hexdigest()
    assert str(path) not in pins or pins[str(path)] == value
    pins[str(path)] = value
    return data


def read(path):
    return json.loads(bytes_at(path))


def git_blob(data):
    return hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()


report = {'status':'incomplete','scope':'Independent saved bytes, JSON provenance and lexical source review only; no XML parser, build, benchmark or network execution.'}
try:
    manifest = read(R/'manifest.json')
    assert pins[str(R/'manifest.json')] == 'c4bef3dbcb2ccea5a65cb940ed51e1fb3b2ec46e5019516821eb0bab06cb83a8'
    selection = read(R/'selection.json')
    assert selection['manifest_sha256'] == pins[str(R/'manifest.json')]
    plan = Path(manifest['criteria']['predeclared_plan'])
    bytes_at(plan)
    assert pins[str(plan)] == manifest['criteria']['predeclared_plan_sha256']
    assert manifest['schema_version'] == 1 and manifest['selected_runtime'] == '78c748d9de5c497858458cbf7a1229148781b2bf'
    assert manifest['schedule'] == {'chunks':[4096,65536],'namespaces':[False,True],'handlers':['minimal','full'],'iterations':1}
    rows = manifest['documents']
    assert len(rows)==6 and len({row['id'] for row in rows})==6
    assert Counter(row['category'] for row in rows)=={'config':2,'mixed_content':2,'namespaces':2}
    assert sorted(Counter(row['repository'] for row in rows).values())==[2,2,2]
    assert {p.relative_to(R).as_posix() for p in (R/'inputs').iterdir()}=={row['file'] for row in rows}
    acquisitions = {}
    for path in sorted((R/'provenance').glob('*acquired.json')):
        values=read(path)
        for value in values if isinstance(values,list) else [values]:
            acquisitions[value['file']]=value
    for row in rows:
        acquisitions[row['file']]=row
    selected_files = {row['file']:row['sha256'] for row in rows}
    for row in rows:
        for notice in row['license_files']:
            assert notice['file'] not in selected_files or selected_files[notice['file']]==notice['sha256']
            selected_files[notice['file']]=notice['sha256']
    assert len(selected_files)==15  # Six input files and nine distinct notices.
    origin_rows=[]
    for filename, expected in selected_files.items():
        path=R/filename
        assert path.is_file() and not path.is_symlink() and path.resolve().is_relative_to(R)
        data=bytes_at(path)
        assert hashlib.sha256(data).hexdigest()==expected
        acquired=acquisitions[filename]
        assert acquired['sha256']==expected and acquired['bytes']==len(data) and git_blob(data)==acquired['git_blob_sha1']
        repository,commit,upstream=acquired['repository'],acquired['commit'],acquired['upstream_path']
        marker='/-/blob/' if repository.startswith('https://gitlab.com/') else '/blob/'
        assert acquired['canonical_url']==repository+marker+commit+'/'+upstream
        if 'blob_response' in acquired:
            response_path=R/acquired['blob_response']
        else:
            assert repository=='https://github.com/spdx/license-list-data'
            response_path=R/'provenance'/('spdx-'+Path(filename).stem+'-content.json')
        response=read(response_path)
        assert response['encoding']=='base64' and response['sha']==git_blob(data)
        assert response['size']==len(data) and base64.b64decode(response['content'])==data
        if repository.startswith('https://gitlab.com/'):
            request=read(response_path.with_name(response_path.stem+'-request.json'))
            assert request['status']==200 and request['method']=='GET' and request['sha256']==pins[str(response_path)]
            assert request['url']=='https://gitlab.com/api/v4/projects/3472737/repository/blobs/'+git_blob(data)
            tree_name='inkscape-gitlab-examples-tree' if upstream.startswith('share/examples/') else 'inkscape-gitlab-root-tree'
            tree_path=R/'provenance'/(tree_name+'.json')
            tree=read(tree_path);tree_request=read(R/'provenance'/(tree_name+'-request.json'))
            assert tree_request['status']==200 and tree_request['sha256']==pins[str(tree_path)] and 'ref='+commit in tree_request['url']
            assert any(item['path']==upstream and item['id']==git_blob(data) and item['type']=='blob' for item in tree)
        else:
            request=read(response_path.with_name(response_path.stem+'-command.json'))
            assert request['exit_code']==0 and request['stdout_sha256']==pins[str(response_path)] and request['argv'][:2]==['gh-auto','api']
            owner_repo=repository.removeprefix('https://github.com/')
            if owner_repo=='spdx/license-list-data':
                assert request['argv'][2]=='repos/'+owner_repo+'/contents/'+upstream+'?ref='+commit
                committed=read(R/'provenance/spdx-license-list-data-commit.json')
                assert committed['sha']==commit
            else:
                assert request['argv'][2]=='repos/'+owner_repo+'/git/blobs/'+git_blob(data)
                prefix=owner_repo.replace('/','--')
                tree_path=R/'provenance'/(prefix+'-tree.json')
                tree=read(tree_path);request_tree=read(R/'provenance'/(prefix+'-tree-command.json'))
                assert request_tree['exit_code']==0 and request_tree['stdout_sha256']==pins[str(tree_path)]
                assert request_tree['argv']==['gh-auto','api','repos/'+owner_repo+'/git/trees/'+commit+'?recursive=1']
                assert tree['truncated'] is False
                assert any(item['path']==upstream and item['sha']==git_blob(data) and item['type']=='blob' for item in tree['tree'])
                committed=read(R/'provenance'/(prefix+'-commit.json'))
                repository_data=read(R/'provenance'/(prefix+'-repository.json'))
                assert committed['sha']==commit and repository_data['html_url']==repository and repository_data['fork'] is False
        origin_rows.append({'file':filename,'repository':repository,'commit':commit,'git_blob_sha1':git_blob(data),'bytes':len(data),'sha256':expected,'decoded_api_body_and_tree_membership':True})
    project=read(R/'provenance/inkscape-gitlab-project.json')
    committed=read(R/'provenance/inkscape-gitlab-commit.json')
    assert project['id']==3472737 and project['web_url']=='https://gitlab.com/inkscape/inkscape'
    assert committed['id']=='884658ec37ad1d90f9c2d21eca0b033ef1308150'
    lexical=[];documents={}
    for row in rows:
        data=bytes_at(R/row['file']);text=data.decode('utf-8',errors='strict');documents[row['id']]=text
        assert 0<len(data)<=1024*1024 and row['encoding']=='UTF-8'
        assert not re.search(r'<!\s*(?:DOCTYPE|ENTITY)\b',text,re.I)
        named=set(re.findall(r'&([A-Za-z_:][A-Za-z0-9_.:-]*);',text))
        assert named<={'amp','lt','gt','apos','quot'}
        declaration=re.match(r'\ufeff?<\?xml\s+([^?]*)\?>',text)
        if declaration:
            encoding=re.search(r'''encoding\s*=\s*(['"])([^'"]+)\1''',declaration[1])
            assert not encoding or encoding[2].lower()=='utf-8'
        tags=dict(Counter(re.findall(r'<([A-Za-z_][A-Za-z0-9_.:-]*)(?=[\s/>])',text)))
        assert tags==row['structure']['lexical_start_tag_counts']
        namespace_matches=re.findall(r'''\bxmlns(?::([A-Za-z_][\w.-]*))?\s*=\s*(['"])(.*?)\2''',text)
        namespaces=[{'prefix':prefix,'uri':uri} for prefix,quote,uri in namespace_matches]
        assert namespaces==row['structure']['namespace_declarations']
        links=sorted(set(value for quote,value in re.findall(r'''\b(?:xlink:)?href\s*=\s*(['"])(.*?)\1''',text)))
        assert links==row['structure']['link_attribute_values']
        if row['category']=='namespaces':
            ids={value for quote,value in re.findall(r'''\bid\s*=\s*(['"])(.*?)\1''',text)}
            assert all(link.startswith('#') and link[1:] in ids for link in links)
            assert len(namespaces)==8 and 'Niko Kiirala' in text and 'http://creativecommons.org/licenses/by-sa/3.0/' in text
        lexical.append({'id':row['id'],'bytes':len(data),'namespace_declarations':len(namespaces),'lexical_start_tags':sum(tags.values()),'named_references':sorted(named),'no_doctype_or_entity_declaration':True,'link_values':links})
    total=sum(row['bytes'] for row in rows)
    assert total==503245==selection['selected_bytes'] and total<=4*1024*1024 and total*8==4025960==selection['additional_input_bytes']
    heldout=Path('/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json')
    heldout_manifest=read(heldout)
    assert pins[str(heldout)]==selection['heldout_manifest_sha256']
    controls=[]
    for project in heldout_manifest['projects']:
        for file in project['files']:
            if file['role'] in ('input','related-small-input'):
                controls.append((project['name'],heldout.parent/file['path'],file['sha256']))
    generated=Path('/tmp/oriole-cdata-finder-pgo-study/pgo/runs/run-sjsgpms8/inputs/manifest.json')
    generated_manifest=read(generated)
    assert pins[str(generated)]==selection['generated_manifest_sha256']
    controls += [('generated/'+row['name'],generated.parent/row['file'],row['sha256']) for row in generated_manifest['rows']]
    assert len(controls)==19
    overlaps=[]
    selected_hashes={row['sha256'] for row in rows}
    for name,path,expected in controls:
        data=bytes_at(path)
        assert pins[str(path)]==expected and expected not in selected_hashes
        if name.startswith('generated/'):
            continue
        control_lines={' '.join(line.split()) for line in data.decode('utf-8').splitlines() if len(' '.join(line.split()))>=40}
        for identifier,text in documents.items():
            selected_lines={' '.join(line.split()) for line in text.splitlines() if len(' '.join(line.split()))>=40}
            shared=selected_lines & control_lines
            overlaps.append({'selected':identifier,'heldout':name,'shared_normalized_lines_at_least_40_chars':len(shared),'shared_characters':sum(map(len,shared)),'selected_unique_line_characters':sum(map(len,selected_lines)),'longest_shared_line':max(shared,key=len,default='')})
    histories={}
    for name in ('blend_modes.svg','turbulence_filters.svg'):
        path=R/'provenance'/('inkscape-'+name+'-history.json')
        history=read(path)
        matches=[row for row in history if row['author_name']=='Niko Kiirala']
        assert matches
        histories[name]=[{key:row[key] for key in ('id','author_name','authored_date','title')} for row in matches]
    for name in ('README.md','NOTICE.md','provenance/standardebooks-colophon.xhtml','provenance/standardebooks-content.opf','provenance/qbittorrent-README.md','provenance/inkscape-README.md','provenance/prospective-artwork-relocations.json'):
        bytes_at(R/name)
    report.update(status='passed_saved_corpus_provenance_review',manifest_sha256=pins[str(R/'manifest.json')],documents=6,projects=3,categories={'config':2,'mixed_content':2,'namespaces':2},unique_input_bytes=total,additional_parses=48,additional_input_bytes=total*8,unique_notices=9,origin_checks=origin_rows,lexical_checks=lexical,byte_hash_disjoint_comparisons=6*19,line_overlap_checks=overlaps,inkscape_authorship_history=histories,
        lineage_review='Canonical qBittorrent Qt dialog sources, Standard Ebooks transcribed Austen XHTML and attributed Inkscape Kiirala SVG examples are distinct projects and source lineages from the six held-out projects. Shared XML formats/namespaces or an application dependency do not establish shared fixture ancestry. No selected source was found derived from a held-out fixture in this bounded provenance/source check.',
        scope_limits=['No XML well-formedness or parser success established; released normal preflight remains required.','Hash disjointness plus normalized-line overlap is not a universal duplicate/derivative detector; primary project/source attribution was also read. No Internet-wide lineage proof is claimed.','The Standard Ebooks jurisdiction caveat is retained; SPDX license texts are mirrors, not grants. Actual SVG rights and author metadata remain in the original bytes.','The two rejected artwork bytes, failed fetch/import/TLS attempts and alternate prospects remain provenance-only. No source was selected or replaced using benchmark feedback.'])
except BaseException as error:
    report['failure']=repr(error)
    raise
finally:
    report['input_sha256']=pins
    report['reader_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    (O/'review.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'path':str(O/'review.json'),'sha256':hashlib.sha256((O/'review.json').read_bytes()).hexdigest(),'status':report['status'],'origins':len(report.get('origin_checks',[])),'max_shared_lines':max((r['shared_normalized_lines_at_least_40_chars'] for r in report.get('line_overlap_checks',[])),default=0)}))
