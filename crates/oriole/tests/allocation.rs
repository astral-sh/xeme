//! Allocation-failure and allocator-routing checks for the complete safe core.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr;

use oriole::{Config, Error, ErrorKind, EventKind, Parser};
use oriole_storage::{Allocator, MemorySuite};

thread_local! {
    static TRACK_GLOBAL: Cell<bool> = const { Cell::new(false) };
    static GLOBAL_CALLS: Cell<usize> = const { Cell::new(0) };
    static CALLS: Cell<usize> = const { Cell::new(0) };
    static FAIL_AT: Cell<usize> = const { Cell::new(0) };
    static LIVE: Cell<usize> = const { Cell::new(0) };
}

struct CheckedGlobal;
// SAFETY: Allocation and deallocation always delegate to the same System allocator;
// the thread-local counters do not allocate or modify the returned allocations.
unsafe impl GlobalAlloc for CheckedGlobal {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_GLOBAL.get() {
            GLOBAL_CALLS.set(GLOBAL_CALLS.get() + 1);
        }
        // SAFETY: The GlobalAlloc caller supplies a valid nonzero layout.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The pointer and layout came from this allocator's System delegation.
        unsafe {
            System.dealloc(pointer, layout);
        }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK_GLOBAL.get() {
            GLOBAL_CALLS.set(GLOBAL_CALLS.get() + 1);
        }
        // SAFETY: The caller supplies a live System allocation and valid new size.
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static GLOBAL: CheckedGlobal = CheckedGlobal;

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}

fn fail_allocation() -> bool {
    CALLS.set(CALLS.get() + 1);
    FAIL_AT.get() != 0 && CALLS.get() >= FAIL_AT.get()
}
unsafe extern "C" fn checked_malloc(size: usize) -> *mut c_void {
    if fail_allocation() {
        return ptr::null_mut();
    }
    // SAFETY: libc malloc accepts any allocation size and reports failure with NULL.
    let pointer = unsafe { malloc(size) };
    if !pointer.is_null() {
        LIVE.set(LIVE.get() + 1);
    }
    pointer
}
unsafe extern "C" fn checked_realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    if fail_allocation() {
        return ptr::null_mut();
    }
    // SAFETY: The storage allocator passes a live block from this allocation suite.
    let result = unsafe { realloc(pointer, size) };
    if pointer.is_null() && !result.is_null() {
        LIVE.set(LIVE.get() + 1);
    }
    result
}
unsafe extern "C" fn checked_free(pointer: *mut c_void) {
    if !pointer.is_null() {
        LIVE.set(LIVE.get() - 1);
    }
    // SAFETY: The storage allocator returns the original suite pointer exactly once.
    unsafe {
        free(pointer);
    }
}

fn next_event(parser: &mut Parser) -> Result<Option<oriole::Event>, Error> {
    let result = parser.next_event();
    if let Err(error) = &result
        && error.kind == ErrorKind::NoMemory
    {
        let calls = CALLS.get();
        assert_eq!(parser.next_event().unwrap_err(), *error);
        assert_eq!(parser.feed(b"<ignored/>", true).unwrap_err(), *error);
        assert_eq!(
            CALLS.get(),
            calls,
            "a terminal error must not allocate on retry"
        );
    }
    result
}

