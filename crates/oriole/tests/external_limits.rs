//! Inheritance must consume the shared work budget before a child allocates.
use std::cell::Cell;
use std::ffi::c_void;

use oriole::{Config, ErrorKind, EventKind, Limits, Parser};
use oriole_storage::{Allocator, MemorySuite};

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}
unsafe extern "C" fn allocate(size: usize) -> *mut c_void {
    ALLOCATIONS.set(ALLOCATIONS.get() + 1);
    // SAFETY: Forward an arbitrary allocation size to the C allocator.
    unsafe { malloc(size) }
}
unsafe extern "C" fn reallocate(pointer: *mut c_void, size: usize) -> *mut c_void {
    ALLOCATIONS.set(ALLOCATIONS.get() + 1);
    // SAFETY: The storage allocator supplies a live block from this suite.
    unsafe { realloc(pointer, size) }
}
unsafe extern "C" fn deallocate(pointer: *mut c_void) {
    // SAFETY: The storage allocator supplies null or a live block from this suite.
    unsafe { free(pointer) };
}
fn parser(budget: usize) -> Parser {
    // SAFETY: The callbacks obey the C allocation contract and remain valid forever.
    let allocator = unsafe {
        Allocator::from_callbacks(MemorySuite {
            malloc: Some(allocate),
            realloc: Some(reallocate),
            free: Some(deallocate),
        })
        .unwrap()
    };
    Parser::try_new_in(
        Config {
            namespace_separator: Some('|'),
            limits: Limits {
                max_entity_expansion_bytes: budget,
                ..Limits::default()
            },
            ..Config::default()
        },
        allocator,
    )
    .unwrap()
}
fn drain(parser: &mut Parser) {
    while parser.next_event().unwrap().is_some() {}
}
fn assert_child_rejected_before_allocation(parent: &Parser, context: Option<&str>) {
    let before = ALLOCATIONS.get();
    assert_eq!(
        parent
            .external_child_with_encoding(context, None)
            .unwrap_err()
            .kind,
        ErrorKind::LimitExceeded
    );
    assert_eq!(ALLOCATIONS.get(), before);
}

#[test]
fn unused_inherited_declarations_are_charged_before_child_allocation() {
    let mut parent = parser(128);
    let mut document = String::from("<!DOCTYPE r [");
    for index in 0..10 {
        document.push_str(&format!("<!ENTITY x{index} '{}'>", "a".repeat(64)));
    }
    document.push_str("]><r>");
    parent.feed(document.as_bytes(), false).unwrap();
    drain(&mut parent);
    assert_child_rejected_before_allocation(&parent, Some(""));
    // Constructor rejection leaves the parent document usable.
    parent.feed(b"</r>", true).unwrap();
    drain(&mut parent);
}

#[test]
fn structural_and_index_work_counts_even_for_empty_values() {
    let mut baseline = parser(1024);
    baseline.feed(b"<r>", false).unwrap();
    drain(&mut baseline);
    assert!(
        baseline
            .external_child_with_encoding(Some(""), None)
            .is_ok()
    );

    for family in 0..4 {
        let mut parent = parser(1024);
        let mut document = String::from("<!DOCTYPE r [");
        for index in 0..16 {
            match family {
                0 => document.push_str(&format!("<!ENTITY x{index} ''>")),
                1 => document.push_str(&format!("<!ENTITY % x{index} ''>")),
                2 => document.push_str(&format!("<!ATTLIST r a{index} CDATA #IMPLIED>")),
                _ => {}
            }
        }
        document.push_str("]><r");
        if family == 3 {
            for index in 0..16 {
                document.push_str(&format!(" xmlns:p{index}='u'"));
            }
        }
        document.push('>');
        parent.feed(document.as_bytes(), false).unwrap();
        drain(&mut parent);
        assert_child_rejected_before_allocation(&parent, Some(""));
    }
}

#[test]
fn repeated_empty_external_references_share_constructor_cost() {
    let mut parent = parser(16 * 1024);
    let mut document = String::from("<!DOCTYPE r [");
    for index in 0..10 {
        document.push_str(&format!("<!ENTITY x{index} '{}'>", "a".repeat(64)));
    }
    document.push_str("<!ENTITY e SYSTEM 's'>]><r>");
    document.push_str(&"&e;".repeat(32));
    document.push_str("</r>");
    parent.feed(document.as_bytes(), true).unwrap();
    let mut children = 0;
    let mut rejected = false;
    while let Some(event) = parent.next_event().unwrap() {
        if let EventKind::ExternalEntityReference { context, .. } = event.kind {
            let before = ALLOCATIONS.get();
            match parent.external_child_with_encoding(context.as_deref(), None) {
                Ok(mut child) => {
                    child.feed(b"", true).unwrap();
                    drain(&mut child);
                    children += 1;
                }
                Err(error) => {
                    assert_eq!(error.kind, ErrorKind::LimitExceeded);
                    assert_eq!(ALLOCATIONS.get(), before);
                    rejected = true;
                    break;
                }
            }
        }
    }
    assert!(rejected);
    assert!((1..32).contains(&children));
}

#[test]
fn context_entry_work_is_charged_before_constructor_allocation() {
    let parent = parser(1024);
    let context = (0..32)
        .map(|index| format!("p{index}=u\u{c}"))
        .collect::<String>();
    assert_child_rejected_before_allocation(&parent, Some(&context));
}

#[test]
fn inherited_custom_encoding_map_is_charged_before_copying() {
    let mut parent = parser(1024);
    parent.set_encoding(Some("custom-map")).unwrap();
    parent.feed(b"<r>", false).unwrap();
    assert_eq!(
        parent.next_event().unwrap_err().kind,
        ErrorKind::UnknownEncoding
    );
    parent
        .set_encoding_map("custom-map", std::array::from_fn(|index| index as i32))
        .unwrap();
    drain(&mut parent);
    // A different encoding does not clone the custom conversion map.
    assert!(
        parent
            .external_child_with_encoding(Some(""), Some("UTF-8"))
            .is_ok()
    );
    let before = ALLOCATIONS.get();
    assert_eq!(
        parent
            .external_child_with_encoding(Some(""), Some("CUSTOM-MAP"))
            .unwrap_err()
            .kind,
        ErrorKind::LimitExceeded
    );
    assert_eq!(ALLOCATIONS.get(), before);
}

#[test]
fn context_entity_chain_depth_is_bounded() {
    let parent = parser(64 * 1024);
    let context = (0..100)
        .map(|index| format!("entity{index}\u{c}"))
        .collect::<String>();
    assert_eq!(
        parent
            .external_child_with_encoding(Some(&context), None)
            .unwrap_err()
            .kind,
        ErrorKind::LimitExceeded
    );
}
