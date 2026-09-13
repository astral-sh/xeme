"""Pure byte-selector equivalence study; does not modify or build the parser.

The reference is a direct transcription of the frozen coalesced selector.
bytes.find models memchr2's first-marker contract; this is not a test of its
Rust call site or code generation. Small caps exercise every relative boundary,
not a performance tuning sweep. Production-cap and valid-UTF8 cases are separate.
"""
from pathlib import Path
import hashlib
import itertools
import json
import random
import tarfile

ROOT = Path(__file__).parent
CAP = 65_536
counts = {}
paths = {'fast_marker': 0, 'fast_short': 0, 'fallback_long': 0, 'fallback_noncoalesced': 0}

def old(data, coalesce, internal, cap=CAP):
    boundary = 0
    chosen = None
    for index, byte in enumerate(data):
        if byte in (60, 38):
            chosen = index
            break
        if byte == 10 or (not internal and byte == 13):
            if not coalesce or index >= cap:
                chosen = boundary if boundary > 0 else index
                break
            boundary = index + (2 if byte == 13 and data[index+1:index+2] == b'\n' else 1)
        if index >= cap and boundary > 0:
            chosen = boundary
            break
    if chosen is None:
        return len(data)
    if chosen == 0 and data[0] in (13, 10):
        return 2 if data.startswith(b'\r\n') else 1
    return chosen

def proposed(data, coalesce, internal, cap=CAP):
    if coalesce:
        stop = min(len(data), cap + 1)
        first = data.find(b'<', 0, stop)
        second = data.find(b'&', 0, stop)
        marker = second if first < 0 else first if second < 0 else min(first, second)
        if marker >= 0:
            paths['fast_marker'] += 1
            return marker
        if len(data) <= cap:
            paths['fast_short'] += 1
            return len(data)
        paths['fallback_long'] += 1
    else:
        paths['fallback_noncoalesced'] += 1
    return old(data, coalesce, internal, cap)

def check(data, coalesce, internal, cap, family):
    expected = old(data, coalesce, internal, cap)
    actual = proposed(data, coalesce, internal, cap)
    assert expected == actual, (data.hex(), coalesce, internal, cap, expected, actual)
    counts[family] = counts.get(family, 0) + 1

# Complete short byte alphabet: both markers/newline kinds, ordinary text,
# forbidden-token punctuation, NUL, and non-ASCII byte. Covers CRLF and all orders.
alphabet = [b'<', b'&', b'\r', b'\n', b'x', b']', b'\0', b'\x80']
for size in range(7):
    for parts in itertools.product(alphabet, repeat=size):
        data = b''.join(parts)
        for internal in (False, True):
            check(data, False, internal, CAP, 'exhaustive_noncoalesced')
            for cap in range(8):
                check(data, True, internal, cap, 'exhaustive_coalesced')

# All ordered marker/newline/control pairs near the real 64KiB cap. Long
# prefixes with and without a prior complete line exercise fallback selection.
points = [0, 1, CAP-2, CAP-1, CAP, CAP+1, CAP+2]
symbols = [b'<', b'&', b'\r', b'\n', b']', b'\0', b'\xc3']
for length in [CAP-1, CAP, CAP+1, CAP+2, CAP+3, 2*CAP+3]:
    for prefix in (b'x', b'\n', b'\r'):
        for first, second in itertools.product(points, repeat=2):
            if first >= length or second >= length:
                continue
            for a, b in itertools.product(symbols, repeat=2):
                data = bytearray(b'x' * length)
                data[0] = prefix[0]
                data[first] = a[0]
                data[second] = b[0]
                data = bytes(data)
                for internal in (False, True):
                    check(data, True, internal, CAP, 'production_cap_pairs')

# UTF8 character-boundary suffixes/chunk cuts, including controls and ]]>.
texts = ['α\r\nβ<&γ', 'a\n]]>z', 'x\r\ny\r', '\U00010000\n\U0010ffff<&', 'a\x00\n&b']
for text in texts:
    boundaries = [len(text[:i].encode()) for i in range(len(text)+1)]
    data = text.encode()
    for start, end in itertools.combinations_with_replacement(boundaries, 2):
        for coalesce, internal in itertools.product((False, True), repeat=2):
            check(data[start:end], coalesce, internal, CAP, 'utf8_all_subspans')
for head in ['α', '\U00010000', 'x\r\n', 'x\n']:
    body = (head * (CAP // len(head.encode()) + 2)).encode()
    for tail in [b'', b'<', b'&', b'\r\n', b']]>', b'\0', b'\xc3\xa9']:
        for internal in (False, True):
            check(body + tail, True, internal, CAP, 'utf8_cap_crossings')

# Seeded long spans supplement exhaustive boundary cases, with absent and late
# markup and both internal CR policies. These are correctness cases, not timings.
rng = random.Random(20260910)
for _ in range(1000):
    length = rng.randrange(CAP-8, 3*CAP)
    data = bytearray(b'x' * length)
    for _ in range(rng.randrange(16)):
        data[rng.randrange(length)] = rng.choice([60, 38, 13, 10, 93, 0, 128])
    for internal in (False, True):
        check(bytes(data), True, internal, CAP, 'seeded_long')

report = {'status': 'passed', 'cases': sum(counts.values()), 'families': counts,
          'candidate_paths': paths, 'production_cap': CAP,
          'scope': 'Pure selector equivalence; reference is a source transcription and first-marker search models memchr2. No Rust parser mutation, compilation, execution, or performance claim.',
          'limitations': 'Later converted-window, trailing CR/]], XML validation, forbidden-token, normalization, publication, and consume paths are unchanged by design; this model does not independently execute those parser paths.'}
(ROOT/'oracle.json').write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps(report))
