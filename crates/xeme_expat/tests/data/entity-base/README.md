# External entity declaration bases

`root.xml` loads `entities.dtd`, which declares `data.txt` relative to its own base.
Set the root parser base to `/root/document.xml` and the DTD child parser base to
`/dtd/entities.dtd`. The later `data.txt` callback must receive `/dtd/entities.dtd`,
even after the DTD child is freed or the root base changes.

The C ABI regression covers internal declarations, child declarations, nested
parameter children, general child DTD snapshots, null/empty/non-UTF-8 bases, and
bytewise `XML_Parse`/`XML_ParseBuffer` input.

The independent probe additionally covers DOCTYPE callback mutation and external
parameter references inside grammar and entity values:

```console
PYTHONPATH=tools python tools/check_entity_bases.py /path/to/libexpat.so /path/to/libxeme_expat.so
```

All 48 probe scenarios agree with Expat 2.8.4. The original implementation fails
because callbacks read the referencing parser's current base instead of the base
captured by the entity declaration.
