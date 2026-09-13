use super::*;

#[derive(Default)]
struct Bases {
    calls: std::vec::Vec<(&'static str, Option<std::vec::Vec<u8>>)>,
    trigger: Option<&'static [u8]>,
    in_entity: bool,
    changed: bool,
}

unsafe fn record(parser: XML_Parser, kind: &'static str, base: *const c_char) {
    // SAFETY: Test callbacks receive a live parser and callback-scoped C string;
    // the copied bytes outlive the callback, and no state borrow spans reentry.
    unsafe {
        let state = XML_GetUserData(parser).cast::<Bases>();
        (*state).calls.push((
            kind,
            (!base.is_null()).then(|| CStr::from_ptr(base).to_bytes().to_vec()),
        ));
    }
}

unsafe extern "C" fn entity(
    arg: *mut c_void,
    name: *const c_char,
    _: c_int,
    _: *const c_char,
    _: c_int,
    base: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
) {
    // SAFETY: Parser-as-handler-argument and declaration strings are live here.
    unsafe {
        if CStr::from_ptr(name).to_bytes() == b"ext" {
            record(arg.cast(), "declaration", base);
        }
    }
}

unsafe extern "C" fn unparsed(
    arg: *mut c_void,
    _: *const c_char,
    base: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
) {
    // SAFETY: The callback borrows a live parser and its declaration metadata.
    unsafe { record(arg.cast(), "unparsed", base) };
}

unsafe extern "C" fn external(
    parser: XML_Parser,
    context: *const c_char,
    base: *const c_char,
    system: *const c_char,
    _: *const c_char,
) -> c_int {
    // SAFETY: Parent and child handles remain live during synchronous parsing.
    // Child callbacks share the test state, without retaining a mutable borrow.
    unsafe {
        if !system.is_null() && CStr::from_ptr(system).to_bytes() == b"parameter" {
            let child = XML_ExternalEntityParserCreate(parser, context, ptr::null());
            assert!(!child.is_null());
            assert_eq!(XML_SetBase(child, c"/changed/\xff".as_ptr()), OK);
            let result = XML_Parse(child, c"%ext;".as_ptr(), 5, 1);
            XML_ParserFree(child);
            return result;
        }
        record(parser, "external", base);
        OK
    }
}

unsafe extern "C" fn default(arg: *mut c_void, text: *const c_char, length: c_int) {
    // SAFETY: The raw token and parser-as-handler-argument remain valid here.
    unsafe {
        let token = std::slice::from_raw_parts(text.cast::<u8>(), length as usize);
        let state = XML_GetUserData(arg.cast()).cast::<Bases>();
        if token == b"<!ENTITY" {
            (*state).in_entity = true;
        }
        if (*state).in_entity && !(*state).changed && (*state).trigger == Some(token) {
            (*state).changed = true;
            assert_eq!(XML_SetBase(arg.cast(), c"/changed/\xff".as_ptr()), OK);
        }
        if token == b">" {
            (*state).in_entity = false;
        }
    }
}

unsafe fn configure(parser: XML_Parser, state: &mut Bases, initial: Option<&CStr>) {
    // SAFETY: Each test keeps its state, strings, and parser live until free.
    unsafe {
        assert!(!parser.is_null());
        XML_SetUserData(parser, ptr::from_mut(state).cast());
        XML_UseParserAsHandlerArg(parser);
        assert_eq!(
            XML_SetBase(parser, initial.map_or(ptr::null(), CStr::as_ptr)),
            OK
        );
        assert_eq!(XML_SetParamEntityParsing(parser, 2), 1);
        XML_SetExternalEntityRefHandler(parser, Some(external));
    }
}

#[test]
fn declaration_bases_survive_set_base_after_the_system_literal() {
    // SAFETY: Test-owned inputs and callback state outlive both parse calls.
    unsafe {
        for initial in [None, Some(c""), Some(c"/original/\xfe")] {
            for (prefix, suffix, unparsed_entity) in [
                (
                    "<!DOCTYPE r [<!ENTITY ext SYSTEM 'file'  ",
                    ">]><r>&ext;</r>",
                    false,
                ),
                (
                    "<!DOCTYPE r [<!ENTITY % ext SYSTEM 'file'  ",
                    ">%ext;]><r/>",
                    false,
                ),
                (
                    "<!DOCTYPE r [<!NOTATION n SYSTEM 'n'><!ENTITY ext SYSTEM 'file'  ",
                    "NDATA n>]><r/>",
                    true,
                ),
            ] {
                let parser = XML_ParserCreate(ptr::null());
                let mut state = Bases::default();
                configure(parser, &mut state, initial);
                XML_SetEntityDeclHandler(parser, Some(entity));
                XML_SetUnparsedEntityDeclHandler(parser, Some(unparsed));
                assert_eq!(
                    XML_Parse(parser, prefix.as_ptr().cast(), prefix.len() as c_int, 0),
                    OK
                );
                assert_eq!(XML_SetBase(parser, c"/changed/\xff".as_ptr()), OK);
                assert_eq!(
                    XML_Parse(parser, suffix.as_ptr().cast(), suffix.len() as c_int, 1),
                    OK
                );
                let base = initial.map(|value| value.to_bytes().to_vec());
                let expected = if unparsed_entity {
                    vec![("unparsed", base)]
                } else {
                    vec![("declaration", base.clone()), ("external", base)]
                };
                assert_eq!(state.calls, expected, "{prefix}");
                XML_ParserFree(parser);
            }
        }
    }
}

#[test]
fn default_callbacks_change_only_bases_not_yet_captured() {
    // SAFETY: All callback strings and the parser remain live during each parse.
    unsafe {
        for trigger in [b"SYSTEM".as_slice(), b"'file'"] {
            for unparsed_entity in [false, true] {
                let document = if unparsed_entity {
                    "<!DOCTYPE r [<!NOTATION n SYSTEM 'n'><!ENTITY ext SYSTEM 'file' NDATA n>]><r/>"
                } else {
                    "<!DOCTYPE r [<!ENTITY ext SYSTEM 'file'>]><r>&ext;</r>"
                };
                let parser = XML_ParserCreate(ptr::null());
                let mut state = Bases {
                    trigger: Some(trigger),
                    ..Bases::default()
                };
                configure(parser, &mut state, None);
                XML_SetDefaultHandler(parser, Some(default));
                XML_SetUnparsedEntityDeclHandler(parser, Some(unparsed));
                assert_eq!(
                    XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                    OK
                );
                assert!(state.changed);
                assert_eq!(
                    state.calls,
                    vec![(
                        if unparsed_entity {
                            "unparsed"
                        } else {
                            "external"
                        },
                        (trigger == b"SYSTEM").then(|| b"/changed/\xff".to_vec()),
                    )]
                );
                XML_ParserFree(parser);
            }
        }
    }
}

#[test]
fn reserved_parameter_requests_do_not_use_foreign_dtd_base_rules() {
    unsafe extern "C" fn change_base(
        arg: *mut c_void,
        _: *const c_char,
        _: *const c_char,
        _: *const c_char,
        _: c_int,
    ) {
        // SAFETY: Parser-as-handler-argument supplies the active live parser.
        unsafe { assert_eq!(XML_SetBase(arg.cast(), c"/changed/\xff".as_ptr()), OK) };
    }
    // SAFETY: The DTD child, parent, state, and declaration bytes remain live.
    unsafe {
        for declaration in [
            "<!ENTITY % ext %p; SYSTEM 'file'>",
            "<!ENTITY % ext PUBLIC 'pub' %p; 'file'>",
        ] {
            let parent = XML_ParserCreate(ptr::null());
            let parser = XML_ExternalEntityParserCreate(parent, ptr::null(), ptr::null());
            let mut state = Bases::default();
            configure(parser, &mut state, Some(c"/original"));
            let document = format!("<!ENTITY % p SYSTEM 'parameter'>{declaration}");
            assert_eq!(
                XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
                OK
            );
            assert_eq!(state.calls, [("external", None)]);
            XML_ParserFree(parser);
            XML_ParserFree(parent);
        }
        let parser = XML_ParserCreate(ptr::null());
        let mut state = Bases::default();
        configure(parser, &mut state, Some(c"/original"));
        assert_eq!(XML_UseForeignDTD(parser, 1), 0);
        XML_SetStartDoctypeDeclHandler(parser, Some(change_base));
        let document = b"<!DOCTYPE r><r/>";
        assert_eq!(
            XML_Parse(parser, document.as_ptr().cast(), document.len() as c_int, 1),
            OK
        );
        assert_eq!(state.calls, [("external", Some(b"/changed/\xff".to_vec()))]);
        XML_ParserFree(parser);
    }
}
