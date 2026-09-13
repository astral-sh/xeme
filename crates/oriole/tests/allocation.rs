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
            name_rules: oriole::NameRules::FourthEdition,
            ..Config::default()
        },
        None,
        allocator,
    )?;
    parser.set_default_events(true);
    parser.feed("\r\n<!DOCTYPE r [ \n<!ENTITY internal '<p:À/>'><!ENTITY external SYSTEM 'child'><!NOTATION n SYSTEM 'notation'><!ATTLIST r a NMTOKENS ' a  b '>\t]>\n<r xmlns='urn:default' xmlns:p='urn:p' p:attr='v'><scope xmlns='urn:inner'><plain/><scope xmlns=''><plain/></scope><plain/></scope>a\r\nb\nc&internal;&external;<!--c--><![CDATA[x]]></r>\r\n".as_bytes(), true)?;
    while let Some(event) = next_event(&mut parser)? {
        if let EventKind::ExternalEntityReference(reference) = event.kind {
            assert_eq!(
                reference.context.as_deref(),
                Some(
                    "=urn:default\u{c}p=urn:p\u{c}xml=http://www.w3.org/XML/1998/namespace\u{c}external"
                )
            );
            let mut child =
                parser.external_child_with_encoding(reference.context.as_deref(), None)?;
            child.feed(b"<?xml encoding='UTF-8'?><plain/><n xmlns='urn:child'><inner/></n><p:x a='value'/>text", true)?;
            let expected = [
                "urn:default|plain",
                "urn:child|n",
                "urn:child|inner",
                "urn:p|x|p",
            ];
            let mut starts = 0;
            while let Some(event) = next_event(&mut child)? {
                if let EventKind::StartElement { name, .. } = event.kind {
                    assert_eq!(name, expected[starts]);
                    starts += 1;
                }
            }
            assert_eq!(starts, expected.len());
            for (context, expected) in [("=urn:override", "urn:override|plain"), ("=", "plain")] {
                let mut child = parser.external_child_with_encoding(Some(context), None)?;
                child.feed(b"<plain/>", true)?;
                let mut seen = false;
                while let Some(event) = next_event(&mut child)? {
                    if let EventKind::StartElement { name, .. } = event.kind {
                        assert_eq!(name, expected);
                        seen = true;
                    }
                }
                assert!(seen);
            }
        }
    }
    // Branches revisit completed entities; depth forces both explicit-stack and
    // active-set growth through the selected allocator.
    let mut attributes = Parser::try_new_in(Config::default(), allocator)?;
    assert!(attributes.set_param_entity_parsing(2));
    attributes.feed(b"<!DOCTYPE r [<!ENTITY % declaration \"<!ATTLIST r extra CDATA 'A&#13;&#10;B'>\">%declaration;<!ENTITY e0 ' A&#13;&#10;B '><!ENTITY e1 '&e0;&e0;'><!ENTITY e2 '&e1;'><!ENTITY e3 '&e2;'><!ENTITY e4 '&e3;'><!ENTITY e5 '&e4;'><!ENTITY e6 '&e5;'><!ENTITY e7 '&e6;'><!ENTITY e8 '&e7;'><!ENTITY e9 '&e8;'><!ATTLIST r default CDATA '&e9;'>]><r a='&e9;&e9;'/>", true)?;
    while next_event(&mut attributes)?.is_some() {}
    let mut streamed = Parser::try_new_in(Config::default(), allocator)?;
    streamed.set_default_events(true);
    streamed.set_attlist_handler_enabled(false);
    streamed.feed(b"<!DOCTYPE r [<!ATTLIST r a (x|y) 'x' b NOTATION (n|m) #IMPLIED c CDATA 'C' d CDATA 'D'>]><r/>", true)?;
    while let Some(event) = next_event(&mut streamed)? {
        if matches!(event.kind, EventKind::AttlistDeclarationPrefix)
            && streamed.current_raw() == Some("(")
        {
            streamed.set_attlist_handler_enabled(true);
        }
        if let EventKind::AttlistDeclaration(value) = event.kind
            && value.name == "b"
        {
            streamed.set_attlist_handler_enabled(false);
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
        if let EventKind::ExternalEntityReference(reference) = event.kind
            && reference.context.is_none()
        {
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
    // Independent fixture: parameter siblings intentionally share DTD skip state.
    let parent = Parser::try_new_in(Config::default(), allocator)?;
    let mut child = parent.external_child(None, None)?;
    child.set_default_events(true);
    child.feed(
        b"<![INCLUDE%a;%b;[<!ENTITY e '&#0;'><!ATTLIST r a CDATA '&;'>]]>",
        true,
    )?;
    while next_event(&mut child)?.is_some() {}
    let parent = Parser::try_new_in(Config::default(), allocator)?;
    let mut missing = parent.external_child(None, None)?;
    missing.set_default_events(true);
    missing.feed(
        b"<!ENTITY e 'before%missing;after'><!ENTITY skipped 'no'>",
        true,
    )?;
    while next_event(&mut missing)?.is_some() {}
    let parent = Parser::try_new_in(Config::default(), allocator)?;
    let mut header = parent.external_child(None, None)?;
    header.set_param_entity_parsing(2);
    header.set_default_events(true);
    header.feed(
        b"<!ENTITY % hook SYSTEM 'hook'><![%hook;%keyword;[<!ENTITY loaded 'yes'>]]>",
        true,
    )?;
    while let Some(event) = next_event(&mut header)? {
        if let EventKind::ExternalEntityReference(reference) = event.kind
            && reference.context.is_none()
        {
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
    let parent = Parser::try_new_in(Config::default(), allocator)?;
    let mut values = parent.external_child(None, None)?;
    values.set_default_events(true);
    values.feed(
        b"<!ENTITY % p SYSTEM 'p'><!ENTITY % q SYSTEM 'q'><!ENTITY % wrapper 'L&#37;p;R'><!ENTITY e '%wrapper;'><!ENTITY after 'A'>",
        true,
    )?;
    while let Some(event) = next_event(&mut values)? {
        if let EventKind::ExternalEntityReference(_) = event.kind {
            // The wrapper frame is still live: rekey its owned membership table
            // under every selected-allocation failure before child creation.
            values.set_hash_salt([23; 16])?;
            let mut child = values.external_child(None, None)?;
            child.feed(b"<?xml version='1.0'?>X%q;Y", true)?;
            while let Some(event) = next_event(&mut child)? {
                if let EventKind::ExternalEntityReference(_) = event.kind {
                    let mut nested = child.external_child(None, None)?;
                    nested.feed(b"\"&#13;Q\"", true)?;
                    while next_event(&mut nested)?.is_some() {}
                    child.merge_external_subset(&nested)?;
                }
            }
            values.merge_external_subset(&child)?;
        }
    }
    let parent = Parser::try_new_in(Config::default(), allocator)?;
    let mut grammar = parent.external_child(None, None)?;
    grammar.set_param_entity_parsing(2);
    grammar.set_default_events(true);
    grammar.feed(
        b"<!ENTITY % p SYSTEM 'p'><!ENTITY % v SYSTEM 'v'><!ATTLIST r a CDATA 'A' %p; b (x|%p;y) 'x'><!ENTITY e %p; 'L%v;R' %p;><!NOTATION n PUBLIC 'pub' %p;><!ELEMENT r (a|(%p;b,c)) %p;>",
        true,
    )?;
    while let Some(event) = next_event(&mut grammar)? {
        if let EventKind::ExternalEntityReference(reference) = event.kind {
            let mut child = grammar.external_child(None, None)?;
            child.feed(
                if reference.system_id.as_deref() == Some("v") {
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
                EventKind::ExternalEntityReference(_) => {
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
    // Exercise scratch reuse and growth while failure injection is active.
    // Every completed event remains owned independently of later lexical scans.
    let mut parser = Parser::try_new_with_encoding_in(Config::default(), None, allocator)?;
    let xml = concat!(
        "<r><a a0='0' a1='1' a2='2' a3='3' a4='4' a5='5' a6='6' a7='7'/>",
        "<b b0='0' b1='1' b2='2' b3='3' b4='4' b5='5' b6='6' b7='7' b8='8'/>",
        "<c final='&amp;é'/></r>",
    );
    let mut chunks = xml.as_bytes().chunks(7).peekable();
    let mut starts = 0;
    while let Some(bytes) = chunks.next() {
        parser.feed(bytes, chunks.peek().is_none())?;
        while let Some(event) = next_event(&mut parser)? {
            if let EventKind::StartElement { name, attributes } = event.kind {
                starts += 1;
                if name == "c" {
                    assert_eq!(attributes.len(), 1);
                    assert_eq!(attributes[0].name, "final");
                    assert_eq!(attributes[0].value, "&é");
                }
            }
        }
    }
    assert_eq!(starts, 4);
    assert!(parser.is_finished());
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
    for character in ['é', 'A'] {
        let mut parser =
            Parser::try_new_with_encoding_in(Config::default(), Some("multibyte"), allocator)?;
        parser.feed(b"<!DOCTYPE \x80\0 [<!ENTITY \x80\0 '\x80\0'><!ATTLIST \x80\0 b CDATA '\x80\0'>]><\x80\0 a='\x80\0'>\x80\0&\x80\0;</\x80\0>", true)?;
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
            if let Err(error) = parser.resolve_encoding_conversion(character as i32) {
                let calls = CALLS.get();
                assert_eq!(parser.next_event().unwrap_err(), error);
                assert_eq!(
                    parser
                        .resolve_encoding_conversion(character as i32)
                        .unwrap_err(),
                    error
                );
                assert_eq!(CALLS.get(), calls);
                return Err(error);
            }
        }
    }
    for mode in [0, 2] {
        let mut missing = Parser::try_new_in(Config::default(), allocator)?;
        missing.set_default_events(true);
        missing.set_param_entity_parsing(mode);
        missing.feed(
            b"<!DOCTYPE r [%missing;<!ENTITY ignored 'v'>]><r>&ignored;</r>",
            true,
        )?;
        while next_event(&mut missing)?.is_some() {}
    }
    // Retained metadata must own both its fields and its allocation suite after
    // the originating parser has been destroyed. Every new payload allocation
    // also participates in the fail-at-each-allocation loop below.
    let mut parser = Parser::try_new_in(Config::default(), allocator)?;
    parser.feed(b"<!DOCTYPE r [<!ENTITY e SYSTEM 'child'><!ATTLIST r a CDATA 'v'><!NOTATION n SYSTEM 'notation'>]><r>&e;</r>", true)?;
    let mut retained: [Option<oriole::Event>; 5] = Default::default();
    while let Some(event) = next_event(&mut parser)? {
        let slot = match &event.kind {
            EventKind::StartDoctype(_) => 0,
            EventKind::EntityDeclaration(_) => 1,
            EventKind::AttlistDeclaration(_) => 2,
            EventKind::NotationDeclaration(_) => 3,
            EventKind::ExternalEntityReference(_) => 4,
            _ => continue,
        };
        retained[slot] = Some(event);
    }
    drop(parser);
    for event in retained {
        match event.expect("retained metadata event").kind {
            EventKind::StartDoctype(value) => assert_eq!(value.name, "r"),
            EventKind::EntityDeclaration(value) => {
                assert_eq!(value.system_id.as_deref(), Some("child"))
            }
            EventKind::AttlistDeclaration(value) => assert_eq!(value.default.as_deref(), Some("v")),
            EventKind::NotationDeclaration(value) => {
                assert_eq!(value.system_id.as_deref(), Some("notation"))
            }
            EventKind::ExternalEntityReference(value) => {
                assert_eq!(value.system_id.as_deref(), Some("child"))
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}

#[test]
fn every_allocation_can_fail_and_all_memory_uses_the_selected_suite() {
    check_allocations(workload);
}

#[test]
fn detached_start_frames_use_the_selected_suite_and_clear_on_every_failure() {
    fn frames(
        allocator: Allocator,
        namespace_separator: Option<char>,
        input: &[u8],
    ) -> Result<(), Error> {
        let mut parser = Parser::try_new_in(
            Config {
                namespace_separator,
                ..Config::default()
            },
            allocator,
        )?;
        parser.feed(input, true)?;
        let mut frame = parser.adapter_frame();
        let result = (|| {
            loop {
                let mut event = None;
                let result = parser.next_event_for_adapter_into(&mut event, &mut frame);
                let token = match result {
                    Ok(Some(token)) => token,
                    Ok(None) => break,
                    Err(error) => {
                        assert!(event.is_none());
                        assert!(!frame.is_active());
                        let calls = CALLS.get();
                        assert_eq!(
                            parser
                                .next_event_for_adapter_into(&mut event, &mut frame)
                                .unwrap_err(),
                            error
                        );
                        assert_eq!(CALLS.get(), calls);
                        return Err(error);
                    }
                };
                if frame.is_active() {
                    assert!(event.is_none());
                    if let Some(name) = frame.take_end_name() {
                        assert_eq!(frame.callback_bytes(), name.len());
                        parser.recycle_end_element(token, name);
                    } else if let Some(bytes) = frame.text_bytes() {
                        assert_eq!(frame.callback_bytes(), bytes.len());
                        assert!(!bytes.is_empty());
                    } else {
                        assert_eq!(frame.name_bytes().last(), Some(&0));
                        for (name, value) in frame.attributes() {
                            assert_eq!(name.last(), Some(&0));
                            assert_eq!(value.last(), Some(&0));
                        }
                    }
                } else {
                    match event.unwrap().kind {
                        EventKind::StartElement { name, attributes } => {
                            parser.recycle_start_element(token, name, attributes)
                        }
                        EventKind::EndElement { name } => parser.recycle_end_element(token, name),
                        _ => {}
                    }
                }
            }
            Ok(())
        })();
        parser.finish_adapter_frame(frame);
        result
    }
    check_allocations(|allocator| {
        frames(allocator, None, b"<r>inline\nabcdefghijklmnopqrstuvwxyz1234567890<n a='first' b='value'/><![CDATA[abcdefghijklmnopqrstuvwxyz1234567890]]><n a='second' b='new'/>fallback\r\n<n a='literal' b='other'/><n a='&amp;'/><n a='last'/><n a0='0' a1='1' a2='2' a3='3' a4='4' a5='5' a6='6' a7='7' a8='8'/><n a0='0' a1='1' a2='2' a3='3' a4='4' a5='5' a6='6' a7='7' a8='8'/>abcdefghijklmnopqrstuvwxyz1234567890</r>")?;
        frames(allocator, Some('|'), b"<r xmlns:p='urn:p'><p:n a='first'/><p:n a='next'/><n xmlns='urn:default'><n a='value'/></n><p:n a0='0' a1='1' a2='2' a3='3' a4='4' a5='5' a6='6' a7='7' a8='8'/><p:n a='last'/></r>")?;
        // Two wide literal tags warm the lexical and callback buffers. The last
        // tag then uses one warmed span, including an empty and Unicode value.
        frames(allocator, None, "<r><n a='abcdefghijklmnopqrstuvwxyz' b='abcdefghijklmnopqrstuvwxyz' c='abcdefghijklmnopqrstuvwxyz'/><n a='abcdefghijklmnopqrstuvwxyz' b='abcdefghijklmnopqrstuvwxyz' c='abcdefghijklmnopqrstuvwxyz'/><n π = '😀' empty=\"\" tail='λ'/></r>".as_bytes())?;
        // Grow both stacks with live undo blocks, then restore shadowed prefixes.
        // Four root declarations also exercise Start publication after commitment.
        frames(allocator, Some('|'), b"<r xmlns:p='one' xmlns:q='q' xmlns:s='s' xmlns:t='t'><n xmlns:p='two'><n xmlns:p='three'><n xmlns:p='four'><n xmlns:p='five'><n xmlns:p='six'><p:leaf></p:leaf><e xmlns:p='empty'/><p:leaf/></n></n></n></n></n><p:leaf/></r>")?;
        // More live prefixed names than cache slots force fresh packed owners;
        // grow the spelling while the expansion scratch is returned for reuse.
        frames(allocator, Some('|'), b"<r xmlns:p='urn:example'><p:node><p:node><p:node><p:longer a='v'/></p:node></p:node></p:node><p:node/></r>")?;
        // A warm oversized raw-name owner precedes nested equal serialized names.
        // Their packed owners must survive both empty and explicit End delivery.
        frames(allocator, Some('\0'), b"<r xmlns:p='p:'><abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz/><p:n><p:n><p:n a='v'/></p:n></p:n></r>")?;
        frames(allocator, Some(':'), b"<r xmlns:p='p'><abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz/><p:n><p:n><p:n a='v'/></p:n></p:n></r>")
    });
}

#[test]
fn detached_end_names_reuse_selected_storage_without_new_allocations() {
    fn ends(allocator: Allocator) -> Result<(), Error> {
        let mut parser = Parser::try_new_in(Config::default(), allocator)?;
        parser.feed(b"<r><n a='v'></n><n a='v'></n></r>", true)?;
        let mut frame = parser.adapter_frame();
        let result = (|| {
            let mut ends = 0;
            loop {
                let calls = CALLS.get();
                let mut event = None;
                let Some(token) = parser.next_event_for_adapter_into(&mut event, &mut frame)?
                else {
                    break;
                };
                if let Some(mut name) = frame.take_end_name() {
                    assert!(event.is_none());
                    name.try_push('\0')?;
                    parser.recycle_end_element(token, name);
                    assert_eq!(CALLS.get(), calls, "warmed native End must not allocate");
                    ends += 1;
                }
            }
            assert_eq!(ends, 3);
            Ok(())
        })();
        parser.finish_adapter_frame(frame);
        result
    }
    check_allocations(ends);
}

fn check_allocations(workload: fn(Allocator) -> Result<(), Error>) {
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
    assert!(allocation_count > 0, "workload must exercise allocation");
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
fn malformed_declarations_preserve_every_selected_allocation_failure() {
    fn declarations(allocator: Allocator) -> Result<(), Error> {
        for native in [false, true] {
            for context in [None, Some(Some("")), Some(None)] {
                for input in [
                    b"<?xml version='1.0' encoding 'UTF-8'?>".as_slice(),
                    b"<?xml version='1.0' encoding='UTF-8' extra='x'?>",
                    b"<?xml encoding='UTF8' version='1.0'?>",
                    b"<?xml version='1.0' encoding='UTF8' extra='x'?>",
                ] {
                    let parent = Parser::try_new_in(Config::default(), allocator)?;
                    let mut parser = match context {
                        None => Parser::try_new_in(Config::default(), allocator)?,
                        Some(context) => parent.external_child(context, None)?,
                    };
                    if native {
                        parser.enable_input_context();
                    }
                    parser.feed(input, true)?;
                    let error = next_event(&mut parser).unwrap_err();
                    if error.kind == ErrorKind::NoMemory {
                        return Err(error);
                    }
                    assert_eq!(
                        error.kind,
                        if context.is_none() {
                            ErrorKind::XmlDeclaration
                        } else {
                            ErrorKind::TextDeclaration
                        },
                    );
                    assert_eq!(parser.next_event().unwrap_err(), error);
                    assert_eq!(parser.feed(&[], true).unwrap_err(), error);
                }
            }
        }
        Ok(())
    }
    check_allocations(declarations);
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
        let mut parser = Parser::try_new_in(
            Config {
                namespace_separator: Some('|'),
                ..Config::default()
            },
            allocator,
        )
        .unwrap();
        parser.feed(b"<!DOCTYPE r [<!ENTITY e 'ok'><!ENTITY % p 'unused'><!ATTLIST r a CDATA 'v'><!ATTLIST n b CDATA 'w'>]><r xmlns='urn:default'>", false).unwrap();
        while parser.next_event().unwrap().is_some() {}
        let retained = parser.external_child(None, None).unwrap();
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
        assert_eq!(retained.hash_salt(), [0; 16]);
        let mut child = retained.external_child(Some(""), None).unwrap();
        child.feed(b"<n>&e;</n>", true).unwrap();
        let mut text_seen = false;
        let mut default_seen = false;
        while let Some(event) = child.next_event().unwrap() {
            match event.kind {
                EventKind::Text(value) => {
                    assert_eq!(value, "ok");
                    text_seen = true;
                }
                EventKind::StartElement { name, attributes } => {
                    assert_eq!(name, "urn:default|n");
                    assert_eq!(attributes.len(), 1);
                    assert_eq!(attributes[0].name, "b");
                    assert_eq!(attributes[0].value, "w");
                    default_seen = true;
                }
                _ => {}
            }
        }
        assert!(text_seen && default_seen);
        // The live parent slot also survives every failed transaction; the
        // retained child above alone would only prove its independent snapshot.
        let mut current = parser.external_child(Some(""), None).unwrap();
        current.feed(b"<n/>", true).unwrap();
        let mut current_seen = false;
        while let Some(event) = current.next_event().unwrap() {
            if let EventKind::StartElement { name, .. } = event.kind {
                assert_eq!(name, "urn:default|n");
                current_seen = true;
            }
        }
        assert!(current_seen);
        drop(current);
        parser.set_hash_salt(*b"0123456789abcdef").unwrap();
        assert_eq!(parser.hash_salt(), *b"0123456789abcdef");
        parser.feed(b"<n/></r>", true).unwrap();
        let mut restored = false;
        while let Some(event) = parser.next_event().unwrap() {
            if let EventKind::EndNamespace { prefix: None } = event.kind {
                restored = true;
            }
        }
        assert!(restored && parser.is_finished());
        drop(child);
        drop(retained);
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

/// Exercise returned original buffers through growth, normalization and entities.
fn recycling_workload(allocator: Allocator, xml: &[u8], recycle: bool) -> Result<(), Error> {
    let mut parser = Parser::try_new_in(Config::default(), allocator)?;
    parser.feed(xml, true)?;
    while let Some((event, token)) = parser.next_event_for_recycling()? {
        if let EventKind::StartElement { mut attributes, .. } = event.kind {
            for attribute in &mut attributes {
                // Match the C bridge's terminator preparation without a callback.
                attribute.name.try_push('\0')?;
                attribute.value.try_push('\0')?;
            }
            if recycle {
                parser.recycle_attributes(token, attributes);
            }
        }
    }
    Ok(())
}

#[test]
fn recycled_attributes_reduce_allocations_and_survive_each_failure() {
    // SAFETY: This complete libc-backed suite preserves ownership on failure.
    let allocator = unsafe {
        Allocator::from_callbacks(MemorySuite {
            malloc: Some(checked_malloc),
            realloc: Some(checked_realloc),
            free: Some(checked_free),
        })
        .unwrap()
    };
    let mut xml = std::string::String::from(
        "<!DOCTYPE r [<!ENTITY e 'entity'><!ATTLIST n a NMTOKENS #IMPLIED>]><r>",
    );
    for _ in 0..24 {
        xml.push_str("<n a='a' b='literal value' c='value' d='another' e='value' f='value' g='value' h='value'/><empty/>");
    }
    xml.push_str("<n a='  a  b  ' b='&e;' c='physical\r\nspace' d='");
    xml.push_str(&"x".repeat(8192));
    xml.push_str("'/><n a='small' b='again'/></r>");
    let run = |failure, recycle| {
        FAIL_AT.set(failure);
        CALLS.set(0);
        GLOBAL_CALLS.set(0);
        TRACK_GLOBAL.set(true);
        let result = recycling_workload(allocator, xml.as_bytes(), recycle);
        TRACK_GLOBAL.set(false);
        assert_eq!(LIVE.get(), 0, "allocation {failure}");
        assert_eq!(GLOBAL_CALLS.get(), 0, "allocation {failure}");
        (result, CALLS.get())
    };
    let (result, ordinary) = run(0, false);
    result.unwrap();
    let (result, recycled) = run(0, true);
    result.unwrap();
    assert!(
        recycled < ordinary / 2,
        "ordinary={ordinary}, recycled={recycled}"
    );
    for failure in 1..=recycled {
        assert_eq!(run(failure, true).0.unwrap_err().kind, ErrorKind::NoMemory);
    }
    FAIL_AT.set(0);
}

#[test]
fn foreign_recycling_token_rejects_storage_from_the_same_custom_suite() {
    // SAFETY: The complete suite routes both parsers through matching libc calls.
    let allocator = unsafe {
        Allocator::from_callbacks(MemorySuite {
            malloc: Some(checked_malloc),
            realloc: Some(checked_realloc),
            free: Some(checked_free),
        })
        .unwrap()
    };
    let input = b"<r a='value' b='another value'/>";
    let run = |return_foreign| {
        FAIL_AT.set(0);
        let mut first = Parser::try_new_in(Config::default(), allocator).unwrap();
        first.feed(input, true).unwrap();
        let (event, token) = first.next_event_for_recycling().unwrap().unwrap();
        let EventKind::StartElement { attributes, .. } = event.kind else {
            panic!()
        };
        let mut second = Parser::try_new_in(Config::default(), allocator).unwrap();
        if return_foreign {
            second.recycle_attributes(token, attributes);
        }
        let before = CALLS.get();
        second.feed(input, true).unwrap();
        while second.next_event().unwrap().is_some() {}
        CALLS.get() - before
    };
    let ordinary = run(false);
    assert_eq!(LIVE.get(), 0);
    assert_eq!(
        run(true),
        ordinary,
        "a foreign cache must not warm another parser"
    );
    assert_eq!(LIVE.get(), 0);
}

fn open_value_workload(allocator: Allocator) -> Result<(), Error> {
    let root = Parser::try_new_in(Config::default(), allocator)?;
    let mut dtd = root.external_child(None, None)?;
    dtd.feed(
        b"<!ENTITY % a SYSTEM 'a'><!ENTITY % b 'B'><!ENTITY n '%a;'>",
        true,
    )?;
    while let Some(event) = next_event(&mut dtd)? {
        if matches!(event.kind, EventKind::ExternalEntityReference(_)) {
            break;
        }
    }
    let mut first = dtd.external_child(None, None)?;
    let mut sibling = dtd.external_child(None, None)?;
    first.feed(b"%b;", true)?;
    while next_event(&mut first)?.is_some() {}
    let general = dtd.external_child(Some(""), None)?;
    let mut copied_dtd = general.external_child(None, None)?;
    drop(first);
    drop(dtd);
    drop(root);
    drop(general);
    sibling.feed(b"%b;", true)?;
    match next_event(&mut sibling) {
        Err(error) if error.kind == ErrorKind::RecursiveEntityReference => {}
        Err(error) => return Err(error),
        _ => panic!("a precreated sibling must retain the open entity state"),
    }
    copied_dtd.feed(b"<!ENTITY m '%b;'>", true)?;
    while next_event(&mut copied_dtd)?.is_some() {}
    Ok(())
}

#[test]
fn shared_open_value_state_survives_every_allocation_failure_and_parent_drop() {
    check_allocations(open_value_workload);
}

fn inherited_attribute_workload(allocator: Allocator) -> Result<(), Error> {
    let mut root = Parser::try_new_in(Config::default(), allocator)?;
    root.feed(b"<!DOCTYPE r [<!ENTITY leaf 'value'>]><r/>", true)?;
    while next_event(&mut root)?.is_some() {}
    let mut child = root.external_child(Some("a\u{c}b\u{c}c\u{c}d\u{c}e\u{c}f\u{c}g\u{c}h\u{c}i\u{c}j\u{c}k\u{c}l\u{c}m\u{c}n\u{c}o\u{c}p\u{c}q\u{c}r\u{c}s\u{c}t\u{c}u\u{c}v\u{c}w\u{c}x\u{c}a\u{c}%leaf"), None)?;
    drop(root);
    child.set_hash_salt([43; 16])?;
    child.feed(b"<r a='&leaf;&leaf;' b='&leaf;'/><r a='&leaf;'/>", true)?;
    let mut count = 0;
    while let Some(event) = next_event(&mut child)? {
        if let EventKind::StartElement { attributes, .. } = event.kind {
            assert_eq!(
                attributes[0].value,
                if count == 0 { "valuevalue" } else { "value" }
            );
            count += 1;
        }
    }
    assert_eq!(count, 2);
    Ok(())
}

#[test]
fn inherited_attribute_index_survives_parent_drop_rekey_and_every_allocation_failure() {
    check_allocations(inherited_attribute_workload);
}

fn shared_tables_workload(allocator: Allocator) -> Result<(), Error> {
    let mut parent = Parser::try_new_in(Config::default(), allocator)?;
    parent.feed(b"<!DOCTYPE r SYSTEM 'd'>", false)?;
    while next_event(&mut parent)?.is_some() {}
    let mut first = parent.external_child(None, None)?;
    let mut retained = parent.external_child(None, None)?;
    first.feed(b"<!ENTITY e 'E'><!ATTLIST r a CDATA 'A'>", false)?;
    while next_event(&mut first)?.is_some() {}
    let mut snapshot = parent.external_child(Some(""), None)?;
    // The hidden core setter rehashes the shared family plus this parser's local
    // indexes. A retained child's different local seed is coherent and usable.
    parent.set_hash_salt([17; 16])?;
    retained.set_hash_salt([29; 16])?;
    first.feed(b"<!", true)?;
    match next_event(&mut first) {
        Err(error) if error.kind == ErrorKind::UnclosedToken => {}
        Err(error) => return Err(error),
        _ => panic!("unfinished markup must fail"),
    }
    drop(first);
    drop(parent);
    retained.feed(
        b"<!ENTITY e 'duplicate'><!ENTITY x 'X'><!ATTLIST r b CDATA 'B'>",
        true,
    )?;
    while let Some(event) = next_event(&mut retained)? {
        if let EventKind::EntityDeclaration(declaration) = event.kind {
            assert_eq!(declaration.name, "x");
        }
    }
    let mut current = retained.external_child(Some(""), None)?;
    drop(retained);
    current.feed(b"<r>&e;&x;</r>", true)?;
    let mut text_count = 0;
    while let Some(event) = next_event(&mut current)? {
        match event.kind {
            EventKind::StartElement { attributes, .. } => {
                assert_eq!(attributes.len(), 2);
                assert_eq!(attributes[0].value, "A");
                assert_eq!(attributes[1].value, "B");
            }
            EventKind::Text(value) => {
                assert!(value == "E" || value == "X");
                text_count += 1;
            }
            _ => {}
        }
    }
    assert_eq!(text_count, 2);
    snapshot.feed(b"<r>&e;</r>", true)?;
    while let Some(event) = next_event(&mut snapshot)? {
        if let EventKind::StartElement { attributes, .. } = event.kind {
            assert_eq!(attributes.len(), 1);
            assert_eq!(attributes[0].value, "A");
        }
    }
    Ok(())
}

#[test]
fn shared_dtd_publication_snapshot_rehash_and_retained_siblings_survive_each_failure() {
    check_allocations(shared_tables_workload);
}

#[test]
fn output_slots_clear_at_every_selected_allocation_failure() {
    fn workload(allocator: Allocator) -> Result<(), Error> {
        let mut parser = Parser::try_new_in(
            Config {
                namespace_separator: Some('|'),
                ..Config::default()
            },
            allocator,
        )?;
        parser.feed(b"<!DOCTYPE r [<!ENTITY e 'expanded'><!ATTLIST r a CDATA 'default'>]><r xmlns:p='u'><p:child b='&e;'/><p:child b='another value'/></r>", true)?;
        let mut output = None;
        loop {
            match parser.next_event_for_recycling_into(&mut output) {
                Ok(Some(_)) => assert!(output.is_some()),
                Ok(None) => {
                    assert!(output.is_none());
                    break;
                }
                Err(error) => {
                    assert_eq!(error.kind, ErrorKind::NoMemory);
                    assert!(output.is_none());
                    let calls = CALLS.get();
                    assert_eq!(
                        parser
                            .next_event_for_recycling_into(&mut output)
                            .unwrap_err(),
                        error
                    );
                    assert!(output.is_none());
                    assert_eq!(calls, CALLS.get());
                    return Err(error);
                }
            }
        }
        Ok(())
    }
    check_allocations(workload);
}

#[test]
fn warmed_utf8_source_growth_preserves_selected_allocator_failures() {
    fn workload(allocator: Allocator) -> Result<(), Error> {
        let mut parser = Parser::try_new_in(Config::default(), allocator)?;
        parser.feed(b"<r>", false)?;
        // Repeated equal feeds warm pending while the unconsumed source grows.
        // Its later growth must remain fallible through the selected suite.
        for _ in 0..4 {
            parser.feed(&[b'x'; 1024], false)?;
        }
        parser.feed(b"</r>", true)?;
        let mut text_bytes = 0;
        while let Some(event) = next_event(&mut parser)? {
            if let EventKind::Text(value) = event.kind {
                text_bytes += value.len();
            }
        }
        assert_eq!(text_bytes, 4096);
        Ok(())
    }
    check_allocations(workload);
}

#[test]
fn direct_start_lowering_preserves_raw_and_every_selected_allocation_failure() {
    fn workload(allocator: Allocator) -> Result<(), Error> {
        let mut parser = Parser::try_new_in(Config::default(), allocator)?;
        parser.enable_input_context();
        // Warm only lexical records. The direct Start must still perform cold
        // frame, duplicate-table and stack-name allocations in the chosen suite.
        parser.feed(
            b"<r><w a='0' b='1' c='2' d='3' e='4' f='5' g='6' h='7' i='8'/>",
            false,
        )?;
        while next_event(&mut parser)?.is_some() {}
        let tag = "<longer a='zero' b='one' c='two' d='three' e='four' f='five' g='six' h='seven' i='eight'>";
        parser.feed(tag.as_bytes(), false)?;
        let mut frame = parser.adapter_frame();
        let result = (|| -> Result<(), Error> {
            let mut event = None;
            match parser.next_event_for_c_text_context_into(&mut event, &mut frame) {
                Ok(Some(_)) => {
                    assert!(event.is_none() && frame.is_active());
                    assert_eq!(frame.name_bytes(), b"longer\0");
                    assert_eq!(frame.attributes().count(), 9);
                    assert_eq!(parser.current_raw(), Some(tag));
                }
                Ok(None) => panic!("complete Start must be delivered"),
                Err(error) => {
                    assert_eq!(error.kind, ErrorKind::NoMemory);
                    assert!(event.is_none() && !frame.is_active());
                    assert_eq!(parser.current_raw(), Some(tag));
                    let calls = CALLS.get();
                    assert_eq!(
                        parser
                            .next_event_for_c_text_context_into(&mut event, &mut frame)
                            .unwrap_err(),
                        error
                    );
                    assert_eq!(parser.feed(b"ignored", true).unwrap_err(), error);
                    assert_eq!(CALLS.get(), calls);
                    return Err(error);
                }
            }
            parser.feed(b"</longer></r>", true)?;
            while next_event(&mut parser)?.is_some() {}
            Ok(())
        })();
        parser.finish_adapter_frame(frame);
        result
    }
    check_allocations(workload);
}

#[test]
fn native_raw_context_views_survive_every_feed_allocation_failure() {
    fn workload(allocator: Allocator) -> Result<(), Error> {
        // The first suffix forces native growth; the second forces Separate.
        let large = [b'x'; 65_536];
        for (input, raw, offset, suffix) in [
            (
                b"<r>twelve-bytes".as_slice(),
                "twelve-bytes",
                3,
                large.as_slice(),
            ),
            (
                b"<r>twelve-bytes".as_slice(),
                "twelve-bytes",
                3,
                b"\xff".as_slice(),
            ),
            (b"<r><n></n>".as_slice(), "</n>", 6, large.as_slice()),
            (b"<r><n></n>".as_slice(), "</n>", 6, b"\xff".as_slice()),
        ] {
            let mut parser = Parser::try_new_in(Config::default(), allocator)?;
            parser.enable_input_context();
            parser.feed(input, false)?;
            assert!(next_event(&mut parser)?.is_some());
            if raw == "</n>" {
                assert!(next_event(&mut parser)?.is_some());
            }
            let mut frame = parser.adapter_frame();
            let result = (|| -> Result<(), Error> {
                let mut event = None;
                let before = CALLS.get();
                assert!(
                    parser
                        .next_event_for_c_text_context_into(&mut event, &mut frame)?
                        .is_some()
                );
                if raw == "</n>" {
                    assert!(event.is_none());
                    assert_eq!(frame.take_end_name().unwrap(), "n");
                } else {
                    assert_eq!(frame.native_text_range_for_c(), Some((3, 12)));
                }
                assert_eq!(parser.current_raw(), Some(raw));
                assert_eq!(
                    CALLS.get(),
                    before,
                    "native raw projection needs no token allocation"
                );
                let (context, start) = parser.input_context();
                assert_eq!(
                    parser.current_raw().unwrap().as_ptr(),
                    context[offset - start..].as_ptr()
                );
                let fed = parser.feed(suffix, false);
                assert_eq!(parser.current_raw(), Some(raw));
                if let Err(error) = fed {
                    assert_eq!(error.kind, ErrorKind::NoMemory);
                    let calls = CALLS.get();
                    assert_eq!(parser.next_event().unwrap_err(), error);
                    assert_eq!(parser.feed(b"ignored", true).unwrap_err(), error);
                    assert_eq!(CALLS.get(), calls);
                    return Err(error);
                }
                // Decoder allocation errors are delivered by next_event even
                // when feed succeeds. Drain before accepting this workload.
                let invalid = loop {
                    match next_event(&mut parser) {
                        Ok(Some(_)) => {}
                        Ok(None) => break false,
                        Err(error)
                            if suffix == b"\xff" && error.kind == ErrorKind::InvalidToken =>
                        {
                            break true;
                        }
                        Err(error) => return Err(error),
                    }
                };
                assert_eq!(invalid, suffix == b"\xff");
                Ok(())
            })();
            parser.finish_adapter_frame(frame);
            result?;
        }
        Ok(())
    }
    check_allocations(workload);
}
