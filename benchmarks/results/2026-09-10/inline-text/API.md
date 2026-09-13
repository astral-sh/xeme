# Experimental immutable character-data payload

`EventKind::Text` changes its Rust payload from `oriole_storage::String` to the
new `oriole::Text` reexport (`oriole_storage::Text`). This is an intentional early
Rust API change. The Expat C interface and callback contents are unchanged.

`Text` provides `as_str()`, `as_bytes()`, `Deref<Target = str>`, `AsRef<str>`,
`Debug`, `Display`, equality, and a fallible `try_clone()`. Heap clones use the
original selected allocator. A short inline clone does not allocate. `From` can
move an existing allocator-owned `String` into `Text` without copying.

Character data is immutable. Consumers that previously appended to returned
payloads should accumulate in their own mutable string, then construct one
`Text` with `try_from_str_in` when the text run ends. The streaming fuzzer and
core test normalizer demonstrate this while keeping growth amortized and
allocation failures explicit.

Inline storage holds up to 23 UTF-8 bytes, copied only from a complete `str`. The
inline length and bytes are private and have no mutation API. Its raw pointer is
valid only while the owner remains in place; C dispatch keeps the owner in its
stack frame for the complete callback. Larger values retain the existing
fallible allocator-backed String. Physical CR normalization keeps its existing
fallible path, while CR introduced by an internal numeric entity remains intact.

Measured x86_64 sizes match the original containing objects: String 56,
Attribute 120, Event 320, Parser 1416 bytes; Text itself is 56 bytes. Other targets
must validate their own layout. Shared output/expansion byte accounting remains
unchanged; inline data is included in the containing event/queue storage.
