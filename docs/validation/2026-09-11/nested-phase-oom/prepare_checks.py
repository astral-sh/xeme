from pathlib import Path
import hashlib
import json
import re

HERE = Path(__file__).resolve().parent
W = Path('/home/dev-user/code/oss/oriole-nested-phase-oom')
source = (W / 'tests/c/integration.c').read_bytes()
assert hashlib.sha256(source).hexdigest() == '88eae7992ed72f9bb039bbfb9a94b126b2dc0959bd2c7fb4234e55622b3264ca'
symbols = sorted(set(re.findall(rb'\b(XML_\w+)\(', source)) - {b'XML_GetUserData'})
assert len(symbols) == 34
wrapper = '''#define _GNU_SOURCE
#include <dlfcn.h>
#include <limits.h>
#define main integration_main
#include "integration.c"
#undef main

static void check_origin(void *symbol, const char *expected) {
    Dl_info info;
    char actual[PATH_MAX];
    assert(dladdr(symbol, &info) && info.dli_fname);
    assert(realpath(info.dli_fname, actual));
    assert(strcmp(actual, expected) == 0);
}

int main(int argc, char **argv) {
    char expected[PATH_MAX];
    assert(argc == 2 && realpath(argv[1], expected));
'''
wrapper += ''.join('    check_origin((void *)' + name.decode() + ', expected);\n' for name in symbols)
wrapper += '''    printf("Verified 34 API symbol origins: %s\\n", expected);
    fflush(stdout);
    int result = integration_main();
    printf("Phase-relative OOM passed: successful_declarations=2 child_error=%d parent_error=%d rejected_allocations=%zu live_allocations=%zu\\n",
           XML_ERROR_NO_MEMORY, XML_ERROR_EXTERNAL_ENTITY_HANDLING,
           nested_oom_failures, allocations - frees);
    return result;
}
'''
(HERE / 'consumer.c').write_text(wrapper)
names = ['tests/c/integration.c', 'tests/c/UPSTREAM-NOTICES.txt', 'include/expat.h', 'CONTRIBUTING.md']
pins = {str(W / name): hashlib.sha256((W / name).read_bytes()).hexdigest() for name in names}
pins[str(HERE / 'consumer.c')] = hashlib.sha256((HERE / 'consumer.c').read_bytes()).hexdigest()
pair = json.loads(Path('/tmp/oriole-context-text-frame-fixed-pgo-study/pair-report.json').read_text())
assert pair['source_manifest_sha256'] == '8a7da2759b67ca85f82fb29bd775392e532b3ff355340eb10f92ea7764bbe642'
normal = Path('/tmp/oriole-context-text-frame-fixed-pgo-study/normal')
pgo = Path('/tmp/oriole-context-text-frame-fixed-pgo-study/pgo/runs/run-n5eot_2d/use')
arms = [{'name': 'expat-pgo-shared', 'path': '/tmp/oriole-pgo-study/expat-use-liboriole_expat.so', 'sha256': '12d33ad26315e8b46a02e598df8581679d0561fb2d98a3d4744e483a573fbdd0', 'linkage': 'shared', 'soname': 'libexpat.so.1'}]
for mode, directory in [('normal', normal), ('pgo', pgo)]:
    for linkage, ext in [('shared', 'so'), ('static', 'a')]:
        name = 'liboriole_expat.' + ext
        expected = pair['normal_libraries'][name] if mode == 'normal' else pair['pgo_libraries']['use/' + name]
        arms.append({'name': 'context-' + mode + '-' + linkage, 'path': str(directory / name), 'sha256': expected, 'linkage': linkage, 'soname': 'liboriole_expat.so' if linkage == 'shared' else None})
receipt = {'status': 'prepared_not_executed', 'worktree': str(W), 'source_pins': pins, 'arms': arms, 'source_symbols': [s.decode() for s in symbols], 'scope': 'One new phase-relative OOM case in the existing complete direct-link integration consumer. C ASan/UBSan; frozen Rust and Expat artifacts uninstrumented, LSan disabled; explicit custom-allocation balance remains asserted.', 'cpu': 3, 'compile_wall_seconds': 120, 'worker_wall_seconds': 120, 'control_source': pair['source_manifest_sha256']}
(HERE / 'checks-preparation.json').write_text(json.dumps(receipt, indent=2) + '\n')
print('Prepared 5 C consumers with 34 runtime API-origin assertions each; no compiler/parser run.')
