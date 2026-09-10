# Third-party notices

Oriole's implementation is original Rust code, licensed under MIT or Apache-2.0.
The C header retains Expat's original copyright and license notices. Validation
artifacts may contain test inputs, assertion excerpts, or diagnostics from these
pinned upstream projects:

| Project | Revision | Notice |
| --- | --- | --- |
| Expat | `12cf0b1f25f026a022fe728ad8f7e3d017285b80` | [MIT](licenses/expat.txt) |
| libxml2 | `97bd75ab83e6c2c2596cc520abe80d6779e4a69c` | [MIT](licenses/libxml2.txt) |
| CPython | `3bb231a6a5dc02b95658877318bf61501a7209e9` | [Python licenses](licenses/cpython.txt) |

Cargo dependencies retain their own licenses. The python-build-standalone bundle
collects notices for the linked crates, Rust runtime, and Expat header alongside
its exact build manifest.