fn workload(allocator: Allocator) -> Result<(), Error> {
    let mut parser = Parser::try_new_with_encoding_in(
        Config {
            namespace_separator: Some('|'),
            namespace_triplets: true,
            ..Config::default()
        },
        None,
        allocator,
    )?;
    parser.set_default_events(true);
    parser.feed(b"\r\n<!DOCTYPE r [ \n<!ENTITY internal '<p:n/>'><!ENTITY external SYSTEM 'child'><!ATTLIST r a NMTOKENS ' a  b '>\t]>\n<r xmlns:p='urn:p' p:attr='v'>&internal;&external;<!--c--><![CDATA[x]]></r>\r\n", true)?;
    while let Some(event) = next_event(&mut parser)? {
        if let EventKind::ExternalEntityReference { context, .. } = event.kind {
            let mut child = parser.external_child_with_encoding(context.as_deref(), None)?;
            child.feed(b"<?xml encoding='UTF-8'?><p:x a='value'/>text", true)?;
            while next_event(&mut child)?.is_some() {}
        }
    }
    let mut content = parser.external_child_with_encoding(Some(""), None)?;
    content.set_encoding(Some("ISO-8859-1"))?;
    content.feed(b"\xff\xfe\xef\xbb\xbftext", true)?;
    while next_event(&mut content)?.is_some() {}
    let mut parser = Parser::try_new_in(Config::default(), allocator)?;
    assert!(parser.set_param_entity_parsing(2));
    parser.feed(b"<!DOCTYPE r SYSTEM 'test.dtd'><r>&external;</r>", true)?;
    while let Some(event) = next_event(&mut parser)? {
        if let EventKind::ExternalEntityReference { context: None, .. } = event.kind {
            let mut child = parser.external_child_with_encoding(None, None)?;
            child.set_default_events(true);
            child.feed(
                b"<!ENTITY % close '><!ENTITY tail '><!ELEMENT before EMPTY %close; 'tail'><!ENTITY % mode 'INCLUDE['><![%mode;<![INCLUDE[<!ENTITY % part 'load'><!ENTITY % indirect '&#37;part;'><!ENTITY external '%indirect;ed'><!ENTITY % cr '&#13;'><!ENTITY quoted 'A%cr;\r\nB'>]]><!ATTLIST r default CDATA 'yes'><!ENTITY % model '(n|'><!ENTITY % attrs 'x CDATA &#34;A&#13;&#10;B&#34;'><!ENTITY % literal '&#34;T&#34;'><!ELEMENT r %model;m)><!ATTLIST r %attrs;><!ENTITY token %literal;><!ATTLIST r before CDATA 'B' %missing; after CDATA 'A'>]]><![IGNORE[ignored %missing; <![ nested ]]>]]>",
                true,
            )?;
            while next_event(&mut child)?.is_some() {}
            if let Err(error) = parser.merge_external_subset(&child) {
                let calls = CALLS.get();
                assert_eq!(parser.merge_external_subset(&child).unwrap_err(), error);
                assert_eq!(parser.next_event().unwrap_err(), error);
                assert_eq!(parser.feed(&[], true).unwrap_err(), error);
                assert_eq!(CALLS.get(), calls);
                return Err(error);
            }
        }
    }
    let parent = Parser::try_new_in(Config::default(), allocator)?;
    let mut duplicate = parent.external_child(None, None)?;
    duplicate.set_default_events(true);
    duplicate.feed(
        b"<!ENTITY e 'first'><!ENTITY e 'second'><!ENTITY e PUBLIC 'pub' 'sys'><!ENTITY e SYSTEM 'sys' NDATA n>",
        true,
    )?;
    while next_event(&mut duplicate)?.is_some() {}
    let mut child = parent.external_child(None, None)?;
    child.set_default_events(true);
    child.feed(
        b"<![INCLUDE%a;%b;[<!ENTITY e '&#0;'><!ATTLIST r a CDATA '&;'>]]>",
        true,
    )?;
    while next_event(&mut child)?.is_some() {}
    let mut missing = parent.external_child(None, None)?;
    missing.set_default_events(true);
    missing.feed(
        b"<!ENTITY e 'before%missing;after'><!ENTITY skipped 'no'>",
        true,
    )?;
    while next_event(&mut missing)?.is_some() {}
    let mut header = parent.external_child(None, None)?;
    header.set_param_entity_parsing(2);
    header.set_default_events(true);
    header.feed(
        b"<!ENTITY % hook SYSTEM 'hook'><![%hook;%keyword;[<!ENTITY loaded 'yes'>]]>",
        true,
    )?;
    while let Some(event) = next_event(&mut header)? {
        if let EventKind::ExternalEntityReference { context: None, .. } = event.kind {
            let mut loaded = header.external_child(None, None)?;
            loaded.feed(b"<!ENTITY % keyword 'INCLUDE'>", true)?;
            while next_event(&mut loaded)?.is_some() {}
            if let Err(error) = header.merge_external_subset(&loaded) {
                let calls = CALLS.get();
                assert_eq!(header.next_event().unwrap_err(), error);
                assert_eq!(header.feed(&[], true).unwrap_err(), error);
                assert_eq!(CALLS.get(), calls);
                return Err(error);
            }
        }
    }
    let mut values = parent.external_child(None, None)?;
    values.set_default_events(true);
    values.feed(
        b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY e 'L%p;R'><!ENTITY after 'A'>",
        true,
    )?;
    while let Some(event) = next_event(&mut values)? {
        if let EventKind::ExternalEntityReference { .. } = event.kind {
            let mut child = values.external_child(None, None)?;
            child.feed(b"<?xml version='1.0'?>X%q;Y", true)?;
            while let Some(event) = next_event(&mut child)? {
                if let EventKind::ExternalEntityReference { .. } = event.kind {
                    let mut nested = child.external_child(None, None)?;
                    nested.feed(b"\"&#13;Q\"", true)?;
                    while next_event(&mut nested)?.is_some() {}
                    child.merge_external_subset(&nested)?;
                }
            }
            values.merge_external_subset(&child)?;
        }
    }
    let mut grammar = parent.external_child(None, None)?;
    grammar.set_param_entity_parsing(2);
    grammar.set_default_events(true);
    grammar.feed(
        b"<!ENTITY % p SYSTEM 'p'><!ENTITY % v SYSTEM 'v'><!ATTLIST r a CDATA 'A' %p; b (x|%p;y) 'x'><!ENTITY e %p; 'L%v;R' %p;><!NOTATION n PUBLIC 'pub' %p;><!ELEMENT r (a|(%p;b,c)) %p;>",
        true,
    )?;
    while let Some(event) = next_event(&mut grammar)? {
        if let EventKind::ExternalEntityReference { system_id, .. } = event.kind {
            let mut child = grammar.external_child(None, None)?;
            child.feed(
                if system_id.as_deref() == Some("v") {
                    b"X"
                } else {
                    b""
                },
                true,
            )?;
            while next_event(&mut child)?.is_some() {}
            grammar.merge_external_subset(&child)?;
        }
    }
    for document in [b"<r/>".as_slice(), b"<!DOCTYPE r []><r/>"] {
        let mut foreign = Parser::try_new_in(Config::default(), allocator)?;
        assert!(foreign.set_use_foreign_dtd(true));
        assert!(foreign.set_param_entity_parsing(2));
        foreign.feed(document, true)?;
        let mut notified = false;
        while let Some(event) = next_event(&mut foreign)? {
            match event.kind {
                EventKind::ExternalEntityReference { .. } => {
                    let mut child = foreign.external_child(None, None)?;
                    child.feed(b"", true)?;
                    while next_event(&mut child)?.is_some() {}
                }
                EventKind::NotStandalone => notified = true,
                _ => {}
            }
        }
        assert!(notified);
    }
    let mut parser =
        Parser::try_new_with_encoding_in(Config::default(), Some("custom"), allocator)?;
    parser.feed(b"<r>\x80</r>", true)?;
    match parser.next_event() {
        Err(error) if error.kind == ErrorKind::UnknownEncoding => {
            let mut map = std::array::from_fn(|index| index as i32);
            map[128] = 0x20ac;
            parser.set_encoding_map("custom", map)?;
        }
        Err(error) => return Err(error),
        _ => panic!("custom encoding unexpectedly resolved without its map"),
    }
    while next_event(&mut parser)?.is_some() {}
    parser.set_completed_encoding(Some("finished-protocol-name"))?;
    parser.set_completed_encoding(None)?;
    let mut child = parser.external_child_with_encoding(Some(""), Some("custom"))?;
    child.feed(b"\x80", true)?;
    while next_event(&mut child)?.is_some() {}
    let mut parser =
        Parser::try_new_with_encoding_in(Config::default(), Some("multibyte"), allocator)?;
    parser.feed(b"<\x80\0 a='\x80\0'>\x80\0</\x80\0>", true)?;
    match parser.next_event() {
        Err(error) if error.kind == ErrorKind::UnknownEncoding => {
            let mut map = std::array::from_fn(|index| index as i32);
            map[128] = -2;
            parser.set_multibyte_encoding_map("multibyte", map)?;
        }
        Err(error) => return Err(error),
        _ => panic!("multibyte encoding unexpectedly resolved without its map"),
    }
    loop {
        if next_event(&mut parser)?.is_some() {
            continue;
        }
        if parser.encoding_conversion().is_none() {
            break;
        }
        if let Err(error) = parser.resolve_encoding_conversion('é' as i32) {
            let calls = CALLS.get();
            assert_eq!(parser.next_event().unwrap_err(), error);
            assert_eq!(
                parser.resolve_encoding_conversion('é' as i32).unwrap_err(),
                error
            );
            assert_eq!(CALLS.get(), calls);
            return Err(error);
        }
    }
    Ok(())
}

