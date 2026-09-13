from pathlib import Path
import re

def methods(path):
    output={};pending=None;subtests=[]
    def add(item,status):
        name,identifier=item
        if name=='setUpClass':return
        assert name.startswith('test') and identifier not in output
        output[identifier]=status
    for line in read(path).decode().splitlines():
        m=re.match(r'^(\w+) \((test\.[^)]+)\)(?: \.\.\. (.*))?$',line)
        if m:
            if pending is not None:
                assert any(f'{pending[0]} ({pending[1]}) ' in t for t in subtests);add(pending,'subtest outcomes')
            pending=(m[1],m[2]);status=m[3]
            if status:add(pending,status);pending=None
        elif pending is not None:
            if re.match(r'^(ok|FAIL|ERROR|expected failure|skipped .+)$',line):add(pending,line);pending=None
            elif line.startswith('  ') and ' ... ' in line:subtests.append(line)
            elif ' ... ' in line and line.rsplit(' ... ',1)[1]:add(pending,line.rsplit(' ... ',1)[1]);pending=None
    assert pending is None
    return output
