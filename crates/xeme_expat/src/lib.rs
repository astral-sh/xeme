//! The narrow-character Expat C ABI.
//!
//! # Safety
//!
//! As with Expat, callers must provide live parser handles, valid buffers and
//! callbacks with the declared ABI, and serialize access to each parser family. A
//! callback may change handlers or stop parsing. Recursive parsing of the same
//! parser is rejected without changing its error state. Callback-time Free is
//! ignored; the caller must free the parser after the outer operation returns.
//! No Rust parser reference crosses an event callback; allocation callbacks cannot
//! reenter parser APIs.
#![allow(non_snake_case, non_camel_case_types)]
#![allow(clippy::missing_safety_doc)] // The common C ABI contract is documented above.

use std::cell::Cell;
use std::ffi::{CStr, c_char, c_int, c_long, c_ulong, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

use xeme::{Config, ErrorKind, EventKind, NameRules, Parser, Position, RecyclingToken};
use xeme_storage::{
    AllocError, AllocationTracker, Allocator, Box as XmlBox, CString, MemorySuite, Queue, Shared,
    String as XmlString, Vec as XmlVec, in_allocator_callback, with_tracking, without_tracking,
};

mod content_model;

const OK: c_int = 1;
const ERROR: c_int = 0;
const SUSPENDED: c_int = 2;
const INVALID_ARGUMENT: c_int = 41;
const UNEXPECTED_STATE: c_int = 23;
const INITIAL_CALLBACK_BYTES: usize = 64 * 1024 * 1024;
const MAX_FAMILY_CHILDREN: usize = 1024;
const MAX_EXTERNAL_DEPTH: usize = 32;
const INPUT_CONTEXT_BYTES: usize = 1024;
// Keep individual input requests bounded independently of document length.
const MAX_INPUT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Default)]
struct FamilyBudget {
    // Checked cumulative statistic only; sibling input grants no source room or
    // work credit. The core owns each source's length and consumed-root budget.
    input_bytes: Cell<usize>,
    callback_bytes: Cell<usize>,
    children: Cell<usize>,
}

fn charge(counter: &Cell<usize>, amount: usize, limit: usize) -> bool {
    // The C contract serializes the entire parser family. No callback or
    // allocation can run between this counter read and its successful update.
    let Some(next) = counter
        .get()
        .checked_add(amount)
        .filter(|&next| next <= limit)
    else {
        return false;
    };
    counter.set(next);
    true
}

pub type XML_Parser = *mut XML_ParserStruct;
type StartElement = Option<unsafe extern "C" fn(*mut c_void, *const c_char, *const *const c_char)>;
type StringHandler = Option<unsafe extern "C" fn(*mut c_void, *const c_char)>;
type TextHandler = Option<unsafe extern "C" fn(*mut c_void, *const c_char, c_int)>;
type PairHandler = Option<unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char)>;
type VoidHandler = Option<unsafe extern "C" fn(*mut c_void)>;
type XmlDecl = Option<unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, c_int)>;
type Doctype =
    Option<unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char, c_int)>;
type EntityDecl = Option<
    unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        c_int,
        *const c_char,
        c_int,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
    ),
>;
type AttlistDecl = Option<
    unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        c_int,
    ),
>;
type UnparsedDecl = Option<
    unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
    ),
>;
type NotationDecl = Option<
    unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char, *const c_char),
>;
type ExternalEntity = Option<
    unsafe extern "C" fn(
        XML_Parser,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> c_int,
>;
type SkippedEntity = Option<unsafe extern "C" fn(*mut c_void, *const c_char, c_int)>;
type NotStandalone = Option<unsafe extern "C" fn(*mut c_void) -> c_int>;
type ElementDecl = Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut XML_Content)>;
type UnknownEncoding =
    Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut XML_Encoding) -> c_int>;

#[derive(Default, Clone, Copy)]
struct Handlers {
    start_element: StartElement,
    end_element: StringHandler,
    text: TextHandler,
    pi: PairHandler,
    comment: StringHandler,
    start_cdata: VoidHandler,
    end_cdata: VoidHandler,
    xml_decl: XmlDecl,
    start_doctype: Doctype,
    end_doctype: VoidHandler,
    start_namespace: PairHandler,
    end_namespace: StringHandler,
    entity_decl: EntityDecl,
    attlist_decl: AttlistDecl,
    default: TextHandler,
    default_expand: bool,
    unparsed: UnparsedDecl,
    notation: NotationDecl,
    external: ExternalEntity,
    skipped: SkippedEntity,
    not_standalone: NotStandalone,
    element_decl: ElementDecl,
    unknown_encoding: UnknownEncoding,
}

/// Opaque C parser. `user_data` must remain first: Expat's XML_GetUserData is a macro.
#[repr(C)]
pub struct XML_ParserStruct {
    user_data: *mut c_void,
    allocator: Allocator,
    tracker: Shared<AllocationTracker>,
    core: Parser,
    config: Config,
    handlers: Handlers,
    handler_arg_is_parser: bool,
    busy: bool,
    destroying: bool,
    state: c_int,
    error: c_int,
    parse_error: c_int,
    final_buffer: bool,
    position: Position,
    specified_attributes: c_int,
    base: Option<CString>,
    buffer: XmlVec<u8>,
    buffer_available: bool,
    input_context_active: bool,
    external_arg: *mut c_void,
    unknown_encoding_arg: *mut c_void,
    default_dispatch: bool,
    attlist_dispatch: bool,
    default_pending: Queue<XmlString>,
    doctype_close_handled: bool,
    family: Shared<FamilyBudget>,
    child_depth: usize,
    encoding_release: Option<unsafe extern "C" fn(*mut c_void)>,
    encoding_convert: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int>,
    encoding_data: *mut c_void,
    lifetime: Shared<AtomicPtr<XML_ParserStruct>>,
    parent_lifetime: Option<Shared<AtomicPtr<XML_ParserStruct>>>,
    external_subset_merged: bool,
}

#[repr(C)]
pub struct XML_Memory_Handling_Suite {
    pub malloc_fcn: Option<unsafe extern "C" fn(usize) -> *mut c_void>,
    pub realloc_fcn: Option<unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void>,
    pub free_fcn: Option<unsafe extern "C" fn(*mut c_void)>,
}

#[repr(C)]
pub struct XML_Content {
    pub kind: c_int,
    pub quant: c_int,
    pub name: *mut c_char,
    pub numchildren: u32,
    pub children: *mut XML_Content,
}

#[repr(C)]
pub struct XML_Encoding {
    pub map: [c_int; 256],
    pub data: *mut c_void,
    pub convert: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int>,
    pub release: Option<unsafe extern "C" fn(*mut c_void)>,
}

#[repr(C)]
pub struct XML_ParsingStatus {
    pub parsing: c_int,
    pub finalBuffer: u8,
}

#[repr(C)]
pub struct XML_Expat_Version {
    pub major: c_int,
    pub minor: c_int,
    pub micro: c_int,
}

fn error_code(kind: &ErrorKind) -> c_int {
    match kind {
        ErrorKind::NoMemory => 1,
        ErrorKind::Syntax => 2,
        ErrorKind::NoElements => 3,
        ErrorKind::InvalidToken => 4,
        ErrorKind::UnclosedToken => 5,
        ErrorKind::PartialCharacter => 6,
        ErrorKind::TagMismatch => 7,
        ErrorKind::DuplicateAttribute => 8,
        ErrorKind::JunkAfterDocumentElement => 9,
        ErrorKind::ParameterEntityReference => 10,
        ErrorKind::UndefinedEntity => 11,
        ErrorKind::RecursiveEntityReference => 12,
        ErrorKind::AsynchronousEntity => 13,
        ErrorKind::IncompleteParameterEntity => 29,
        ErrorKind::BadCharacterReference => 14,
        ErrorKind::BinaryEntityReference => 15,
        ErrorKind::ExternalEntityInAttribute => 16,
        ErrorKind::MisplacedXmlDeclaration => 17,
        ErrorKind::XmlDeclaration => 30,
        ErrorKind::TextDeclaration => 31,
        ErrorKind::PublicId => 32,
        ErrorKind::UndeclaringPrefix => 28,
        ErrorKind::UnknownEncoding => 18,
        ErrorKind::IncorrectEncoding => 19,
        ErrorKind::UnclosedCdataSection => 20,
        ErrorKind::ExternalEntityHandling => 21,
        ErrorKind::EntityDeclaredInParameterEntity => 24,
        ErrorKind::UndefinedPrefix => 27,
        ErrorKind::ReservedPrefixXml => 38,
        ErrorKind::ReservedPrefixXmlns => 39,
        ErrorKind::ReservedNamespaceUri => 40,
        ErrorKind::LimitExceeded => 43,
        ErrorKind::Finished => 36,
    }
}

/// Borrow a nullable C string only for the duration of the enclosing C call.
unsafe fn input_string<'a>(value: *const c_char) -> Result<Option<&'a str>, AllocError> {
    if value.is_null() {
        return Ok(None);
    }
    // SAFETY: The caller keeps its readable NUL-terminated string live for this call.
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map(Some)
        .map_err(|_| AllocError::InteriorNul)
}

fn cstring(value: XmlString) -> Result<CString, AllocError> {
    CString::try_from_string(value)
}
fn optional_cstring(value: Option<XmlString>) -> Result<Option<CString>, AllocError> {
    value.map(cstring).transpose()
}
fn cptr(value: &Option<CString>) -> *const c_char {
    value.as_ref().map_or(ptr::null(), CString::as_ptr)
}

/// Terminate opaque base metadata while its owner remains live across the callback.
fn base_cptr(base: &mut Option<XmlVec<u8>>) -> Result<*const c_char, AllocError> {
    if let Some(base) = base {
        base.try_reserve(1)?;
        base.push(0);
        Ok(base.as_ptr().cast())
    } else {
        Ok(ptr::null())
    }
}

unsafe fn create(
    encoding: *const c_char,
    separator: Option<char>,
    allocator: Allocator,
) -> XML_Parser {
    if in_allocator_callback() || separator.is_some_and(|separator| !separator.is_ascii()) {
        return ptr::null_mut();
    }
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<XML_Parser, AllocError> {
        let allocator = allocator.trackable();
        let tracker = AllocationTracker::try_new_in(allocator)?;
        with_tracking(&tracker, || -> Result<XML_Parser, AllocError> {
            // SAFETY: Input is borrowed only until the fallible constructor copies it.
            let encoding = unsafe { input_string(encoding)? };
            let config = Config {
                namespace_separator: separator,
                name_rules: NameRules::FourthEdition,
                // Iterative expansion supports Expat's deep-entity workloads.
                // Shared work, live-allocation and external-child bounds still apply.
                limits: xeme::Limits {
                    // The core also clamps source offsets to isize::MAX. Keep
                    // individual requests and retained allocations independent.
                    max_total_bytes: c_long::MAX as usize,
                    max_work_amplification: Some(100),
                    max_entities: 100_000,
                    max_entity_depth: 100_000,
                    ..xeme::Limits::default()
                },
                ..Config::default()
            };
            let mut core = Parser::try_new_with_encoding_in(config.clone(), encoding, allocator)
                .map_err(|_| AllocError::OutOfMemory)?;
            core.enable_input_context();
            core.set_notation_handler_enabled(false);
            core.set_attlist_handler_enabled(false);
            let position = core.position();
            let family = Shared::try_new_in(FamilyBudget::default(), allocator)?;
            let lifetime = Shared::try_new_in(AtomicPtr::new(ptr::null_mut()), allocator)?;
            let parser = XmlBox::try_new_in(
                XML_ParserStruct {
                    user_data: ptr::null_mut(),
                    allocator,
                    tracker: Shared::clone(&tracker),
                    core,
                    config,
                    handlers: Handlers::default(),
                    handler_arg_is_parser: false,
                    busy: false,
                    destroying: false,
                    state: 0,
                    error: 0,
                    parse_error: 0,
                    final_buffer: false,
                    position,
                    specified_attributes: 0,
                    base: None,
                    buffer: XmlVec::new_in(allocator),
                    buffer_available: false,
                    input_context_active: false,
                    external_arg: ptr::null_mut(),
                    unknown_encoding_arg: ptr::null_mut(),
                    default_dispatch: false,
                    attlist_dispatch: false,
                    default_pending: Queue::new_in(allocator),
                    doctype_close_handled: false,
                    family,
                    child_depth: 0,
                    encoding_release: None,
                    encoding_convert: None,
                    encoding_data: ptr::null_mut(),
                    lifetime,
                    parent_lifetime: None,
                    external_subset_merged: false,
                },
                allocator,
            )?;
            Ok(activate_handle(parser))
        })
    }));
    result.ok().and_then(Result::ok).unwrap_or(ptr::null_mut())
}