#[test]
fn every_allocation_can_fail_and_all_memory_uses_the_selected_suite() {
    // SAFETY: The callbacks use a complete libc-backed suite with failure injection;
    // every pointer remains valid until realloc succeeds or its matching free call.
    let allocator = unsafe {
        Allocator::from_callbacks(MemorySuite {
            malloc: Some(checked_malloc),
            realloc: Some(checked_realloc),
            free: Some(checked_free),
        })
        .unwrap()
    };
    FAIL_AT.set(0);
    CALLS.set(0);
    LIVE.set(0);
    GLOBAL_CALLS.set(0);
    TRACK_GLOBAL.set(true);
    let result = workload(allocator);
    TRACK_GLOBAL.set(false);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(LIVE.get(), 0);
    assert_eq!(
        GLOBAL_CALLS.get(),
        0,
        "parser bypassed the selected allocator"
    );
    let allocation_count = CALLS.get();
    assert!(
        allocation_count > 100,
        "workload must exercise substantial allocation"
    );
    for failure in 1..=allocation_count {
        FAIL_AT.set(failure);
        CALLS.set(0);
        GLOBAL_CALLS.set(0);
        TRACK_GLOBAL.set(true);
        let result = workload(allocator);
        TRACK_GLOBAL.set(false);
        assert_eq!(
            result.unwrap_err().kind,
            ErrorKind::NoMemory,
            "allocation {failure}"
        );
        assert_eq!(LIVE.get(), 0, "leaked allocation at failure {failure}");
        assert_eq!(
            GLOBAL_CALLS.get(),
            0,
            "global allocation at failure {failure}"
        );
    }
    FAIL_AT.set(0);
}

