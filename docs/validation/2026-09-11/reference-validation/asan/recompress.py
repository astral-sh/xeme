"""Losslessly re-encode the original ASan evidence tar as XZ.

The original package.py and all tar members stay unchanged. This converter updates
only publication metadata. Use --reuse-candidate for the already verified trial.
"""
from pathlib import Path
import argparse,copy,gzip,hashlib,json,lzma,os,shutil
assert os.sched_getaffinity(0)=={6}
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source',type=Path,default=Path('/tmp/oriole-reference-frame-asan-package'))
parser.add_argument('--output',type=Path,default=Path('/tmp/oriole-reference-frame-asan-publication'))
parser.add_argument('--reuse-candidate',type=Path)
args=parser.parse_args()
SOURCE,OUTPUT=args.source,args.output
EXPECTED_GZIP='a9c5b323e1099b60dd47ba1837b719a3301e7269ae5e88913be45ca026d92efc'
EXPECTED_XZ='73579260ee1df28286ea2561deb10a3cc0edfd17a34d02e16207b28c6c4d02b7'
EXPECTED_TAR='da9f0f2528ccf5ba6282761240f3abccd661644dc0fc5e7695ca5bf18894ac18'
EXPECTED_TAR_BYTES=272291840
def sha(path):
    with Path(path).open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()
def encoded(value):return (json.dumps(value,sort_keys=True,separators=(',',':'))+'\n').encode()
def read(path):return json.loads(Path(path).read_bytes())
def stream_pin(stream):
    h=hashlib.sha256();size=0
    while data:=stream.read(1024*1024):h.update(data);size+=len(data)
    return h.hexdigest(),size
assert not OUTPUT.exists(),'Preserve previous publication attempts.'
original_files={p.name:{'sha256':sha(p),'bytes':p.stat().st_size} for p in SOURCE.iterdir() if p.is_file()}
original=read(SOURCE/'asan.json');old=original['package']
assert old['sha256']==EXPECTED_GZIP and sha(SOURCE/old['archive'])==EXPECTED_GZIP
assert old['members']==72087
assert sha(SOURCE/old['index'])==old['index_sha256']
index=read_index=json.loads(gzip.decompress((SOURCE/old['index']).read_bytes()))
assert index['archive']==old['archive'] and index['sha256']==EXPECTED_GZIP
assert len(index['members'])==72087
with gzip.open(SOURCE/old['archive'],'rb') as stream:assert stream_pin(stream)==(EXPECTED_TAR,EXPECTED_TAR_BYTES)
OUTPUT.mkdir()
archive_path=OUTPUT/'asan-evidence.tar.xz'
if args.reuse_candidate:
    assert sha(args.reuse_candidate)==EXPECTED_XZ
    shutil.copyfile(args.reuse_candidate,archive_path)
else:
    compressor=lzma.LZMACompressor(format=lzma.FORMAT_XZ,check=lzma.CHECK_CRC64,preset=9)
    with gzip.open(SOURCE/old['archive'],'rb') as source,archive_path.open('xb') as output:
        while data:=source.read(1024*1024):output.write(compressor.compress(data))
        output.write(compressor.flush())
assert sha(archive_path)==EXPECTED_XZ
with lzma.open(archive_path,'rb') as stream:assert stream_pin(stream)==(EXPECTED_TAR,EXPECTED_TAR_BYTES)
new_index=copy.deepcopy(index)
new_index['archive']=archive_path.name;new_index['sha256']=EXPECTED_XZ
index_path=OUTPUT/old['index']
with index_path.open('xb') as raw:
    with gzip.GzipFile(fileobj=raw,filename='',mode='wb',mtime=0,compresslevel=9) as zipped:zipped.write(encoded(new_index))