unsafe fn with_parser_tracking<R>(parser: XML_Parser, operation: impl FnOnce() -> R) -> R {
    if parser.is_null() || in_allocator_callback() {
        return operation();
    }
    // SAFETY: Retain a local owning tracker clone, never a parser-field reference,
    // across callbacks. The operation checks its normal parser-state preconditions.
    let tracker = unsafe { Shared::clone(&(*parser).tracker) };
    with_tracking(&tracker, operation)
}

fn activate_handle(parser: XmlBox<XML_ParserStruct>) -> XML_Parser {
    let parser = XmlBox::into_raw(parser);
    // SAFETY: The new handle is exclusively owned and not yet visible to C.
    unsafe {
        let lifetime = &(*parser).lifetime;
        lifetime.store(parser, Ordering::Release);
    }
    parser
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_ParserCreate(encoding: *const c_char) -> XML_Parser {
    // SAFETY: Forward the C string contract to create.
    unsafe { create(encoding, None, Allocator::System) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_ParserCreateNS(
    encoding: *const c_char,
    separator: c_char,
) -> XML_Parser {
    // SAFETY: Forward the C string contract to create.
    unsafe {
        create(
            encoding,
            Some(char::from(separator as u8)),
            Allocator::System,
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_ParserCreate_MM(
    encoding: *const c_char,
    suite: *const XML_Memory_Handling_Suite,
    separator: *const c_char,
) -> XML_Parser {
    if in_allocator_callback() {
        return ptr::null_mut();
    }
    // SAFETY: The caller provides a readable suite and guarantees its C allocation
    // functions remain valid for every parser/model/child allocation's lifetime.
    let allocator = unsafe {
        if suite.is_null() {
            Allocator::System
        } else {
            let Ok(allocator) = Allocator::from_callbacks(MemorySuite {
                malloc: (*suite).malloc_fcn,
                realloc: (*suite).realloc_fcn,
                free: (*suite).free_fcn,
            }) else {
                return ptr::null_mut();
            };
            allocator
        }
    };
    // SAFETY: A non-null separator points to one readable byte.
    let separator = if separator.is_null() {
        None
    } else {
        // SAFETY: A non-null separator points to one readable byte.
        Some(char::from(unsafe { *separator } as u8))
    };
    // SAFETY: Forward the caller's string lifetime and validated allocator contract.
    unsafe { create(encoding, separator, allocator) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_ParserFree(parser: XML_Parser) {
    // SAFETY: The scope owns a tracker clone and rejects allocator-callback reentry
    // before reading parser state; all temporary allocations use the same family.
    let operation = || {
        if parser.is_null() || in_allocator_callback() {
            return;
        }
        // SAFETY: Caller provides a live, exclusively accessed handle. Busy parsers
        // remain allocated until dispatch returns; no callback owns a Rust reference.
        unsafe {
            if !(*parser).busy {
                destroy(parser);
            }
        }
    };
    // SAFETY: The operation checks the handle before use; its tracker clone owns
    // the accounting context even when an operation destroys the parser.
    unsafe { with_parser_tracking(parser, operation) }
}

/// Record a processor failure separately from a recoverable API diagnostic.
unsafe fn fail_parse(parser: XML_Parser, error: c_int) {
    // SAFETY: The caller owns the live handle and no parser reference crosses a callback.
    unsafe {
        (*parser).parse_error = error;
        (*parser).error = error;
    }
}

unsafe fn release_encoding(parser: XML_Parser) {
    // SAFETY: Take ownership of the release callback before invoking it, so
    // callback reentry cannot release the same encoding data a second time.
    unsafe {
        let release = (*parser).encoding_release.take();
        (*parser).encoding_convert = None;
        let data = std::mem::replace(&mut (*parser).encoding_data, ptr::null_mut());
        if let Some(release) = release {
            release(data);
        }
    }
}

unsafe fn destroy(parser: XML_Parser) {
    // SAFETY: Final destruction owns the live handle. A release callback sees a
    // busy parser, so recursive Free is ignored during this single drop.
    unsafe {
        (*parser).busy = true;
        (*parser).destroying = true;
        {
            let lifetime = &(*parser).lifetime;
            lifetime.store(ptr::null_mut(), Ordering::Release);
        }
        release_encoding(parser);
        let allocator = (*parser).allocator;
        drop(XmlBox::from_raw_in(parser, allocator));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_ParserReset(parser: XML_Parser, encoding: *const c_char) -> u8 {
    // SAFETY: The scope owns a tracker clone and rejects allocator-callback reentry
    // before reading parser state; all temporary allocations use the same family.
    let operation = || {
        if parser.is_null() || in_allocator_callback() {
            return 0;
        }
        // SAFETY: Guard the optional encoding-release callback like other C callbacks.
        unsafe {
            if (*parser).busy {
                return 0;
            }
            if (*parser).child_depth != 0 {
                return 0;
            }
            (*parser).busy = true;
        }
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<c_int, AllocError> {
            // SAFETY: Build replacement storage before releasing callback data or
            // changing parser fields. Allocator callbacks cannot reenter any C API.
            unsafe {
                let allocator = (*parser).allocator;
                let encoding = input_string(encoding)?
                    .map(|value| XmlString::try_from_str_in(value, allocator))
                    .transpose()?;
                let mut core = Parser::try_new_with_encoding_in(
                    (*parser).config.clone(),
                    encoding.as_deref(),
                    allocator,
                )
                .map_err(|_| AllocError::OutOfMemory)?;
                core.enable_input_context();
                core.set_hash_salt((*parser).core.hash_salt())
                    .map_err(|_| AllocError::OutOfMemory)?;
                let family = Shared::try_new_in(FamilyBudget::default(), allocator)?;
                let lifetime = Shared::try_new_in(AtomicPtr::new(parser), allocator)?;
                release_encoding(parser);
                if (*parser).destroying {
                    return Ok(ERROR);
                }
                {
                    // Old DTD children belong to the previous document. Invalidate
                    // their merge destination before replacing the parent's core.
                    let old_lifetime = &(*parser).lifetime;
                    old_lifetime.store(ptr::null_mut(), Ordering::Release);
                }
                (*parser).lifetime = lifetime;
                core.set_notation_handler_enabled(false);
                core.set_attlist_handler_enabled(false);
                (*parser).core = core;
                (*parser).position = (*parser).core.position();
                let unknown_encoding = (*parser).handlers.unknown_encoding;
                (*parser).handlers = Handlers {
                    unknown_encoding,
                    ..Handlers::default()
                };
                (*parser).user_data = ptr::null_mut();
                (*parser).handler_arg_is_parser = false;
                (*parser).state = 0;
                (*parser).error = 0;
                (*parser).parse_error = 0;
                (*parser).final_buffer = false;
                (*parser).doctype_close_handled = false;
                (*parser).attlist_dispatch = false;
                (*parser).specified_attributes = 0;
                (*parser).base = None;
                (*parser).buffer.clear();
                (*parser).buffer_available = false;
                (*parser).external_arg = ptr::null_mut();
                while (*parser).default_pending.pop_front().is_some() {}
                (*parser).family = family;
                let tracker = &(*parser).tracker;
                tracker.reset_direct_bytes();
            }
            Ok(OK)
        }));
        // SAFETY: Reset owns the busy guard until completion.
        unsafe { u8::from(finish_fallible_operation(parser, result) == OK) }
    };
    // SAFETY: The operation checks the handle before use; its tracker clone owns
    // the accounting context even when an operation destroys the parser.
    unsafe { with_parser_tracking(parser, operation) }
}

/// Run callbacks after releasing all references to the opaque parser.
unsafe fn dispatch(
    parser: XML_Parser,
    kind: EventKind,
    recycling: RecyclingToken,
) -> Result<(), AllocError> {
    // A foreign DTD has no declared external identifier. Expat uses the current
    // base when requesting it, including changes in an earlier doctype callback.
    let foreign_dtd = matches!(&kind, EventKind::ExternalEntityReference(_))
        // SAFETY: The guarded parser owns the marker for its current event.
        && unsafe { (*parser).core.is_foreign_dtd_reference() };
    let needs_base = foreign_dtd
        || matches!(&kind, EventKind::NotationDeclaration(_))
        || matches!(&kind, EventKind::EntityDeclaration(declaration) if declaration.value.is_some());
    // SAFETY: Parser is pinned by the busy flag until the outer parse exits.
    // Copies and owned strings are the only values retained across callbacks.
    let (h, arg, base_bytes) = unsafe {
        (
            (*parser).handlers,
            if (*parser).handler_arg_is_parser {
                parser.cast()
            } else {
                (*parser).user_data
            },
            if needs_base {
                (*parser)
                    .base
                    .as_ref()
                    .map_or(0, |base| base.as_c_str().to_bytes().len())
            } else {
                0
            },
        )
    };
    let optional_len = |value: &Option<XmlString>| value.as_ref().map_or(0, |value| value.len());
    let callback_bytes = match &kind {
        EventKind::Default
        | EventKind::DoctypeClosingPrefix
        | EventKind::EntityDeclarationPrefix
        | EventKind::AttlistDeclarationPrefix
        | EventKind::ElementDeclarationPrefix
        | EventKind::NotationDeclarationPrefix
        | EventKind::EntityDeclarationDuplicate { .. } => {
            // SAFETY: Only the copied length survives this read, before any callback.
            unsafe { (*parser).core.current_raw().map_or(0, str::len) }
        }
        EventKind::StartElement { name, attributes } => {
            name.len()
                + attributes
                    .iter()
                    .map(|a| a.name.len() + a.value.len())
                    .sum::<usize>()
        }
        EventKind::EndElement { name } | EventKind::SkippedEntity { name, .. } => name.len(),
        EventKind::Text(value) => value.len(),
        EventKind::Comment(value) => value.len(),
        EventKind::ProcessingInstruction { target, data } => target.len() + data.len(),
        EventKind::ElementDeclaration { name, model } => name.len() + model.len(),
        EventKind::XmlDeclaration {
            version, encoding, ..
        } => version.len() + optional_len(encoding),
        EventKind::TextDeclaration { version, encoding } => optional_len(version) + encoding.len(),
        EventKind::ExternalEntityReference(reference) => {
            reference.base.as_ref().map_or(0, |base| base.len())
                + optional_len(&reference.context)
                + optional_len(&reference.system_id)
                + optional_len(&reference.public_id)
        }
        EventKind::StartDoctype(declaration) => {
            declaration.name.len()
                + optional_len(&declaration.system_id)
                + optional_len(&declaration.public_id)
        }
        EventKind::NotationDeclaration(declaration) => {
            declaration.name.len()
                + optional_len(&declaration.system_id)
                + optional_len(&declaration.public_id)
        }
        EventKind::StartNamespace { prefix, uri } => optional_len(prefix) + optional_len(uri),
        EventKind::EndNamespace { prefix } => optional_len(prefix),
        EventKind::EntityDeclaration(declaration) => {
            declaration.base.as_ref().map_or(0, |base| base.len())
                + declaration.name.len()
                + optional_len(&declaration.value)
                + optional_len(&declaration.system_id)
                + optional_len(&declaration.public_id)
                + optional_len(&declaration.notation)
        }
        EventKind::AttlistDeclaration(declaration) => {
            declaration.element.len()
                + declaration.name.len()
                + declaration.attribute_type.len()
                + optional_len(&declaration.default)
        }
        EventKind::StartCdata
        | EventKind::EndCdata
        | EventKind::EndDoctype
        | EventKind::NotStandalone => 0,
    };
    // SAFETY: Family access is serialized; charging never invokes user code.
    unsafe {
        let family = &(*parser).family;
        if !charge(
            &family.callback_bytes,
            callback_bytes + base_bytes,
            (*parser).core.work_bytes_limit(INITIAL_CALLBACK_BYTES),
        ) {
            fail_parse(parser, 43);
            return Ok(());
        }
    }
    // SAFETY: Charge repeated base metadata before cloning. The clone owns its
    // bytes across callbacks, including callbacks that replace the parser base.
    let base = unsafe {
        if needs_base {
            (*parser)
                .base
                .as_ref()
                .map(CString::try_clone)
                .transpose()?
        } else {
            None
        }
    };
    let split_default = matches!(
        &kind,
        EventKind::StartDoctype(_)
            | EventKind::DoctypeClosingPrefix
            | EventKind::EndDoctype
            | EventKind::EntityDeclaration(_)
            | EventKind::AttlistDeclaration(_)
            | EventKind::ElementDeclaration { .. }
            | EventKind::NotationDeclaration(_)
            | EventKind::EntityDeclarationPrefix
            | EventKind::AttlistDeclarationPrefix
            | EventKind::ElementDeclarationPrefix
            | EventKind::NotationDeclarationPrefix
            | EventKind::EntityDeclarationDuplicate { .. }
    );
    let duplicate_default = match &kind {
        EventKind::EntityDeclarationDuplicate { external, unparsed } if h.entity_decl.is_some() => {
            Some(*external && !*unparsed)
        }
        _ => None,
    };
    let tag_event = matches!(
        &kind,
        EventKind::StartElement { .. } | EventKind::EndElement { .. }
    );
    let mut handled = true;
    // SAFETY: Handlers were installed by the caller with the corresponding C
    // signature. C strings and attribute arrays live for the whole callback.
    unsafe {
        match kind {
            EventKind::Default => handled = false,
            EventKind::DoctypeClosingPrefix => handled = h.start_doctype.is_some(),
            EventKind::EntityDeclarationPrefix => handled = h.entity_decl.is_some(),
            EventKind::AttlistDeclarationPrefix => handled = h.attlist_decl.is_some(),
            EventKind::ElementDeclarationPrefix => handled = h.element_decl.is_some(),
            EventKind::NotationDeclarationPrefix => handled = h.notation.is_some(),
            EventKind::EntityDeclarationDuplicate { .. } => handled = false,
            EventKind::StartElement {
                mut name,
                mut attributes,
            } => {
                (*parser).specified_attributes =
                    (attributes.iter().filter(|a| a.specified).count() * 2)
                        .try_into()
                        .unwrap_or(c_int::MAX);
                if let Some(callback) = h.start_element {
                    if name.as_bytes().contains(&0) {
                        return Err(AllocError::InteriorNul);
                    }
                    name.try_push('\0')?;
                    let allocator = (*parser).allocator;
                    let mut local_pointers = [ptr::null(); 17];
                    let mut pointers = XmlVec::new_in(allocator);
                    let pointer_count = attributes.len() * 2 + 1;
                    let output = if pointer_count <= local_pointers.len() {
                        &mut local_pointers[..pointer_count]
                    } else {
                        pointers.try_reserve_exact(pointer_count)?;
                        pointers.resize(pointer_count, ptr::null());
                        &mut pointers
                    };
                    for (index, attribute) in attributes.iter_mut().enumerate() {
                        // XML forbids embedded NUL. Reuse the owned event strings
                        // as C strings instead of allocating another owner array.
                        attribute.name.try_push('\0')?;
                        attribute.value.try_push('\0')?;
                        output[index * 2] = attribute.name.as_ptr().cast();
                        output[index * 2 + 1] = attribute.value.as_ptr().cast();
                    }
                    callback(arg, name.as_ptr().cast(), output.as_ptr());
                } else {
                    // Expat routes an empty tag through its element handlers
                    // when either handler is installed, including end-only use.
                    handled = h.end_element.is_some()
                        && (*parser)
                            .core
                            .current_raw()
                            .is_some_and(|raw| raw.ends_with("/>"));
                }
                // The callback and pointer-array borrow have ended. The busy
                // guard still pins the parser, including after Stop or ignored
                // callback-time Free/Reset; no core borrow crossed the callback.
                (*parser)
                    .core
                    .recycle_start_element(recycling, name, attributes);
            }
            EventKind::EndElement { mut name } => {
                if let Some(callback) = h.end_element {
                    if name.as_bytes().contains(&0) {
                        return Err(AllocError::InteriorNul);
                    }
                    name.try_push('\0')?;
                    callback(arg, name.as_ptr().cast());
                } else {
                    handled = false;
                }
                (*parser).core.recycle_end_element(recycling, name);
            }
            EventKind::Text(text) => {
                if let Some(callback) = h.text {
                    callback(arg, text.as_ptr().cast(), text.len() as c_int);
                } else {
                    handled = false;
                }
            }
            EventKind::Comment(text) => {
                if let Some(callback) = h.comment {
                    callback(arg, cstring(text)?.as_ptr());
                } else {
                    handled = false;
                }
            }
            EventKind::ProcessingInstruction { target, data } => {
                if let Some(callback) = h.pi {
                    callback(arg, cstring(target)?.as_ptr(), cstring(data)?.as_ptr());
                } else {
                    handled = false;
                }
            }
            EventKind::StartCdata => {
                if let Some(callback) = h.start_cdata {
                    callback(arg);
                } else {
                    handled = false;
                }
            }
            EventKind::EndCdata => {
                if let Some(callback) = h.end_cdata {
                    callback(arg);
                } else {
                    handled = false;
                }
            }
            EventKind::XmlDeclaration {
                version,
                encoding,
                standalone,
            } => {
                if let Some(callback) = h.xml_decl {
                    callback(
                        arg,
                        cstring(version)?.as_ptr(),
                        cptr(&optional_cstring(encoding)?),
                        standalone.map_or(-1, c_int::from),
                    );
                } else {
                    handled = false;
                }
            }
            EventKind::TextDeclaration { version, encoding } => {
                if let Some(callback) = h.xml_decl {
                    callback(
                        arg,
                        cptr(&optional_cstring(version)?),
                        cstring(encoding)?.as_ptr(),
                        -1,
                    );
                } else {
                    handled = false;
                }
            }
            EventKind::ExternalEntityReference(declaration) => {
                let xeme::ExternalEntityReference {
                    base: mut declaration_base,
                    context,
                    system_id,
                    public_id,
                } = XmlBox::into_inner(declaration);
                if let Some(callback) = h.external {
                    let base = if foreign_dtd {
                        cptr(&base)
                    } else {
                        base_cptr(&mut declaration_base)?
                    };
                    let handler_arg = if (*parser).external_arg.is_null() {
                        parser
                    } else {
                        (*parser).external_arg.cast()
                    };
                    if callback(
                        handler_arg,
                        cptr(&optional_cstring(context)?),
                        base,
                        cptr(&optional_cstring(system_id)?),
                        cptr(&optional_cstring(public_id)?),
                    ) == 0
                    {
                        fail_parse(parser, 21);
                    }
                } else {
                    (*parser).core.external_entity_handler_absent();
                    handled = false;
                }
            }
            EventKind::NotStandalone => {
                if let Some(callback) = h.not_standalone
                    && callback(arg) == 0
                {
                    fail_parse(parser, 22);
                }
            }
            EventKind::StartDoctype(declaration) => {
                let xeme::DoctypeDeclaration {
                    name,
                    system_id,
                    public_id,
                    has_internal_subset,
                } = XmlBox::into_inner(declaration);
                // The start callback handles the closing token only when this
                // declaration has no internal subset. Preserve that decision
                // across handler changes and an intervening external callback.
                (*parser).doctype_close_handled = !has_internal_subset && h.start_doctype.is_some();
                if let Some(callback) = h.start_doctype {
                    callback(
                        arg,
                        cstring(name)?.as_ptr(),
                        cptr(&optional_cstring(system_id)?),
                        cptr(&optional_cstring(public_id)?),
                        c_int::from(has_internal_subset),
                    );
                } else {
                    handled = false;
                }
            }
            EventKind::EndDoctype => {
                if let Some(callback) = h.end_doctype {
                    callback(arg);
                } else {
                    handled = (*parser).doctype_close_handled;
                }
                (*parser).doctype_close_handled = false;
            }
            EventKind::StartNamespace { prefix, uri } => {
                if let Some(callback) = h.start_namespace {
                    callback(
                        arg,
                        cptr(&optional_cstring(prefix)?),
                        cptr(&optional_cstring(uri)?),
                    );
                }
            }
            EventKind::EndNamespace { prefix } => {
                if let Some(callback) = h.end_namespace {
                    callback(arg, cptr(&optional_cstring(prefix)?));
                }
            }
            EventKind::EntityDeclaration(declaration) => {
                let xeme::EntityDeclaration {
                    base: mut declaration_base,
                    name,
                    value,
                    parameter,
                    system_id,
                    public_id,
                    notation,
                } = XmlBox::into_inner(declaration);
                let base = if value.is_some() {
                    cptr(&base)
                } else {
                    base_cptr(&mut declaration_base)?
                };
                if let (Some(callback), Some(notation_name)) = (h.unparsed, notation.as_ref()) {
                    let notation_name =
                        CString::try_from_str_in(notation_name, (*parser).allocator)?;
                    callback(
                        arg,
                        cstring(name)?.as_ptr(),
                        base,
                        cptr(&optional_cstring(system_id)?),
                        cptr(&optional_cstring(public_id)?),
                        notation_name.as_ptr(),
                    );
                } else if let Some(callback) = h.entity_decl {
                    let len = value.as_ref().map_or(0, |value| value.len()) as c_int;
                    callback(
                        arg,
                        cstring(name)?.as_ptr(),
                        c_int::from(parameter),
                        cptr(&optional_cstring(value)?),
                        len,
                        base,
                        cptr(&optional_cstring(system_id)?),
                        cptr(&optional_cstring(public_id)?),
                        cptr(&optional_cstring(notation)?),
                    );
                } else {
                    handled = false;
                }
            }
            EventKind::AttlistDeclaration(declaration) => {
                let xeme::AttributeDeclaration {
                    element,
                    name,
                    attribute_type,
                    default,
                    required,
                } = XmlBox::into_inner(declaration);
                if let Some(callback) = h.attlist_decl {
                    let element = cstring(element)?;
                    let name = cstring(name)?;
                    let attribute_type = cstring(attribute_type)?;
                    let default = optional_cstring(default)?;
                    (*parser).attlist_dispatch = true;
                    callback(
                        arg,
                        element.as_ptr(),
                        name.as_ptr(),
                        attribute_type.as_ptr(),
                        cptr(&default),
                        c_int::from(required),
                    );
                    (*parser).attlist_dispatch = false;
                } else {
                    handled = false;
                }
            }
            EventKind::NotationDeclaration(declaration) => {
                let xeme::NotationDeclaration {
                    name,
                    system_id,
                    public_id,
                } = XmlBox::into_inner(declaration);
                if let Some(callback) = h.notation {
                    callback(
                        arg,
                        cstring(name)?.as_ptr(),
                        cptr(&base),
                        cptr(&optional_cstring(system_id)?),
                        cptr(&optional_cstring(public_id)?),
                    );
                } else {
                    handled = false;
                }
            }
            EventKind::ElementDeclaration { name, model } => {
                if let Some(callback) = h.element_decl {
                    let name = cstring(name)?;
                    match content_model::allocate(&model, (*parser).allocator) {
                        Ok(model) => callback(arg, name.as_ptr(), model),
                        Err(error) => fail_parse(parser, error),
                    }
                } else {
                    handled = false;
                }
            }
            EventKind::SkippedEntity { name, parameter } => {
                if let Some(callback) = h.skipped {
                    callback(arg, cstring(name)?.as_ptr(), c_int::from(parameter));
                } else {
                    handled = false;
                }
            }
        }
    }
    if !handled {
        // SAFETY: The raw fallback uses the same guarded, owned-fragment path.
        unsafe {
            dispatch_unhandled(parser, split_default, duplicate_default, tag_event)?;
        }
    }
    Ok(())
}

unsafe fn dispatch_unhandled(
    parser: XML_Parser,
    split_default: bool,
    duplicate_default: Option<bool>,
    tag_event: bool,
) -> Result<(), AllocError> {
    // SAFETY: No callback retains parser-owned data. The raw token must be
    // copied since a default callback can change parser configuration.
    unsafe {
        if !(*parser).destroying && (*parser).handlers.default.is_some() {
            let raw = (*parser)
                .core
                .current_raw()
                .map(|raw| XmlString::try_from_str_in(raw, (*parser).allocator))
                .transpose()?;
            if let Some(raw) = raw {
                if tag_event {
                    dispatch_default_fragment(parser, &raw, true);
                    return Ok(());
                }
                let single_fragment = split_default
                    && duplicate_default.is_none()
                    && DtdFragments(raw.as_str())
                        .next()
                        .is_some_and(|fragment| fragment.len() == raw.len());
                if split_default && !single_fragment {
                    let mut duplicate_name = raw.starts_with("<!ENTITY");
                    for fragment in DtdFragments(raw.as_str()) {
                        if let Some(closing) = duplicate_default {
                            if fragment.chars().all(char::is_whitespace)
                                || matches!(fragment, "<!ENTITY" | "%")
                            {
                                continue;
                            }
                            if duplicate_name {
                                duplicate_name = false;
                            } else if fragment == "NDATA" {
                                duplicate_name = true;
                                continue;
                            } else if matches!(fragment, "SYSTEM" | "PUBLIC")
                                || (fragment == ">" && !closing)
                            {
                                continue;
                            }
                        }
                        (*parser)
                            .default_pending
                            .try_push_back(XmlString::try_from_str_in(
                                fragment,
                                (*parser).allocator,
                            )?)?;
                    }
                } else {
                    (*parser).default_pending.try_push_back(raw)?;
                }
                drain_default_fragments(parser);
            }
        }
    }
    Ok(())
}

/// Lexical DTD fragments expected by consumers such as ElementTree's `_default`.
struct DtdFragments<'a>(&'a str);

impl<'a> Iterator for DtdFragments<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        let first = self.0.chars().next()?;
        let length = match first {
            '\'' | '"' => self.0[1..].find(first).map_or(self.0.len(), |end| end + 2),
            '[' | ']' | '>' | '(' | ')' | '|' | ',' | '%' | '?' | '*' | '+' | '=' => {
                first.len_utf8()
            }
            ' ' | '\r' | '\n' | '\t' => self
                .0
                .find(|c| !matches!(c, ' ' | '\r' | '\n' | '\t'))
                .unwrap_or(self.0.len()),
            _ => self
                .0
                .find([
                    ' ', '\r', '\n', '\t', '\'', '"', '[', ']', '>', '(', ')', '|', ',', '%', '?',
                    '*', '+', '=',
                ])
                .unwrap_or(self.0.len()),
        };
        let (fragment, rest) = self.0.split_at(length);
        self.0 = rest;
        Some(fragment)
    }
}

unsafe fn dispatch_default_fragment(parser: XML_Parser, raw: &str, tag_event: bool) -> bool {
    // SAFETY: Called only by the guarded parse loop. Each callback can replace
    // handlers or stop; re-read scalar state between fragments.
    unsafe {
        if (*parser).destroying
            || ((*parser).state == 3 && !tag_event)
            || ((*parser).parse_error != 0 && !((*parser).parse_error == 35 && tag_event))
        {
            return false;
        }
        let Some(callback) = (*parser).handlers.default else {
            return false;
        };
        let arg = if (*parser).handler_arg_is_parser {
            parser.cast()
        } else {
            (*parser).user_data
        };
        (*parser).default_dispatch = true;
        callback(arg, raw.as_ptr().cast(), raw.len() as c_int);
        (*parser).default_dispatch = false;
        !(*parser).destroying && (*parser).state != 3 && (*parser).parse_error == 0
    }
}

unsafe fn drain_default_fragments(parser: XML_Parser) {
    // SAFETY: The queue is accessed only outside callbacks; a popped String owns
    // its data during dispatch. Remaining fragments survive suspension/resumption.
    unsafe {
        while !(*parser).destroying && (*parser).state != 3 && (*parser).parse_error == 0 {
            if (*parser).handlers.default.is_none() {
                while (*parser).default_pending.pop_front().is_some() {}
                return;
            }
            let Some(fragment) = (*parser).default_pending.pop_front() else {
                return;
            };
            if !dispatch_default_fragment(parser, &fragment, false) {
                return;
            }
        }
    }
}

unsafe fn dispatch_start_frame(
    parser: XML_Parser,
    frame: &xeme::AdapterFrame,
) -> Result<(), AllocError> {
    // SAFETY: The busy guard pins the parser. Frame bytes are independently
    // owned by run_events, and only scalar handler/allocator copies escape.
    let (callback, arg, allocator) = unsafe {
        let family = &(*parser).family;
        if !charge(
            &family.callback_bytes,
            frame.callback_bytes(),
            (*parser).core.work_bytes_limit(INITIAL_CALLBACK_BYTES),
        ) {
            fail_parse(parser, 43);
            return Ok(());
        }
        (*parser).specified_attributes = (frame.attributes().len() * 2)
            .try_into()
            .unwrap_or(c_int::MAX);
        (
            (*parser).handlers.start_element,
            if (*parser).handler_arg_is_parser {
                parser.cast()
            } else {
                (*parser).user_data
            },
            (*parser).allocator,
        )
    };
    let Some(callback) = callback else {
        // SAFETY: The guarded parser is borrowed only to inspect current state;
        // the empty tag's queued end callback will consume this token.
        if unsafe {
            (*parser).handlers.end_element.is_some()
                && (*parser)
                    .core
                    .current_raw()
                    .is_some_and(|raw| raw.ends_with("/>"))
        } {
            return Ok(());
        }
        // SAFETY: No parser borrow crosses the owned raw-fragment callbacks.
        return unsafe { dispatch_unhandled(parser, false, None, true) };
    };
    let mut local_pointers = [ptr::null(); 17];
    let mut pointers = XmlVec::new_in(allocator);
    let pointer_count = frame.attributes().len() * 2 + 1;
    let output = if pointer_count <= local_pointers.len() {
        &mut local_pointers[..pointer_count]
    } else {
        pointers.try_reserve_exact(pointer_count)?;
        pointers.resize(pointer_count, ptr::null());
        &mut pointers
    };
    for (index, (name, value)) in frame.attributes().enumerate() {
        output[index * 2] = name.as_ptr().cast();
        output[index * 2 + 1] = value.as_ptr().cast();
    }
    // SAFETY: The validated native strings have one final NUL and no interior
    // NUL. The separately owned immutable arena and pointer array remain live
    // through the callback; no parser/source/map reference is retained.
    unsafe {
        callback(arg, frame.name_bytes().as_ptr().cast(), output.as_ptr());
    }
    Ok(())
}

unsafe fn dispatch_text_frame(parser: XML_Parser, bytes: &[u8]) -> Result<(), AllocError> {
    // SAFETY: The busy guard pins the parser. The bytes belong to the detached
    // frame, and only scalar callback/argument copies cross the callback.
    let (callback, arg) = unsafe {
        let family = &(*parser).family;
        if !charge(
            &family.callback_bytes,
            bytes.len(),
            (*parser).core.work_bytes_limit(INITIAL_CALLBACK_BYTES),
        ) {
            fail_parse(parser, 43);
            return Ok(());
        }
        (
            (*parser).handlers.text,
            if (*parser).handler_arg_is_parser {
                parser.cast()
            } else {
                (*parser).user_data
            },
        )
    };
    if let Some(callback) = callback {
        // SAFETY: The independent owned frame remains live through the callback.
        unsafe { callback(arg, bytes.as_ptr().cast(), bytes.len() as c_int) };
        Ok(())
    } else {
        // SAFETY: The default path uses the core's independently owned raw token.
        unsafe { dispatch_unhandled(parser, false, None, false) }
    }
}

/// Dispatch a validated native Text range without borrowing parser bytes across C.
unsafe fn dispatch_context_text(
    parser: XML_Parser,
    start: usize,
    count: usize,
) -> Result<(), AllocError> {
    // SAFETY: Called under the busy guard. Borrow only the accounting fields,
    // then capture scalars exactly as for owned Text dispatch.
    let (callback, arg) = unsafe {
        let family = &(*parser).family;
        if !charge(
            &family.callback_bytes,
            count,
            (*parser).core.work_bytes_limit(INITIAL_CALLBACK_BYTES),
        ) {
            fail_parse(parser, 43);
            return Ok(());
        }
        (
            (*parser).handlers.text,
            if (*parser).handler_arg_is_parser {
                parser.cast()
            } else {
                (*parser).user_data
            },
        )
    };
    let Some(callback) = callback else {
        // SAFETY: Eager raw storage and the default dispatch path are unchanged.
        return unsafe { dispatch_unhandled(parser, false, None, false) };
    };
    // SAFETY: The core's immutable context borrow ends before the callback.
    // Checked offsets preserve the stable input allocation's provenance.
    let span = unsafe {
        (|| {
            if !(*parser).busy
                || !(*parser).input_context_active
                || (*parser).destroying
                || count == 0
                || count > 4096
            {
                return None;
            }
            let (context, context_start) = (*parser).core.input_context();
            let offset = start.checked_sub(context_start)?;
            let remaining = context.len().checked_sub(offset)?;
            if count > remaining {
                return None;
            }
            let length = c_int::try_from(count).ok()?;
            Some((context.as_ptr().add(offset).cast::<c_char>(), length))
        })()
    };
    let Some((pointer, length)) = span else {
        // SAFETY: An invalid host range is a protocol error, never an allocation error.
        unsafe { fail_parse(parser, UNEXPECTED_STATE) };
        return Ok(());
    };
    // SAFETY: No parser/context reference crosses this call. The busy guard
    // prevents same-parser feed, reset or destruction even after StopParser;
    // permitted setters borrow disjoint fields. The pointer is used only here.
    unsafe { callback(arg, pointer, length) };
    Ok(())
}

unsafe fn run_events(parser: XML_Parser) -> c_int {
    // SAFETY: Called under the busy guard. The detached frame carries owned
    // storage or scalar ranges; no core borrow crosses callback-time setters.
    unsafe {
        if (*parser).destroying {
            return ERROR;
        }
        let mut frame = (*parser).core.adapter_frame();
        let result = run_events_with_frame(parser, &mut frame);
        (*parser).core.finish_adapter_frame(frame);
        result
    }
}

unsafe fn run_events_with_frame(parser: XML_Parser, frame: &mut xeme::AdapterFrame) -> c_int {
    loop {
        // SAFETY: Pending lexical fragments belong to a previously suspended event.
        unsafe {
            if !(*parser).default_pending.is_empty() {
                drain_default_fragments(parser);
            }
        }
        let mut event = None;
        // SAFETY: No references to parser fields escape this scope or cross
        // dispatch. The busy guard prevents freeing or reparsing the core.
        let recycling = unsafe {
            if (*parser).destroying {
                return ERROR;
            }
            // Expat completes a tag's namespace and empty-element callbacks
            // after StopParser. Allocation and processing errors still stop now.
            let finishing_tag = (*parser).core.has_pending_tag_event();
            if (*parser).parse_error != 0 && !((*parser).parse_error == 35 && finishing_tag) {
                (*parser).error = (*parser).parse_error;
                return ERROR;
            }
            if (*parser).state == 3 && !finishing_tag {
                (*parser).position = (*parser).core.position_between_callbacks(true);
                (*parser).error = 0;
                return SUSPENDED;
            }
            match (*parser)
                .core
                .next_event_for_c_text_context_into(&mut event, frame)
            {
                Ok(Some(recycling)) => recycling,
                Ok(None) => {
                    if resolve_pending_conversion(parser) {
                        continue;
                    }
                    if (*parser).parse_error != 0 {
                        return ERROR;
                    }
                    (*parser).position = (*parser).core.position_between_callbacks(false);
                    if (*parser).core.is_finished() {
                        if !merge_external_subset(parser) {
                            return ERROR;
                        }
                        (*parser).state = 2;
                    }
                    (*parser).error = 0;
                    return OK;
                }
                Err(error) => {
                    if error.kind == ErrorKind::UnknownEncoding && resolve_unknown_encoding(parser)
                    {
                        continue;
                    }
                    if (*parser).destroying {
                        return ERROR;
                    }
                    if (*parser).parse_error != 0 {
                        return ERROR;
                    }
                    fail_parse(parser, error_code(&error.kind));
                    (*parser).position = error.position;
                    return ERROR;
                }
            }
        };
        if frame.is_active() {
            // SAFETY: Both the frame and its position are owned outside CParser.
            unsafe {
                (*parser).position = frame.position();
                let result = if let Some((start, count)) = frame.native_text_range_for_c() {
                    dispatch_context_text(parser, start, count)
                } else if let Some(mut name) = frame.take_end_name() {
                    // The original name is local, independently of the reusable
                    // arena and parser. Match owned End dispatch: charge before
                    // its terminator, recycle after callbacks, then raw fallback.
                    (|| -> Result<(), AllocError> {
                        let callback = (*parser).handlers.end_element;
                        let arg = if (*parser).handler_arg_is_parser {
                            parser.cast()
                        } else {
                            (*parser).user_data
                        };
                        let charged = {
                            let family = &(*parser).family;
                            charge(
                                &family.callback_bytes,
                                name.len(),
                                (*parser).core.work_bytes_limit(INITIAL_CALLBACK_BYTES),
                            )
                        };
                        if !charged {
                            fail_parse(parser, 43);
                            return Ok(());
                        }
                        if let Some(callback) = callback {
                            if name.as_bytes().contains(&0) {
                                return Err(AllocError::InteriorNul);
                            }
                            name.try_push('\0')?;
                            callback(arg, name.as_ptr().cast());
                        }
                        (*parser).core.recycle_end_element(recycling, name);
                        if callback.is_none() {
                            dispatch_unhandled(parser, false, None, true)?;
                        }
                        Ok(())
                    })()
                } else if let Some(bytes) = frame.text_bytes() {
                    dispatch_text_frame(parser, bytes)
                } else {
                    dispatch_start_frame(parser, frame)
                };
                if result.is_err() {
                    fail_parse(parser, 1);
                    return ERROR;
                }
            }
            continue;
        }
        let event = event.expect("recycling token accompanies an owned event");
        // SAFETY: The event owns its data and the parser remains busy.
        unsafe {
            (*parser).position = event.position;
            if dispatch(parser, event.kind, recycling).is_err() {
                fail_parse(parser, 1);
                return ERROR;
            }
        }
    }
}

unsafe fn merge_external_subset(parser: XML_Parser) -> bool {
    // SAFETY: Callers serialize related-parser access, so a loaded parent stays
    // live throughout this merge. The shared token detects earlier destruction
    // or reset; allocator callbacks cannot reenter while either core is borrowed.
    unsafe {
        if (*parser).external_subset_merged || !(*parser).core.is_external_subset() {
            return true;
        }
        let parent_lifetime = (*parser).parent_lifetime.clone();
        if let Some(parent_lifetime) = parent_lifetime {
            let parent = parent_lifetime.load(Ordering::Acquire);
            if !parent.is_null()
                && !(*parent).destroying
                && let Err(error) = (*parent).core.merge_external_subset(&(*parser).core)
            {
                fail_parse(parser, error_code(&error.kind));
                (*parser).position = error.position;
                return false;
            }
        }
        (*parser).external_subset_merged = true;
        true
    }
}

unsafe fn resolve_unknown_encoding(parser: XML_Parser) -> bool {
    // SAFETY: The caller owns the busy guard and has released its core borrow.
    let result = unsafe { try_resolve_unknown_encoding(parser) };
    match result {
        Ok(resolved) => resolved,
        Err(_) => {
            // SAFETY: Error reporting is a scalar write and does not allocate.
            unsafe {
                fail_parse(parser, 1);
            }
            false
        }
    }
}

unsafe fn resolve_pending_conversion(parser: XML_Parser) -> bool {
    // SAFETY: The core returns a copied sequence. No parser reference or input
    // buffer borrow survives the converter call, and the outer busy guard remains.
    unsafe {
        let Some(request) = (*parser).core.encoding_conversion() else {
            return false;
        };
        let Some(convert) = (*parser).encoding_convert else {
            fail_parse(parser, 18);
            return false;
        };
        let data = (*parser).encoding_data;
        (*parser).position = request.position;
        let value = convert(data, request.bytes.as_ptr().cast());
        if (*parser).parse_error != 0 || (*parser).destroying {
            return false;
        }
        if let Err(error) = (*parser).core.resolve_encoding_conversion(value) {
            fail_parse(parser, error_code(&error.kind));
            (*parser).position = error.position;
            return false;
        }
        true
    }
}

unsafe fn try_resolve_unknown_encoding(parser: XML_Parser) -> Result<bool, AllocError> {
    // SAFETY: Only owned names and stack callback output survive the C call.
    unsafe {
        let Some(handler) = (*parser).handlers.unknown_encoding else {
            return Ok(false);
        };
        let Some(name) = (*parser).core.unknown_encoding() else {
            return Ok(false);
        };
        let name = XmlString::try_from_str_in(name, (*parser).allocator)?;
        let c_name = CString::try_from_str_in(&name, (*parser).allocator)?;
        let arg = (*parser).unknown_encoding_arg;
        let mut info = XML_Encoding {
            map: [-1; 256],
            data: ptr::null_mut(),
            convert: None,
            release: None,
        };
        let accepted = handler(arg, c_name.as_ptr(), &mut info) != 0;
        if accepted && !(*parser).destroying && (*parser).parse_error == 0 {
            let installed = if info.convert.is_some() {
                (*parser).core.set_multibyte_encoding_map(&name, info.map)
            } else {
                (*parser).core.set_encoding_map(&name, info.map)
            };
            match installed {
                Ok(()) => {
                    release_encoding(parser);
                    if !(*parser).destroying && (*parser).parse_error == 0 {
                        (*parser).encoding_release = info.release;
                        (*parser).encoding_convert = info.convert;
                        (*parser).encoding_data = info.data;
                        return Ok(true);
                    }
                }
                Err(error) if error.kind == ErrorKind::NoMemory => fail_parse(parser, 1),
                Err(_) => {}
            }
        }
        if let Some(release) = info.release {
            release(info.data);
        }
        Ok(false)
    }
}

unsafe fn finish_fallible_operation(
    parser: XML_Parser,
    result: Result<Result<c_int, AllocError>, Box<dyn std::any::Any + Send>>,
) -> c_int {
    // SAFETY: Allocating operations own the busy guard and can set a scalar error
    // without allocating; the common finalizer releases the operation guard.
    unsafe {
        let result = result.map(|result| {
            result.unwrap_or_else(|_| {
                (*parser).error = 1;
                ERROR
            })
        });
        finish_operation(parser, result)
    }
}

/// End a guarded operation after every callback has returned.
unsafe fn finish_operation(
    parser: XML_Parser,
    result: Result<c_int, Box<dyn std::any::Any + Send>>,
) -> c_int {
    // SAFETY: Only the outer guarded operation clears busy; callback-time Free is ignored.
    unsafe {
        let result = result.unwrap_or_else(|_| {
            fail_parse(parser, UNEXPECTED_STATE);
            ERROR
        });
        (*parser).input_context_active = false;
        (*parser).busy = false;
        result
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_Parse(
    parser: XML_Parser,
    input: *const c_char,
    len: c_int,
    final_input: c_int,
) -> c_int {
    // SAFETY: The scope owns a tracker clone and rejects allocator-callback reentry
    // before reading parser state; all temporary allocations use the same family.
    let operation = || {
        if parser.is_null() || in_allocator_callback() {
            return ERROR;
        }
        // SAFETY: Caller provides a live handle and readable len-byte input. Reject
        // same-parser recursion before touching the Rust core or its input storage.
        unsafe {
            if (*parser).busy {
                return ERROR;
            }
            if len < 0 || (input.is_null() && len != 0) {
                (*parser).error = INVALID_ARGUMENT;
                return ERROR;
            }
            if (*parser).state == 3 {
                (*parser).error = 33;
                return ERROR;
            }
            if (*parser).state == 2 {
                (*parser).error = 36;
                return ERROR;
            }
            if (*parser).parse_error != 0 {
                return ERROR;
            }
            (*parser).error = 0;
            if len as usize > MAX_INPUT_BYTES
                || len as usize > (*parser).core.input_bytes_remaining()
            {
                fail_parse(parser, 43);
                return ERROR;
            }
            let family = &(*parser).family;
            if !charge(&family.input_bytes, len as usize, usize::MAX) {
                fail_parse(parser, 43);
                return ERROR;
            }
            let tracker = &(*parser).tracker;
            if (*parser).child_depth == 0 && !tracker.add_direct_bytes(len as u64) {
                fail_parse(parser, 1);
                return ERROR;
            }
            (*parser).busy = true;
            (*parser).input_context_active = true;
            (*parser).state = 1;
            (*parser).final_buffer = final_input != 0;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let input = if len == 0 {
                    &[]
                } else {
                    std::slice::from_raw_parts(input.cast::<u8>(), len as usize)
                };
                let fed = match (*parser).core.feed_with_input_context(
                    input,
                    final_input != 0,
                    INPUT_CONTEXT_BYTES,
                ) {
                    Ok(result) => result,
                    Err(_) => {
                        fail_parse(parser, 1);
                        return ERROR;
                    }
                };
                if let Err(error) = fed {
                    if error.kind == ErrorKind::UnknownEncoding && resolve_unknown_encoding(parser)
                    {
                        return run_events(parser);
                    }
                    if (*parser).destroying {
                        return ERROR;
                    }
                    if (*parser).parse_error != 0 {
                        return ERROR;
                    }
                    fail_parse(parser, error_code(&error.kind));
                    (*parser).position = error.position;
                    return ERROR;
                }
                run_events(parser)
            }));
            finish_operation(parser, result)
        }
    };
    // SAFETY: The operation checks the handle before use; its tracker clone owns
    // the accounting context even when an operation destroys the parser.
    unsafe { with_parser_tracking(parser, operation) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_GetBuffer(parser: XML_Parser, len: c_int) -> *mut c_void {
    // SAFETY: The scope owns a tracker clone and rejects allocator-callback reentry
    // before reading parser state; all temporary allocations use the same family.
    let operation = || {
        if parser.is_null() || in_allocator_callback() {
            return ptr::null_mut();
        }
        catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: No callback runs while resizing the exclusively accessed buffer.
            unsafe {
                if (*parser).busy {
                    return ptr::null_mut();
                }
                if len < 0 {
                    (*parser).error = 1;
                    return ptr::null_mut();
                }
                if (*parser).state == 2 {
                    (*parser).error = 36;
                    return ptr::null_mut();
                }
                if (*parser).state == 3 {
                    (*parser).error = 33;
                    return ptr::null_mut();
                }
                let len = len as usize;
                let remaining = (*parser).core.input_bytes_remaining().min(MAX_INPUT_BYTES);
                if len > remaining {
                    (*parser).error = 43;
                    return ptr::null_mut();
                }
                let additional = len.max(1).saturating_sub((*parser).buffer.len());
                if (*parser).buffer.try_reserve(additional).is_err() {
                    (*parser).error = 1;
                    return ptr::null_mut();
                }
                (*parser).buffer.resize(len.max(1), 0);
                (*parser).buffer_available = true;
                (*parser).buffer.as_mut_ptr().cast()
            }
        }))
        .unwrap_or(ptr::null_mut())
    };
    // SAFETY: The operation checks the handle before use; its tracker clone owns
    // the accounting context even when an operation destroys the parser.
    unsafe { with_parser_tracking(parser, operation) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_ParseBuffer(
    parser: XML_Parser,
    len: c_int,
    final_input: c_int,
) -> c_int {
    if parser.is_null() || in_allocator_callback() {
        return ERROR;
    }
    // SAFETY: No references to the buffer survive entering XML_Parse. feed copies
    // the input before any callback can modify or free the parser.
    unsafe {
        if (*parser).busy {
            return ERROR;
        }
        if len < 0 {
            (*parser).error = INVALID_ARGUMENT;
            return ERROR;
        }
        if (*parser).state == 3 {
            (*parser).error = 33;
            return ERROR;
        }
        if (*parser).state == 2 {
            (*parser).error = 36;
            return ERROR;
        }
        // Once parsing has started, a zero-length call finishes already-owned
        // input without requiring a caller-visible writable buffer reservation.
        // Initialized parsers and all positive lengths retain that requirement.
        if len == 0 && (*parser).state == 1 {
            return XML_Parse(parser, ptr::null(), 0, final_input);
        }
        if !(*parser).buffer_available {
            (*parser).error = 42;
            return ERROR;
        }
        if len as usize > (*parser).buffer.len() {
            (*parser).error = INVALID_ARGUMENT;
            return ERROR;
        }
        let input = (*parser).buffer.as_ptr();
        XML_Parse(parser, input.cast(), len, final_input)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_StopParser(parser: XML_Parser, resumable: u8) -> c_int {
    if parser.is_null() || in_allocator_callback() {
        return ERROR;
    }
    // SAFETY: Only scalar state is changed; this operation is callback-safe.
    unsafe {
        if (*parser).state == 0 {
            (*parser).error = 44;
            return ERROR;
        }
        if (*parser).state == 2 {
            (*parser).error = 36;
            return ERROR;
        }
        if (*parser).state == 3 && resumable != 0 {
            (*parser).error = 33;
            return ERROR;
        }
        if resumable != 0 && (*parser).core.is_external_subset() {
            (*parser).error = 37;
            return ERROR;
        }
        if resumable != 0 {
            (*parser).state = 3;
        } else {
            fail_parse(parser, 35);
            (*parser).state = 2;
        }
        OK
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_ResumeParser(parser: XML_Parser) -> c_int {
    // SAFETY: The scope owns a tracker clone and rejects allocator-callback reentry
    // before reading parser state; all temporary allocations use the same family.
    let operation = || {
        if parser.is_null() || in_allocator_callback() {
            return ERROR;
        }
        // SAFETY: The busy guard and panic boundary match XML_Parse.
        unsafe {
            if (*parser).busy {
                return ERROR;
            }
            if (*parser).state != 3 {
                (*parser).error = 34;
                return ERROR;
            }
            if (*parser).parse_error != 0 {
                return ERROR;
            }
            (*parser).error = 0;
            (*parser).busy = true;
            (*parser).input_context_active = true;
            (*parser).state = 1;
            let result = catch_unwind(AssertUnwindSafe(|| run_events(parser)));
            finish_operation(parser, result)
        }
    };
    // SAFETY: The operation checks the handle before use; its tracker clone owns
    // the accounting context even when an operation destroys the parser.
    unsafe { with_parser_tracking(parser, operation) }
}

/// Install a callback and optionally synchronize the core's handler-availability
/// flag, which controls the declaration payloads retained while tokenizing.
macro_rules! setter {
    ($name:ident, $field:ident, $ty:ty $(, $enabled:ident)?) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(parser: XML_Parser, handler: $ty) {
            if !parser.is_null() && !in_allocator_callback() {
                // SAFETY: Scalar updates on the caller's live, serialized handle.
                unsafe {
                    (*parser).handlers.$field = handler;
                    $((*parser).core.$enabled(handler.is_some());)?
                }
            }
        }
    };
}
setter!(XML_SetStartElementHandler, start_element, StartElement);
setter!(XML_SetEndElementHandler, end_element, StringHandler);
setter!(XML_SetCharacterDataHandler, text, TextHandler);
setter!(XML_SetProcessingInstructionHandler, pi, PairHandler);
setter!(XML_SetCommentHandler, comment, StringHandler);
setter!(XML_SetStartCdataSectionHandler, start_cdata, VoidHandler);
setter!(XML_SetEndCdataSectionHandler, end_cdata, VoidHandler);
setter!(XML_SetXmlDeclHandler, xml_decl, XmlDecl);
setter!(XML_SetStartDoctypeDeclHandler, start_doctype, Doctype);
setter!(XML_SetEndDoctypeDeclHandler, end_doctype, VoidHandler);
setter!(
    XML_SetStartNamespaceDeclHandler,
    start_namespace,
    PairHandler
);
setter!(XML_SetEndNamespaceDeclHandler, end_namespace, StringHandler);
setter!(XML_SetEntityDeclHandler, entity_decl, EntityDecl);
setter!(
    XML_SetAttlistDeclHandler,
    attlist_decl,
    AttlistDecl,
    set_attlist_handler_enabled
);
setter!(XML_SetUnparsedEntityDeclHandler, unparsed, UnparsedDecl);
setter!(
    XML_SetNotationDeclHandler,
    notation,
    NotationDecl,
    set_notation_handler_enabled
);
setter!(XML_SetExternalEntityRefHandler, external, ExternalEntity);
setter!(XML_SetSkippedEntityHandler, skipped, SkippedEntity);
setter!(XML_SetNotStandaloneHandler, not_standalone, NotStandalone);
setter!(XML_SetElementDeclHandler, element_decl, ElementDecl);

macro_rules! pair_setter {
    ($name:ident, $first:ident, $first_ty:ty, $second:ident, $second_ty:ty) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(parser: XML_Parser, first: $first_ty, second: $second_ty) {
            if !parser.is_null() && !in_allocator_callback() {
                // SAFETY: Scalar updates on the caller's serialized live handle.
                unsafe {
                    (*parser).handlers.$first = first;
                    (*parser).handlers.$second = second;
                }
            }
        }
    };
}
pair_setter!(
    XML_SetElementHandler,
    start_element,
    StartElement,
    end_element,
    StringHandler
);
pair_setter!(
    XML_SetCdataSectionHandler,
    start_cdata,
    VoidHandler,
    end_cdata,
    VoidHandler
);
pair_setter!(
    XML_SetDoctypeDeclHandler,
    start_doctype,
    Doctype,
    end_doctype,
    VoidHandler
);
pair_setter!(
    XML_SetNamespaceDeclHandler,
    start_namespace,
    PairHandler,
    end_namespace,
    StringHandler
);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetDefaultHandler(parser: XML_Parser, handler: TextHandler) {
    if !parser.is_null() && !in_allocator_callback() {
        // SAFETY: Scalar updates on the caller's serialized live handle.
        unsafe {
            (*parser).handlers.default = handler;
            (*parser).core.set_default_events(handler.is_some());
            (*parser).handlers.default_expand = false;
            (*parser)
                .core
                .set_expand_internal_entities(handler.is_none());
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetDefaultHandlerExpand(parser: XML_Parser, handler: TextHandler) {
    if !parser.is_null() && !in_allocator_callback() {
        // SAFETY: Scalar updates on the caller's serialized live handle.
        unsafe {
            (*parser).handlers.default = handler;
            (*parser).core.set_default_events(handler.is_some());
            (*parser).handlers.default_expand = true;
            (*parser).core.set_expand_internal_entities(true);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_DefaultCurrent(parser: XML_Parser) {
    // SAFETY: The scope owns a tracker clone and rejects allocator-callback reentry
    // before reading parser state; all temporary allocations use the same family.
    let operation = || {
        if parser.is_null() || in_allocator_callback() {
            return;
        }
        // SAFETY: A default callback may only run within an already guarded parse.
        unsafe {
            if !(*parser).busy || (*parser).destroying {
                return;
            }
            if (*parser).default_dispatch {
                fail_parse(parser, UNEXPECTED_STATE);
                return;
            }
            let callback = (*parser).handlers.default;
            let arg = if (*parser).handler_arg_is_parser {
                parser.cast()
            } else {
                (*parser).user_data
            };
            let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), AllocError> {
                let raw = (*parser)
                    .core
                    .current_raw()
                    .map(|raw| XmlString::try_from_str_in(raw, (*parser).allocator))
                    .transpose()?;
                let raw = raw.or_else(|| {
                    (*parser)
                        .attlist_dispatch
                        .then(|| XmlString::new_in((*parser).allocator))
                });
                if let (Some(callback), Some(raw)) = (callback, raw) {
                    (*parser).default_dispatch = true;
                    callback(arg, raw.as_ptr().cast(), raw.len() as c_int);
                    (*parser).default_dispatch = false;
                }
                Ok(())
            }));
            if !matches!(result, Ok(Ok(()))) {
                fail_parse(parser, 1);
            }
        }
    };
    // SAFETY: The operation checks the handle before use; its tracker clone owns
    // the accounting context even when an operation destroys the parser.
    unsafe { with_parser_tracking(parser, operation) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetUserData(parser: XML_Parser, data: *mut c_void) {
    if !parser.is_null() && !in_allocator_callback() {
        // SAFETY: Scalar update on a serialized live handle.
        unsafe {
            (*parser).user_data = data;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_GetUserData(parser: XML_Parser) -> *mut c_void {
    if parser.is_null() || in_allocator_callback() {
        return ptr::null_mut();
    }
    // SAFETY: Scalar read from a serialized live handle.
    unsafe { (*parser).user_data }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_UseParserAsHandlerArg(parser: XML_Parser) {
    if !parser.is_null() && !in_allocator_callback() {
        // SAFETY: Scalar update on a serialized live handle.
        unsafe {
            (*parser).handler_arg_is_parser = true;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetReturnNSTriplet(parser: XML_Parser, enabled: c_int) {
    if parser.is_null() || in_allocator_callback() {
        return;
    }
    // SAFETY: No core reference survives this call and no callback is invoked.
    unsafe {
        if (*parser).state != 0 {
            return;
        }
        (*parser).config.namespace_triplets = enabled != 0;
        (*parser).core.set_namespace_triplets(enabled != 0);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetEncoding(parser: XML_Parser, encoding: *const c_char) -> c_int {
    // SAFETY: The scope owns a tracker clone and rejects allocator-callback reentry
    // before reading parser state; all temporary allocations use the same family.
    let operation = || {
        if parser.is_null() || in_allocator_callback() {
            return ERROR;
        }
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<c_int, AllocError> {
            // SAFETY: A finished parser may be inside a callback that aborted it.
            // Dispatch owns its event, so no core borrow crosses that callback.
            // Only protocol metadata changes; allocator reentry remains barred.
            unsafe {
                if (*parser).destroying || !matches!((*parser).state, 0 | 2) {
                    return Ok(ERROR);
                }
                let encoding = input_string(encoding)?;
                let result = if (*parser).state == 0 {
                    (*parser).core.set_encoding(encoding)
                } else {
                    (*parser).core.set_completed_encoding(encoding)
                };
                result.map_err(|_| AllocError::OutOfMemory)?;
            }
            Ok(OK)
        }));
        result.ok().and_then(Result::ok).unwrap_or(ERROR)
    };
    // SAFETY: The operation checks the handle before use; its tracker clone owns
    // the accounting context even when an operation destroys the parser.
    unsafe { with_parser_tracking(parser, operation) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetBase(parser: XML_Parser, base: *const c_char) -> c_int {
    // SAFETY: The scope owns a tracker clone and rejects allocator-callback reentry
    // before reading parser state; all temporary allocations use the same family.
    let operation = || {
        if parser.is_null() || in_allocator_callback() {
            return ERROR;
        }
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<c_int, AllocError> {
            // SAFETY: Copy before replacing storage, including input from GetBase.
            unsafe {
                let base = if base.is_null() {
                    None
                } else {
                    Some(CString::try_from_cstr_in(
                        CStr::from_ptr(base),
                        (*parser).allocator,
                    )?)
                };
                (*parser)
                    .core
                    .set_base(base.as_ref().map(|base| base.as_c_str().to_bytes()))
                    .map_err(|_| AllocError::OutOfMemory)?;
                (*parser).base = base;
            }
            Ok(OK)
        }));
        result.ok().and_then(Result::ok).unwrap_or(ERROR)
    };
    // SAFETY: The operation checks the handle before use; its tracker clone owns
    // the accounting context even when an operation destroys the parser.
    unsafe { with_parser_tracking(parser, operation) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_GetBase(parser: XML_Parser) -> *const c_char {
    if parser.is_null() || in_allocator_callback() {
        return ptr::null();
    }
    // SAFETY: Returned pointer follows Expat's lifetime: until base change/free.
    unsafe { cptr(&(*parser).base) }
}

macro_rules! getter {
    ($name:ident, $ty:ty, $default:expr, $field:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(parser: XML_Parser) -> $ty {
            if parser.is_null() || in_allocator_callback() {
                return $default;
            }
            // SAFETY: Scalar read from the caller's serialized live handle.
            unsafe { (*parser).$field }
        }
    };
}
getter!(XML_GetErrorCode, c_int, INVALID_ARGUMENT, error);
getter!(
    XML_GetSpecifiedAttributeCount,
    c_int,
    -1,
    specified_attributes
);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_GetIdAttributeIndex(parser: XML_Parser) -> c_int {
    if parser.is_null() || in_allocator_callback() {
        return -1;
    }
    // SAFETY: This read does not invoke callbacks or retain a core reference.
    unsafe {
        (*parser)
            .core
            .id_attribute_index()
            .and_then(|index| index.checked_mul(2))
            .and_then(|index| c_int::try_from(index).ok())
            .unwrap_or(-1)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_GetCurrentLineNumber(parser: XML_Parser) -> c_ulong {
    if parser.is_null() || in_allocator_callback() {
        return 0;
    }
    // SAFETY: Scalar read from the caller's serialized live handle.
    unsafe { (*parser).position.line as c_ulong }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_GetCurrentColumnNumber(parser: XML_Parser) -> c_ulong {
    if parser.is_null() || in_allocator_callback() {
        return 0;
    }
    // SAFETY: Scalar read from the caller's serialized live handle.
    unsafe { (*parser).position.column as c_ulong }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_GetCurrentByteIndex(parser: XML_Parser) -> c_long {
    if parser.is_null() || in_allocator_callback() {
        return -1;
    }
    // SAFETY: Scalar read from the caller's serialized live handle.
    unsafe {
        if (*parser).state == 0 {
            return -1;
        }
        (*parser)
            .position
            .byte_index
            .try_into()
            .unwrap_or(c_long::MAX)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_GetCurrentByteCount(parser: XML_Parser) -> c_int {
    if parser.is_null() || in_allocator_callback() {
        return 0;
    }
    // SAFETY: Scalar read from the caller's serialized live handle.
    unsafe {
        (*parser)
            .position
            .byte_count
            .try_into()
            .unwrap_or(c_int::MAX)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_GetParsingStatus(parser: XML_Parser, status: *mut XML_ParsingStatus) {
    if parser.is_null() || status.is_null() || in_allocator_callback() {
        return;
    }
    // SAFETY: Caller supplies a live parser and writable status object.
    unsafe {
        ptr::write(
            status,
            XML_ParsingStatus {
                parsing: (*parser).state,
                finalBuffer: u8::from((*parser).final_buffer),
            },
        );
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_GetInputContext(
    parser: XML_Parser,
    offset: *mut c_int,
    size: *mut c_int,
) -> *const c_char {
    if parser.is_null() || offset.is_null() || size.is_null() || in_allocator_callback() {
        return ptr::null();
    }
    // SAFETY: The caller supplies a serialized live parser and writable outputs.
    // Active parsing owns the input storage; recursive parse/reset/free cannot
    // invalidate it before the requesting callback returns.
    unsafe {
        if !(*parser).input_context_active || (*parser).destroying {
            return ptr::null();
        }
        let (context, context_start) = (*parser).core.input_context();
        if context.is_empty() {
            return ptr::null();
        }
        let Some(start) = (*parser).position.byte_index.checked_sub(context_start) else {
            return ptr::null();
        };
        if start > context.len() || (*parser).position.byte_count > context.len() - start {
            return ptr::null();
        }
        let (Ok(start), Ok(length)) = (c_int::try_from(start), c_int::try_from(context.len()))
        else {
            return ptr::null();
        };
        *offset = start;
        *size = length;
        context.as_ptr().cast()
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetExternalEntityRefHandlerArg(parser: XML_Parser, arg: *mut c_void) {
    if !parser.is_null() && !in_allocator_callback() {
        // SAFETY: Scalar update on a serialized live handle.
        unsafe {
            (*parser).external_arg = arg;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetUnknownEncodingHandler(
    parser: XML_Parser,
    handler: UnknownEncoding,
    arg: *mut c_void,
) {
    if !parser.is_null() && !in_allocator_callback() {
        // SAFETY: Scalar updates on a serialized live handle.
        unsafe {
            (*parser).handlers.unknown_encoding = handler;
            (*parser).unknown_encoding_arg = arg;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_ExternalEntityParserCreate(
    parser: XML_Parser,
    context: *const c_char,
    encoding: *const c_char,
) -> XML_Parser {
    // SAFETY: The scope owns a tracker clone and rejects allocator-callback reentry
    // before reading parser state; all temporary allocations use the same family.
    let operation = || {
        if parser.is_null() || in_allocator_callback() {
            return ptr::null_mut();
        }
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<XML_Parser, AllocError> {
            // SAFETY: The child owns its environment and allocator. Shared budgets and
            // lifetime tokens own their storage; no parser reference crosses a callback.
            unsafe {
                if (*parser).destroying || (*parser).child_depth >= MAX_EXTERNAL_DEPTH {
                    (*parser).error = 43;
                    return Ok(ptr::null_mut());
                }
                let family = &(*parser).family;
                if !charge(&family.children, 1, MAX_FAMILY_CHILDREN) {
                    (*parser).error = 43;
                    return Ok(ptr::null_mut());
                }
                let context = input_string(context)?;
                let encoding = input_string(encoding)?;
                let mut core = (*parser)
                    .core
                    .external_child_with_encoding(context, encoding)
                    .map_err(|_| AllocError::OutOfMemory)?;
                core.enable_input_context();
                let position = core.position();
                let allocator = (*parser).allocator;
                let lifetime = Shared::try_new_in(AtomicPtr::new(ptr::null_mut()), allocator)?;
                let child = XmlBox::try_new_in(
                    XML_ParserStruct {
                        user_data: (*parser).user_data,
                        allocator,
                        tracker: Shared::clone(&(*parser).tracker),
                        core,
                        config: (*parser).config.clone(),
                        handlers: (*parser).handlers,
                        handler_arg_is_parser: (*parser).handler_arg_is_parser,
                        busy: false,
                        destroying: false,
                        state: 0,
                        error: 0,
                        parse_error: 0,
                        final_buffer: false,
                        position,
                        specified_attributes: 0,
                        base: None,
                        buffer: XmlVec::new_in(allocator),
                        buffer_available: false,
                        input_context_active: false,
                        external_arg: (*parser).external_arg,
                        unknown_encoding_arg: (*parser).unknown_encoding_arg,
                        default_dispatch: false,
                        attlist_dispatch: false,
                        default_pending: Queue::new_in(allocator),
                        doctype_close_handled: false,
                        family: Shared::clone(&(*parser).family),
                        child_depth: (*parser).child_depth + 1,
                        encoding_release: None,
                        encoding_convert: None,
                        encoding_data: ptr::null_mut(),
                        lifetime,
                        parent_lifetime: Some(Shared::clone(&(*parser).lifetime)),
                        external_subset_merged: false,
                    },
                    allocator,
                )?;
                Ok(activate_handle(child))
            }
        }));
        result.ok().and_then(Result::ok).unwrap_or(ptr::null_mut())
    };
    // SAFETY: The operation checks the handle before use; its tracker clone owns
    // the accounting context even when an operation destroys the parser.
    unsafe { with_parser_tracking(parser, operation) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetParamEntityParsing(parser: XML_Parser, mode: c_int) -> c_int {
    if parser.is_null() || !(0..=2).contains(&mode) || in_allocator_callback() {
        return 0;
    }
    // SAFETY: Configuration changes happen outside any borrowed core operation.
    unsafe {
        c_int::from((*parser).state == 0 && (*parser).core.set_param_entity_parsing(mode as u8))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_UseForeignDTD(parser: XML_Parser, enabled: u8) -> c_int {
    if parser.is_null() || in_allocator_callback() {
        return INVALID_ARGUMENT;
    }
    // SAFETY: Scalar read from a serialized live handle.
    unsafe {
        if (*parser).state != 0 {
            return 26;
        }
        if (*parser).core.set_use_foreign_dtd(enabled != 0) {
            0
        } else {
            26
        }
    }
}

/// Configure the live root while retaining independent child table hash states.
unsafe fn set_hash_salt(mut parser: XML_Parser, salt: [u8; 16]) -> c_int {
    if parser.is_null() || in_allocator_callback() {
        return 0;
    }
    // SAFETY: Family operations are serialized. Lifetime tokens prevent walking
    // into a parent freed or replaced by reset; allocator reentry is rejected
    // before any parser reference is created.
    unsafe {
        while let Some(parent) = &(*parser).parent_lifetime {
            parser = parent.load(Ordering::Acquire);
            if parser.is_null() {
                return 0;
            }
        }
        if (*parser).destroying || (*parser).busy || !matches!((*parser).state, 0 | 2) {
            return 0;
        }
        with_parser_tracking(parser, || {
            catch_unwind(AssertUnwindSafe(|| (*parser).core.set_hash_salt(salt)))
                .ok()
                .and_then(Result::ok)
                .map_or(0, |()| 1)
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetHashSalt(parser: XML_Parser, salt: c_ulong) -> c_int {
    let mut entropy = [0; 16];
    let salt = salt.to_le_bytes();
    entropy[8..8 + salt.len()].copy_from_slice(&salt);
    // SAFETY: Forward the live, serialized parser contract to the common setter.
    unsafe { set_hash_salt(parser, entropy) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetHashSalt16Bytes(parser: XML_Parser, entropy: *const u8) -> u8 {
    if parser.is_null() || entropy.is_null() || in_allocator_callback() {
        return 0;
    }
    // SAFETY: The caller provides sixteen readable bytes, copied before any
    // allocation callback. The common setter checks the parser family state.
    unsafe { set_hash_salt(parser, *entropy.cast::<[u8; 16]>()) as u8 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetReparseDeferralEnabled(parser: XML_Parser, enabled: u8) -> u8 {
    if parser.is_null() || enabled > 1 || in_allocator_callback() {
        return 0;
    }
    // SAFETY: The core setting is changed between event dispatches, with no
    // reference retained by the parse loop across this callback-safe operation.
    unsafe {
        (*parser).core.set_reparse_deferral_enabled(enabled != 0);
    }
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetBillionLaughsAttackProtectionMaximumAmplification(
    parser: XML_Parser,
    factor: f32,
) -> u8 {
    if parser.is_null() || in_allocator_callback() {
        return 0;
    }
    // SAFETY: Only shared atomic root-family settings change; no core borrow
    // survives an event callback and no allocation callback can reenter here.
    unsafe { u8::from((*parser).core.set_entity_maximum_amplification(factor)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetBillionLaughsAttackProtectionActivationThreshold(
    parser: XML_Parser,
    bytes: u64,
) -> u8 {
    if parser.is_null() || in_allocator_callback() {
        return 0;
    }
    // SAFETY: The live parser's shared family settings contain only atomic data.
    unsafe { u8::from((*parser).core.set_entity_activation_threshold(bytes)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetAllocTrackerMaximumAmplification(
    parser: XML_Parser,
    factor: f32,
) -> u8 {
    if parser.is_null() || in_allocator_callback() {
        return 0;
    }
    // SAFETY: Only shared atomic tracker settings are updated on root parsers.
    unsafe {
        if (*parser).child_depth != 0 {
            return 0;
        }
        let tracker = &(*parser).tracker;
        u8::from(tracker.set_maximum_amplification(factor))
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_SetAllocTrackerActivationThreshold(
    parser: XML_Parser,
    bytes: u64,
) -> u8 {
    if parser.is_null() || in_allocator_callback() {
        return 0;
    }
    // SAFETY: Only a shared atomic tracker setting is updated on root parsers.
    unsafe {
        if (*parser).child_depth != 0 {
            return 0;
        }
        let tracker = &(*parser).tracker;
        tracker.set_activation_threshold(bytes);
        1
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_MemMalloc(parser: XML_Parser, size: usize) -> *mut c_void {
    if in_allocator_callback() {
        return ptr::null_mut();
    }
    // Public memory helpers allocate application-owned storage. Expat excludes
    // them from parser amplification accounting, including calls made inside a
    // parse callback. Explicitly clear the inherited tracking scope while keeping
    // the selected allocator and allocation-owned layout metadata.
    without_tracking(|| {
        // SAFETY: Copy the live parser's allocator before invoking user code.
        unsafe {
            let allocator = if parser.is_null() {
                Allocator::System
            } else {
                (*parser).allocator
            };
            allocator.tracked_malloc(size)
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_MemRealloc(
    parser: XML_Parser,
    pointer: *mut c_void,
    size: usize,
) -> *mut c_void {
    if in_allocator_callback() {
        return ptr::null_mut();
    }
    // Clearing the ambient scope also covers realloc(NULL, size) from a parse
    // callback. Existing helper blocks retain their original untracked metadata.
    without_tracking(|| {
        // SAFETY: The pointer is NULL or a live block from these helpers using
        // this allocator suite. Failed growth preserves the original allocation.
        unsafe {
            let allocator = if parser.is_null() {
                Allocator::System
            } else {
                (*parser).allocator
            };
            allocator.tracked_realloc(pointer, size)
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_MemFree(parser: XML_Parser, pointer: *mut c_void) {
    if in_allocator_callback() {
        return;
    }
    // SAFETY: Copy the suite before calling free, which must accept this pointer.
    unsafe {
        let allocator = if parser.is_null() {
            Allocator::System
        } else {
            (*parser).allocator
        };
        allocator.tracked_free(pointer);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn XML_FreeContentModel(_parser: XML_Parser, model: *mut XML_Content) {
    if in_allocator_callback() {
        return;
    }
    // SAFETY: The caller supplies NULL or a live model allocation from its callback.
    unsafe {
        content_model::free(model);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn XML_ExpatVersion() -> *const c_char {
    c"xeme_compat_2.8.4".as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn XML_ExpatVersionInfo() -> XML_Expat_Version {
    // This is the targeted C API revision, matching the header so consumers do
    // not skip newer compatibility/security tests. ExpatVersion names the same
    // target while identifying Xeme as the implementation.
    XML_Expat_Version {
        major: 2,
        minor: 8,
        micro: 4,
    }
}

#[repr(C)]
pub struct XML_Feature {
    pub feature: c_int,
    pub name: *const c_char,
    pub value: c_long,
}

// SAFETY: The feature table is immutable and points only to static C strings.
unsafe impl Sync for XML_Feature {}

static FEATURES: [XML_Feature; 11] = [
    XML_Feature {
        feature: 6,
        name: c"sizeof(XML_Char)".as_ptr(),
        value: 1,
    },
    XML_Feature {
        feature: 7,
        name: c"sizeof(XML_LChar)".as_ptr(),
        value: 1,
    },
    XML_Feature {
        feature: 3,
        name: c"XML_DTD".as_ptr(),
        value: 0,
    },
    XML_Feature {
        feature: 8,
        name: c"XML_NS".as_ptr(),
        value: 0,
    },
    XML_Feature {
        feature: 4,
        name: c"XML_CONTEXT_BYTES".as_ptr(),
        value: INPUT_CONTEXT_BYTES as c_long,
    },
    XML_Feature {
        feature: 11,
        name: c"XML_BLAP_MAX_AMP".as_ptr(),
        value: 100,
    },
    XML_Feature {
        feature: 12,
        name: c"XML_BLAP_ACT_THRES".as_ptr(),
        value: 8 * 1024 * 1024,
    },
    XML_Feature {
        feature: 13,
        name: c"XML_GE".as_ptr(),
        value: 0,
    },
    XML_Feature {
        feature: 14,
        name: c"XML_AT_MAX_AMP".as_ptr(),
        value: 100,
    },
    XML_Feature {
        feature: 15,
        name: c"XML_AT_ACT_THRES".as_ptr(),
        value: 64 * 1024 * 1024,
    },
    XML_Feature {
        feature: 0,
        name: ptr::null(),
        value: 0,
    },
];

#[unsafe(no_mangle)]
pub extern "C" fn XML_GetFeatureList() -> *const XML_Feature {
    FEATURES.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn XML_ErrorString(code: c_int) -> *const c_char {
    let value = match code {
        0 => return ptr::null(),
        1 => c"out of memory",
        2 => c"syntax error",
        3 => c"no element found",
        4 => c"not well-formed (invalid token)",
        5 => c"unclosed token",
        6 => c"partial character",
        7 => c"mismatched tag",
        8 => c"duplicate attribute",
        9 => c"junk after document element",
        10 => c"illegal parameter entity reference",
        11 => c"undefined entity",
        12 => c"recursive entity reference",
        13 => c"asynchronous entity",
        14 => c"reference to invalid character number",
        15 => c"reference to binary entity",
        16 => c"reference to external entity in attribute",
        17 => c"XML or text declaration not at start of entity",
        18 => c"unknown encoding",
        19 => c"encoding specified in XML declaration is incorrect",
        20 => c"unclosed CDATA section",
        21 => c"error in processing external entity reference",
        22 => c"document is not standalone",
        23 => c"unexpected parser state - please send a bug report",
        24 => c"entity declared in parameter entity",
        25 => c"requested feature requires XML_DTD support in Expat",
        26 => c"cannot change setting once parsing has begun",
        27 => c"unbound prefix",
        28 => c"must not undeclare prefix",
        29 => c"incomplete markup in parameter entity",
        30 => c"XML declaration not well-formed",
        31 => c"text declaration not well-formed",
        32 => c"illegal character(s) in public id",
        33 => c"parser suspended",
        34 => c"parser not suspended",
        35 => c"parsing aborted",
        36 => c"parsing finished",
        37 => c"cannot suspend in external parameter entity",
        38 => c"reserved prefix (xml) must not be undeclared or bound to another namespace name",
        39 => c"reserved prefix (xmlns) must not be declared or undeclared",
        40 => c"prefix must not be bound to one of the reserved namespace names",
        41 => c"invalid argument",
        42 => c"a successful prior call to function XML_GetBuffer is required",
        43 => c"limit on input amplification factor (from DTD and entities) breached",
        44 => c"parser not started",
        _ => return ptr::null(),
    };
    value.as_ptr()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod context_text_tests;

#[cfg(test)]
mod declaration_base_tests;

#[cfg(test)]
mod tag_stop_tests;

#[cfg(test)]
mod suspension_position_tests;