#[test]
fn hash_salt_reconfiguration_is_transactional_at_every_allocation() {
    // SAFETY: The complete libc-backed suite retains ownership on failure and
    // frees each successful allocation through the same callbacks.
    let allocator = unsafe {
        Allocator::from_callbacks(MemorySuite {
            malloc: Some(checked_malloc),
            realloc: Some(checked_realloc),
            free: Some(checked_free),
        })
        .unwrap()
    };
    let attempt = |failure| {
        FAIL_AT.set(0);
        CALLS.set(0);
        GLOBAL_CALLS.set(0);
        TRACK_GLOBAL.set(true);
        let mut parser = Parser::try_new_in(Config::default(), allocator).unwrap();
        parser.feed(b"<!DOCTYPE r [<!ENTITY e 'ok'><!ENTITY % p 'unused'><!ATTLIST r a CDATA 'v'><!ATTLIST n b CDATA 'w'>]><r/>", true).unwrap();
        while parser.next_event().unwrap().is_some() {}
        let before = CALLS.get();
        if failure != 0 {
            FAIL_AT.set(before + failure);
        }
        let result = parser.set_hash_salt(*b"0123456789abcdef");
        let count = CALLS.get() - before;
        if failure == 0 {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err().kind, ErrorKind::NoMemory);
            assert_eq!(parser.hash_salt(), [0; 16]);
        }
        FAIL_AT.set(0);
        // Existing declarations and defaults still resolve after every failure.
        let mut child = parser.external_child(Some(""), None).unwrap();
        child.feed(b"<n>&e;</n>", true).unwrap();
        let mut text_seen = false;
        let mut default_seen = false;
        while let Some(event) = child.next_event().unwrap() {
            match event.kind {
                EventKind::Text(value) => {
                    assert_eq!(value, "ok");
                    text_seen = true;
                }
                EventKind::StartElement { attributes, .. } => {
                    assert_eq!(attributes.len(), 1);
                    assert_eq!(attributes[0].name, "b");
                    assert_eq!(attributes[0].value, "w");
                    default_seen = true;
                }
                _ => {}
            }
        }
        assert!(text_seen && default_seen);
        parser.set_hash_salt(*b"0123456789abcdef").unwrap();
        assert_eq!(parser.hash_salt(), *b"0123456789abcdef");
        drop(child);
        drop(parser);
        TRACK_GLOBAL.set(false);
        assert_eq!(LIVE.get(), 0, "failure {failure}");
        assert_eq!(GLOBAL_CALLS.get(), 0, "failure {failure}");
        count
    };
    let count = attempt(0);
    assert!(count > 4, "exercise populated tables and nested indexes");
    for failure in 1..=count {
        attempt(failure);
    }
}