assert json.loads(gzip.decompress(index_path.read_bytes()))['members']==index['members']
compact=copy.deepcopy(original)
meta=compact['package']
meta.update({'archive':archive_path.name,'sha256':EXPECTED_XZ,'bytes':archive_path.stat().st_size,'index_sha256':sha(index_path)})
meta['recompression']={
 'format':'XZ','preset':9,'dictionary_bytes':67108864,'check':'CRC64','threads':1,
 'original_gzip_archive':old['archive'],'original_gzip_sha256':EXPECTED_GZIP,'original_gzip_bytes':old['bytes'],
 'original_index_sha256':old['index_sha256'],'original_packager_sha256':old['packager_sha256'],
 'uncompressed_tar_sha256':EXPECTED_TAR,'uncompressed_tar_bytes':EXPECTED_TAR_BYTES,
 'converter':'recompress.py','converter_sha256':sha(__file__),'receipt':'recompression.json',
 'scope':'Only the outer compression and publication metadata changed. The original package.py produced the preserved gzip tar; recompress.py produced this XZ representation. All original tar headers, members, order and padding are identical.'}
(OUTPUT/'asan.json').write_text(json.dumps(compact,indent=2)+'\n')
text=(SOURCE/'ASAN.md').read_text()
assert text.count(EXPECTED_GZIP)==1
text=text.replace('(asan-evidence.tar.gz)','(asan-evidence.tar.xz)').replace(EXPECTED_GZIP,EXPECTED_XZ)
text+='''
## Lossless publication compression

The published archive uses XZ to reduce the original 54,168,255-byte gzip archive to 12,797,748 bytes (12.20 MiB). This changes only the outer compression: all 272,291,840 uncompressed tar bytes, including every member, header and padding byte, are identical. Uncompressed SHA-256: `da9f0f2528ccf5ba6282761240f3abccd661644dc0fc5e7695ca5bf18894ac18`.

The retained [original packager](package.py) produced `asan-evidence.tar.gz`, SHA-256 `a9c5b323e1099b60dd47ba1837b719a3301e7269ae5e88913be45ca026d92efc`. The separate [converter](recompress.py) produced this XZ representation using single-threaded LZMA preset 9, a 64 MiB dictionary and CRC64. The [conversion receipt](recompression.json) preserves the original packet hashes and byte-for-byte tar binding. The outer member index points to the XZ archive; its 72,087 member rows are unchanged. References to gzip inside the preserved raw records describe that original package.

The original gzip package and its completed review receipts remain unchanged. The XZ archive was decompressed and all original members, source/corpus hashes and physical origins were checked with the adapted saved-data reader before publication. No compiler, parser, fuzzer or benchmark target ran during compression or readback.
'''
(OUTPUT/'ASAN.md').write_text(text)
shutil.copyfile(SOURCE/'package.py',OUTPUT/'package.py')
shutil.copyfile(Path(__file__),OUTPUT/'recompress.py')
assert sha(OUTPUT/'package.py')==old['packager_sha256']
for name,row in original_files.items():assert sha(SOURCE/name)==row['sha256'] and (SOURCE/name).stat().st_size==row['bytes']
receipt={
 'status':'passed exact uncompressed tar identity; original gzip packet unchanged',
 'scope':'Lossless saved-data recompression only; full archive-member/source/corpus readback is a separate receipt.',
 'source_packet':str(SOURCE),'original_files':original_files,
 'original_gzip_archive':{'name':old['archive'],'bytes':old['bytes'],'sha256':EXPECTED_GZIP},
 'xz_archive':{'name':archive_path.name,'bytes':archive_path.stat().st_size,'sha256':EXPECTED_XZ},
 'uncompressed_tar':{'sha256':EXPECTED_TAR,'bytes':EXPECTED_TAR_BYTES,'gzip_and_xz_identical':True},
 'member_index_rows_unchanged':72087,'recipe':meta['recompression'],
 'reused_verified_trial':str(args.reuse_candidate) if args.reuse_candidate else None,
 'output_files':{p.name:{'bytes':p.stat().st_size,'sha256':sha(p)} for p in OUTPUT.iterdir() if p.is_file()},
}
(OUTPUT/'recompression.json').write_text(json.dumps(receipt,indent=2,sort_keys=True)+'\n')
print(json.dumps({'output':str(OUTPUT),'xz_archive':receipt['xz_archive'],'tar':receipt['uncompressed_tar'],'receipt_sha256':sha(OUTPUT/'recompression.json')},indent=2))
