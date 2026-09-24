//! A safe, incremental XML 1.0 parser with explicit resource limits.
//!
//! Feed bytes with [`Parser::feed`], then drain owned events with
//! [`Parser::next_event`]. No borrowed parser state crosses an event boundary.
#![forbid(unsafe_code)]

mod accounting;
mod active;
mod arena;
#[cfg(test)]
mod default_attribute_tests;
mod dtd;
mod dtd_tables;
mod encoding;
#[cfg(test)]
mod large_token_tests;
mod lexical;
mod names;
#[cfg(test)]
mod prolog_whitespace_tests;
mod recycling;
mod reference;
mod tag;
mod text;
mod value;
mod value_lexer;

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use xeme_storage::{
    AllocError, Allocator, HashMap, HashSet, Queue, Shared, String, TryClone, Vec, hash_map,
    try_insert, try_push, try_set_insert,
};

use accounting::EntityBudget;
use dtd_tables::DtdTables;

pub use arena::AdapterFrame;
pub use recycling::RecyclingToken;
pub use xeme_storage::Text;

use recycling::{EventRecycling, copy_attribute_string};

use encoding::{Decoder, InputContext, Source};
pub use names::NameRules;
use names::{invalid_xml_char, is_uri_char, is_xml_char, whitespace};

/// Bounds applied independently of input chunking.
#[derive(Clone, Debug)]
pub struct Limits {
    pub max_depth: usize,
    pub max_token_bytes: usize,
    pub max_total_bytes: usize,
    /// Work allowance shared with external entity children. Counts bytes from
    /// entity expansion, reused defaults, expanded namespace URIs, repeated
    /// declaration callback names, skipped conditional-reference callbacks, and
    /// external reference identifiers and namespace contexts. Child construction
    /// also counts inherited declaration, namespace, encoding, and context
    /// storage, including per-entry overhead. Parameter children share definitions
    /// without copying them.
    ///
    /// With `max_work_amplification: None`, this is an absolute limit. Otherwise,
    /// the limit is the greater of this allowance and the amplification factor
    /// times consumed original root-input bytes.
    pub max_entity_expansion_bytes: usize,
    /// Optional input-relative allowance for cumulative indirect and adapter work.
    /// `None` preserves absolute work limits. Children share consumed root credit;
    /// buffered but unconsumed bytes and external input do not increase it.
    pub max_work_amplification: Option<usize>,
    pub max_entity_depth: usize,
    pub max_attributes: usize,
    pub max_entities: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 256,
            max_token_bytes: 16 * 1024 * 1024,
            max_total_bytes: 256 * 1024 * 1024,
            max_entity_expansion_bytes: 8 * 1024 * 1024,
            max_work_amplification: None,
            max_entity_depth: 32,
            max_attributes: 10_000,
            max_entities: 10_000,
        }
    }
}

/// Parser configuration. Namespace processing is opt-in, as in Expat.
#[derive(Clone, Debug, Default)]
pub struct Config {
    /// XML edition used for all names. Defaults to Fifth Edition.
    pub name_rules: NameRules,
    pub namespace_separator: Option<char>,
    pub namespace_triplets: bool,
    pub encoding: Option<std::string::String>,
    /// Let an encoding declaration override a UTF-8 BOM, matching Expat.
    /// Disabled by default because XML requires the declaration to match the BOM.
    pub allow_utf8_bom_encoding_mismatch: bool,
    /// Preserve Expat's truncation and persistent recursion state for internal
    /// parameter references in external entity values. Disabled by default.
    pub expat_external_value_compatibility: bool,
    /// Accept Expat's permissive declaration version values. Disabled by default;
    /// native declarations require `1.` followed by one or more ASCII digits.
    pub allow_invalid_xml_versions: bool,
    /// Accept inferred UTF-16 without a BOM, encoding declaration, or external
    /// encoding information, matching Expat. Disabled by default.
    pub allow_undeclared_utf16: bool,
    pub limits: Limits,
}

/// A location in the original byte stream. Lines start at one; columns at zero.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Position {
    pub byte_index: usize,
    pub line: usize,
    pub column: usize,
    pub byte_count: usize,
}

struct EventOutput<'a> {
    event: &'a mut Option<Event>,
    frame: Option<&'a mut AdapterFrame>,
    c_text_context: bool,
}

impl EventOutput<'_> {
    fn has_frame(&self) -> bool {
        self.frame.as_ref().is_some_and(|frame| frame.active)
    }

    fn clear_frame(&mut self) {
        if let Some(frame) = self.frame.as_deref_mut() {
            frame.clear();
        }
    }
}

/// One complete application-defined encoded character, owned across callbacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodingConversion {
    /// The first `length` bytes form the sequence; remaining bytes are padding.
    pub bytes: [u8; 4],
    pub length: u8,
    pub position: Position,
}

/// An XML parse failure. A parser remains failed after returning an error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: &'static str,
    pub position: Position,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    NoMemory,
    Syntax,
    NoElements,
    InvalidToken,
    UnclosedToken,
    PartialCharacter,
    TagMismatch,
    DuplicateAttribute,
    JunkAfterDocumentElement,
    ParameterEntityReference,
    UndefinedEntity,
    RecursiveEntityReference,
    AsynchronousEntity,
    IncompleteParameterEntity,
    BadCharacterReference,
    BinaryEntityReference,
    ExternalEntityHandling,
    UnknownEncoding,
    IncorrectEncoding,
    UnclosedCdataSection,
    ExternalEntityInAttribute,
    EntityDeclaredInParameterEntity,
    MisplacedXmlDeclaration,
    XmlDeclaration,
    PublicId,
    UndeclaringPrefix,
    UndefinedPrefix,
    ReservedPrefixXml,
    ReservedPrefixXmlns,
    ReservedNamespaceUri,
    LimitExceeded,
    Finished,
    /// Malformed declaration at the start of an external text entity.
    TextDeclaration,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.message, self.position.line, self.position.column
        )
    }
}
impl std::error::Error for Error {}
impl Error {
    pub(crate) fn bare(kind: ErrorKind, message: &'static str) -> Self {
        Self {
            kind,
            message,
            position: Position {
                line: 1,
                ..Position::default()
            },
        }
    }
}
impl From<AllocError> for Error {
    fn from(_: AllocError) -> Self {
        Self::bare(ErrorKind::NoMemory, "out of memory")
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
    pub specified: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Event {
    pub kind: EventKind,
    pub position: Position,
}

/// An external entity request with owned callback metadata.
#[derive(Debug, Eq, PartialEq)]
pub struct ExternalEntityReference {
    /// Opaque base URI bytes captured when the external identifier was declared.
    pub base: Option<Vec<u8>>,
    pub context: Option<String>,
    pub system_id: Option<String>,
    pub public_id: Option<String>,
}

/// The opening declaration of the document type.
#[derive(Debug, Eq, PartialEq)]
pub struct DoctypeDeclaration {
    pub name: String,
    pub system_id: Option<String>,
    pub public_id: Option<String>,
    pub has_internal_subset: bool,
}

/// An internal, external, or unparsed entity declaration.
#[derive(Debug, Eq, PartialEq)]
pub struct EntityDeclaration {
    /// Opaque base URI bytes captured with an external system identifier.
    /// Internal declarations have no captured base.
    pub base: Option<Vec<u8>>,
    pub name: String,
    pub value: Option<String>,
    pub parameter: bool,
    pub system_id: Option<String>,
    pub public_id: Option<String>,
    pub notation: Option<String>,
}

/// One attribute declared in an attribute-list declaration.
#[derive(Debug, Eq, PartialEq)]
pub struct AttributeDeclaration {
    pub element: String,
    pub name: String,
    pub attribute_type: String,
    pub default: Option<String>,
    pub required: bool,
}

/// A notation declaration with its external identifier.
#[derive(Debug, Eq, PartialEq)]
pub struct NotationDeclaration {
    pub name: String,
    pub system_id: Option<String>,
    pub public_id: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum EventKind {
    /// Opt-in whitespace callback outside element content. Read [`Parser::current_raw`]
    /// before advancing the parser; this event does not own the raw token.
    Default,
    /// Raw declaration bytes delivered only when no entity declaration handler
    /// is installed by a compatibility consumer.
    EntityDeclarationPrefix,
    /// Raw declaration bytes delivered only when no attribute declaration
    /// handler is installed by a compatibility consumer.
    AttlistDeclarationPrefix,
    /// Raw declaration bytes delivered only when no element declaration handler
    /// is installed by a compatibility consumer.
    ElementDeclarationPrefix,
    /// Raw declaration bytes delivered only when no notation declaration handler
    /// is installed by a compatibility consumer.
    NotationDeclarationPrefix,
    /// An ignored duplicate entity declaration, available with default events.
    EntityDeclarationDuplicate {
        external: bool,
        unparsed: bool,
    },
    StartElement {
        name: String,
        attributes: Vec<Attribute>,
    },
    EndElement {
        name: String,
    },
    /// Owned immutable character data. Short values are stored inline.
    Text(Text),
    Comment(String),
    ProcessingInstruction {
        target: String,
        data: String,
    },
    StartCdata,
    EndCdata,
    XmlDeclaration {
        version: String,
        encoding: Option<String>,
        standalone: Option<bool>,
    },
    TextDeclaration {
        version: Option<String>,
        encoding: String,
    },
    ExternalEntityReference(xeme_storage::Box<ExternalEntityReference>),
    StartDoctype(xeme_storage::Box<DoctypeDeclaration>),
    /// Internal-subset closing bracket or whitespace before the final `>`.
    /// A C adapter suppresses default delivery when its start-doctype handler
    /// handles this grammar token.
    DoctypeClosingPrefix,
    EndDoctype,
    NotStandalone,
    StartNamespace {
        prefix: Option<String>,
        uri: Option<String>,
    },
    EndNamespace {
        prefix: Option<String>,
    },
    EntityDeclaration(xeme_storage::Box<EntityDeclaration>),
    AttlistDeclaration(xeme_storage::Box<AttributeDeclaration>),
    NotationDeclaration(xeme_storage::Box<NotationDeclaration>),
    ElementDeclaration {
        name: String,
        model: String,
    },
    SkippedEntity {
        name: String,
        parameter: bool,
    },
}

/// Owned namespace undo records, freed when their declaring element closes.
type NamespaceBindings = Vec<(String, Option<String>)>;

#[derive(Debug)]
struct Element {
    name: ElementName,
    raw_encoding: Option<xeme_storage::Box<Vec<lexical::NameEncoding>>>,
    namespace_scope: Option<NonZeroUsize>,
}

/// Keep raw matching and a different expanded spelling in one stack owner.
#[derive(Debug)]
struct ElementName {
    value: String,
    raw_start: usize,
    expanded_end: usize,
}

/// Layout shared by ordinary and arena-backed expanded element names.
struct ElementNameLayout {
    raw_start: usize,
    expanded_end: usize,
    capacity: usize,
}

impl ElementNameLayout {
    /// Reuse a raw-name suffix and reserve one trailing C terminator byte.
    #[inline]
    fn new(expanded: &str, raw: &str) -> Result<Self, AllocError> {
        let expanded_end = expanded.len();
        let suffix = expanded.ends_with(raw);
        let capacity = expanded_end
            .checked_add(if suffix { 0 } else { raw.len() })
            .and_then(|length| length.checked_add(1))
            .ok_or(AllocError::CapacityOverflow)?;
        Ok(Self {
            raw_start: if suffix {
                expanded_end - raw.len()
            } else {
                expanded_end
            },
            expanded_end,
            capacity,
        })
    }

    /// Append only a missing raw spelling after the caller reserves and copies.
    #[inline]
    fn finish(self, mut value: String, raw: &str) -> Result<ElementName, AllocError> {
        debug_assert_eq!(value.len(), self.expanded_end);
        if self.raw_start == self.expanded_end {
            value.try_push_str(raw)?;
        }
        Ok(ElementName {
            value,
            raw_start: self.raw_start,
            expanded_end: self.expanded_end,
        })
    }
}

impl ElementName {
    /// Compare end tags against their decoded raw spelling, before expansion.
    fn raw_name(&self) -> &str {
        &self.value[self.raw_start..]
    }

    /// Return an owned event name without allocating or moving its bytes.
    fn into_event_name(mut self) -> String {
        self.value.truncate(self.expanded_end);
        self.value
    }
}

#[derive(Debug)]
struct Entity {
    // Share immutable bases across declarations and DTD snapshots.
    base: Option<Shared<Vec<u8>>>,
    value: Option<String>,
    system_id: Option<String>,
    public_id: Option<String>,
    notation: Option<String>,
    declared_in_parameter_entity: bool,
    // External value parsers can leave an internal parameter entity open in
    // Expat's shared DTD even after the child is freed. General entities do not
    // need this state; ordinary active recursion uses the source/value stacks.
    value_open: Option<Shared<AtomicBool>>,
}

/// Flags shared by the DTD and its external parameter children. General-content
/// children have independent DTD state even though byte budgets remain shared.
#[derive(Debug)]
struct ParameterState {
    read: AtomicBool,
    declarations_skipped: AtomicBool,
}

#[derive(Debug)]
struct PendingEvent {
    event: Event,
    raw: Option<String>,
}

/// Reserve a replacement table without changing the live table or its secret keys.
fn prepare_salted_map<K: Eq + std::hash::Hash, V>(
    map: &HashMap<K, V>,
    salt: [u8; 16],
) -> Result<HashMap<K, V>, AllocError> {
    let mut replacement = HashMap::with_hasher_in(map.hasher().with_salt(salt), *map.allocator());
    replacement.try_reserve(map.len())?;
    Ok(replacement)
}

/// Move entries into a fully reserved table; keys and values retain their allocator.
fn replace_hash_map<K: Eq + std::hash::Hash, V>(
    map: &mut HashMap<K, V>,
    mut replacement: HashMap<K, V>,
) {
    debug_assert!(replacement.is_empty() && replacement.capacity() >= map.len());
    for (key, value) in map.drain() {
        replacement.insert(key, value);
    }
    *map = replacement;
}

#[derive(Debug)]
struct DefaultAttribute {
    name: String,
    attribute_type: String,
    value: Option<String>,
}

/// Declaration order determines the order of omitted attributes in callbacks.
/// The index keeps repeated type, ID, and duplicate checks independent of that order.
#[derive(Debug)]
struct DefaultAttributes {
    ordered: Vec<DefaultAttribute>,
    by_name: HashMap<String, usize>,
    // Omitted attributes only need declarations with an actual default value.
    default_indices: Vec<usize>,
}

impl DefaultAttributes {
    fn new(allocator: Allocator, salt: [u8; 16]) -> Self {
        let map: HashMap<String, usize> = hash_map(allocator);
        Self {
            ordered: Vec::new_in(allocator),
            by_name: HashMap::with_hasher_in(map.hasher().with_salt(salt), allocator),
            default_indices: Vec::new_in(allocator),
        }
    }

    fn get(&self, name: &str) -> Option<&DefaultAttribute> {
        self.by_name.get(name).map(|&index| &self.ordered[index])
    }

    fn try_insert(&mut self, attribute: DefaultAttribute) -> Result<(), AllocError> {
        debug_assert!(self.get(&attribute.name).is_none());
        let name = attribute.name.try_clone()?;
        // Reserve every container before changing its logical contents so an
        // allocation failure cannot leave an index without its declaration.
        self.ordered
            .try_reserve(1)
            .map_err(|_| AllocError::OutOfMemory)?;
        self.by_name
            .try_reserve(1)
            .map_err(|_| AllocError::OutOfMemory)?;
        let index = self.ordered.len();
        if attribute.value.is_some() {
            self.default_indices.try_reserve(1)?;
            self.default_indices.push(index);
        }
        self.ordered.push(attribute);
        self.by_name.insert(name, index);
        Ok(())
    }
}

impl TryClone for DefaultAttributes {
    fn try_clone(&self) -> Result<Self, AllocError> {
        let mut cloned = Self::new(*self.ordered.allocator(), self.by_name.hasher().salt());
        for attribute in &self.ordered {
            cloned.try_insert(attribute.try_clone()?)?;
        }
        Ok(cloned)
    }
}

impl DefaultAttribute {
    /// Copy imported strings into the receiving parser's allocator.
    fn clone_in(&self, allocator: Allocator) -> Result<Self, AllocError> {
        Ok(Self {
            name: String::try_from_str_in(&self.name, allocator)?,
            attribute_type: String::try_from_str_in(&self.attribute_type, allocator)?,
            value: self
                .value
                .as_deref()
                .map(|value| String::try_from_str_in(value, allocator))
                .transpose()?,
        })
    }
}

impl Entity {
    /// Copy imported storage while preserving the declaration's shared open flag.
    fn clone_in(&self, allocator: Allocator) -> Result<Self, AllocError> {
        let copy = |value: &Option<String>| {
            value
                .as_deref()
                .map(|value| String::try_from_str_in(value, allocator))
                .transpose()
        };
        Ok(Self {
            base: self
                .base
                .as_ref()
                .map(|base| {
                    let mut bytes = Vec::new_in(allocator);
                    xeme_storage::try_extend_from_slice(&mut bytes, base)?;
                    Shared::try_new_in(bytes, allocator)
                })
                .transpose()?,
            value: copy(&self.value)?,
            system_id: copy(&self.system_id)?,
            public_id: copy(&self.public_id)?,
            notation: copy(&self.notation)?,
            declared_in_parameter_entity: self.declared_in_parameter_entity,
            value_open: self.value_open.clone(),
        })
    }

    fn is_value_open(&self) -> bool {
        self.value_open
            .as_ref()
            .is_some_and(|open| open.load(Ordering::Relaxed))
    }

    /// General-content children copy Expat's DTD table. A reserved slot without
    /// a system ID becomes an empty internal value in that copy; parameter
    /// children retain the unfinished slot and its already parsed identifiers.
    fn clone_for_general_child(
        &self,
        parameter_entity: bool,
        allocator: Allocator,
    ) -> Result<Self, AllocError> {
        if self.value.is_none() && self.system_id.is_none() {
            Ok(Self {
                base: None,
                value: Some(String::new_in(allocator)),
                system_id: None,
                public_id: None,
                notation: self.notation.try_clone()?,
                declared_in_parameter_entity: self.declared_in_parameter_entity,
                value_open: parameter_entity
                    .then(|| Shared::try_new_in(AtomicBool::new(false), allocator))
                    .transpose()?,
            })
        } else {
            let mut entity = self.try_clone()?;
            if self.value_open.is_some() {
                entity.value_open = Some(Shared::try_new_in(AtomicBool::new(false), allocator)?);
            }
            Ok(entity)
        }
    }
}

impl TryClone for Entity {
    fn try_clone(&self) -> Result<Self, AllocError> {
        Ok(Self {
            base: self.base.clone(),
            value: self.value.try_clone()?,
            system_id: self.system_id.try_clone()?,
            public_id: self.public_id.try_clone()?,
            notation: self.notation.try_clone()?,
            declared_in_parameter_entity: self.declared_in_parameter_entity,
            value_open: self.value_open.clone(),
        })
    }
}
impl TryClone for DefaultAttribute {
    fn try_clone(&self) -> Result<Self, AllocError> {
        Ok(Self {
            name: self.name.try_clone()?,
            attribute_type: self.attribute_type.try_clone()?,
            value: self.value.try_clone()?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct NativeRawRange {
    start: usize,
    count: NonZeroUsize,
}

/// An incremental, non-validating XML 1.0 parser.
///
/// External entity references produce events for the application to resolve;
/// the parser never performs I/O.
///
/// Parameter entity processing is opt-in and supports
/// references between declarations and nested INCLUDE/IGNORE sections in external
/// DTDs. Internal parameter entities may select a conditional keyword or expand
/// inside entity values in external DTDs and parameter entities, and supply
/// complete lexical tokens and grammar delimiters inside declarations. Names,
/// quoted literals, and references retain their original lexical boundaries;
/// references between declarations must contain complete declarations.
///
/// External references between declaration tokens and in conditional headers
/// load separate DTDs; their declarations are available before grammar resumes.
/// Completed attributes and declarations are emitted before a subsequent child
/// is requested, and a child cannot replace its parent's reserved entity name.
/// External references inside entity values create value children whose output
/// continues the pending declaration, preserving each child's lexical boundary.
/// Encoding declarations in value children must appear before any value content.
#[derive(Debug)]
pub struct Parser {
    config: Config,
    allocator: Allocator,
    decoder: Decoder,
    base: Option<Shared<Vec<u8>>>,
    input_context: Option<InputContext>,
    sources: Vec<Source>,
    pending: Queue<PendingEvent>,
    stack: Vec<Element>,
    // Nonempty undo blocks correspond in order to the declaring elements in
    // `stack`. Elements without declarations leave their ancestors' blocks alone.
    namespace_scopes: Vec<NamespaceBindings>,
    // Only nonempty prefixes live in the salted map. This slot owns the sole
    // current default URI; scope undo records keep the displaced owners.
    namespaces: HashMap<String, String>,
    default_namespace: Option<String>,
    tables: DtdTables,
    shared_tables: OnceLock<Shared<xeme_storage::TryLock<DtdTables>>>,
    parameter_mode: u8,
    foreign_dtd: bool,
    foreign_dtd_pending: Option<dtd::ForeignDtd>,
    in_doctype: bool,
    conditional: dtd::ConditionalState,
    value_state: Option<xeme_storage::Box<value::State>>,
    declarations_skipped: bool,
    doctype_external: Option<ExternalEntityReference>,
    seen_root: bool,
    closed_root: bool,
    seen_doctype: bool,
    declaration_allowed: bool,
    in_cdata: bool,
    final_input: bool,
    finished: bool,
    error: Option<Error>,
    received: usize,
    feed_start_byte: usize,
    #[cfg(test)]
    prolog_bytes_inspected: usize,
    #[cfg(test)]
    default_candidates_visited: usize,
    expanded: Shared<EntityBudget>,
    fragment: bool,
    external_subset: bool,
    // External parsers plus active general-entity sources in their ancestors.
    external_depth: usize,
    entity_chain: Vec<String>,
    active_entities: active::ActiveEntities,
    parameter_state: OnceLock<Shared<ParameterState>>,
    parameter_encoding_initialized: bool,
    active_parameter_reference: Option<(String, Position)>,
    inherited_parameter_depth: usize,
    has_external_subset: bool,
    standalone: bool,
    reparse_deferral: bool,
    last_position: Position,
    current_raw: String,
    native_raw: Option<NativeRawRange>,
    token_scratch: lexical::Buffer,
    raw_attributes: Vec<RawAttribute>,
    tag_scanner: tag::TagScanner,
    event_recycling: EventRecycling,
    expand_internal_entities: bool,
    default_events: bool,
    text_line_boundaries: bool,
    notation_handler_enabled: bool,
    attlist_handler_enabled: bool,
    decoding_error: Option<(ErrorKind, &'static str)>,
    id_attribute_index: Option<usize>,
}

/// Borrow only the fields changed by a nonempty identity Start.
/// Source stays in the parser, and every borrow ends before event delivery.
struct IdentityStartState<'a> {
    source: &'a Source,
    allocator: Allocator,
    limits: &'a Limits,
    namespaces: &'a HashMap<String, String>,
    raw_attributes: &'a mut Vec<RawAttribute>,
    event_recycling: &'a mut EventRecycling,
    stack: &'a mut Vec<Element>,
    id_attribute_index: &'a mut Option<usize>,
    seen_root: &'a mut bool,
    declaration_allowed: &'a mut bool,
}

impl IdentityStartState<'_> {
    /// Keep the selected copy/error order for both borrowed and copied tokens.
    fn lower(
        self,
        name: &str,
        rest: &str,
        position: Position,
        frame: &mut AdapterFrame,
    ) -> Result<(), Error> {
        let mut raw_attrs = std::mem::replace(self.raw_attributes, Vec::new_in(self.allocator));
        if raw_attrs.len() > self.limits.max_attributes {
            return Err(Error {
                kind: ErrorKind::LimitExceeded,
                message: "attribute count limit exceeded",
                position: self.source.position(0),
            });
        }
        self.event_recycling
            .reserve_adapter(&frame.generation, arena::RETAINED_ARENA_BYTES);
        frame.prepare(raw_attrs.len())?;
        let literal_span = !raw_attrs.is_empty() && frame.fits_literal_attributes(rest, name);
        let mut names =
            AttributeNames::new(raw_attrs.len(), self.namespaces.hasher(), self.allocator);
        for (index, attribute) in raw_attrs.iter().enumerate() {
            let (attr_name, value, attribute_offset, _) = attribute.parts(rest);
            names.check(
                attr_name,
                raw_attrs[..index]
                    .iter()
                    .map(|attribute| attribute.name(rest)),
                || {
                    self.source
                        .position_at(1 + name.len() + attribute_offset, 0)
                },
            )?;
            if !literal_span {
                frame.push_attribute(attr_name, value)?;
            }
        }
        if literal_span {
            frame.push_literal_attributes(rest, &raw_attrs)?;
        }
        *self.id_attribute_index = None;
        frame.set_name(name)?;
        *self.seen_root = true;
        *self.declaration_allowed = false;
        let reusable = self.event_recycling.take_name();
        let value = recycling::copy_name(name, reusable, self.allocator)?;
        try_push(
            self.stack,
            Element {
                name: ElementName {
                    raw_start: 0,
                    expanded_end: value.len(),
                    value,
                },
                raw_encoding: None,
                namespace_scope: None,
            },
        )?;
        if raw_attrs.capacity() <= 128 {
            raw_attrs.clear();
            *self.raw_attributes = raw_attrs;
        }
        frame.publish(position);
        Ok(())
    }
}

impl Parser {
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self::try_new_in(config, Allocator::System).expect("XML parser allocation failed")
    }

    /// Construct a parser whose owned data uses the selected allocator.
    pub fn try_new_in(mut config: Config, allocator: Allocator) -> Result<Self, Error> {
        let encoding = config.encoding.take();
        Self::try_new_with_encoding_in(config, encoding.as_deref(), allocator)
    }

    /// Construct a parser from a borrowed encoding name, avoiding a host allocation.
    pub fn try_new_with_encoding_in(
        mut config: Config,
        encoding: Option<&str>,
        allocator: Allocator,
    ) -> Result<Self, Error> {
        config.encoding = None;
        let decoder = Decoder::new(encoding, allocator, &config)?;
        let mut namespaces = hash_map(allocator);
        if config.namespace_separator.is_some() {
            try_insert(
                &mut namespaces,
                string("xml", allocator)?,
                string("http://www.w3.org/XML/1998/namespace", allocator)?,
            )?;
        }
        let mut sources = Vec::new_in(allocator);
        try_push(&mut sources, Source::new(allocator, config.name_rules))?;
        let tables = DtdTables::new(allocator, &config.limits);
        let active_entities = active::ActiveEntities::new(allocator, tables.entities.hasher());
        Ok(Self {
            config,
            allocator,
            decoder,
            input_context: None,
            base: None,
            sources,
            namespaces,
            default_namespace: None,
            pending: Queue::new_in(allocator),
            stack: Vec::new_in(allocator),
            namespace_scopes: Vec::new_in(allocator),
            tables,
            shared_tables: OnceLock::new(),
            parameter_mode: 0,
            foreign_dtd: false,
            foreign_dtd_pending: None,
            in_doctype: false,
            conditional: dtd::ConditionalState::new(allocator),
            value_state: None,
            declarations_skipped: false,
            doctype_external: None,
            seen_root: false,
            closed_root: false,
            seen_doctype: false,
            declaration_allowed: true,
            in_cdata: false,
            final_input: false,
            finished: false,
            error: None,
            received: 0,
            feed_start_byte: 0,
            #[cfg(test)]
            prolog_bytes_inspected: 0,
            #[cfg(test)]
            default_candidates_visited: 0,
            expanded: Shared::try_new_in(EntityBudget::new(), allocator)?,
            fragment: false,
            external_subset: false,
            external_depth: 0,
            entity_chain: Vec::new_in(allocator),
            active_entities,
            parameter_state: OnceLock::new(),
            parameter_encoding_initialized: false,
            active_parameter_reference: None,
            inherited_parameter_depth: 0,
            has_external_subset: false,
            standalone: false,
            reparse_deferral: true,
            last_position: Position {
                line: 1,
                ..Position::default()
            },
            current_raw: String::new_in(allocator),
            native_raw: None,
            token_scratch: lexical::Buffer::new_in(allocator),
            raw_attributes: Vec::new_in(allocator),
            tag_scanner: tag::TagScanner::default(),
            event_recycling: EventRecycling::new(allocator)?,
            expand_internal_entities: true,
            default_events: false,
            text_line_boundaries: false,
            notation_handler_enabled: true,
            attlist_handler_enabled: true,
            decoding_error: None,
            id_attribute_index: None,
        })
    }

    #[must_use]
    pub fn allocator(&self) -> Allocator {
        self.allocator
    }

    /// Iterate both owners; None denotes the default prefix without inventing
    /// an empty String owner or widening the temporary two-reference records.
    fn namespace_bindings(&self) -> impl Iterator<Item = (Option<&String>, &String)> {
        self.namespaces
            .iter()
            .map(|(prefix, uri)| (Some(prefix), uri))
            .chain(self.default_namespace.as_ref().map(|uri| (None, uri)))
    }

    /// Install one authoritative owner, retaining fallible prefixed-map growth.
    fn insert_namespace(
        &mut self,
        prefix: String,
        uri: String,
    ) -> Result<Option<String>, AllocError> {
        if prefix.is_empty() {
            Ok(self.default_namespace.replace(uri))
        } else {
            try_insert(&mut self.namespaces, prefix, uri)
        }
    }

    /// Remove the active binding so its owner can become an undo record.
    fn remove_namespace(&mut self, prefix: &str) -> Option<String> {
        if prefix.is_empty() {
            self.default_namespace.take()
        } else {
            self.namespaces.remove(prefix)
        }
    }

    /// Return this parser's local caller salt, without exposing secret randomized keys.
    /// Parameter children share their DTD seed separately from local namespace and
    /// active-name indexes, so changing another parser's seed does not change this value.
    #[must_use]
    pub fn hash_salt(&self) -> [u8; 16] {
        self.namespaces.hasher().salt()
    }

    /// Replace the caller salt while retaining randomized hash protection.
    ///
    /// Every table is prepared before changing any table, so allocation failure
    /// preserves all contents and the previous salt. Parameter children share
    /// the DTD seed; namespace and active-name indexes retain parser-local seeds.
    /// General children retain independent snapshots. Adapters enforce timing rules.
    #[doc(hidden)]
    pub fn set_hash_salt(&mut self, salt: [u8; 16]) -> Result<(), Error> {
        self.with_dtd_tables(|parser| parser.set_hash_salt_inner(salt))
    }

    fn set_hash_salt_inner(&mut self, salt: [u8; 16]) -> Result<(), Error> {
        if self.hash_salt() == salt && self.tables.salt == salt {
            return Ok(());
        }
        let namespaces = prepare_salted_map(&self.namespaces, salt)?;
        let entities = prepare_salted_map(&self.tables.entities, salt)?;
        let parameters = prepare_salted_map(&self.tables.parameter_entities, salt)?;
        let defaults = prepare_salted_map(&self.tables.defaults, salt)?;
        let active = prepare_salted_map(&self.active_entities.names, salt)?;
        let value_active = self
            .value_state
            .as_ref()
            .map(|state| prepare_salted_map(&state.active.names, salt))
            .transpose()?;
        let mut indexes = Vec::new_in(self.allocator);
        indexes
            .try_reserve_exact(self.tables.defaults.len())
            .map_err(AllocError::from)?;
        for attributes in self.tables.defaults.values() {
            indexes.push(prepare_salted_map(&attributes.by_name, salt)?);
        }
        // No growth can fail after preparation. Rebuild inner indexes before
        // changing the outer table's iteration order; drops return old table
        // storage through its original allocator.
        for (attributes, index) in self.tables.defaults.values_mut().zip(indexes) {
            replace_hash_map(&mut attributes.by_name, index);
        }
        replace_hash_map(&mut self.namespaces, namespaces);
        replace_hash_map(&mut self.tables.entities, entities);
        replace_hash_map(&mut self.tables.parameter_entities, parameters);
        replace_hash_map(&mut self.tables.defaults, defaults);
        replace_hash_map(&mut self.active_entities.names, active);
        if let (Some(state), Some(active)) = (&mut self.value_state, value_active) {
            replace_hash_map(&mut state.active.names, active);
        }
        self.tables.salt = salt;
        Ok(())
    }

    /// Insert owned membership only after reserving the matching stack entry.
    fn push_entity_source(&mut self, source: Source) -> Result<(), Error> {
        self.sources.try_reserve(1).map_err(AllocError::from)?;
        let name = source.entity_name.as_deref().expect("named entity source");
        self.active_entities.insert_source_name(name, true)?;
        self.sources.push(source);
        Ok(())
    }

    fn pop_entity_source(&mut self) -> Source {
        let source = self.sources.pop().expect("entity source exists");
        let name = source.entity_name.as_deref().expect("named entity source");
        self.active_entities.remove_source_name(name);
        source
    }

    fn inherit_entity_name(&mut self, name: String) -> Result<(), Error> {
        self.entity_chain.try_reserve(1).map_err(AllocError::from)?;
        self.active_entities.insert_source_name(&name, false)?;
        self.entity_chain.push(name);
        Ok(())
    }

    fn is_active_source_name(&self, name: &str) -> bool {
        self.active_entities.contains(name, false)
    }

    /// Change the protocol encoding before receiving input, preserving parser options.
    pub fn set_encoding(&mut self, encoding: Option<&str>) -> Result<(), Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.received != 0 || self.final_input {
            return Err(self.err(
                ErrorKind::Finished,
                "encoding cannot change after input begins",
            ));
        }
        // This initialization setter is transactional: an allocation failure must
        // leave the previous decoder available for autodetection during parsing.
        self.decoder = Decoder::new(encoding, self.allocator, &self.config)?;
        Ok(())
    }

    /// Update the encoding name after an adapter has finished or aborted parsing.
    /// Leaves the active decoder, custom map, buffered input, and positions intact.
    /// The adapter must never feed this parser again; the core cannot verify that.
    #[doc(hidden)]
    pub fn set_completed_encoding(&mut self, encoding: Option<&str>) -> Result<(), Error> {
        self.decoder.set_completed_encoding(encoding)
    }

    /// Construct a parser for an application-provided external entity.
    ///
    /// Pass the context from [`EventKind::ExternalEntityReference`]. Namespace
    /// bindings, declarations, recursion tracking, and the expansion budget are
    /// inherited. The application supplies the child's input.
    /// A `None` context creates an external DTD parser, or a value parser when
    /// resolving a reference inside an entity value. DTD children immediately share
    /// committed declarations, including declarations preceding an error. Value
    /// children append their output to the suspended declaration. Process each
    /// child before requesting the parent's next event.
    pub fn external_child(
        &self,
        context: Option<&str>,
        encoding: Option<std::string::String>,
    ) -> Result<Self, Error> {
        self.external_child_with_encoding(context, encoding.as_deref())
    }

    /// Construct an external entity parser without allocating an encoding argument.
    pub fn external_child_with_encoding(
        &self,
        context: Option<&str>,
        encoding: Option<&str>,
    ) -> Result<Self, Error> {
        if self.external_depth >= self.config.limits.max_entity_depth {
            return Err(self.err(
                ErrorKind::LimitExceeded,
                "external entity nesting limit exceeded",
            ));
        }
        // Parameter children publish into one family immediately. General-content
        // children copy a snapshot while the parent's tables are briefly guarded.
        let owner = if context.is_none() {
            Some(self.ensure_shared_tables()?.clone())
        } else {
            self.shared_tables.get().cloned()
        };
        let snapshot = if context.is_some() {
            owner
                .as_ref()
                .map(|owner| owner.try_lock().ok_or_else(|| self.dtd_busy()))
                .transpose()?
        } else {
            None
        };
        let tables = snapshot.as_deref().unwrap_or(&self.tables);
        self.charge_external_child_storage(context, encoding)?;
        if context.is_some() {
            self.charge_dtd_snapshot(tables)?;
        }
        // The stored config contains only inline values, so this cannot allocate.
        debug_assert!(self.config.encoding.is_none());
        let mut child =
            Self::try_new_with_encoding_in(self.config.clone(), encoding, self.allocator)?;
        child.set_hash_salt_inner(self.hash_salt())?;
        child
            .decoder
            .inherit_map(&self.decoder, &mut child.sources[0])?;
        if context.is_some() && owner.is_some() {
            // The DTD family's salt may differ from this parser's local index salt.
            child.tables = tables.empty_like();
            for (name, entity) in &tables.entities {
                try_insert(
                    &mut child.tables.entities,
                    name.try_clone()?,
                    entity.clone_for_general_child(false, self.allocator)?,
                )?;
            }
            for (name, attributes) in &tables.defaults {
                try_insert(
                    &mut child.tables.defaults,
                    name.try_clone()?,
                    attributes.try_clone()?,
                )?;
            }
            for (name, entity) in &tables.parameter_entities {
                try_insert(
                    &mut child.tables.parameter_entities,
                    name.try_clone()?,
                    entity.clone_for_general_child(true, self.allocator)?,
                )?;
            }
            child.tables.max_default_attributes = tables.max_default_attributes;
            // General-content children form independent families, with their own caps.
            child.tables.max_entities = child.config.limits.max_entities;
            child.tables.max_attributes = child.config.limits.max_attributes;
            child.publish_copied_tables()?;
        }
        drop(snapshot);
        if context.is_none() {
            child.shared_tables = OnceLock::from(owner.expect("parameter DTD owner"));
        }
        child.namespaces.clear();
        child.default_namespace = self.default_namespace.try_clone()?;
        for (prefix, uri) in &self.namespaces {
            try_insert(&mut child.namespaces, prefix.try_clone()?, uri.try_clone()?)?;
        }
        child.expanded = self.expanded.clone();
        child.fragment = true;
        child.external_subset = context.is_none();
        // General children replace the parent's source stack. Carry its active
        // internal entities across that boundary; DTD children account for their
        // parameter sources separately in `inherit_parameter_context`.
        child.external_depth = self.external_depth
            + if context.is_some() {
                self.sources.len()
            } else {
                1
            };
        child.inherited_parameter_depth = self.inherited_parameter_depth;
        child.declarations_skipped = self.declarations_skipped();
        if context.is_none() {
            child.parameter_state = OnceLock::from(self.shared_parameter_state()?.clone());
        }
        child.expand_internal_entities = self.expand_internal_entities;
        child.default_events = self.default_events;
        child.text_line_boundaries = self.text_line_boundaries;
        child.notation_handler_enabled = self.notation_handler_enabled;
        child.attlist_handler_enabled = self.attlist_handler_enabled;
        child.parameter_mode = self.parameter_mode;
        child.has_external_subset = self.has_external_subset;
        child.standalone = self.standalone;
        child.reparse_deferral = self.reparse_deferral;
        for name in &self.entity_chain {
            child.inherit_entity_name(name.try_clone()?)?;
        }
        if context.is_none() && !self.inherit_value_context(&mut child)? {
            self.inherit_parameter_context(&mut child)?;
        }
        if let Some(context) = context {
            for part in context.split('\u{c}').filter(|part| !part.is_empty()) {
                if let Some((prefix, uri)) = part.split_once('=') {
                    if uri.is_empty() {
                        child.remove_namespace(prefix);
                    } else {
                        child.insert_namespace(
                            string(prefix, self.allocator)?,
                            string(uri, self.allocator)?,
                        )?;
                    }
                } else if !child.is_active_source_name(part) {
                    if child.entity_chain.len() >= child.config.limits.max_entity_depth {
                        return Err(self.err(
                            ErrorKind::LimitExceeded,
                            "external entity context nesting limit exceeded",
                        ));
                    }
                    child.inherit_entity_name(string(part, self.allocator)?)?;
                }
            }
        }
        Ok(child)
    }

    fn new_parameter_value_open(&self, needed: bool) -> Result<Option<Shared<AtomicBool>>, Error> {
        if !needed {
            return Ok(None);
        }
        self.charge_expansion(size_of::<AtomicBool>())?;
        Ok(Some(Shared::try_new_in(
            AtomicBool::new(false),
            self.allocator,
        )?))
    }

    /// Charge the DTD copies needed by a general-content child before allocating.
    /// Include empty declarations so repeated empty external references cannot
    /// bypass the work limit while copying large DTDs.
    fn charge_dtd_snapshot(&self, tables: &DtdTables) -> Result<(), Error> {
        for entity in tables.parameter_entities.values() {
            if entity.value.is_none() && entity.system_id.is_none() {
                self.charge_expansion(size_of::<AtomicBool>())?;
            }
        }
        for (name, entity) in tables.entities.iter().chain(&tables.parameter_entities) {
            self.charge_expansion(size_of::<(String, Entity)>())?;
            if entity.value_open.is_some() {
                self.charge_expansion(size_of::<AtomicBool>())?;
            }
            self.charge_expansion(name.len())?;
            for value in [
                &entity.value,
                &entity.system_id,
                &entity.public_id,
                &entity.notation,
            ]
            .into_iter()
            .flatten()
            {
                self.charge_expansion(value.len())?;
            }
        }
        for (element, attributes) in &tables.defaults {
            self.charge_expansion(size_of::<(String, DefaultAttributes)>())?;
            self.charge_expansion(element.len())?;
            for attribute in &attributes.ordered {
                self.charge_expansion(size_of::<DefaultAttribute>())?;
                self.charge_expansion(attribute.name.len())?;
                self.charge_expansion(attribute.attribute_type.len())?;
                if let Some(value) = &attribute.value {
                    self.charge_expansion(size_of::<usize>())?;
                    self.charge_expansion(value.len())?;
                }
                // The ordered declaration and its lookup index each own a name.
                self.charge_expansion(size_of::<(String, usize)>())?;
                self.charge_expansion(attribute.name.len())?;
            }
        }
        Ok(())
    }

    fn charge_external_child_storage(
        &self,
        context: Option<&str>,
        encoding: Option<&str>,
    ) -> Result<(), Error> {
        for (prefix, uri) in self.namespace_bindings() {
            // Preserve the logical key/URI charge even for the dedicated slot.
            self.charge_expansion(size_of::<(String, String)>())?;
            self.charge_expansion(prefix.map_or(0, |prefix| prefix.len()))?;
            self.charge_expansion(uri.len())?;
        }
        for name in &self.entity_chain {
            self.charge_expansion(size_of::<String>())?;
            self.charge_expansion(name.len())?;
        }
        if let Some(context) = context {
            // Also account for delimiter scanning and entries that remove or
            // replace a binding, rather than allocating a distinct final entry.
            self.charge_expansion(context.len())?;
            for part in context.split('\u{c}').filter(|part| !part.is_empty()) {
                self.charge_expansion(if part.contains('=') {
                    size_of::<(String, String)>()
                } else {
                    size_of::<String>()
                })?;
            }
        }
        if let Some(encoding) = encoding {
            self.charge_expansion(encoding.len())?;
        }
        let (encoding_name, encoding_map) = self.decoder.inherited_map_bytes(encoding);
        self.charge_expansion(encoding_name)?;
        self.charge_expansion(encoding_map)?;
        Ok(())
    }

    /// Select parameter references between declarations: 0 never, 1 unless the
    /// root document is standalone, 2 always. Explicitly selecting mode 1 on an
    /// external DTD child enables its references even for a standalone root.
    /// Internal references in an external DTD's entity values are always expanded.
    pub fn set_param_entity_parsing(&mut self, mode: u8) -> bool {
        if mode > 2 || self.received != 0 {
            return false;
        }
        self.parameter_mode = mode;
        true
    }
    pub fn set_use_foreign_dtd(&mut self, enabled: bool) -> bool {
        if self.received != 0 {
            return false;
        }
        self.foreign_dtd = enabled;
        true
    }
    #[must_use]
    pub fn is_external_subset(&self) -> bool {
        self.external_subset
    }
    /// Validate a completed external DTD. Same-family definitions are already visible;
    /// unrelated DTDs are imported with existing declarations taking precedence.
    pub fn merge_external_subset(&mut self, child: &Self) -> Result<(), Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let result = self.merge_external_subset_inner(child);
        if let Err(error) = result {
            self.error = Some(error);
            self.pending.clear();
        }
        result
    }

    fn merge_external_subset_inner(&mut self, child: &Self) -> Result<(), Error> {
        if child.is_external_value() && child.finished {
            return Ok(());
        }
        if !child.external_subset || !child.finished {
            return Err(self.err(
                ErrorKind::ExternalEntityHandling,
                "external subset has not completed",
            ));
        }
        let same_family = self
            .shared_tables
            .get()
            .zip(child.shared_tables.get())
            .is_some_and(|(left, right)| std::ptr::eq(&**left, &**right));
        if !same_family {
            self.ensure_shared_tables()?;
            let child_owner = child.shared_tables.get();
            let child_guard = child_owner
                .map(|owner| owner.try_lock().ok_or_else(|| self.dtd_busy()))
                .transpose()?;
            let tables = child_guard.as_deref().unwrap_or(&child.tables);
            self.with_dtd_tables(|parser| parser.import_dtd_tables(tables))?;
        }
        self.set_declarations_skipped(self.declarations_skipped() || child.declarations_skipped());
        self.standalone |= child.standalone;
        Ok(())
    }

    /// Charge only new declarations before copying an unrelated DTD into this family.
    fn charge_dtd_import(&self, tables: &DtdTables) -> Result<(), Error> {
        for (source, destination) in [
            (&tables.entities, &self.tables.entities),
            (&tables.parameter_entities, &self.tables.parameter_entities),
        ] {
            for (name, entity) in source {
                if destination.contains_key(name) {
                    continue;
                }
                self.charge_expansion(size_of::<(String, Entity)>())?;
                self.charge_expansion(name.len())?;
                if let Some(base) = &entity.base {
                    self.charge_expansion(size_of::<Vec<u8>>())?;
                    self.charge_expansion(base.len())?;
                }
                for value in [
                    &entity.value,
                    &entity.system_id,
                    &entity.public_id,
                    &entity.notation,
                ]
                .into_iter()
                .flatten()
                {
                    self.charge_expansion(value.len())?;
                }
            }
        }
        for (name, attributes) in &tables.defaults {
            let existing = self.tables.defaults.get(name);
            if existing.is_none() {
                self.charge_expansion(size_of::<(String, DefaultAttributes)>())?;
                self.charge_expansion(name.len())?;
            }
            for attribute in &attributes.ordered {
                if existing.is_some_and(|attributes| attributes.get(&attribute.name).is_some()) {
                    continue;
                }
                self.charge_expansion(size_of::<DefaultAttribute>())?;
                self.charge_expansion(size_of::<(String, usize)>())?;
                self.charge_expansion(attribute.name.len())?;
                self.charge_expansion(attribute.name.len())?;
                self.charge_expansion(attribute.attribute_type.len())?;
                if let Some(value) = &attribute.value {
                    self.charge_expansion(size_of::<usize>())?;
                    self.charge_expansion(value.len())?;
                }
            }
        }
        Ok(())
    }

    fn import_dtd_tables(&mut self, tables: &DtdTables) -> Result<(), Error> {
        self.charge_dtd_import(tables)?;
        for (name, entity) in &tables.entities {
            if !self.tables.entities.contains_key(name) {
                if self.tables.entities.len() + self.tables.parameter_entities.len()
                    >= self.entity_limit()
                {
                    return self.fail(
                        ErrorKind::LimitExceeded,
                        "entity declaration count limit exceeded",
                    );
                }
                try_insert(
                    &mut self.tables.entities,
                    string(name, self.allocator)?,
                    entity.clone_in(self.allocator)?,
                )?;
            }
        }
        for (name, entity) in &tables.parameter_entities {
            if !self.tables.parameter_entities.contains_key(name) {
                if self.tables.entities.len() + self.tables.parameter_entities.len()
                    >= self.entity_limit()
                {
                    return self.fail(
                        ErrorKind::LimitExceeded,
                        "entity declaration count limit exceeded",
                    );
                }
                try_insert(
                    &mut self.tables.parameter_entities,
                    string(name, self.allocator)?,
                    entity.clone_in(self.allocator)?,
                )?;
            }
        }
        let attribute_limit = self.default_attribute_limit();
        for (name, attributes) in &tables.defaults {
            if !self.tables.defaults.contains_key(name) {
                try_insert(
                    &mut self.tables.defaults,
                    string(name, self.allocator)?,
                    DefaultAttributes::new(self.allocator, self.tables.salt),
                )?;
            }
            let target = self
                .tables
                .defaults
                .get_mut(name)
                .expect("default list exists");
            for attribute in &attributes.ordered {
                if target.get(&attribute.name).is_none() {
                    if target.ordered.len() >= attribute_limit {
                        return Err(self.err(
                            ErrorKind::LimitExceeded,
                            "default attribute count limit exceeded",
                        ));
                    }
                    target.try_insert(attribute.clone_in(self.allocator)?)?;
                    self.tables.max_default_attributes =
                        self.tables.max_default_attributes.max(target.ordered.len());
                }
            }
        }
        Ok(())
    }

    /// Bytes that can still be supplied to this parser's original input source.
    ///
    /// The representable source bound leaves room for one-based line numbers,
    /// even when an explicit input limit is larger than a signed pointer offset.
    /// External children have their own source offsets and input allowance.
    #[doc(hidden)]
    #[must_use]
    pub fn input_bytes_remaining(&self) -> usize {
        self.config
            .limits
            .max_total_bytes
            .min(isize::MAX as usize)
            .saturating_sub(self.received)
    }

    /// Cumulative work allowance based on consumed original root-input bytes.
    ///
    /// Adapters use this policy for their own work counters. The threshold may
    /// saturate, but callers must still reject overflow of the charged counter.
    #[doc(hidden)]
    #[must_use]
    pub fn work_bytes_limit(&self, initial: usize) -> usize {
        self.expanded
            .work_limit(initial, self.config.limits.max_work_amplification)
    }

    /// Append input without calling user code. Drain events before feeding more data.
    pub fn feed(&mut self, bytes: &[u8], is_final: bool) -> Result<(), Error> {
        if self.input_context.is_some() {
            return match self.feed_with_input_context(bytes, is_final, 1024) {
                Ok(result) => result,
                Err(_) => self.fail(ErrorKind::NoMemory, "out of memory"),
            };
        }
        self.feed_inner(bytes, is_final, false)
    }

    /// Use the root native input owner for C context. Enable before the first feed.
    #[doc(hidden)]
    pub fn enable_input_context(&mut self) {
        assert_eq!(self.received, 0);
        assert!(self.input_context.is_none());
        self.input_context = Some(InputContext::new(
            self.allocator,
            &self.decoder,
            &mut self.sources[0],
        ));
    }

    /// Retain original input before decoding. The outer result reports allocation
    /// failures while retaining that input; the inner result reports feed errors.
    #[doc(hidden)]
    pub fn feed_with_input_context(
        &mut self,
        bytes: &[u8],
        is_final: bool,
        history: usize,
    ) -> Result<Result<(), Error>, AllocError> {
        if self.error.is_some() || self.final_input || bytes.len() > self.input_bytes_remaining() {
            return Ok(self.feed_inner(bytes, is_final, false));
        }
        let retain_from = self.input_context_byte_index().saturating_sub(history);
        let minimum = history.min(self.config.limits.max_total_bytes);
        let native = self
            .input_context
            .as_mut()
            .expect("context input enabled")
            .preserve(
                &mut self.decoder,
                &mut self.sources[0],
                bytes,
                retain_from,
                minimum,
            )?;
        Ok(self.feed_inner(bytes, is_final, native))
    }

    /// Original input bytes and their absolute starting offset. Any pointer
    /// derived by the C adapter is valid only until the next feed or destruction.
    #[doc(hidden)]
    #[must_use]
    pub fn input_context(&self) -> (&[u8], usize) {
        self.input_context
            .as_ref()
            .map_or((&[], 0), |context| context.view(&self.sources[0]))
    }

    fn declaration_context(&self) -> DeclarationContext {
        if self.fragment && !self.is_external_value() {
            DeclarationContext::Text
        } else {
            DeclarationContext::Document
        }
    }

    fn feed_inner(&mut self, bytes: &[u8], is_final: bool, native: bool) -> Result<(), Error> {
        if let Some(error) = &self.error {
            return Err(*error);
        }
        if self.final_input {
            return self.fail(ErrorKind::Finished, "input has already been finalized");
        }
        if bytes.len() > self.input_bytes_remaining() {
            return self.fail(ErrorKind::LimitExceeded, "input byte limit exceeded");
        }
        self.feed_start_byte = self.sources[0].position(0).byte_index;
        // The remaining-input check proves this addition and every original
        // source coordinate fit, before decoder or input state is changed.
        self.received += bytes.len();
        if self.fragment
            && let Err(error) = self.charge_expansion(bytes.len())
        {
            self.error = Some(error);
            return Err(error);
        }
        self.final_input = is_final;
        self.mark_parameter_read();
        if self.decoding_error.is_some() {
            if let Err(error) = self.decoder.append_pending(bytes) {
                self.error = Some(error);
                return Err(error);
            }
            return Ok(());
        }
        let declaration_context = self.declaration_context();
        let decoded = if native {
            self.input_context
                .as_mut()
                .expect("context input enabled")
                .decode(
                    &mut self.decoder,
                    &mut self.sources[0],
                    is_final,
                    self.config.limits.max_token_bytes,
                    self.fragment && !self.external_subset,
                    declaration_context,
                )
        } else {
            self.decoder.feed(
                bytes,
                is_final,
                &mut self.sources[0],
                self.config.limits.max_token_bytes,
                self.fragment && !self.external_subset,
                declaration_context,
            )
        };
        if let Err(error) = decoded {
            self.decoding_error = Some((error.kind, error.message));
        }
        // Encoding detection may retain a complete BOM while awaiting a text
        // declaration. These bytes already form a token, even on a nonfinal feed.
        let prefix = if let Some(context) = &self.input_context {
            context.pending_bom_len(
                &self.decoder,
                &self.sources[0],
                self.fragment && !self.external_subset,
            )
        } else {
            self.decoder
                .pending_bom_len(self.fragment && !self.external_subset)
        };
        let bytes = self.sources[0].unaccounted_prefix(prefix);
        if !self.expanded.account(bytes, self.fragment, true) {
            return self.fail(
                ErrorKind::LimitExceeded,
                "entity amplification limit exceeded",
            );
        }
        self.sources[0].mark_accounted(bytes);
        Ok(())
    }

    /// Lazily share DTD flags before returning the first parameter child.
    /// Allocate outside OnceLock initialization so allocator callbacks cannot
    /// deadlock its initialization lock. A concurrent initializer may win; its
    /// state is then used and the losing allocation is freed by the same suite.
    fn shared_parameter_state(&self) -> Result<&Shared<ParameterState>, Error> {
        if self.parameter_state.get().is_none() {
            self.charge_expansion(size_of::<ParameterState>())?;
            let state = Shared::try_new_in(
                ParameterState {
                    read: AtomicBool::new(false),
                    declarations_skipped: AtomicBool::new(self.declarations_skipped),
                },
                self.allocator,
            )?;
            let _ = self.parameter_state.set(state);
        }
        Ok(self
            .parameter_state
            .get()
            .expect("initialized parameter state"))
    }

    fn declarations_skipped(&self) -> bool {
        self.parameter_state
            .get()
            .map_or(self.declarations_skipped, |state| {
                state.declarations_skipped.load(Ordering::Relaxed)
            })
    }

    fn set_declarations_skipped(&mut self, skipped: bool) {
        self.declarations_skipped = skipped;
        if let Some(state) = self.parameter_state.get() {
            state.declarations_skipped.store(skipped, Ordering::Relaxed);
        }
    }

    fn mark_parameter_read(&mut self) {
        if !self.parameter_encoding_initialized && self.decoder.protocol_encoding_ready() {
            self.parameter_encoding_initialized = true;
            if let Some(state) = self.parameter_state.get() {
                state.read.store(true, Ordering::Relaxed);
            }
        }
    }

    /// The unresolved encoding name, available after an `UnknownEncoding` error.
    #[must_use]
    pub fn unknown_encoding(&self) -> Option<&str> {
        self.decoder.unknown_encoding()
    }
    /// Supply a single-byte decoder and retry buffered input without feeding it again.
    ///
    /// Each entry is a Unicode scalar value or `-1` for an undefined byte.
    /// ASCII markup must retain its meaning; multibyte custom encodings are rejected.
    pub fn set_encoding_map(&mut self, name: &str, map: [i32; 256]) -> Result<(), Error> {
        self.install_encoding_map(name, map, false)
    }

    /// Install a map with two- to four-byte sequences resolved by the application.
    ///
    /// After draining available events, inspect [`Self::encoding_conversion`] and
    /// supply its result using [`Self::resolve_encoding_conversion`]. No callback
    /// runs while the parser is borrowed. Entries `-2` through `-4` give byte widths.
    /// Converted values may include ASCII. Original encoded spellings retain their
    /// lexical roles, so a converted `<` is data rather than an opening delimiter.
    pub fn set_multibyte_encoding_map(&mut self, name: &str, map: [i32; 256]) -> Result<(), Error> {
        self.install_encoding_map(name, map, true)
    }

    fn install_encoding_map(
        &mut self,
        name: &str,
        map: [i32; 256],
        multibyte: bool,
    ) -> Result<(), Error> {
        if let Some(error) = self.error.filter(|error| error.kind == ErrorKind::NoMemory) {
            return Err(error);
        }
        if self
            .decoding_error
            .is_none_or(|(kind, _)| kind != ErrorKind::UnknownEncoding)
        {
            return Err(self.err(
                ErrorKind::UnknownEncoding,
                "no unresolved encoding is pending",
            ));
        }
        if let Err(error) = self
            .decoder
            .install_map(name, map, multibyte, &mut self.sources[0])
        {
            if error.kind == ErrorKind::NoMemory {
                self.error = Some(error);
            }
            return Err(error);
        }
        self.mark_parameter_read();
        self.error = None;
        self.decoding_error = None;
        self.resume_decoder();
        Ok(())
    }

    /// Return an owned conversion request after available XML events are drained.
    #[must_use]
    pub fn encoding_conversion(&self) -> Option<EncodingConversion> {
        if self.error.is_some() || self.decoding_error.is_some() {
            return None;
        }
        self.decoder.conversion().map(|(bytes, length)| {
            let mut position = self.sources[0].end_position();
            position.byte_count = usize::from(length);
            EncodingConversion {
                bytes,
                length,
                position,
            }
        })
    }

    /// Resolve the pending sequence to an XML character in the BMP, or pass `-1` for invalid data.
    ///
    /// Converted ASCII remains distinct from raw ASCII syntax until semantic decoding.
    pub fn resolve_encoding_conversion(&mut self, value: i32) -> Result<(), Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if let Err(mut error) = self.decoder.resolve_conversion(value, &mut self.sources[0]) {
            error.position = self.sources[0].end_position();
            self.error = Some(error);
            return Err(error);
        }
        self.resume_decoder();
        Ok(())
    }

    /// Resume buffered input after a custom encoding map or conversion is supplied.
    /// Stage decoding errors so preceding XML events can still be delivered first.
    fn resume_decoder(&mut self) {
        let declaration_context = self.declaration_context();
        if let Err(error) = self.decoder.feed(
            &[],
            self.final_input,
            &mut self.sources[0],
            self.config.limits.max_token_bytes,
            self.fragment && !self.external_subset,
            declaration_context,
        ) {
            self.decoding_error = Some((error.kind, error.message));
        }
    }

    /// Return an owned event and a token for an adapter that returns its storage.
    ///
    /// The token requires no allocation, cannot be cloned, and identifies this
    /// parser generation without borrowing the parser.
    #[doc(hidden)]
    #[inline(always)]
    pub fn next_event_for_recycling(&mut self) -> Result<Option<(Event, RecyclingToken)>, Error> {
        Ok(self
            .next_event()?
            .map(|event| (event, self.event_recycling.token())))
    }

    /// Write an owned event into adapter storage and return its recycling token.
    ///
    /// Clears any previous event before parsing. The slot belongs to the caller;
    /// no reference into this parser is retained when the operation returns.
    #[doc(hidden)]
    #[inline(always)]
    pub fn next_event_for_recycling_into(
        &mut self,
        output: &mut Option<Event>,
    ) -> Result<Option<RecyclingToken>, Error> {
        *output = None;
        self.next_event_into(output)?;
        Ok(output.as_ref().map(|_| self.event_recycling.token()))
    }

    /// Return the original attribute storage after an adapter finishes its callback.
    ///
    /// The token rejects accidental returns to another parser, including a reset
    /// parser at the same address. The adapter must return the original buffers:
    /// substituting foreign storage can violate allocator routing and accounting.
    /// The token does not verify buffer identity. Each allocation retains its
    /// original allocator, and rejected or oversized storage is dropped.
    /// This operation never allocates and retains at most 64 KiB of capacity.
    #[doc(hidden)]
    pub fn recycle_attributes(&mut self, token: RecyclingToken, attributes: Vec<Attribute>) {
        self.event_recycling.recycle(token, attributes);
    }

    /// Return the original start-event name and attributes after the callback.
    ///
    /// The generation and original-owner contract of [`Self::recycle_attributes`]
    /// applies. At most two name capacities of 4 KiB share its 64 KiB total budget.
    #[doc(hidden)]
    pub fn recycle_start_element(
        &mut self,
        token: RecyclingToken,
        name: String,
        attributes: Vec<Attribute>,
    ) {
        self.event_recycling.recycle_start(token, name, attributes);
    }

    /// Return an original end-event name under the start-event recycling contract.
    #[doc(hidden)]
    pub fn recycle_end_element(&mut self, token: RecyclingToken, name: String) {
        self.event_recycling.recycle_end(token, name);
    }

    /// Return the next event, or `None` when input or a custom conversion is needed,
    /// or parsing is done. Inspect [`Self::encoding_conversion`] before feeding
    /// more input when using a multibyte custom map.
    pub fn next_event(&mut self) -> Result<Option<Event>, Error> {
        let mut output = None;
        self.next_event_into(&mut output)?;
        Ok(output)
    }

    fn next_event_into(&mut self, output: &mut Option<Event>) -> Result<(), Error> {
        self.next_delivery_into(&mut EventOutput {
            event: output,
            frame: None,
            c_text_context: false,
        })
    }

    /// Create allocation-free detached adapter storage for this parser generation.
    #[doc(hidden)]
    pub fn adapter_frame(&self) -> AdapterFrame {
        AdapterFrame::new(self.allocator, self.event_recycling.token())
    }

    /// Fill caller-owned event/frame slots without retaining a parser reference.
    /// The frame must come from this parser; foreign generations use owned events.
    #[doc(hidden)]
    pub fn next_event_for_adapter_into(
        &mut self,
        event: &mut Option<Event>,
        frame: &mut AdapterFrame,
    ) -> Result<Option<RecyclingToken>, Error> {
        self.next_event_for_adapter_mode_into(event, frame, false)
    }

    /// Fill event and frame slots for a C adapter that retains the original input.
    /// Native text frames contain byte offsets into that input. The adapter must
    /// validate those offsets and read the input directly; calling
    /// [`AdapterFrame::text_bytes`] on these frames panics.
    #[doc(hidden)]
    pub fn next_event_for_c_text_context_into(
        &mut self,
        event: &mut Option<Event>,
        frame: &mut AdapterFrame,
    ) -> Result<Option<RecyclingToken>, Error> {
        self.next_event_for_adapter_mode_into(event, frame, true)
    }

    /// Whether the already consumed tag has another callback to deliver.
    /// C hosts finish these callbacks before honoring suspension or abortion.
    #[doc(hidden)]
    pub fn has_pending_tag_event(&self) -> bool {
        self.pending.front().is_some_and(|pending| {
            matches!(
                pending.event.kind,
                EventKind::StartElement { .. }
                    | EventKind::EndElement { .. }
                    | EventKind::StartNamespace { .. }
                    | EventKind::EndNamespace { .. }
            )
        })
    }

    /// Position for a C host between successful or suspended parsing calls.
    /// DTD continuations can retain lookahead, and internal replacements use
    /// reference-anchored positions instead of the physical source cursor.
    #[doc(hidden)]
    pub fn position_between_callbacks(&self, suspended: bool) -> Position {
        if self.sources.len() != 1 || self.in_doctype || self.external_subset {
            return self.last_position;
        }
        let mut position = self.source().position(0);
        if suspended && !self.seen_root && !self.fragment {
            // Expat's prolog processor retains the event's byte range while
            // its line/column cursor advances through the consumed token.
            position.byte_index = self.last_position.byte_index;
            position.byte_count = self.last_position.byte_count;
        }
        position
    }

    fn next_event_for_adapter_mode_into(
        &mut self,
        event: &mut Option<Event>,
        frame: &mut AdapterFrame,
        c_text_context: bool,
    ) -> Result<Option<RecyclingToken>, Error> {
        *event = None;
        frame.clear();
        if !self.event_recycling.accepts(&frame.generation) {
            return self.next_event_for_recycling_into(event);
        }
        let mut output = EventOutput {
            event,
            frame: Some(frame),
            c_text_context,
        };
        match self.next_native_character_data_for_c(&mut output) {
            Ok(true) => {}
            Ok(false) => self.next_delivery_into(&mut output)?,
            Err(error) => self.finish_event_error(error, &mut output)?,
        }
        Ok((event.is_some() || frame.active).then(|| self.event_recycling.token()))
    }

    /// Deliver native text or a scalar reference without re-entering XML grammar.
    /// Every callback still commits its own allocation, raw span and work credit.
    /// Uncertain input and parser continuations retain the ordinary event path.
    fn next_native_character_data_for_c(
        &mut self,
        output: &mut EventOutput<'_>,
    ) -> Result<bool, Error> {
        if !output.c_text_context || !self.text_line_boundaries || self.sources.len() != 1 {
            return Ok(false);
        }
        let source = &self.sources[0];
        let text = source.remaining();
        if text.is_empty() || matches!(text.as_bytes()[0], b'<' | b']' | b'\r') {
            return Ok(false);
        }
        if self.error.is_some()
            || !self.pending.is_empty()
            || self.stack.is_empty()
            || self.fragment
            || self.external_subset
            || self.in_doctype
            || self.in_cdata
            || self.shared_tables.get().is_some()
            || self.foreign_dtd_pending.is_some()
            || self.active_parameter_reference.is_some()
            || self.value_state.is_some()
            || self.has_header_composition()
            || self.has_declaration_composition()
            || self.input_context.is_none()
            || source.has_conversions()
            || !source.accounted_to_cursor()
        {
            return Ok(false);
        }
        let Some(start) = source.native_utf8_byte_index() else {
            return Ok(false);
        };
        if text.starts_with('&') {
            if self.reparse_deferral
                && !self.is_source_final()
                && source.should_defer(self.config.limits.max_token_bytes)
            {
                return Ok(false);
            }
            let Some(reference) =
                reference::ScalarReference::scan(text, self.config.limits.max_token_bytes)
            else {
                return Ok(false);
            };
            let position = source.position(reference.bytes);
            self.native_raw = Some(NativeRawRange {
                start,
                count: NonZeroUsize::new(reference.bytes).unwrap(),
            });
            self.account_source_bytes(reference.bytes)?;
            if reference.predefined {
                let accepted = if let Some(budget) = self.expanded.get_mut() {
                    budget.account_mut(1, true, false)
                } else {
                    self.expanded.account(1, true, false)
                };
                if !accepted {
                    return Err(self.err(
                        ErrorKind::LimitExceeded,
                        "entity amplification limit exceeded",
                    ));
                }
            }
            // The plan proves a native ASCII spelling without line breaks;
            // source and predefined-entity charges are complete before delivery.
            self.source_mut().consume_ascii_tag(reference.bytes);
            let mut bytes = [0; 4];
            let frame = output.frame.as_deref_mut().unwrap();
            frame.prepare_text(reference.character.encode_utf8(&mut bytes))?;
            frame.publish(position);
            self.last_position = position;
            return Ok(true);
        }
        // Inspect one byte beyond the arena bound to distinguish an actual
        // delimiter from a truncated long line. Never split a UTF-8 scalar.
        let limit = text.floor_char_boundary((arena::MAX_ARENA_BYTES + 1).min(text.len()));
        let Some(plan) = text::TextPlan::scan(&text[..limit], false) else {
            return Ok(false);
        };
        let count = plan.end;
        if count == 0 || count > arena::MAX_ARENA_BYTES || (count == limit && limit < text.len()) {
            return Ok(false);
        }
        let position = source.position(count);
        let frame = output.frame.as_deref_mut().unwrap();
        if count > arena::INLINE_TEXT_BYTES {
            self.event_recycling
                .reserve_adapter(&frame.generation, arena::RETAINED_ARENA_BYTES);
        }
        frame.prepare_native_text(start, count)?;
        self.native_raw = Some(NativeRawRange {
            start,
            count: NonZeroUsize::new(count).unwrap(),
        });
        self.declaration_allowed = false;
        frame.publish(position);
        // Match parse_text's published-prefix behavior if accounting fails.
        self.last_position = position;
        // Native UTF-8 and the accounting cursor check prove this token's
        // original byte delta. Keep the charge after publication on rejection.
        self.account_source_bytes(count)?;
        self.source_mut().consume_text(plan);
        Ok(true)
    }

    /// Drop the detached cache before releasing its share of retained-memory space.
    #[doc(hidden)]
    pub fn finish_adapter_frame(&mut self, frame: AdapterFrame) {
        let token = frame.finish();
        self.event_recycling.release_adapter(&token);
    }

    fn next_delivery_into(&mut self, output: &mut EventOutput<'_>) -> Result<(), Error> {
        // Keep ordinary documents outside the owned-table publication frame.
        if self.shared_tables.get().is_none() && !self.in_doctype {
            return self.next_event_scoped(output);
        }
        self.next_event_with_tables(output)
    }

    fn next_event_with_tables(&mut self, output: &mut EventOutput<'_>) -> Result<(), Error> {
        // DOCTYPE always yields its start event before parsing any declarations.
        // A child created by that callback may have initialized this owner first.
        if self.error.is_none()
            && self.in_doctype
            && self.shared_tables.get().is_none()
            && let Err(error) = self.ensure_shared_tables()
        {
            self.error = Some(error);
            self.pending.clear();
            return Err(error);
        }
        self.with_dtd_tables(|parser| parser.next_event_scoped(output))
    }

    fn next_event_scoped(&mut self, output: &mut EventOutput<'_>) -> Result<(), Error> {
        if let Some(event) = self.finish_foreign_dtd() {
            self.last_position = event.position;
            *output.event = Some(event);
            return Ok(());
        }
        if let Some(event) = self.pop_event() {
            self.last_position = event.position;
            *output.event = Some(event);
            return Ok(());
        }
        if let Some(error) = &self.error {
            return Err(*error);
        }
        if let Err(error) = self.next_event_inner(output) {
            self.finish_event_error(error, output)?;
        }
        if let Some(event) = output.event {
            self.last_position = event.position;
        }
        if output.has_frame() {
            self.last_position = output.frame.as_ref().unwrap().position();
        }
        Ok(())
    }

    /// Apply decoder precedence and preserve queued or published error prefixes.
    /// Success delivers a valid prefix before the recorded error is returned.
    /// Allocation failures discard that prefix and return immediately.
    fn finish_event_error(
        &mut self,
        mut error: Error,
        output: &mut EventOutput<'_>,
    ) -> Result<(), Error> {
        if let Some((kind, message)) = self.decoding_error
            && matches!(
                error.kind,
                ErrorKind::UnclosedToken
                    | ErrorKind::NoElements
                    | ErrorKind::UnclosedCdataSection
                    | ErrorKind::IncompleteParameterEntity
            )
        {
            let source = &self.sources[0];
            let (kind, message) = if self.in_cdata && kind == ErrorKind::UnclosedToken {
                (ErrorKind::UnclosedCdataSection, "unclosed CDATA section")
            } else {
                (kind, message)
            };
            error = Error {
                kind,
                message,
                position: if matches!(
                    kind,
                    ErrorKind::UnknownEncoding | ErrorKind::IncorrectEncoding
                ) {
                    self.decoder
                        .encoding_error_position()
                        .unwrap_or_else(|| source.end_position())
                } else {
                    source.end_position()
                },
            };
        }
        // Ordinary character data publishes before consume. Preserve that
        // prefix on a subsequent accounting error, as the pending queue did.
        // CDATA and start frames publish only after their fallible work.
        let text_prefix = error.kind != ErrorKind::NoMemory
            && output.frame.as_ref().is_some_and(|frame| frame.is_text());
        if !text_prefix {
            output.clear_frame();
        }
        self.error = Some(error);
        // Unknown encodings can be installed after an error and resume
        // parsing. Other terminal failures no longer need copied names;
        // source bytes remain available for diagnostics and child context.
        if error.kind == ErrorKind::NoMemory
            || self
                .decoding_error
                .is_none_or(|(kind, _)| kind != ErrorKind::UnknownEncoding)
        {
            self.active_entities.names.clear();
        }
        if error.kind == ErrorKind::NoMemory {
            self.pending.clear();
            return Err(error);
        }
        let cleanup = (|| -> Result<(), Error> {
            let mut prefixes = Vec::new_in(self.allocator);
            for event in self.pending.iter() {
                if let EventKind::StartNamespace { prefix, .. } = &event.event.kind {
                    try_push(&mut prefixes, prefix.try_clone()?)?;
                }
            }
            for prefix in prefixes.into_iter().rev() {
                self.emit(EventKind::EndNamespace { prefix }, error.position)?;
            }
            Ok(())
        })();
        if let Err(error) = cleanup {
            output.clear_frame();
            self.error = Some(error);
            self.pending.clear();
            return Err(error);
        }
        if text_prefix {
            debug_assert!(self.pending.is_empty());
            Ok(())
        } else if let Some(event) = self.pop_event() {
            *output.event = Some(event);
            Ok(())
        } else {
            Err(error)
        }
    }

    #[must_use]
    pub fn position(&self) -> Position {
        self.last_position
    }
    /// Earliest original input byte that a pending event can still reference.
    ///
    /// Adapters retaining a raw input window can discard bytes before this
    /// offset. Consumed whitespace need not be retained merely because it did
    /// not emit an event; incomplete declarations and entity frames keep their
    /// original anchors until every dependent event has been delivered.
    #[must_use]
    pub fn input_context_byte_index(&self) -> usize {
        self.sources
            .iter()
            .map(|source| source.position(0).byte_index)
            .chain(
                self.pending
                    .iter()
                    .map(|pending| pending.event.position.byte_index),
            )
            .chain(self.declaration_context_byte_index())
            .chain(
                self.active_parameter_reference
                    .as_ref()
                    .map(|(_, position)| position.byte_index),
            )
            .chain(self.value_context_byte_index())
            .chain(self.native_raw.map(|raw| raw.start))
            .min()
            .expect("a parser always has its original input source")
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
    }
    /// Set the maximum consumed-input amplification for this root and its children.
    /// A factor below one or NaN is invalid; infinity disables the relative check.
    pub fn set_entity_maximum_amplification(&self, factor: f32) -> bool {
        !self.fragment && self.expanded.set_factor(factor)
    }

    /// Set the combined direct/indirect byte threshold for relative amplification.
    /// Input, expansion-work and nesting policies remain independent.
    pub fn set_entity_activation_threshold(&self, bytes: u64) -> bool {
        if self.fragment {
            return false;
        }
        self.expanded.set_threshold(bytes);
        true
    }

    pub fn set_namespace_triplets(&mut self, enabled: bool) {
        self.config.namespace_triplets = enabled;
    }
    pub fn set_reparse_deferral_enabled(&mut self, enabled: bool) {
        self.reparse_deferral = enabled;
    }
    #[must_use]
    pub fn reparse_deferral_enabled(&self) -> bool {
        self.reparse_deferral
    }
    pub fn set_expand_internal_entities(&mut self, enabled: bool) {
        self.expand_internal_entities = enabled;
    }
    /// Include whitespace-only [`EventKind::Default`] events in the prolog, epilog,
    /// and DTD. Disabled by default; the C interface enables these for its default
    /// handler. Read [`Self::current_raw`] before the next parser operation.
    pub fn set_default_events(&mut self, enabled: bool) {
        self.default_events = enabled;
    }

    /// Preserve literal newline boundaries for character-data callbacks.
    /// Rust consumers coalesce text by default; external children inherit this
    /// preference. Custom-encoding aliases retain their lexical token boundaries.
    #[doc(hidden)]
    pub fn set_text_line_boundaries(&mut self, enabled: bool) {
        self.text_line_boundaries = enabled;
    }

    /// Update a foreign interface's notation-handler availability. Safe event
    /// consumers leave this enabled. A continuation captures this preference
    /// when it reads the notation name, matching Expat's callback prerequisites.
    #[doc(hidden)]
    pub fn set_notation_handler_enabled(&mut self, enabled: bool) {
        self.notation_handler_enabled = enabled;
    }

    /// Update a foreign interface's ATTLIST-handler availability. Enumeration
    /// callback types retain the members read while that handler was installed;
    /// the semantic default definition always keeps its complete type.
    #[doc(hidden)]
    pub fn set_attlist_handler_enabled(&mut self, enabled: bool) {
        self.attlist_handler_enabled = enabled;
    }

    /// Index of the declared ID attribute on the current start element, if present.
    #[must_use]
    pub fn id_attribute_index(&self) -> Option<usize> {
        self.id_attribute_index
    }
    /// Raw XML for the token responsible for the most recently returned event.
    #[must_use]
    pub fn current_raw(&self) -> Option<&str> {
        if let Some(raw) = self.native_raw {
            let (context, start) = self.input_context();
            let offset = raw
                .start
                .checked_sub(start)
                .expect("retained native raw start");
            let end = offset.checked_add(raw.count.get()).expect("native raw end");
            let bytes = context.get(offset..end).expect("retained native raw range");
            // Only this validated native span is UTF-8. A later feed may have
            // moved the context to separate storage with an invalid suffix.
            return Some(std::str::from_utf8(bytes).expect("native raw UTF-8"));
        }
        (!self.current_raw.is_empty()).then_some(self.current_raw.as_str())
    }
    /// Replace limits before parsing begins.
    pub fn set_limits(&mut self, limits: Limits) -> Result<(), Error> {
        if self.received != 0 {
            return self.fail(ErrorKind::Syntax, "limits must be set before parsing");
        }
        self.with_dtd_tables(|parser| {
            if parser.tables.entities.len() + parser.tables.parameter_entities.len()
                > limits.max_entities
                || parser.tables.max_default_attributes > limits.max_attributes
            {
                return Err(parser.err(
                    ErrorKind::LimitExceeded,
                    "existing declarations exceed new limits",
                ));
            }
            if !parser.external_subset {
                parser.tables.max_entities = limits.max_entities;
                parser.tables.max_attributes = limits.max_attributes;
            }
            parser.config.limits = limits;
            Ok(())
        })
    }

    fn source(&self) -> &Source {
        self.sources
            .last()
            .expect("document source is always present")
    }
    fn source_mut(&mut self) -> &mut Source {
        self.sources
            .last_mut()
            .expect("document source is always present")
    }
    fn here(&self) -> Position {
        self.source().position(0)
    }
    fn err(&self, kind: ErrorKind, message: &'static str) -> Error {
        Error {
            kind,
            message,
            position: self.here(),
        }
    }
    fn err_at(&self, kind: ErrorKind, message: &'static str, offset: usize) -> Error {
        Error {
            kind,
            message,
            position: self.source().position_at(offset, 0),
        }
    }
    fn fail<T>(&mut self, kind: ErrorKind, message: &'static str) -> Result<T, Error> {
        let error = self.err(kind, message);
        self.error = Some(error);
        Err(error)
    }
    #[inline(always)]
    fn emit(&mut self, kind: EventKind, position: Position) -> Result<(), Error> {
        self.pending.try_push_back(PendingEvent {
            event: Event { kind, position },
            raw: None,
        })?;
        Ok(())
    }
    #[inline(always)]
    fn pop_event(&mut self) -> Option<Event> {
        let pending = self.pending.pop_front()?;
        if matches!(
            &pending.event.kind,
            EventKind::ExternalEntityReference(reference)
                if reference.context.is_none() && reference.system_id.is_none()
        ) && let Some(foreign) = &mut self.foreign_dtd_pending
        {
            foreign.delivered = true;
            self.parameter_state
                .get()
                .expect("foreign DTD read marker")
                .read
                .store(false, Ordering::Relaxed);
        }
        if let Some(raw) = pending.raw {
            self.native_raw = None;
            if raw.is_empty() {
                self.current_raw.clear();
            } else {
                self.current_raw = raw;
            }
        }
        Some(pending.event)
    }

    fn save_current_raw(&mut self, count: usize) -> Result<(), Error> {
        // Preserve the previous value if growth fails, and retain capacity across
        // events. Returned event payloads remain independently owned.
        self.current_raw
            .try_reserve(count.saturating_sub(self.current_raw.len()))?;
        self.current_raw.clear();
        self.native_raw = None;
        let source = self
            .sources
            .last()
            .expect("parser always has an input source");
        if !source.has_conversions() {
            self.current_raw
                .try_push_str(&source.remaining()[..count])?;
            return Ok(());
        }
        let value = source
            .lexical_remaining()
            .for_slice(&source.remaining()[..count]);
        if value.has_ascii_aliases() {
            self.current_raw
                .try_push_str(&value.decode(self.allocator)?)?;
        } else {
            self.current_raw.try_push_str(&value)?;
        }
        Ok(())
    }
    fn event_raw(&mut self, raw: &str) -> Result<(), Error> {
        if let Some(pending) = self.pending.back_mut() {
            pending.raw = Some(string(raw, self.allocator)?);
        }
        Ok(())
    }
    #[inline]
    fn account_source(&mut self, count: usize) -> Result<(), Error> {
        let bytes = self.source().accounting_bytes(count);
        if bytes == 0 {
            return Ok(());
        }
        self.account_source_bytes(bytes)
    }

    /// Charge a nonempty source delta before advancing its accounting cursor.
    fn account_source_bytes(&mut self, bytes: usize) -> Result<(), Error> {
        let indirect = self.fragment || self.sources.len() > 1;
        let accepted = if let Some(budget) = self.expanded.get_mut() {
            budget.account_mut(bytes, indirect, true)
        } else {
            self.expanded.account(bytes, indirect, true)
        };
        if !accepted {
            return Err(self.err(
                ErrorKind::LimitExceeded,
                "entity amplification limit exceeded",
            ));
        }
        self.source_mut().mark_accounted(bytes);
        Ok(())
    }

    fn account_entity_bytes(&self, bytes: usize, enforce: bool) -> Result<(), Error> {
        if !self.expanded.account(bytes, true, enforce) {
            return Err(self.err(
                ErrorKind::LimitExceeded,
                "entity amplification limit exceeded",
            ));
        }
        Ok(())
    }

    fn consume(&mut self, count: usize) -> Result<(), Error> {
        self.account_source(count)?;
        self.source_mut().consume(count);
        Ok(())
    }
    fn is_source_final(&self) -> bool {
        self.sources.len() > 1
            || (self.final_input && self.decoder.conversion().is_none())
            || self.decoding_error.is_some()
    }

    /// Recognize a complete native root end tag using its validated opening name.
    /// Incomplete and ineligible tags keep the resumable lexical scanner.
    fn matching_root_end_tag(&self, limit: usize) -> Option<usize> {
        if self.fragment || self.sources.len() != 1 {
            return None;
        }
        let source = self.source();
        source.native_utf8_byte_index()?;
        if source.has_conversions() {
            return None;
        }
        let element = self.stack.last()?;
        if element.raw_encoding.is_some() {
            return None;
        }
        let end = element.name.raw_name().len().checked_add(3)?;
        let bytes = source.remaining().as_bytes();
        // Check the closing delimiter before comparing a potentially long name.
        // Otherwise, one-byte feeds without deferral could repeat a long prefix
        // comparison while the resumable scanner has no new complete token.
        if end > limit || bytes.get(end - 1) != Some(&b'>') {
            return None;
        }
        (bytes.starts_with(b"</")
            && bytes.get(2..end - 1) == Some(element.name.raw_name().as_bytes()))
        .then_some(end)
    }

    fn next_event_inner(&mut self, output: &mut EventOutput<'_>) -> Result<(), Error> {
        if self.decoder.requires_encoding_declaration() {
            let text = self.sources[0].remaining();
            if !self.final_input && "<?xml".starts_with(text) {
                return Ok(());
            }
            // Decode the initial declaration provisionally, but do not publish
            // other events or append external value text without encoding evidence.
            if !text
                .strip_prefix("<?xml")
                .is_some_and(|rest| rest.starts_with(whitespace))
            {
                self.decoder
                    .ensure_declared_encoding()
                    .map_err(|error| self.err(error.kind, error.message))?;
            }
        }
        // Encoding detection consumes a BOM without producing a text token.
        // Charge that prefix even for empty input or an incomplete next token.
        self.account_source(0)?;
        if self.active_parameter_reference.is_some()
            && let Some((_, position)) = self.active_parameter_reference.take()
        {
            if self
                .parameter_state
                .get()
                .is_some_and(|state| state.read.load(Ordering::Relaxed))
            {
                if !self.standalone {
                    self.emit(EventKind::NotStandalone, position)?;
                    self.event_raw("")?;
                }
            } else {
                self.set_declarations_skipped(self.declarations_skipped() || !self.standalone);
            }
        }
        loop {
            if output.has_frame() {
                return Ok(());
            }
            if let Some(event) = self.pop_event() {
                *output.event = Some(event);
                return Ok(());
            }
            if self.finished {
                return Ok(());
            }
            if self.value_state.is_some() {
                if !self.continue_value()? {
                    return Ok(());
                }
                continue;
            }
            if self.has_header_composition() {
                if !self.continue_header_composition()? {
                    return Ok(());
                }
                continue;
            }
            if self.has_declaration_composition() {
                if !self.continue_declaration_composition()? {
                    return Ok(());
                }
                continue;
            }
            if self.source().remaining().is_empty() {
                if self.sources.len() > 1 {
                    self.finish_conditional_source()?;
                    let source = self.pop_entity_source();
                    if self.stack.len() != source.initial_depth || self.in_cdata {
                        // The parent has already consumed the reference. Keep
                        // the error anchored to the entity that failed to close.
                        let mut position = source.position(0);
                        position.byte_count = 0;
                        return Err(Error {
                            kind: ErrorKind::AsynchronousEntity,
                            message: "entity replacement is not balanced",
                            position,
                        });
                    }
                    continue;
                }
                if let Some((kind, message)) = self.decoding_error {
                    let mut error = self.err(kind, message);
                    if matches!(
                        kind,
                        ErrorKind::UnknownEncoding | ErrorKind::IncorrectEncoding
                    ) && let Some(position) = self.decoder.encoding_error_position()
                    {
                        error.position = position;
                    }
                    return Err(error);
                }
                if !self.is_source_final() {
                    return Ok(());
                }
                self.finish_conditional_source()?;
                self.last_position = self.here();
                if self.in_doctype {
                    return Err(
                        self.err(ErrorKind::NoElements, "unclosed document type declaration")
                    );
                }
                if self.in_cdata {
                    return Err(self.err(ErrorKind::UnclosedCdataSection, "unclosed CDATA section"));
                }
                if !self.seen_root && !self.fragment {
                    return Err(self.err(ErrorKind::NoElements, "document contains no element"));
                }
                if !self.stack.is_empty() {
                    return Err(self.err(
                        if self.fragment {
                            ErrorKind::AsynchronousEntity
                        } else {
                            ErrorKind::NoElements
                        },
                        "unclosed element",
                    ));
                }
                self.finished = true;
                return Ok(());
            }
            if self.in_doctype || self.external_subset {
                if !self.parse_dtd_step()? {
                    return Ok(());
                }
                continue;
            }
            if self.in_cdata {
                if !self.parse_cdata(output)? {
                    return Ok(());
                }
                continue;
            }
            let first = self.source().remaining().as_bytes()[0];
            if first == b'&' {
                if self.stack.is_empty() && !self.fragment {
                    return Err(self.err(
                        ErrorKind::InvalidToken,
                        "entity reference outside the document element",
                    ));
                }
                if !self.parse_reference(output)? {
                    return Ok(());
                }
                continue;
            }
            if first != b'<' {
                if !self.parse_text(output)? {
                    return Ok(());
                }
                continue;
            }
            let remaining = self.source().remaining();
            if remaining.starts_with("<![") && self.stack.is_empty() && !self.fragment {
                return Err(self.err(
                    if self.closed_root {
                        ErrorKind::JunkAfterDocumentElement
                    } else {
                        ErrorKind::Syntax
                    },
                    "CDATA outside the document element",
                ));
            }
            let mode = if remaining.starts_with("<!--") {
                ScanMode::Comment
            } else if remaining.starts_with("<![CDATA[") {
                if self.stack.is_empty() && !self.fragment {
                    return Err(self.err(ErrorKind::Syntax, "CDATA outside the document element"));
                }
                let position = self.source().position(9);
                self.save_current_raw(9)?;
                self.consume(9)?;
                self.in_cdata = true;
                self.emit(EventKind::StartCdata, position)?;
                continue;
            } else if remaining.starts_with("<?") {
                ScanMode::Pi
            } else if remaining.starts_with("<!DOCTYPE") {
                ScanMode::Doctype
            } else if remaining.starts_with("</") {
                ScanMode::Tag
            } else if remaining.len() == 1
                // Expat requires the complete six-character CDATA opener before
                // deciding whether text following `<![` is valid markup.
                || (remaining.starts_with("<![") && remaining.len() < 9)
                || (remaining.starts_with("<!")
                    && ["<!--", "<![CDATA[", "<!DOCTYPE"]
                        .iter()
                        .any(|prefix| prefix.starts_with(remaining)))
            {
                if self.is_source_final() {
                    return Err(self.err(ErrorKind::UnclosedToken, "incomplete markup"));
                }
                return Ok(());
            } else if remaining.starts_with("<!") {
                return Err(self.err(ErrorKind::InvalidToken, "unknown markup declaration"));
            } else {
                ScanMode::Tag
            };
            if mode == ScanMode::Pi
                && remaining[2..]
                    .chars()
                    .next()
                    .is_some_and(|character| !self.config.name_rules.is_name_start(character))
            {
                return Err(self.err_at(
                    ErrorKind::InvalidToken,
                    "invalid processing instruction target",
                    2,
                ));
            }
            if mode == ScanMode::Tag {
                let name_offset = if remaining.starts_with("</") { 2 } else { 1 };
                if let Some(character) = remaining[name_offset..].chars().next()
                    && !self.config.name_rules.is_name_start(character)
                {
                    return Err(self.err_at(
                        ErrorKind::InvalidToken,
                        "invalid element name",
                        name_offset,
                    ));
                }
            }
            let final_input = self.is_source_final();
            let max_token = self.config.limits.max_token_bytes;
            let deferral = self.reparse_deferral && !final_input;
            if deferral && self.source().should_defer(max_token) {
                return Ok(());
            }
            let matched_end = if mode == ScanMode::Tag && remaining.starts_with("</") {
                self.matching_root_end_tag(max_token)
            } else {
                None
            };
            let planned = if mode == ScanMode::Tag
                && !self.source().remaining().starts_with("</")
                // The active table scope includes declarations published by
                // external children. Types and IDs also require owned lowering,
                // even when no declaration supplies a default value.
                && self.tables.defaults.is_empty()
                && !self.foreign_dtd
                && !self.fragment
                && self.sources.len() == 1
                && let Some(source_index) = self.source().native_utf8_byte_index()
            {
                self.tag_scanner.scan(
                    self.sources[0].remaining(),
                    source_index,
                    max_token,
                    self.config.limits.max_attributes,
                    self.config.name_rules,
                    &mut self.raw_attributes,
                )
            } else {
                tag::Planned::Fallback
            };
            let end = if matched_end.is_some() {
                matched_end
            } else {
                match planned {
                    tag::Planned::Complete { end, .. } => Some(end),
                    tag::Planned::Incomplete => None,
                    tag::Planned::Fallback => self
                        .source_mut()
                        .scan_token(mode, max_token)
                        .map_err(|(kind, offset)| {
                            self.err_at(kind, "invalid or oversized XML token", offset)
                        })?,
                }
            };
            let Some(end) = end else {
                if self.sources.len() == 1
                    && self.source().position(0).byte_index == self.feed_start_byte
                {
                    self.source_mut().mark_deferred();
                }
                if final_input {
                    return Err(self.err(ErrorKind::UnclosedToken, "unclosed XML token"));
                }
                return Ok(());
            };
            if !self.seen_root && mode == ScanMode::Tag && self.foreign_dtd {
                self.foreign_dtd = false;
                self.start_foreign_dtd(self.here())?;
                if self.parameter_entities_enabled() {
                    continue;
                }
            }
            self.account_source(end)?;
            let position = self.source().position(end);
            if matched_end.is_some()
                && output.c_text_context
                && self.input_context.is_some()
                && !self.foreign_dtd
                && self
                    .stack
                    .last()
                    .is_some_and(|element| element.namespace_scope.is_none())
                && let Some(frame) = output.frame.as_deref_mut()
            {
                // Matching proves native UTF-8 spelling and a nonempty range.
                // Keep the existing detached name; raw markup stays in context.
                self.prepare_end_frame(frame);
                self.native_raw = Some(NativeRawRange {
                    start: position.byte_index,
                    count: NonZeroUsize::new(end).expect("nonempty matched End"),
                });
                // The complete token was charged before semantic processing;
                // name detachment cannot allocate. Commit coordinates now.
                self.source_mut().consume(end);
                frame.publish(position);
                continue;
            }
            let direct_start_name = if let tag::Planned::Complete { name_end, .. } = planned
                && output.c_text_context
                && self.input_context.is_some()
                && !self.closed_root
                && self.stack.len() < self.config.limits.max_depth
                && end <= arena::MAX_ARENA_BYTES
                && self.raw_attributes.len() <= arena::MAX_ARENA_ATTRIBUTES
            {
                let token = &self.source().remaining()[..end];
                (!token.ends_with("/>")
                    && self.identity_frame_names(&token[1..name_end], &token[name_end..end - 1]))
                .then_some(name_end)
            } else {
                None
            };
            let mut framed_end = false;
            if let Some(name_end) = direct_start_name
                && let Some(frame) = output.frame.as_deref_mut()
            {
                let state = self.identity_start_state();
                let source = state.source;
                let token = &source.remaining()[..end];
                let parsed = state.lower(
                    &token[1..name_end],
                    &token[name_end..end - 1],
                    position,
                    frame,
                );
                // Lowering has released every source/field borrow. Publish the
                // complete token before either its frame or terminal error.
                self.native_raw = Some(NativeRawRange {
                    start: position.byte_index,
                    count: NonZeroUsize::new(end).expect("nonempty planned Start"),
                });
                parsed?;
            } else {
                let mut token = std::mem::replace(
                    &mut self.token_scratch,
                    lexical::Buffer::new_in(self.allocator),
                );
                token.clear();
                token.append(
                    self.source()
                        .lexical_remaining()
                        .for_slice(&self.source().remaining()[..end]),
                )?;
                let parsed = (|| {
                    if matched_end.is_none()
                        && !matches!(planned, tag::Planned::Complete { .. })
                        && let Some(offset) = invalid_xml_char(&token)
                    {
                        return Err(self.err_at(
                            ErrorKind::InvalidToken,
                            "invalid XML character",
                            offset,
                        ));
                    }
                    match mode {
                        ScanMode::Comment => {
                            let text = &token[4..token.len() - 3];
                            if let Some(offset) = text.find("--") {
                                return Err(self.err_at(
                                    ErrorKind::InvalidToken,
                                    "double hyphen in comment",
                                    4 + offset + 2,
                                ));
                            }
                            if text.ends_with('-') {
                                return Err(self.err_at(
                                    ErrorKind::InvalidToken,
                                    "double hyphen in comment",
                                    5 + text.len(),
                                ));
                            }
                            self.declaration_allowed = false;
                            self.emit(
                                EventKind::Comment(self.markup_text(token.view().for_slice(text))?),
                                position,
                            )?;
                        }
                        ScanMode::Pi => self.parse_pi(token.view(), position)?,
                        ScanMode::Doctype => self.parse_doctype(token.view(), position)?,
                        ScanMode::Tag if matched_end.is_some() => {
                            if !self.foreign_dtd
                                && self
                                    .stack
                                    .last()
                                    .is_some_and(|element| element.namespace_scope.is_none())
                                && let Some(frame) = output.frame.as_deref_mut()
                            {
                                self.prepare_end_frame(frame);
                                framed_end = true;
                            } else {
                                self.end_element(position)?;
                            }
                        }
                        ScanMode::Tag if token.starts_with("</") => {
                            self.parse_end(token.view(), position)?
                        }
                        ScanMode::Tag => self.parse_start(
                            token.view(),
                            position,
                            planned,
                            output.frame.as_deref_mut(),
                        )?,
                        ScanMode::DtdDeclaration => {
                            unreachable!("DTD scanner only runs in DTD context")
                        }
                    }
                    Ok(())
                })();
                // Parsing only writes queued raw overrides. Publish the owned token
                // before returning either its events or its terminal error.
                token.swap_decoded(&mut self.current_raw)?;
                self.native_raw = None;
                parsed?;
                self.token_scratch = token;
            }
            if matches!(
                planned,
                tag::Planned::Complete {
                    ascii_bare: true,
                    ..
                }
            ) && self.source().native_utf8_byte_index().is_some()
                && !self.source().has_conversions()
            {
                self.source_mut().consume_ascii_tag(end);
            } else {
                self.source_mut().consume(end);
            }
            if framed_end {
                // Native token publication only swaps owners. Consume sees the
                // bytes already charged above, so neither can fail after the
                // matched name is detached. Keep raw/position updates first.
                output.frame.as_deref_mut().unwrap().publish(position);
            }
        }
    }

    fn parse_prolog_literal(&mut self) -> Result<bool, Error> {
        let limit = self.config.limits.max_token_bytes;
        let end = self
            .source_mut()
            .scan_prolog_literal(limit)
            .map_err(|(kind, offset)| self.err_at(kind, "invalid prolog literal", offset))?;
        if let Some(end) = end {
            let remaining = self.source().remaining();
            if let Some(next) = remaining.as_bytes().get(end) {
                if matches!(*next, b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'%' | b'[') {
                    return Err(self.err(ErrorKind::Syntax, "literal outside a declaration"));
                }
                return Err(self.err_at(ErrorKind::InvalidToken, "invalid literal delimiter", end));
            }
            if self.decoding_error.is_some() {
                return Err(self.err_at(ErrorKind::InvalidToken, "invalid literal delimiter", end));
            }
            if self.is_source_final() {
                return Err(self.err(ErrorKind::Syntax, "literal outside a declaration"));
            }
        } else if let Some((kind, message)) = self.decoding_error {
            return Err(self.err_at(
                kind,
                message,
                if kind == ErrorKind::PartialCharacter {
                    0
                } else {
                    self.source().remaining().len()
                },
            ));
        } else if self.is_source_final() {
            return Err(self.err(ErrorKind::UnclosedToken, "unclosed prolog literal"));
        }
        Ok(false)
    }

    fn parse_text(&mut self, output: &mut EventOutput<'_>) -> Result<bool, Error> {
        let internal = self.sources.len() > 1;
        if !self.seen_root
            && !self.fragment
            && !self.is_source_final()
            && self
                .source()
                .should_defer(self.config.limits.max_token_bytes)
        {
            return Ok(false);
        }
        if !self.seen_root
            && !self.fragment
            && matches!(self.source().remaining().as_bytes()[0], b'\'' | b'"')
        {
            return self.parse_prolog_literal();
        }
        let mut limit = self.source().converted_text_limit();
        let mut before_prolog_literal = false;
        if !self.seen_root && !self.fragment {
            // A quote ends a horizontal whitespace token. Newlines already end
            // text tokens, so looking beyond one would rescan the remaining
            // prolog for each line. Converted text also has a bounded window.
            let remaining = &self.source().remaining()[..limit];
            #[cfg(test)]
            let mut inspected = 0;
            let whitespace_end = remaining
                .bytes()
                .position(|byte| {
                    #[cfg(test)]
                    {
                        inspected += 1;
                    }
                    !matches!(byte, b' ' | b'\t')
                })
                .unwrap_or(remaining.len());
            if whitespace_end > 0
                && matches!(remaining.as_bytes().get(whitespace_end), Some(b'\'' | b'"'))
            {
                limit = limit.min(whitespace_end);
                before_prolog_literal = limit == whitespace_end;
            }
            #[cfg(test)]
            {
                self.prolog_bytes_inspected += inspected;
            }
        }
        let text = &self.source().remaining()[..limit];
        // A known quote completes the preceding horizontal whitespace token.
        let final_text = before_prolog_literal
            || (self.is_source_final() && limit == self.source().remaining().len());
        let character_data = !self.stack.is_empty() || self.fragment;
        let coalesce = character_data && !self.text_line_boundaries;
        let text_plan = (character_data
            && !self.fragment
            && !internal
            && !self.source().has_conversions()
            && self.source().native_utf8_byte_index().is_some())
        .then(|| text::TextPlan::scan(text, coalesce))
        .flatten();
        let text_plan = if !character_data {
            (!self.fragment
                && !internal
                && !self.source().has_conversions()
                && self.source().native_utf8_byte_index().is_some())
            .then(|| text::TextPlan::scan_outer_whitespace(text))
            .flatten()
        } else {
            text_plan
        };
        let fast_end = text_plan
            .map(|plan| plan.end)
            .or_else(|| coalesce.then(|| coalesced_text_end(text)).flatten());
        let mut end = fast_end.unwrap_or_else(|| {
            let mut boundary = 0;
            text.bytes()
                .enumerate()
                .find_map(|(index, byte)| {
                    if matches!(byte, b'<' | b'&') {
                        return Some(index);
                    }
                    if byte == b'\n' || (!internal && byte == b'\r') {
                        if !coalesce || index >= 65_536 {
                            return Some(if boundary > 0 { boundary } else { index });
                        }
                        boundary = index
                            + if byte == b'\r' && text.as_bytes().get(index + 1) == Some(&b'\n') {
                                2
                            } else {
                                1
                            };
                    }
                    // Bound merging across lines, preserving the existing span of
                    // an individual long line and its malformed-input prefix.
                    (index >= 65_536 && boundary > 0).then_some(boundary)
                })
                .map_or(text.len(), |index| {
                    if index == 0 && matches!(text.as_bytes()[0], b'\r' | b'\n') {
                        if text.starts_with("\r\n") { 2 } else { 1 }
                    } else {
                        index
                    }
                })
        });
        if coalesce && end == text.len() && limit < self.source().remaining().len() {
            // Keep a converted buffer boundary at the last complete line when
            // possible. Otherwise merging earlier lines could shift the next
            // conversion window into a malformed token and emit extra data.
            let complete = text.strip_suffix('\r').unwrap_or(text);
            if let Some(newline) = complete
                .bytes()
                .rposition(|byte| byte == b'\n' || (!internal && byte == b'\r'))
            {
                end = newline + 1;
            }
        }
        if end == text.len() && !final_text {
            if text.ends_with('\r') {
                end -= 1;
            }
            // A forbidden CDATA terminator may straddle input chunks.
            while end > 0 && end + 2 >= text.len() && text.as_bytes()[end - 1] == b']' {
                end -= 1;
            }
        }
        if end == 0 {
            return Ok(false);
        }
        let (invalid, forbidden) = if text_plan.is_some() {
            (None, None)
        } else {
            let invalid = invalid_xml_char(&text[..end]);
            // Borrow the fixed needle once; each search keeps its own local state.
            static CDATA_END: OnceLock<memchr::memmem::Finder<'static>> = OnceLock::new();
            let forbidden = CDATA_END
                .get_or_init(|| memchr::memmem::Finder::new(b"]]>"))
                .find(&text.as_bytes()[..end]);
            (invalid, forbidden)
        };
        if let Some(forbidden) =
            forbidden.filter(|forbidden| invalid.is_none_or(|invalid| invalid > *forbidden))
        {
            if self.stack.is_empty() && !self.fragment {
                let whitespace_prefix = text[..forbidden].chars().all(whitespace);
                return Err(self.err_at(
                    if self.closed_root {
                        ErrorKind::JunkAfterDocumentElement
                    } else if whitespace_prefix {
                        ErrorKind::Syntax
                    } else {
                        ErrorKind::InvalidToken
                    },
                    "CDATA terminator outside the document element",
                    if self.closed_root { 0 } else { forbidden },
                ));
            }
            // Earlier lines were complete tokens before coalescing. Deliver
            // their valid prefix before diagnosing the malformed final line.
            if let Some(newline) = text[..forbidden]
                .bytes()
                .rposition(|byte| byte == b'\n' || (!internal && byte == b'\r'))
            {
                end = newline + 1;
            } else {
                return Err(self.err_at(
                    ErrorKind::InvalidToken,
                    "CDATA terminator in character data",
                    forbidden + 2,
                ));
            }
        }
        if let Some(stop) = invalid.filter(|stop| *stop < end) {
            if stop == 0 {
                return Err(self.err_at(ErrorKind::InvalidToken, "invalid XML character data", 0));
            }
            end = stop;
        }
        let text = &text[..end];
        if self.stack.is_empty()
            && !self.fragment
            && text_plan.is_none()
            && !text.chars().all(whitespace)
        {
            if !self.seen_root
                && self.config.name_rules.is_name(text)
                && end == self.source().remaining().len()
            {
                if end > self.config.limits.max_token_bytes {
                    return Err(self.err(ErrorKind::LimitExceeded, "oversized prolog token"));
                }
                if !self.is_source_final() {
                    // A name in the prolog is not a complete token yet. Wait
                    // for its delimiter so decoding errors retain their correct
                    // precedence, without rescanning every one-byte feed.
                    self.source_mut().mark_deferred();
                    return Ok(false);
                }
            }
            if !self.seen_root
                && self.config.name_rules.is_name(text)
                && end == self.source().remaining().len()
                && let Some((kind, message)) = self.decoding_error
            {
                return Err(self.err_at(kind, message, end));
            }
            if !self.seen_root
                && self.config.name_rules.is_name(text)
                && self.source().remaining().as_bytes().get(end) == Some(&b'<')
            {
                return Err(self.err_at(ErrorKind::InvalidToken, "invalid prolog token", end));
            }
            return Err(self.err(
                if self.closed_root && !self.fragment {
                    ErrorKind::JunkAfterDocumentElement
                } else {
                    ErrorKind::Syntax
                },
                "character data outside the document element",
            ));
        }
        let position = self.source().position(end);
        let value = if character_data {
            self.prepare_character_data(
                end,
                output.frame.as_deref_mut(),
                output.c_text_context,
                text_plan.is_some(),
            )?
        } else {
            None
        };
        if self.input_context.is_some()
            && let Some((start, count)) = output
                .frame
                .as_ref()
                .and_then(|frame| frame.prepared_native_text_range())
        {
            debug_assert_eq!(count, end);
            // The core owns the context and retains this absolute range across
            // feeds. The stale String stays available for the next owned token.
            self.native_raw = Some(NativeRawRange {
                start,
                count: NonZeroUsize::new(count).expect("nonempty native Text"),
            });
        } else {
            self.save_current_raw(end)?;
        }
        self.declaration_allowed = false;
        if let Some(value) = value {
            self.emit(EventKind::Text(value), position)?;
        } else if character_data {
            debug_assert!(self.pending.is_empty());
            output.frame.as_deref_mut().unwrap().publish(position);
        } else if self.default_events {
            self.emit(EventKind::Default, position)?;
        }
        if let Some(plan) = text_plan {
            debug_assert_eq!(end, plan.end);
            self.account_source(end)?;
            self.source_mut().consume_text(plan);
        } else {
            self.consume(end)?;
        }
        Ok(true)
    }

    fn parse_cdata(&mut self, output: &mut EventOutput<'_>) -> Result<bool, Error> {
        let internal = self.sources.len() > 1;
        let limit = self.source().converted_text_limit();
        let text = &self.source().remaining()[..limit];
        let final_text = self.is_source_final() && limit == self.source().remaining().len();
        if text.starts_with("]]>") {
            let position = self.source().position(3);
            self.save_current_raw(3)?;
            self.consume(3)?;
            self.in_cdata = false;
            self.emit(EventKind::EndCdata, position)?;
            return Ok(true);
        }
        // Stop on the first callback boundary in a single pass. Looking for the
        // closing delimiter across all remaining input for each newline is quadratic.
        let mut end = text
            .bytes()
            .enumerate()
            .find_map(|(index, byte)| {
                if byte == b'\n' || (!internal && byte == b'\r') {
                    Some(if index == 0 {
                        if text.starts_with("\r\n") { 2 } else { 1 }
                    } else {
                        index
                    })
                } else if byte == b']' && text[index..].starts_with("]]>") {
                    Some(index)
                } else {
                    None
                }
            })
            .unwrap_or(text.len());
        if end == text.len() && !final_text {
            if text.ends_with('\r') {
                end -= 1;
            }
            while end > 0 && end + 2 >= text.len() && text.as_bytes()[end - 1] == b']' {
                end -= 1;
            }
        }
        if end == 0 {
            return Ok(false);
        }
        if let Some(invalid) = invalid_xml_char(&text[..end]) {
            if invalid == 0 {
                return Err(self.err(ErrorKind::InvalidToken, "invalid XML character"));
            }
            end = invalid;
        }
        let position = self.source().position(end);
        let value = self.prepare_character_data(end, output.frame.as_deref_mut(), false, false)?;
        self.save_current_raw(end)?;
        self.consume(end)?;
        if let Some(value) = value {
            self.emit(EventKind::Text(value), position)?;
        } else {
            debug_assert!(self.pending.is_empty());
            output.frame.as_deref_mut().unwrap().publish(position);
        }
        Ok(true)
    }

    fn parse_reference(&mut self, output: &mut EventOutput<'_>) -> Result<bool, Error> {
        let limit = self.config.limits.max_token_bytes;
        if self.reparse_deferral && !self.is_source_final() && self.source().should_defer(limit) {
            return Ok(false);
        }
        let end = self
            .source_mut()
            .scan_reference(limit)
            .map_err(|(kind, offset)| self.err_at(kind, "invalid entity reference", offset))?;
        let text = self.source().remaining();
        let Some(end) = end else {
            if text.len() > self.config.limits.max_token_bytes {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "entity reference byte limit exceeded",
                ));
            }
            if self.is_source_final() {
                return Err(self.err(ErrorKind::UnclosedToken, "unclosed entity reference"));
            }
            if self.sources.len() == 1
                && self.source().position(0).byte_index == self.feed_start_byte
            {
                self.source_mut().mark_deferred();
            }
            return Ok(false);
        };
        if end > self.config.limits.max_token_bytes {
            return Err(self.err(
                ErrorKind::LimitExceeded,
                "entity reference byte limit exceeded",
            ));
        }
        let character = character_reference(&text[1..end]);
        let name = if matches!(character, Ok(None)) {
            Some(
                self.source()
                    .lexical_remaining()
                    .for_slice(&text[1..end])
                    .decode(self.allocator)?,
            )
        } else {
            None
        };
        let position = self.source().position(end + 1);
        self.save_current_raw(end + 1)?;
        self.account_source(end + 1)?;
        if let Some(character) = character
            .map_err(|(kind, offset)| self.err_at(kind, "invalid character reference", offset))?
        {
            if !self.source().remaining()[1..end].starts_with('#') {
                self.account_entity_bytes(1, false)?;
            }
            self.consume(end + 1)?;
            let mut bytes = [0; 4];
            let text = character.encode_utf8(&mut bytes);
            if let Some(frame) = output.frame.as_deref_mut() {
                // A decoded scalar fits in detached inline storage. Publish
                // only after the same source and entity accounting as owned Text.
                frame.prepare_text(text)?;
                frame.publish(position);
            } else {
                self.emit(
                    EventKind::Text(Text::try_from_str_in(text, self.allocator)?),
                    position,
                )?;
            }
            return Ok(true);
        }
        let name = name.expect("general entity references have an owned name");
        let raw_name = &self.source().remaining()[1..end];
        if !self.config.name_rules.is_name(raw_name) {
            return Err(self.err(ErrorKind::InvalidToken, "invalid entity name"));
        }
        if self.config.namespace_separator.is_some()
            && let Some(colon) = raw_name.find(':')
        {
            return Err(self.err_at(
                ErrorKind::InvalidToken,
                "entity names cannot contain colons with namespaces enabled",
                colon + 1,
            ));
        }
        let Some(entity) = self.tables.entities.get(&name) else {
            if self.has_external_subset && !self.standalone {
                self.consume(end + 1)?;
                self.emit(
                    EventKind::SkippedEntity {
                        name,
                        parameter: false,
                    },
                    position,
                )?;
                return Ok(true);
            }
            return Err(self.err(ErrorKind::UndefinedEntity, "undefined entity"));
        };
        if entity.declared_in_parameter_entity && self.requires_internal_entity_declaration() {
            return Err(self.err(
                ErrorKind::EntityDeclaredInParameterEntity,
                "entity was declared in a parameter entity",
            ));
        }
        if entity.notation.is_some() {
            return Err(self.err(
                ErrorKind::BinaryEntityReference,
                "unparsed entity in content",
            ));
        }
        if self.active_entities.contains(&name, false) {
            return Err(self.err(
                ErrorKind::RecursiveEntityReference,
                "recursive entity reference",
            ));
        }
        if self.sources.len() + self.external_depth > self.config.limits.max_entity_depth {
            return Err(self.err(ErrorKind::LimitExceeded, "entity nesting limit exceeded"));
        }
        if entity.value.is_none() {
            self.charge_external_identifiers(entity)?;
            // A name reserved by an unfinished DTD declaration has no system
            // identifier yet; Expat still requests this entity with a null ID.
            let base = self.copy_external_base(entity.base.as_ref())?;
            let system_id = entity.system_id.try_clone()?;
            let public_id = entity.public_id.try_clone()?;
            let mut context = String::new_in(self.allocator);
            if self.config.namespace_separator.is_some() {
                let mut bindings = Vec::new_in(self.allocator);
                for binding in self.namespace_bindings() {
                    try_push(&mut bindings, binding)?;
                }
                bindings.sort_unstable_by_key(|(prefix, _)| prefix.map_or("", String::as_str));
                for (prefix, uri) in bindings {
                    let prefix = prefix.map_or("", String::as_str);
                    self.charge_expansion(prefix.len())?;
                    self.charge_expansion(uri.len())?;
                    self.charge_expansion(2)?;
                    context.try_push_str(prefix)?;
                    context.try_push('=')?;
                    context.try_push_str(uri)?;
                    context.try_push('\u{c}')?;
                }
            }
            for name in &self.entity_chain {
                self.charge_expansion(name.len())?;
                self.charge_expansion(1)?;
                context.try_push_str(name)?;
                context.try_push('\u{c}')?;
            }
            for source in &self.sources {
                if let Some(name) = &source.entity_name {
                    self.charge_expansion(name.len())?;
                    self.charge_expansion(1)?;
                    context.try_push_str(name)?;
                    context.try_push('\u{c}')?;
                }
            }
            self.charge_expansion(name.len())?;
            context.try_push_str(&name)?;
            self.consume(end + 1)?;
            self.emit(
                EventKind::ExternalEntityReference(xeme_storage::try_box(
                    crate::ExternalEntityReference {
                        base,
                        context: Some(context),
                        system_id,
                        public_id,
                    },
                    self.allocator,
                )?),
                position,
            )?;
            return Ok(true);
        }
        if !self.expand_internal_entities {
            self.consume(end + 1)?;
            self.emit(
                EventKind::SkippedEntity {
                    name,
                    parameter: false,
                },
                position,
            )?;
            return Ok(true);
        }
        let value = entity
            .value
            .as_ref()
            .expect("internal entity has replacement text");
        self.charge_expansion(value.len())?;
        let value = value.try_clone()?;
        self.consume(end + 1)?;
        self.push_entity_source(Source::entity(
            value,
            name,
            position,
            self.stack.len(),
            self.config.name_rules,
        ))?;
        Ok(true)
    }

    fn parse_pi(&mut self, token: lexical::Slice<'_>, position: Position) -> Result<(), Error> {
        let body = token
            .strip_prefix("<?")
            .and_then(|body| body.strip_suffix("?>"))
            .ok_or_else(|| self.err(ErrorKind::InvalidToken, "invalid processing instruction"))?;
        let (target, rest) = take_name(body, self.config.name_rules).ok_or_else(|| {
            self.err(
                ErrorKind::InvalidToken,
                "invalid processing instruction target",
            )
        })?;
        if !rest.is_empty() && !rest.starts_with(whitespace) {
            return Err(self.err(
                ErrorKind::InvalidToken,
                "processing instruction requires whitespace",
            ));
        }
        if target.eq_ignore_ascii_case("xml") {
            if target != "xml" {
                return Err(self.err(
                    ErrorKind::InvalidToken,
                    "reserved processing instruction target",
                ));
            }
            if !self.declaration_allowed || self.sources.len() > 1 {
                return Err(self.err(
                    ErrorKind::MisplacedXmlDeclaration,
                    "XML declaration is not at the beginning",
                ));
            }
            let context = self.declaration_context();
            let declaration_error = context.error_kind();
            let mut attrs = Vec::new_in(self.allocator);
            parse_raw_attributes(rest, false, &mut attrs, 3, self.config.name_rules).map_err(
                |error| {
                    self.err_at(
                        if error.kind == ErrorKind::NoMemory {
                            ErrorKind::NoMemory
                        } else {
                            declaration_error
                        },
                        error.message,
                        2 + target.len() + error.position.byte_index,
                    )
                },
            )?;
            let allocator = self.allocator;
            let Declaration {
                version,
                encoding,
                standalone,
            } = declaration_fields(
                rest,
                &attrs,
                context,
                self.config.allow_invalid_xml_versions,
                |value| token.for_slice(value).decoded(allocator),
            )
            .map_err(|failure| match failure {
                DeclarationFailure::Allocation(error) => Error::from(error),
                DeclarationFailure::Syntax {
                    message,
                    offset: Some(offset),
                } => self.err_at(declaration_error, message, 2 + target.len() + offset),
                DeclarationFailure::Syntax {
                    message,
                    offset: None,
                } => self.err(declaration_error, message),
            })?;
            let version = version
                .map(|version| version.into_owned(allocator))
                .transpose()?;
            // Grammar is complete before an encoding error can invoke an adapter callback.
            if let Some(encoding) = &encoding {
                self.decoder
                    .check_declaration(encoding)
                    .map_err(|error| self.err(error.kind, error.message))?;
            }
            self.decoder
                .ensure_declared_encoding()
                .map_err(|error| self.err(error.kind, error.message))?;
            if context == DeclarationContext::Text {
                self.declaration_allowed = false;
                self.emit(
                    EventKind::TextDeclaration {
                        version,
                        encoding: string(&encoding.expect("validated text encoding"), allocator)?,
                    },
                    position,
                )?;
                return Ok(());
            }
            let version = version.expect("validated document version");
            let encoding = encoding
                .map(|encoding| encoding.into_owned(allocator))
                .transpose()?;
            drop(attrs);
            self.standalone =
                standalone == Some(true) || (self.is_external_value() && self.standalone);
            if standalone == Some(true) && self.parameter_mode == 1 {
                // Children inherit the effective mode after a standalone root
                // disables conditional parameter processing, as in Expat.
                self.parameter_mode = 0;
            }
            self.emit(
                EventKind::XmlDeclaration {
                    version,
                    encoding,
                    standalone,
                },
                position,
            )?;
        } else {
            if self.config.namespace_separator.is_some() && target.contains(':') {
                return Err(self.err(
                    ErrorKind::InvalidToken,
                    "colon in processing instruction target",
                ));
            }
            self.emit(
                EventKind::ProcessingInstruction {
                    target: token.for_slice(target).decode(self.allocator)?,
                    data: self.markup_text(token.for_slice(rest.trim_start_matches(whitespace)))?,
                },
                position,
            )?;
        }
        self.declaration_allowed = false;
        Ok(())
    }

    fn parse_start(
        &mut self,
        token: lexical::Slice<'_>,
        position: Position,
        planned: tag::Planned,
        mut frame: Option<&mut AdapterFrame>,
    ) -> Result<(), Error> {
        if self.closed_root {
            return Err(self.err(
                ErrorKind::JunkAfterDocumentElement,
                "multiple document elements",
            ));
        }
        if self.stack.len() >= self.config.limits.max_depth {
            return Err(self.err(ErrorKind::LimitExceeded, "element nesting limit exceeded"));
        }
        let empty = token.ends_with("/>");
        let body = &token[1..token.len() - if empty { 2 } else { 1 }];
        let (raw_name, rest) = if let tag::Planned::Complete { name_end, .. } = planned {
            body.split_at(name_end - 1)
        } else {
            take_name(body, self.config.name_rules)
                .ok_or_else(|| self.err_at(ErrorKind::InvalidToken, "invalid element name", 1))?
        };
        if let Some(frame) = frame.as_deref_mut().filter(|_| {
            matches!(planned, tag::Planned::Complete { .. })
                && token.len() <= arena::MAX_ARENA_BYTES
                && self.raw_attributes.len() <= arena::MAX_ARENA_ATTRIBUTES
                // A frame contains final callback spellings. Namespace-aware
                // tags can share this storage only when expansion is identity.
                && self.identity_frame_names(raw_name, rest)
        }) {
            return self.parse_start_frame::<false>(token, position, raw_name, rest, frame);
        }
        if let Some(frame) = frame.filter(|_| {
            matches!(planned, tag::Planned::Complete { .. })
                && !self.source().has_conversions()
                && self.raw_attributes.len() <= arena::MAX_ARENA_ATTRIBUTES
                && self.expanded_names_fit_frame(raw_name, rest, token.len())
        }) {
            return self.parse_start_frame::<true>(token, position, raw_name, rest, frame);
        }
        if self.config.namespace_separator.is_some() && !self.config.name_rules.is_qname(raw_name) {
            return Err(self.err(ErrorKind::InvalidToken, "invalid qualified element name"));
        }
        let name_value = token.for_slice(raw_name).decoded(self.allocator)?;
        let name: &str = &name_value;
        // Offset records retain no references into the reusable lexical buffer.
        // Validate the complete tag before expanding values or emitting callbacks.
        let mut raw_attrs =
            std::mem::replace(&mut self.raw_attributes, Vec::new_in(self.allocator));
        if !matches!(planned, tag::Planned::Complete { .. }) {
            parse_raw_attributes(
                rest,
                true,
                &mut raw_attrs,
                self.config.limits.max_attributes,
                self.config.name_rules,
            )
            .map_err(|error| {
                self.err_at(
                    error.kind,
                    error.message,
                    1 + raw_name.len() + error.position.byte_index,
                )
            })?;
        }
        if raw_attrs.len() > self.config.limits.max_attributes {
            return Err(self.err(ErrorKind::LimitExceeded, "attribute count limit exceeded"));
        }
        // Empty tags need no attribute buffer and must not evict a warm cache.
        let mut attrs = if raw_attrs.is_empty() {
            Vec::new_in(self.allocator)
        } else {
            self.event_recycling.take()
        };
        attrs.truncate(raw_attrs.len());
        attrs
            .try_reserve(raw_attrs.len().saturating_sub(attrs.len()))
            .map_err(|_| AllocError::OutOfMemory)?;
        let mut decoded_names = Vec::new_in(self.allocator);
        if token.has_ascii_aliases() {
            decoded_names
                .try_reserve_exact(raw_attrs.len())
                .map_err(AllocError::from)?;
            for attribute in &raw_attrs {
                decoded_names.push(
                    token
                        .for_slice(attribute.name(rest))
                        .decoded(self.allocator)?,
                );
            }
        }
        // Most elements have only a few attributes. Keep the linear scan bounded;
        // larger elements retain a randomized hash table against collision attacks.
        let mut names =
            AttributeNames::new(raw_attrs.len(), self.namespaces.hasher(), self.allocator);
        for (index, attribute) in raw_attrs.iter().enumerate() {
            let (attr_name, value, attribute_offset, _) = attribute.parts(rest);
            if self.config.namespace_separator.is_some()
                && !self.config.name_rules.is_qname(attr_name)
            {
                return Err(self.err(ErrorKind::InvalidToken, "invalid qualified attribute name"));
            }
            let attr_name = decoded_names.get(index).map_or(attr_name, |name| &**name);
            names.check(
                attr_name,
                raw_attrs[..index]
                    .iter()
                    .enumerate()
                    .map(|(prior, attribute)| {
                        decoded_names
                            .get(prior)
                            .map_or(attribute.name(rest), |name| &**name)
                    }),
                || {
                    self.source()
                        .position_at(1 + raw_name.len() + attribute_offset, 0)
                },
            )?;
            if index == attrs.len() {
                try_push(
                    &mut attrs,
                    Attribute {
                        name: String::new_in(self.allocator),
                        value: String::new_in(self.allocator),
                        specified: true,
                    },
                )?;
            }
            let tokenized = self
                .tables
                .defaults
                .get(name)
                .and_then(|decls| decls.get(attr_name))
                .is_some_and(|decl| decl.attribute_type != "CDATA")
                && attribute_needs_normalization(value);
            let attribute = &mut attrs[index];
            self.expand_attribute_into(
                token.for_slice(value),
                &mut attribute.value,
                tokenized,
                self.sources.len() == 1,
            )?;
            copy_attribute_string(&mut attribute.name, attr_name)?;
            attribute.specified = true;
        }
        if let Some(defaults) = self.tables.defaults.get(name) {
            for &index in &defaults.default_indices {
                let default = &defaults.ordered[index];
                #[cfg(test)]
                {
                    self.default_candidates_visited += 1;
                }
                if !names.index.as_ref().map_or_else(
                    || {
                        raw_attrs.iter().enumerate().any(|(index, attribute)| {
                            decoded_names
                                .get(index)
                                .map_or(attribute.name(rest), |name| &**name)
                                == default.name.as_str()
                        })
                    },
                    |names| names.contains(default.name.as_str()),
                ) && let Some(value) = &default.value
                {
                    if attrs.len() >= self.config.limits.max_attributes {
                        return Err(
                            self.err(ErrorKind::LimitExceeded, "attribute count limit exceeded")
                        );
                    }
                    // Each reused declaration produces indirect output, even when
                    // its value contains no entity references. Charge before cloning
                    // so repeated empty elements cannot bypass expansion limits.
                    self.charge_expansion(default.name.len())?;
                    self.charge_expansion(value.len())?;
                    try_push(
                        &mut attrs,
                        Attribute {
                            name: default.name.try_clone()?,
                            value: value.try_clone()?,
                            specified: false,
                        },
                    )?;
                }
            }
        }
        self.id_attribute_index = attrs.iter().position(|attribute| {
            self.tables
                .defaults
                .get(name)
                .and_then(|declarations| declarations.get(&attribute.name))
                .is_some_and(|declaration| declaration.attribute_type == "ID")
        });
        let mut bindings = Vec::new_in(self.allocator);
        if self.config.namespace_separator.is_some() {
            for attr in &attrs {
                let prefix = if attr.name == "xmlns" {
                    Some("")
                } else {
                    attr.name.strip_prefix("xmlns:")
                };
                if let Some(prefix) = prefix {
                    let uri = &attr.value;
                    if prefix == "xmlns" {
                        return Err(self.err(
                            ErrorKind::ReservedPrefixXmlns,
                            "the xmlns prefix is reserved",
                        ));
                    }
                    if prefix == "xml" && uri != "http://www.w3.org/XML/1998/namespace" {
                        return Err(self.err(
                            ErrorKind::ReservedPrefixXml,
                            "the xml prefix has a fixed namespace",
                        ));
                    }
                    if uri == "http://www.w3.org/2000/xmlns/"
                        || (uri == "http://www.w3.org/XML/1998/namespace" && prefix != "xml")
                    {
                        return Err(
                            self.err(ErrorKind::ReservedNamespaceUri, "reserved namespace URI")
                        );
                    }
                    if !prefix.is_empty() && uri.is_empty() {
                        return Err(self.err(
                            ErrorKind::UndeclaringPrefix,
                            "a namespace prefix cannot be undeclared in XML 1.0",
                        ));
                    }
                    if self.config.namespace_separator.is_some_and(|separator| {
                        separator != '\0' && !is_uri_char(separator) && uri.contains(separator)
                    }) {
                        return Err(self.err(
                            ErrorKind::Syntax,
                            "namespace URI contains the namespace separator",
                        ));
                    }
                    let previous = if uri.is_empty() {
                        self.remove_namespace(prefix)
                    } else {
                        self.insert_namespace(string(prefix, self.allocator)?, uri.try_clone()?)?
                    };
                    try_push(&mut bindings, (string(prefix, self.allocator)?, previous))?;
                    self.emit(
                        EventKind::StartNamespace {
                            prefix: if prefix.is_empty() {
                                None
                            } else {
                                Some(string(prefix, self.allocator)?)
                            },
                            uri: if uri.is_empty() {
                                None
                            } else {
                                Some(uri.try_clone()?)
                            },
                        },
                        position,
                    )?;
                }
            }
            let id_name = self
                .id_attribute_index
                .map(|index| attrs[index].name.try_clone())
                .transpose()?;
            attrs.retain(|attr| attr.name != "xmlns" && !attr.name.starts_with("xmlns:"));
            self.id_attribute_index =
                id_name.and_then(|name| attrs.iter().position(|attribute| attribute.name == name));
            let mut expanded = (attrs.len() > 8)
                .then(|| HashSet::with_hasher_in(self.namespaces.hasher().clone(), self.allocator));
            let mut key_lengths = [0; 8];
            for index in 0..attrs.len() {
                let (prior, current) = attrs.split_at_mut(index);
                let attr = &mut current[0];
                // An unprefixed attribute has no namespace. Its raw name was
                // already checked for duplicates, and may legitimately equal
                // another attribute's serialized expanded name.
                if !attr.name.contains(':') {
                    continue;
                }
                if let Some(expanded) = &mut expanded {
                    let key = self.expand_name(&attr.name, true, false, None)?;
                    if !try_set_insert(expanded, key)? {
                        return Err(self.err(
                            ErrorKind::DuplicateAttribute,
                            "duplicate expanded attribute name",
                        ));
                    }
                    attr.name =
                        self.expand_name(&attr.name, true, self.config.namespace_triplets, None)?;
                } else {
                    let (name, key_length) = self.expand_small_attribute_name(
                        &attr.name,
                        prior
                            .iter()
                            .zip(key_lengths)
                            .filter(|(_, length)| *length != 0)
                            .map(|(attribute, length)| &attribute.name.as_bytes()[..length]),
                    )?;
                    key_lengths[index] = key_length;
                    attr.name = name;
                }
            }
        }
        let reusable = self.event_recycling.take_name();
        let expanded_name =
            self.expand_name(name, false, self.config.namespace_triplets, reusable)?;
        let raw_encoding = token.for_slice(raw_name).name_encoding(self.allocator)?;
        let raw_encoding = if raw_encoding.is_empty() {
            None
        } else {
            Some(xeme_storage::try_box(raw_encoding, self.allocator)?)
        };
        self.seen_root = true;
        self.declaration_allowed = false;
        let stack_name = if expanded_name.as_str() != name {
            let layout = ElementNameLayout::new(&expanded_name, name)?;
            let mut value = self
                .event_recycling
                .take_name()
                .unwrap_or_else(|| String::new_in(self.allocator));
            value.clear();
            value.try_reserve(layout.capacity)?;
            value.try_push_str(&expanded_name)?;
            layout.finish(value, name)?
        } else {
            let value = match name_value {
                lexical::Decoded::Borrowed(name) => {
                    let reusable = self.event_recycling.take_name();
                    recycling::copy_name(name, reusable, self.allocator)?
                }
                lexical::Decoded::Owned(name) => name,
            };
            ElementName {
                expanded_end: value.len(),
                value,
                raw_start: 0,
            }
        };
        // Reserve both slots before moving either owner. An error still drops
        // the local undo block without restoring the already updated map.
        self.stack.try_reserve(1).map_err(AllocError::from)?;
        let namespace_scope = if bindings.is_empty() {
            None
        } else {
            self.namespace_scopes
                .try_reserve(1)
                .map_err(AllocError::from)?;
            self.namespace_scopes.push(bindings);
            // The successful reserve bounds the length; after push it is nonzero.
            NonZeroUsize::new(self.namespace_scopes.len())
        };
        self.stack.push(Element {
            name: stack_name,
            raw_encoding,
            namespace_scope,
        });
        self.emit(
            EventKind::StartElement {
                name: expanded_name,
                attributes: attrs,
            },
            position,
        )?;
        if empty {
            let first_end = self.pending.len();
            let mut end_position = self.source().position_at(token.len(), 0);
            if self.sources.len() > 1 {
                end_position = position;
            }
            self.end_element(end_position)?;
            if let Some(pending) = self.pending.get_mut(first_end) {
                pending.raw = Some(String::new_in(self.allocator));
            }
        }
        // Cache at most 4 KiB of offset records after a tag. A large attribute
        // list remains valid but does not permanently enlarge each parser.
        if raw_attrs.capacity() <= 128 {
            raw_attrs.clear();
            self.raw_attributes = raw_attrs;
        }
        Ok(())
    }

    /// Apply the same namespace identity test before either token representation.
    fn identity_frame_names(&self, name: &str, rest: &str) -> bool {
        self.config.namespace_separator.is_none()
            || (!name.contains(':')
                && self.default_namespace.is_none()
                && self.raw_attributes.iter().all(|attribute| {
                    let name = attribute.name(rest);
                    name != "xmlns" && !name.contains(':')
                }))
    }

    /// Split mutation fields from the live root source without moving its owner.
    fn identity_start_state(&mut self) -> IdentityStartState<'_> {
        debug_assert!(self.tables.defaults.is_empty());
        debug_assert!(!self.fragment && self.sources.len() == 1);
        IdentityStartState {
            source: self
                .sources
                .last()
                .expect("document source is always present"),
            allocator: self.allocator,
            limits: &self.config.limits,
            namespaces: &self.namespaces,
            raw_attributes: &mut self.raw_attributes,
            event_recycling: &mut self.event_recycling,
            stack: &mut self.stack,
            id_attribute_index: &mut self.id_attribute_index,
            seen_root: &mut self.seen_root,
            declaration_allowed: &mut self.declaration_allowed,
        }
    }

    /// Bound final spellings without charging URI work or changing error order.
    /// The caller supplies the native tag planner's existing Name proofs.
    fn expanded_names_fit_frame(&self, name: &str, rest: &str, token_bytes: usize) -> bool {
        let Some(mut bytes) = self
            .frame_namespace_name_bytes(name, false)
            .and_then(|extra| token_bytes.checked_add(extra))
        else {
            return false;
        };
        for attribute in &self.raw_attributes {
            let name = attribute.name(rest);
            if name == "xmlns" {
                return false;
            }
            if name.contains(':') {
                // Keep expanded duplicate checks bounded and allocation-free.
                if self.raw_attributes.len() > 8 {
                    return false;
                }
                let Some(total) = self
                    .frame_namespace_name_bytes(name, true)
                    .and_then(|extra| bytes.checked_add(extra))
                else {
                    return false;
                };
                bytes = total;
            }
        }
        bytes <= arena::MAX_ARENA_BYTES
    }

    /// Additional arena bytes for a validated native QName and its live binding.
    fn frame_namespace_name_bytes(&self, name: &str, attribute: bool) -> Option<usize> {
        let separator = self.config.namespace_separator?;
        let uri = match name.split_once(':') {
            Some((prefix, local)) => {
                // The Name proof covers the prefix and all local continuations.
                // QName adds nonempty parts, a local NameStart, and only one colon.
                if prefix.is_empty()
                    || !local
                        .chars()
                        .next()
                        .is_some_and(|first| self.config.name_rules.is_name_start(first))
                    || local.contains(':')
                    || prefix == "xmlns"
                {
                    return None;
                }
                Some(self.namespaces.get(prefix)?)
            }
            None if !attribute => self.default_namespace.as_ref(),
            None => None,
        };
        let Some(uri) = uri else {
            return Some(0);
        };
        // Literal fields and their NULs fit within the token. Its raw QName
        // also covers a triplet's prefix; add the URI and both possible separators.
        let separators = if separator == '\0' {
            0
        } else {
            2 * separator.len_utf8()
        };
        uri.len().checked_add(separators)
    }

    /// Lower a literal tag, optionally expanding its element and attribute names.
    /// Keep fallible copies in semantic order and publish after any end event.
    fn parse_start_frame<const EXPAND_NAMES: bool>(
        &mut self,
        token: lexical::Slice<'_>,
        position: Position,
        name: &str,
        rest: &str,
        frame: &mut AdapterFrame,
    ) -> Result<(), Error> {
        debug_assert!(self.tables.defaults.is_empty());
        debug_assert!(!self.source().has_conversions());
        debug_assert!(!self.fragment && self.sources.len() == 1);
        if !EXPAND_NAMES && !token.ends_with("/>") {
            return self
                .identity_start_state()
                .lower(name, rest, position, frame);
        }
        let mut raw_attrs =
            std::mem::replace(&mut self.raw_attributes, Vec::new_in(self.allocator));
        if raw_attrs.len() > self.config.limits.max_attributes {
            return Err(self.err(ErrorKind::LimitExceeded, "attribute count limit exceeded"));
        }
        self.event_recycling
            .reserve_adapter(&frame.generation, arena::RETAINED_ARENA_BYTES);
        frame.prepare(raw_attrs.len())?;
        let literal_span =
            !EXPAND_NAMES && !raw_attrs.is_empty() && frame.fits_literal_attributes(rest, name);
        let mut names =
            AttributeNames::new(raw_attrs.len(), self.namespaces.hasher(), self.allocator);
        for (index, attribute) in raw_attrs.iter().enumerate() {
            let (attr_name, value, attribute_offset, _) = attribute.parts(rest);
            names.check(
                attr_name,
                raw_attrs[..index]
                    .iter()
                    .map(|attribute| attribute.name(rest)),
                || {
                    self.source()
                        .position_at(1 + name.len() + attribute_offset, 0)
                },
            )?;
            if !EXPAND_NAMES && !literal_span {
                frame.push_attribute(attr_name, value)?;
            }
        }
        if EXPAND_NAMES && !raw_attrs.is_empty() {
            let mut key_lengths = [0; 8];
            for (index, attribute) in raw_attrs.iter().enumerate() {
                let name = attribute.name(rest);
                if name.contains(':') {
                    let (name, key_length) = self.expand_small_attribute_name(
                        name,
                        frame
                            .attributes()
                            .zip(key_lengths)
                            .filter(|(_, length)| *length != 0)
                            .map(|((name, _), length)| &name[..length]),
                    )?;
                    key_lengths[index] = key_length;
                    frame.push_attribute(&name, attribute.value(rest))?;
                } else {
                    frame.push_attribute(name, attribute.value(rest))?;
                }
            }
        }
        if literal_span {
            frame.push_literal_attributes(rest, &raw_attrs)?;
        }
        self.id_attribute_index = None;
        let expanded_name = if EXPAND_NAMES {
            let reusable = self.event_recycling.take_name();
            Some(self.expand_name(name, false, self.config.namespace_triplets, reusable)?)
        } else {
            frame.set_name(name)?;
            None
        };
        self.seen_root = true;
        self.declaration_allowed = false;
        let stack_name = if let Some(mut value) = expanded_name {
            // Detach callback bytes before extending the stack's packed owner.
            // Matching and End delivery keep the same raw/expanded slices.
            frame.set_name(&value)?;
            let layout = ElementNameLayout::new(&value, name)?;
            if value.capacity() < layout.capacity {
                // Keep the expansion scratch reusable when a nested name needs
                // a larger packed owner. Suffixes with spare capacity still move.
                let mut packed = self
                    .event_recycling
                    .take_name()
                    .unwrap_or_else(|| String::new_in(self.allocator));
                packed.clear();
                packed.try_reserve(layout.capacity)?;
                packed.try_push_str(&value)?;
                self.event_recycling
                    .recycle_end(self.event_recycling.token(), value);
                value = packed;
            }
            layout.finish(value, name)?
        } else {
            let reusable = self.event_recycling.take_name();
            let value = recycling::copy_name(name, reusable, self.allocator)?;
            ElementName {
                raw_start: 0,
                expanded_end: value.len(),
                value,
            }
        };
        try_push(
            &mut self.stack,
            Element {
                name: stack_name,
                raw_encoding: None,
                namespace_scope: None,
            },
        )?;
        if token.ends_with("/>") {
            let first_end = self.pending.len();
            let end_position = self.source().position_at(token.len(), 0);
            self.end_element(end_position)?;
            if let Some(pending) = self.pending.get_mut(first_end) {
                pending.raw = Some(String::new_in(self.allocator));
            }
        }
        if raw_attrs.capacity() <= 128 {
            raw_attrs.clear();
            self.raw_attributes = raw_attrs;
        }
        frame.publish(position);
        Ok(())
    }

    fn parse_end(&mut self, token: lexical::Slice<'_>, position: Position) -> Result<(), Error> {
        if self.stack.is_empty() && !self.fragment {
            return Err(self.err_at(
                ErrorKind::InvalidToken,
                "end tag outside the root element",
                1,
            ));
        }
        let body = &token[2..token.len() - 1];
        let (name, rest) = take_name(body, self.config.name_rules)
            .ok_or_else(|| self.err(ErrorKind::InvalidToken, "invalid end tag"))?;
        let decoded_name = token.for_slice(name).decoded(self.allocator)?;
        if !rest.chars().all(whitespace) {
            return Err(self.err(ErrorKind::InvalidToken, "unexpected text in end tag"));
        }
        if self.sources.len() > 1 && self.stack.len() <= self.source().initial_depth {
            return Err(self.err(
                ErrorKind::AsynchronousEntity,
                "entity closes an element outside its replacement text",
            ));
        }
        if self.stack.last().is_none_or(|element| {
            element.name.raw_name() != &*decoded_name
                || !token.for_slice(name).same_name_encoding(
                    element
                        .raw_encoding
                        .as_deref()
                        .map_or(&[], |value| value.as_slice()),
                )
        }) {
            return Err(self.err_at(ErrorKind::TagMismatch, "mismatched end tag", 2));
        }
        self.end_element(position)
    }

    fn end_element(&mut self, position: Position) -> Result<(), Error> {
        let element = self
            .stack
            .pop()
            .ok_or_else(|| self.err(ErrorKind::TagMismatch, "unexpected end tag"))?;
        // Detach before any fallible emission or restoration. Errors must drop
        // the unprocessed undo records, not leave an orphaned parser-owned block.
        let bindings = element
            .namespace_scope
            .map(|scope| self.take_namespace_scope(scope));
        self.emit(
            EventKind::EndElement {
                name: element.name.into_event_name(),
            },
            position,
        )?;
        for (prefix, previous) in bindings.into_iter().flatten().rev() {
            match previous {
                Some(uri) => {
                    self.insert_namespace(prefix.try_clone()?, uri)?;
                }
                None => {
                    self.remove_namespace(&prefix);
                }
            }
            self.emit(
                EventKind::EndNamespace {
                    prefix: (!prefix.is_empty()).then_some(prefix),
                },
                position,
            )?;
        }
        if self.stack.is_empty() && !self.fragment {
            self.closed_root = true;
        }
        Ok(())
    }

    /// Detach the closing element's top block while bounding empty-pool retention.
    fn take_namespace_scope(&mut self, scope: NonZeroUsize) -> NamespaceBindings {
        debug_assert_eq!(scope.get(), self.namespace_scopes.len());
        let bindings = self
            .namespace_scopes
            .pop()
            .expect("declaring element owns the top namespace scope");
        if self.namespace_scopes.is_empty()
            && self.namespace_scopes.capacity() > 4096 / size_of::<NamespaceBindings>()
        {
            self.namespace_scopes = Vec::new_in(self.allocator);
        }
        bindings
    }

    /// Move a matched native End name out without an Event or namespace undo.
    fn prepare_end_frame(&mut self, frame: &mut AdapterFrame) {
        debug_assert!(self.pending.is_empty() && !frame.is_active());
        debug_assert!(!self.fragment && self.sources.len() == 1);
        let element = self
            .stack
            .pop()
            .expect("matched end has an opening element");
        debug_assert!(element.namespace_scope.is_none() && element.raw_encoding.is_none());
        frame.prepare_end(element.name.into_event_name());
        if self.stack.is_empty() {
            self.closed_root = true;
        }
    }

    /// Expand once for a bounded attribute list, retaining the pre-triplet key.
    /// Large lists keep the randomized set; callers exclude unprefixed attributes.
    fn expand_small_attribute_name<'a>(
        &self,
        name: &str,
        mut prior: impl Iterator<Item = &'a [u8]>,
    ) -> Result<(String, usize), Error> {
        let mut expanded = self.expand_name(name, true, false, None)?;
        if prior.any(|previous| previous == expanded.as_bytes()) {
            return Err(self.err(
                ErrorKind::DuplicateAttribute,
                "duplicate expanded attribute name",
            ));
        }
        let key_length = expanded.len();
        let (prefix, local) = name.split_once(':').expect("qualified attribute");
        let separator = self
            .config
            .namespace_separator
            .expect("namespace processing");
        let separator_bytes = if separator == '\0' {
            0
        } else {
            separator.len_utf8()
        };
        // Preserve both URI charges from duplicate-key and callback expansion,
        // including their order relative to duplicate errors and triplet storage.
        self.charge_expansion(key_length - local.len() - separator_bytes)?;
        if self.config.namespace_triplets && separator != '\0' {
            expanded.try_push(separator)?;
            expanded.try_push_str(prefix)?;
        }
        Ok((expanded, key_length))
    }

    fn expand_name(
        &self,
        name: &str,
        attribute: bool,
        triplets: bool,
        reusable: Option<String>,
    ) -> Result<String, Error> {
        let Some(separator) = self.config.namespace_separator else {
            return recycling::copy_name(name, reusable, self.allocator).map_err(Into::into);
        };
        let (prefix, local) = match name.split_once(':') {
            // Raw QNames were validated before custom characters were decoded.
            // A converted colon can introduce an empty or colon-containing local
            // name; Expat expands the first decoded colon without retokenizing.
            Some((prefix, local)) => (Some(prefix), local),
            None => (None, name),
        };
        let uri =
            match prefix {
                Some("xmlns") => {
                    return Err(self.err(
                        ErrorKind::ReservedPrefixXmlns,
                        "xmlns cannot prefix an element or ordinary attribute",
                    ));
                }
                Some("") => Some(self.default_namespace.as_ref().ok_or_else(|| {
                    self.err(ErrorKind::UndefinedPrefix, "unbound namespace prefix")
                })?),
                Some(prefix) => Some(self.namespaces.get(prefix).ok_or_else(|| {
                    self.err(ErrorKind::UndefinedPrefix, "unbound namespace prefix")
                })?),
                None if !attribute => self.default_namespace.as_ref(),
                None => None,
            };
        let Some(uri) = uri else {
            return recycling::copy_name(local, reusable, self.allocator).map_err(Into::into);
        };
        // A short qualified name can reuse an arbitrarily long URI on every
        // element or attribute. Bound that copied output independently of input.
        self.charge_expansion(uri.len())?;
        let capacity = uri.len() + local.len() + prefix.map_or(2, |p| p.len() + 2);
        let mut result = if let Some(mut result) = reusable {
            result.clear();
            result.try_reserve(capacity)?;
            result
        } else {
            String::try_with_capacity_in(capacity, self.allocator)?
        };
        result.try_push_str(uri)?;
        if separator != '\0' {
            result.try_push(separator)?;
        }
        result.try_push_str(local)?;
        if triplets
            && separator != '\0'
            && let Some(prefix) = prefix
        {
            result.try_push(separator)?;
            result.try_push_str(prefix)?;
        }
        Ok(result)
    }

    /// Set the base URI captured by subsequent external entity declarations.
    /// Existing declarations retain their original base, including a missing base.
    pub fn set_base(&mut self, base: Option<&[u8]>) -> Result<(), Error> {
        let base = base
            .map(|base| {
                let mut bytes = Vec::new_in(self.allocator);
                xeme_storage::try_extend_from_slice(&mut bytes, base)?;
                Shared::try_new_in(bytes, self.allocator)
            })
            .transpose()?;
        self.base = base;
        Ok(())
    }

    fn copy_external_base(&self, base: Option<&Shared<Vec<u8>>>) -> Result<Option<Vec<u8>>, Error> {
        base.map(|base| {
            self.charge_expansion(base.len())?;
            Ok((**base).try_clone()?)
        })
        .transpose()
    }

    fn charge_external_identifiers(&self, entity: &Entity) -> Result<(), Error> {
        // A short reference can replay long declaration identifiers many times,
        // even when the application declines to load the external entity.
        if let Some(system_id) = &entity.system_id {
            self.charge_expansion(system_id.len())?;
        }
        if let Some(public_id) = &entity.public_id {
            self.charge_expansion(public_id.len())?;
        }
        Ok(())
    }

    fn charge_expansion(&self, size: usize) -> Result<(), Error> {
        let limit = self.work_bytes_limit(self.config.limits.max_entity_expansion_bytes);
        self.expanded
            .expanded
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |expanded| {
                expanded
                    .checked_add(size)
                    .filter(|expanded| *expanded <= limit)
            })
            .map_err(|_| {
                self.err(
                    ErrorKind::LimitExceeded,
                    "entity expansion byte limit exceeded",
                )
            })?;
        Ok(())
    }

    /// Comments and PI data normalize the converted callback string itself.
    fn markup_text(&self, text: lexical::Slice<'_>) -> Result<String, Error> {
        normalize_newlines(&text.decoded(self.allocator)?, self.allocator)
    }

    /// Prepare detached Text or an explicit C-host range, or keep the owned projection.
    /// A prepared frame is not visible until the caller reaches its emit point.
    fn prepare_character_data(
        &mut self,
        count: usize,
        frame: Option<&mut AdapterFrame>,
        c_text_context: bool,
        no_carriage_returns: bool,
    ) -> Result<Option<Text>, Error> {
        let text = &self.source().remaining()[..count];
        if let Some(frame) = frame
            && count <= arena::MAX_ARENA_BYTES
            && !self.source().has_conversions()
            && (no_carriage_returns || self.sources.len() > 1 || !text.contains('\r'))
        {
            if count > arena::INLINE_TEXT_BYTES {
                self.event_recycling
                    .reserve_adapter(&frame.generation, arena::RETAINED_ARENA_BYTES);
            }
            if c_text_context
                && count > 0
                && !self.fragment
                && !self.external_subset
                && self.sources.len() == 1
                && let Some(start) = self.source().native_utf8_byte_index()
            {
                frame.prepare_native_text(start, count)?;
            } else {
                frame.prepare_text(&self.source().remaining()[..count])?;
            }
            Ok(None)
        } else if no_carriage_returns {
            Text::try_from_str_in(text, self.allocator)
                .map(Some)
                .map_err(Into::into)
        } else {
            self.character_data(text).map(Some)
        }
    }

    fn character_data(&self, text: &str) -> Result<Text, Error> {
        if self.source().has_conversions() {
            let lexical = self.source().lexical_remaining().for_slice(text);
            if lexical.has_ascii_aliases() {
                return self.source_text(lexical).map(Text::from);
            }
        }
        if self.sources.len() == 1 && text.contains('\r') {
            normalize_newlines(text, self.allocator).map(Text::from)
        } else {
            Text::try_from_str_in(text, self.allocator).map_err(Into::into)
        }
    }

    fn source_text(&self, text: lexical::Slice<'_>) -> Result<String, Error> {
        if self.sources.len() > 1 {
            // Literal line endings were normalized when the entity was declared.
            text.decode(self.allocator).map_err(Into::into)
        } else {
            text.normalized(self.allocator)
        }
    }

    fn requires_internal_entity_declaration(&self) -> bool {
        if self.external_subset {
            return false;
        }
        if self.in_doctype && self.standalone {
            // Defaults inside a parameter entity may use its own declarations;
            // the standalone constraint applies to references in the document.
            self.sources.len() == 1
        } else {
            self.standalone || !self.has_external_subset
        }
    }

    /// Refill common literal values without allocating a new owner. Keep
    /// converted aliases and normalization on the provenance-aware expansion path.
    fn expand_attribute_into(
        &self,
        value: lexical::Slice<'_>,
        output: &mut String,
        tokenized: bool,
        normalize_line_endings: bool,
    ) -> Result<(), Error> {
        if tokenized
            || value.has_ascii_aliases()
            || value
                .bytes()
                .any(|byte| matches!(byte, b'&' | b'<' | b'\t' | b'\r' | b'\n'))
        {
            output.try_reserve(value.len().saturating_sub(output.len()))?;
            output.clear();
            self.append_attribute(value, output, tokenized, normalize_line_endings)?;
            if tokenized && output.ends_with(' ') {
                output.truncate(output.len() - 1);
            }
        } else {
            copy_attribute_string(output, &value)?;
        }
        Ok(())
    }

    fn expand_attribute(
        &self,
        value: lexical::Slice<'_>,
        tokenized: bool,
        normalize_line_endings: bool,
    ) -> Result<String, Error> {
        let mut output = String::try_with_capacity_in(value.len(), self.allocator)?;
        self.append_attribute(value, &mut output, tokenized, normalize_line_endings)?;
        if tokenized && output.ends_with(' ') {
            output.truncate(output.len() - 1);
        }
        Ok(output)
    }

    /// Expand borrowed replacement frames into one output. Original lexical
    /// slices survive each frame so converted ASCII keeps its literal role.
    /// No parser table mutation or application callback occurs during expansion.
    fn append_attribute(
        &self,
        value: lexical::Slice<'_>,
        output: &mut String,
        tokenized: bool,
        normalize_line_endings: bool,
    ) -> Result<(), Error> {
        struct Frame<'a> {
            rest: lexical::Slice<'a>,
            name: &'a str,
        }
        let mut frames = Vec::new_in(self.allocator);
        let mut active =
            HashSet::with_hasher_in(self.tables.entities.hasher().clone(), self.allocator);
        let mut rest = value;
        loop {
            if rest.is_empty() {
                let Some(Frame { rest: parent, name }) = frames.pop() else {
                    break;
                };
                active.remove(name);
                rest = parent;
                continue;
            }
            let end = rest.find(['&', '<']).unwrap_or(rest.len());
            if !frames.is_empty() {
                self.account_entity_bytes(end, true)?;
            }
            let literal = rest.for_slice(&rest.as_str()[..end]);
            if tokenized {
                for (character, raw_ascii) in literal.decoded_chars() {
                    if raw_ascii && whitespace(character) {
                        append_attribute_space(output)?;
                    } else {
                        output.try_push(character)?;
                    }
                }
            } else {
                // Stored replacements have already normalized their physical line
                // endings; remaining CR and LF are independent data characters.
                append_lexical_attribute(
                    output,
                    literal,
                    normalize_line_endings && frames.is_empty(),
                )?;
            }
            rest = rest.for_slice(&rest.as_str()[end..]);
            if rest.is_empty() {
                continue;
            }
            if rest.starts_with('<') {
                return Err(self.err(
                    ErrorKind::InvalidToken,
                    "literal less-than sign in an attribute",
                ));
            }
            let end = rest.find(';').ok_or_else(|| {
                self.err(
                    ErrorKind::InvalidToken,
                    "unclosed attribute entity reference",
                )
            })?;
            if !frames.is_empty() {
                self.account_entity_bytes(end + 1, true)?;
            }
            let raw_name = &rest.as_str()[1..end];
            if let Some(character) = character_reference(raw_name)
                .map_err(|(kind, _)| self.err(kind, "invalid character reference"))?
            {
                if !raw_name.starts_with('#') {
                    self.account_entity_bytes(1, false)?;
                }
                if tokenized && character == ' ' {
                    append_attribute_space(output)?;
                } else {
                    output.try_push(character)?;
                }
                rest = rest.for_slice(&rest.as_str()[end + 1..]);
                continue;
            }
            let decoded_name = rest.for_slice(raw_name).decoded(self.allocator)?;
            let name: &str = &decoded_name;
            if !self.config.name_rules.is_name(raw_name)
                || (self.config.namespace_separator.is_some() && raw_name.contains(':'))
            {
                return Err(self.err(ErrorKind::InvalidToken, "invalid entity name"));
            }
            if active.contains(name) || self.active_entities.inherited_contains(name, false) {
                return Err(self.err(
                    ErrorKind::RecursiveEntityReference,
                    "recursive entity in attribute",
                ));
            }
            if frames.len()
                + self.sources.len()
                + self.external_depth
                + self.inherited_parameter_depth
                > self.config.limits.max_entity_depth
            {
                return Err(self.err(ErrorKind::LimitExceeded, "entity nesting limit exceeded"));
            }
            let Some((key, entity)) = self.tables.entities.get_key_value(name) else {
                if !self.requires_internal_entity_declaration() {
                    rest = rest.for_slice(&rest.as_str()[end + 1..]);
                    continue;
                }
                return Err(self.err(ErrorKind::UndefinedEntity, "undefined entity in attribute"));
            };
            if entity.declared_in_parameter_entity && self.requires_internal_entity_declaration() {
                return Err(self.err(
                    ErrorKind::EntityDeclaredInParameterEntity,
                    "entity was declared in a parameter entity",
                ));
            }
            if entity.notation.is_some() {
                return Err(self.err(
                    ErrorKind::BinaryEntityReference,
                    "unparsed entity in attribute",
                ));
            }
            let value = entity.value.as_ref().ok_or_else(|| {
                self.err(
                    ErrorKind::ExternalEntityInAttribute,
                    "external entity in attribute",
                )
            })?;
            self.charge_expansion(value.len())?;
            // Use the stable map key: a decoded custom name can be a temporary
            // owner, while both the frame and active index outlive this iteration.
            try_set_insert(&mut active, key.as_str())?;
            try_push(
                &mut frames,
                Frame {
                    rest: rest.for_slice(&rest.as_str()[end + 1..]),
                    name: key.as_str(),
                },
            )?;
            rest = lexical::Slice::plain(value);
        }
        Ok(())
    }
}

#[cfg(test)]
mod context_text_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanMode {
    Tag,
    Comment,
    Pi,
    Doctype,
    DtdDeclaration,
}

fn take_name(input: &str, name_rules: NameRules) -> Option<(&str, &str)> {
    let mut chars = input.char_indices();
    if !name_rules.is_name_start(chars.next()?.1) {
        return None;
    }
    let end = chars
        .find(|(_, c)| !name_rules.is_name_char(*c))
        .map_or(input.len(), |(index, _)| index);
    Some((&input[..end], &input[end..]))
}

/// Byte offsets within one tag's attribute text. Scratch records never borrow input.
#[derive(Clone, Copy, Debug)]
struct RawAttribute {
    name_start: usize,
    name_end: usize,
    value_start: usize,
    value_end: usize,
}

impl RawAttribute {
    fn name<'a>(&self, text: &'a str) -> &'a str {
        &text[self.name_start..self.name_end]
    }

    fn value<'a>(&self, text: &'a str) -> &'a str {
        &text[self.value_start..self.value_end]
    }

    fn parts<'a>(&self, text: &'a str) -> (&'a str, &'a str, usize, usize) {
        (
            self.name(text),
            self.value(text),
            self.name_start,
            self.value_start,
        )
    }
}

/// One duplicate-name policy for ordinary and arena-backed start events.
struct AttributeNames<'a> {
    index: Option<HashSet<&'a str>>,
}

impl<'a> AttributeNames<'a> {
    /// Keep short attribute lists allocation-free and larger lists randomized.
    #[inline]
    fn new(count: usize, hasher: &xeme_storage::SaltedRandomState, allocator: Allocator) -> Self {
        Self {
            index: (count > 8).then(|| HashSet::with_hasher_in(hasher.clone(), allocator)),
        }
    }

    /// Hash large lists, scan small lists, and compute coordinates only on failure.
    #[inline]
    fn check(
        &mut self,
        name: &'a str,
        mut prior: impl Iterator<Item = &'a str>,
        position: impl FnOnce() -> Position,
    ) -> Result<(), Error> {
        let duplicate = if let Some(names) = &mut self.index {
            !try_set_insert(names, name)?
        } else {
            prior.any(|previous| previous == name)
        };
        if duplicate {
            return Err(Error {
                kind: ErrorKind::DuplicateAttribute,
                message: "duplicate attribute",
                position: position(),
            });
        }
        Ok(())
    }
}

/// Declaration grammar is independent of the encoding-detection BOM policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeclarationContext {
    Document,
    Text,
}

impl DeclarationContext {
    fn error_kind(self) -> ErrorKind {
        match self {
            Self::Document => ErrorKind::XmlDeclaration,
            Self::Text => ErrorKind::TextDeclaration,
        }
    }
}

struct Declaration<'a> {
    version: Option<lexical::Decoded<'a>>,
    encoding: Option<lexical::Decoded<'a>>,
    standalone: Option<bool>,
}

enum DeclarationFailure {
    Syntax {
        message: &'static str,
        offset: Option<usize>,
    },
    Allocation(AllocError),
}

/// Validate complete fields before checking their declared encoding. The caller
/// supplies lexical decoding; bootstrap ASCII values use borrowed slices only.
fn declaration_fields<'a>(
    rest: &'a str,
    attrs: &[RawAttribute],
    context: DeclarationContext,
    allow_invalid_xml_versions: bool,
    mut decode: impl FnMut(&'a str) -> Result<lexical::Decoded<'a>, AllocError>,
) -> Result<Declaration<'a>, DeclarationFailure> {
    let syntax = |message| DeclarationFailure::Syntax {
        message,
        offset: None,
    };
    let mut decode = |value: &'a str| decode(value).map_err(DeclarationFailure::Allocation);
    if context == DeclarationContext::Text {
        let mut attrs = attrs.iter().map(|attribute| attribute.parts(rest));
        let first = attrs
            .next()
            .ok_or_else(|| syntax("empty text declaration"))?;
        let (version, encoding_attr) = if first.0 == "version" {
            (Some(decode(first.1)?), attrs.next())
        } else {
            (None, Some(first))
        };
        if !allow_invalid_xml_versions
            && version
                .as_ref()
                .is_some_and(|version| !valid_xml_version(version))
        {
            return Err(syntax("invalid XML version"));
        }
        let (name, encoding, _, _) =
            encoding_attr.ok_or_else(|| syntax("text declaration requires an encoding"))?;
        let encoding = decode(encoding)?;
        if name != "encoding" || !valid_encoding_name(&encoding) || attrs.next().is_some() {
            return Err(syntax("invalid text declaration"));
        }
        return Ok(Declaration {
            version,
            encoding: Some(encoding),
            standalone: None,
        });
    }
    let version = attrs
        .first()
        .map(|attribute| decode(attribute.value(rest)))
        .transpose()?;
    if attrs.is_empty()
        || attrs[0].name(rest) != "version"
        || !version.as_ref().is_some_and(|version| {
            if !allow_invalid_xml_versions {
                return valid_xml_version(version);
            }
            !version.is_empty()
                && version
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        })
    {
        return Err(DeclarationFailure::Syntax {
            message: "XML declaration must begin with a version",
            offset: Some(attrs.first().map_or(0, |attribute| attribute.name_start)),
        });
    }
    let mut encoding = None;
    let mut standalone = None;
    for attribute in attrs.iter().skip(1) {
        let (name, value, _, _) = attribute.parts(rest);
        match name {
            "encoding" if encoding.is_none() && standalone.is_none() => {
                let value = decode(value)?;
                if !valid_encoding_name(&value) {
                    return Err(syntax("invalid encoding name"));
                }
                encoding = Some(value);
            }
            "standalone" if standalone.is_none() => {
                standalone = Some(match value {
                    "yes" => true,
                    "no" => false,
                    _ => return Err(syntax("invalid standalone declaration")),
                });
            }
            _ => return Err(syntax("invalid XML declaration attribute")),
        }
    }
    Ok(Declaration {
        version,
        encoding,
        standalone,
    })
}

fn valid_xml_version(version: &str) -> bool {
    version
        .strip_prefix("1.")
        .is_some_and(|minor| !minor.is_empty() && minor.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Prove malformed syntax only in a complete bounded ASCII bootstrap declaration.
/// Ordinary parsing still reports the error after token limits, raw publication
/// and source accounting. This helper creates no owned values or parser state.
fn malformed_ascii_declaration(
    rest: &str,
    context: DeclarationContext,
    allow_invalid_xml_versions: bool,
) -> bool {
    debug_assert!(rest.is_ascii());
    if invalid_xml_char(rest).is_some() {
        return true;
    }
    let empty = RawAttribute {
        name_start: 0,
        name_end: 0,
        value_start: 0,
        value_end: 0,
    };
    let mut attrs = [empty; 3];
    let mut count = 0;
    let mut scanner = tag::AttributeScanner::new(0);
    loop {
        // Fourth/Fifth edition name rules agree for this proven ASCII input.
        match scanner.next(rest, true, false, false, 3, NameRules::default()) {
            Ok(tag::Step::Attribute(attribute)) => {
                attrs[count] = attribute;
                count += 1;
            }
            Ok(tag::Step::End) => {
                return declaration_fields(
                    rest,
                    &attrs[..count],
                    context,
                    allow_invalid_xml_versions,
                    |value| Ok(lexical::Decoded::Borrowed(value)),
                )
                .is_err();
            }
            Err(_) => return true,
            Ok(tag::Step::Incomplete | tag::Step::TagEnd { .. }) => {
                unreachable!("complete attribute view excludes a tag delimiter")
            }
        }
    }
}

fn parse_raw_attributes(
    text: &str,
    allow_refs: bool,
    result: &mut Vec<RawAttribute>,
    limit: usize,
    name_rules: NameRules,
) -> Result<(), Error> {
    result.clear();
    let mut scanner = tag::AttributeScanner::new(0);
    loop {
        let step = scanner
            .next(text, true, false, allow_refs, limit, name_rules)
            .map_err(|error| Error {
                kind: error.kind,
                message: error.message,
                position: Position {
                    byte_index: error.offset,
                    line: 1,
                    column: 0,
                    byte_count: 0,
                },
            })?;
        match step {
            tag::Step::Attribute(attribute) => try_push(result, attribute)?,
            tag::Step::End => return Ok(()),
            tag::Step::Incomplete | tag::Step::TagEnd { .. } => {
                unreachable!("complete attribute views have no tag delimiter")
            }
        }
    }
}

/// Resolve a coalesced span before newline boundaries can stop the scalar scan.
fn coalesced_text_end(text: &str) -> Option<usize> {
    // Markup takes precedence even at the complete-line cutoff. Search bytes so
    // the bounded prefix may end inside a UTF-8 character without slicing str.
    let bytes = text.as_bytes();
    if let Some(end) = memchr::memchr2(b'<', b'&', &bytes[..bytes.len().min(65_537)]) {
        return Some(end);
    }
    (bytes.len() <= 65_536).then_some(bytes.len())
}

fn string(text: &str, allocator: Allocator) -> Result<String, Error> {
    Ok(String::try_from_str_in(text, allocator)?)
}
fn normalize_newlines(text: &str, allocator: Allocator) -> Result<String, Error> {
    if !text.contains('\r') {
        return string(text, allocator);
    }
    let mut output = String::try_with_capacity_in(text.len(), allocator)?;
    let mut previous_cr = false;
    for character in text.chars() {
        if character == '\n' && previous_cr {
            previous_cr = false;
            continue;
        }
        previous_cr = character == '\r';
        output.try_push(if previous_cr { '\n' } else { character })?;
    }
    Ok(output)
}
fn append_lexical_attribute(
    output: &mut String,
    text: lexical::Slice<'_>,
    normalize_line_endings: bool,
) -> Result<(), Error> {
    if !text.has_ascii_aliases() {
        let text = text.as_str();
        let mut start = 0;
        while let Some(offset) = memchr::memchr3(b'\t', b'\r', b'\n', &text.as_bytes()[start..]) {
            let end = start + offset;
            output.try_push_str(&text[start..end])?;
            output.try_push(' ')?;
            start = end + 1;
            if normalize_line_endings
                && text.as_bytes()[end] == b'\r'
                && text.as_bytes().get(start) == Some(&b'\n')
            {
                start += 1;
            }
        }
        output.try_push_str(&text[start..])?;
        return Ok(());
    }
    let mut previous_cr = false;
    for (character, raw_ascii) in text.decoded_chars() {
        if raw_ascii && character == '\n' && previous_cr {
            previous_cr = false;
            continue;
        }
        previous_cr = normalize_line_endings && raw_ascii && character == '\r';
        output.try_push(if raw_ascii && whitespace(character) {
            ' '
        } else {
            character
        })?;
    }
    Ok(())
}

/// The direct attribute-copy path in Expat checks raw token spelling before
/// decoding, so a converted space alone does not require tokenized normalization.
fn attribute_needs_normalization(value: &str) -> bool {
    value.starts_with(' ')
        || value.ends_with(' ')
        || value.contains("  ")
        || value.contains(['\t', '\r', '\n', '&'])
}

fn append_attribute_space(output: &mut String) -> Result<(), AllocError> {
    if !output.is_empty() && !output.ends_with(' ') {
        output.try_push(' ')?;
    }
    Ok(())
}

fn valid_encoding_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
/// Decode a reference, returning an error offset relative to its opening `&`.
fn character_reference(name: &str) -> Result<Option<char>, (ErrorKind, usize)> {
    match name {
        "lt" => Ok(Some('<')),
        "gt" => Ok(Some('>')),
        "amp" => Ok(Some('&')),
        "apos" => Ok(Some('\'')),
        "quot" => Ok(Some('"')),
        name if name.starts_with('#') => {
            let (digits, radix) = name
                .strip_prefix("#x")
                .map_or((&name[1..], 10), |digits| (digits, 16));
            let invalid = digits.bytes().position(|c| {
                if radix == 16 {
                    !c.is_ascii_hexdigit()
                } else {
                    !c.is_ascii_digit()
                }
            });
            if digits.is_empty() || invalid.is_some() {
                return Err((
                    ErrorKind::InvalidToken,
                    1 + name.len() - digits.len() + invalid.unwrap_or(0),
                ));
            }
            let value = u32::from_str_radix(digits, radix)
                .ok()
                .and_then(char::from_u32)
                .filter(|c| is_xml_char(*c))
                .ok_or((ErrorKind::BadCharacterReference, 0))?;
            Ok(Some(value))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod input_bound_tests {
    use super::*;

    #[test]
    fn source_limit_rejection_preserves_input_state() {
        for limit in [0, 7, Limits::default().max_total_bytes, usize::MAX] {
            let mut config = Config::default();
            config.limits.max_total_bytes = limit;
            let mut parser = Parser::new(config);
            let bound = limit.min(isize::MAX as usize);
            // Seed cumulative history without allocating the preceding bytes.
            parser.received = bound.saturating_sub(1);
            if bound != 0 {
                assert_eq!(parser.input_bytes_remaining(), 1);
                parser.feed(b" ", false).unwrap();
            }
            assert_eq!(parser.received, bound);
            assert_eq!(parser.input_bytes_remaining(), 0);
            parser.feed_start_byte = 17;
            let before = format!("{:?}{:?}", parser.sources, parser.decoder);
            let error = parser.feed(b"x", true).unwrap_err();
            assert_eq!(error.kind, ErrorKind::LimitExceeded);
            assert_eq!(parser.received, bound);
            assert_eq!(parser.feed_start_byte, 17);
            assert!(!parser.final_input);
            assert_eq!(format!("{:?}{:?}", parser.sources, parser.decoder), before);
        }
        let mut config = Config::default();
        config.limits.max_total_bytes = 0;
        let mut parser = Parser::new(config);
        parser.feed(b"", true).unwrap();
        assert_eq!(parser.received, 0);
        assert!(parser.final_input);
    }

    #[test]
    fn adapter_input_owner_matches_decoded_events_and_positions() {
        let utf16 = "<r>é</r>"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<std::vec::Vec<_>>();
        for input in [
            b"<r a='v'>abc<n/>tail</r>".as_slice(),
            "\u{feff}<?xml version='1.0'?><r>é\r\n&amp;z</r>".as_bytes(),
            b"<?xml version='1.0' encoding='ISO-8859-1'?><r>\xe9</r>",
            b"<r>abc\xfftail</r>",
            b"<r>abc\xc3",
            &utf16,
        ] {
            for width in [1, 2, 3, 4, 7, input.len()] {
                let mut ordinary = Parser::new(Config::default());
                let mut context = Parser::new(Config::default());
                context.enable_input_context();
                let mut fed = 0;
                for chunk in input.chunks(width) {
                    fed += chunk.len();
                    let final_input = fed == input.len();
                    assert_eq!(
                        context.feed(chunk, final_input),
                        ordinary.feed(chunk, final_input)
                    );
                    let (raw, start) = context.input_context();
                    assert_eq!(raw, &input[start..fed]);
                    loop {
                        let expected = ordinary.next_event();
                        let actual = context.next_event();
                        assert_eq!(actual, expected, "width={width}, fed={fed}");
                        assert_eq!(context.current_raw(), ordinary.current_raw());
                        if !matches!(actual, Ok(Some(_))) {
                            break;
                        }
                    }
                    if context.error.is_some() {
                        break;
                    }
                }
            }
        }
    }

    #[test]
    fn adapter_input_owner_covers_first_feed_detection_and_fallback() {
        for input in ["<r>é</r>", "\u{feff}<r>é</r>"] {
            let mut parser = Parser::new(Config::default());
            parser.enable_input_context();
            parser.feed(input.as_bytes(), true).unwrap();
            assert_eq!(
                parser.input_context().0.as_ptr(),
                parser.sources[0].text.as_bytes().as_ptr()
            );
            assert_eq!(parser.input_context().0, input.as_bytes());
            assert_eq!(
                parser.sources[0].remaining(),
                input.trim_start_matches('\u{feff}')
            );
            while parser.next_event().unwrap().is_some() {}
        }
        let mut unaligned = false;
        for padding in 0..4 {
            let input = format!("<r>{}{}<n/>", "€".repeat(23_000), "x".repeat(padding));
            let mut parser = Parser::new(Config::default());
            parser.enable_input_context();
            parser.feed(input.as_bytes(), false).unwrap();
            while parser.next_event().unwrap().is_some() {}
            parser.feed(b"</r>", true).unwrap();
            let (context, start) = parser.input_context();
            assert_eq!(start, input.len() - 1024);
            assert_eq!(context, [&input.as_bytes()[start..], b"</r>"].concat());
            let text = parser.sources[0].text.as_bytes();
            let offset = text.len() - context.len();
            assert!(offset <= 3);
            assert_eq!(context.as_ptr(), text.as_ptr().wrapping_add(offset));
            unaligned |= offset != 0;
            while parser.next_event().unwrap().is_some() {}
        }
        assert!(unaligned);
        let mut parser = Parser::new(Config::default());
        parser.enable_input_context();
        parser.feed(b"<?xml version='1.0'", false).unwrap();
        assert_eq!(
            parser.input_context().0.as_ptr(),
            parser.sources[0].text.as_bytes().as_ptr()
        );
        assert!(parser.sources[0].remaining().is_empty());
        assert_eq!(parser.next_event().unwrap(), None);
        parser.feed(b"?><r>abc", false).unwrap();
        assert_eq!(
            parser.input_context().0.as_ptr(),
            parser.sources[0].text.as_bytes().as_ptr()
        );
        while parser.next_event().unwrap().is_some() {}
        parser.feed(b"\xc3", false).unwrap();
        assert_ne!(
            parser.input_context().0.as_ptr(),
            parser.sources[0].text.as_bytes().as_ptr()
        );
        parser.feed(b"\xa9</r>", true).unwrap();
        assert_ne!(
            parser.input_context().0.as_ptr(),
            parser.sources[0].text.as_bytes().as_ptr()
        );
        while parser.next_event().unwrap().is_some() {}

        // A child may already inherit a custom map before its first feed.
        let mut parent = Parser::new(Config {
            encoding: Some("test-map".to_owned()),
            ..Config::default()
        });
        parent.feed(b"<r>", false).unwrap();
        assert_eq!(
            parent.next_event().unwrap_err().kind,
            ErrorKind::UnknownEncoding
        );
        parent
            .set_encoding_map("test-map", std::array::from_fn(|byte| byte as i32))
            .unwrap();
        while parent.next_event().unwrap().is_some() {}
        let mut child = parent
            .external_child_with_encoding(Some(""), Some("test-map"))
            .unwrap();
        child.enable_input_context();
        child.feed(b"abc", true).unwrap();
        assert_ne!(
            child.input_context().0.as_ptr(),
            child.sources[0].text.as_bytes().as_ptr()
        );
        assert!(matches!(
            child.next_event().unwrap().unwrap().kind,
            EventKind::Text(_)
        ));
        assert_eq!(child.next_event().unwrap(), None);
    }

    #[test]
    fn external_input_allowance_belongs_to_each_source() {
        let mut config = Config::default();
        config.limits.max_total_bytes = 7;
        let mut parent = Parser::new(config);
        parent.feed(b"<r>", false).unwrap();
        while parent.next_event().unwrap().is_some() {}
        for context in [None, Some("")] {
            let mut child = parent.external_child(context, None).unwrap();
            assert_eq!(child.input_bytes_remaining(), 7);
            let text = if context.is_some() { b"<c/>" } else { b"    " };
            child.feed(text, true).unwrap();
            while child.next_event().unwrap().is_some() {}
            assert_eq!(child.input_bytes_remaining(), 3);
            assert_eq!(parent.input_bytes_remaining(), 4);
        }
        parent.feed(b"</r>", true).unwrap();
        while parent.next_event().unwrap().is_some() {}
        assert_eq!(parent.input_bytes_remaining(), 0);
        assert_eq!(parent.sources[0].position(0).byte_index, 7);
        assert!(parent.is_finished());
    }
}

#[cfg(test)]
mod streaming_work_tests {
    use super::*;

    fn drain(parser: &mut Parser) -> Result<(), Error> {
        while parser.next_event()?.is_some() {}
        Ok(())
    }

    #[test]
    fn linear_namespace_and_default_work_can_outlive_the_initial_allowance() {
        let documents = [
            format!("<r xmlns:p='urn:test'>{}</r>", "<p:e/>".repeat(100)),
            format!(
                "<!DOCTYPE r [<!ATTLIST e a CDATA 'value'>]><r>{}</r>",
                "<e/>".repeat(100)
            ),
        ];
        for document in documents {
            for factor in [None, Some(100)] {
                let mut config = Config {
                    namespace_separator: Some(' '),
                    ..Config::default()
                };
                config.limits.max_entity_expansion_bytes = 64;
                config.limits.max_work_amplification = factor;
                let mut parser = Parser::new(config);
                parser.feed(document.as_bytes(), true).unwrap();
                let result = drain(&mut parser);
                if factor.is_some() {
                    result.unwrap();
                    assert!(parser.is_finished());
                    assert!(parser.expanded.expanded.load(Ordering::Relaxed) > 64);
                } else {
                    assert_eq!(result.unwrap_err().kind, ErrorKind::LimitExceeded);
                }
            }
        }
    }

    #[test]
    fn large_reused_names_and_defaults_still_exhaust_relative_work() {
        let value = "x".repeat(2048);
        let documents = [
            format!("<r xmlns:p='{value}'>{}</r>", "<p:e/>".repeat(300)),
            format!(
                "<!DOCTYPE r [<!ATTLIST e a CDATA '{value}'>]><r>{}</r>",
                "<e/>".repeat(300)
            ),
        ];
        for document in documents {
            // If merely feeding bytes earned work credit, this suffix would
            // finance all of the earlier expansion before it was consumed.
            let input = format!("{document}{}", " ".repeat(64 * 1024));
            for chunk_size in [input.len(), 37] {
                let mut config = Config {
                    namespace_separator: Some(' '),
                    ..Config::default()
                };
                config.limits.max_entity_expansion_bytes = 64;
                config.limits.max_work_amplification = Some(100);
                let mut parser = Parser::new(config);
                assert!(parser.set_entity_maximum_amplification(f32::INFINITY));
                assert!(parser.set_entity_activation_threshold(u64::MAX));
                let result = input
                    .as_bytes()
                    .chunks(chunk_size)
                    .enumerate()
                    .try_for_each(|(index, chunk)| {
                        let final_chunk = (index + 1) * chunk_size >= input.len();
                        parser.feed(chunk, final_chunk)?;
                        drain(&mut parser)
                    });
                let error = result.unwrap_err();
                assert_eq!(error.kind, ErrorKind::LimitExceeded);
                assert!(error.position.byte_index < document.len());
                assert!(parser.expanded.expanded.load(Ordering::Relaxed) > 64);
                assert!(parser.work_bytes_limit(0) <= 100 * document.len());
                if chunk_size == input.len() {
                    assert_eq!(parser.received, input.len());
                }
            }
        }
    }

    #[test]
    fn saturated_work_threshold_still_rejects_counter_overflow() {
        let mut config = Config::default();
        config.limits.max_work_amplification = Some(100);
        let parser = Parser::new(config);
        assert!(parser.expanded.account(usize::MAX, false, false));
        assert_eq!(parser.work_bytes_limit(0), usize::MAX);
        parser
            .expanded
            .expanded
            .store(usize::MAX, Ordering::Relaxed);
        parser.charge_expansion(0).unwrap();
        assert_eq!(
            parser.charge_expansion(1).unwrap_err().kind,
            ErrorKind::LimitExceeded
        );
        assert_eq!(parser.expanded.expanded.load(Ordering::Relaxed), usize::MAX);
    }
}

#[cfg(test)]
mod matching_end_tests {
    use super::*;

    #[test]
    fn matching_end_tags_resume_across_every_byte_split() {
        for name in ["r", "prefix:local", "é", "雪", "a·b", &"n".repeat(1024)] {
            let opening = format!("<{name}>");
            let closing = format!("</{name}>");
            for split in 0..closing.len() {
                let mut parser = Parser::new(Config::default());
                parser.set_reparse_deferral_enabled(false);
                parser.feed(opening.as_bytes(), false).unwrap();
                assert!(matches!(
                    parser.next_event().unwrap().unwrap().kind,
                    EventKind::StartElement { .. }
                ));
                parser.feed(&closing.as_bytes()[..split], false).unwrap();
                assert_eq!(parser.matching_root_end_tag(usize::MAX), None);
                assert!(parser.next_event().unwrap().is_none());
                parser.feed(&closing.as_bytes()[split..], true).unwrap();
                assert_eq!(
                    parser.matching_root_end_tag(closing.len()),
                    Some(closing.len())
                );
                assert!(matches!(
                    parser.next_event().unwrap().unwrap().kind,
                    EventKind::EndElement { name: actual } if actual.as_str() == name
                ));
                assert!(parser.next_event().unwrap().is_none());
            }
        }
    }

    #[test]
    fn matching_end_tags_keep_limits_and_fallback_syntax() {
        for closing in ["</r >", "</r\r\n>", "</other>", "</r:other>", "</r<>"] {
            let mut parser = Parser::new(Config::default());
            parser.feed(b"<r>", false).unwrap();
            parser.next_event().unwrap().unwrap();
            parser.feed(closing.as_bytes(), true).unwrap();
            assert_eq!(parser.matching_root_end_tag(usize::MAX), None);
        }
        let mut config = Config::default();
        config.limits.max_token_bytes = 3;
        let mut parser = Parser::new(config);
        parser.feed(b"<r></r>", true).unwrap();
        parser.next_event().unwrap().unwrap();
        assert_eq!(parser.matching_root_end_tag(3), None);
        assert_eq!(
            parser.next_event().unwrap_err().kind,
            ErrorKind::LimitExceeded
        );

        let mut parser = Parser::new(Config {
            encoding: Some("UTF-16LE".into()),
            ..Config::default()
        });
        let utf16: std::vec::Vec<u8> = "<r></r>"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        parser.feed(&utf16, true).unwrap();
        parser.next_event().unwrap().unwrap();
        assert_eq!(parser.matching_root_end_tag(usize::MAX), None);
        assert!(matches!(
            parser.next_event().unwrap().unwrap().kind,
            EventKind::EndElement { .. }
        ));
    }
}

#[cfg(test)]
mod namespace_scope_tests {
    use super::*;

    #[test]
    fn planned_qname_eligibility_matches_full_name_validation() {
        let mut names: std::vec::Vec<std::string::String> = [
            "n",
            "p:n",
            ":n",
            "p:",
            "p:1",
            "p:-",
            "p:.",
            "p::n",
            "p:n:q",
            "xmlns:n",
            "missing:n",
            "xml:n",
            "é:n",
            "p:é",
            "p:a\u{301}",
            "p:\u{301}",
            "p:\u{901}",
            "p:\u{10000}",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        for length in [15, 16, 17, 31, 32, 33, 1024] {
            let local = "n".repeat(length);
            names.push(local.clone());
            names.push(format!("p:{local}"));
            names.push(format!("é:{local}"));
            names.push(format!("p:{local}:"));
        }
        for name_rules in [NameRules::FourthEdition, NameRules::FifthEdition] {
            for separator in [None, Some('|'), Some('\0'), Some('λ')] {
                let mut parser = Parser::new(Config {
                    namespace_separator: separator,
                    name_rules,
                    ..Config::default()
                });
                parser.default_namespace = Some(string("u", Allocator::System).unwrap());
                for prefix in ["p", "é"] {
                    try_insert(
                        &mut parser.namespaces,
                        string(prefix, Allocator::System).unwrap(),
                        string("u", Allocator::System).unwrap(),
                    )
                    .unwrap();
                }
                for name in &names {
                    // The optimized predicate intentionally requires the planner's
                    // existing Name proof; invalid Names stay on the general path.
                    if !name_rules.is_name(name) {
                        continue;
                    }
                    for token_bytes in [name.len() + 3, 4093, 4094, 4095, 4096, usize::MAX] {
                        // Retain the original full-QName eligibility as the oracle.
                        let expected = separator.is_some_and(|separator| {
                            if !name_rules.is_qname(name) {
                                return false;
                            }
                            let uri = match name.split_once(':') {
                                Some(("xmlns", _)) => return false,
                                Some((prefix, _)) => parser.namespaces.get(prefix),
                                None => parser.default_namespace.as_ref(),
                            };
                            let Some(uri) = uri else {
                                return false;
                            };
                            let separators = if separator == '\0' {
                                0
                            } else {
                                2 * separator.len_utf8()
                            };
                            token_bytes
                                .checked_add(uri.len())
                                .and_then(|bytes| bytes.checked_add(separators))
                                .is_some_and(|bytes| bytes <= arena::MAX_ARENA_BYTES)
                        });
                        assert_eq!(
                            parser.expanded_names_fit_frame(name, "", token_bytes),
                            expected,
                            "{name:?} {name_rules:?} {separator:?} {token_bytes}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn large_namespace_scope_metadata_is_released_while_root_stays_open() {
        let mut parser = Parser::new(Config {
            namespace_separator: Some('|'),
            ..Config::default()
        });
        parser.feed(b"<r>", false).unwrap();
        while parser.next_event().unwrap().is_some() {}
        assert_eq!(parser.namespace_scopes.capacity(), 0);

        // Exceed the retention allowance without closing the ordinary root.
        for _ in 0..128 {
            parser.feed(b"<n xmlns:p='u'>", false).unwrap();
            while parser.next_event().unwrap().is_some() {}
        }
        assert!(parser.namespace_scopes.capacity() > 4096 / size_of::<NamespaceBindings>());
        for _ in 0..128 {
            parser.feed(b"</n>", false).unwrap();
            while parser.next_event().unwrap().is_some() {}
        }
        assert_eq!(parser.stack.len(), 1);
        assert!(!parser.closed_root);
        assert!(!parser.namespaces.contains_key("p"));
        assert_eq!(parser.namespace_scopes.capacity(), 0);

        // A later small scope remains reusable, with all binding storage gone.
        parser.feed(b"<e xmlns:p='v'/>", false).unwrap();
        while parser.next_event().unwrap().is_some() {}
        assert!(parser.namespace_scopes.is_empty());
        assert!(parser.namespace_scopes.capacity() > 0);
        assert!(parser.namespace_scopes.capacity() <= 4096 / size_of::<NamespaceBindings>());
        assert!(!parser.namespaces.contains_key("p"));
        parser.feed(b"</r>", true).unwrap();
        while parser.next_event().unwrap().is_some() {}
        assert!(parser.is_finished());
    }
}

#[cfg(test)]
mod hash_salt_tests {
    use super::*;
    use std::hash::BuildHasher;

    #[test]
    fn salt_rebuilds_every_table_and_owned_children() {
        let mut parser = Parser::new(Config {
            namespace_separator: Some('|'),
            ..Config::default()
        });
        parser.set_param_entity_parsing(2);
        parser.feed(b"<!DOCTYPE r [<!ENTITY e 'ok'><!ENTITY % p \"<!ATTLIST n b CDATA 'v'>\">%p;<!ATTLIST r a CDATA 'v'>]><r xmlns='urn:default'>", false).unwrap();
        while parser.next_event().unwrap().is_some() {}
        let previous = parser.namespaces.hasher().hash_one("xml");
        let default_owner = parser.default_namespace.as_ref().unwrap().as_ptr();
        parser.set_hash_salt(*b"0123456789abcdef").unwrap();
        assert_ne!(parser.namespaces.hasher().hash_one("xml"), previous);
        assert_eq!(parser.default_namespace.as_deref(), Some("urn:default"));
        assert_eq!(
            parser.default_namespace.as_ref().unwrap().as_ptr(),
            default_owner
        );
        assert!(!parser.namespaces.contains_key(""));
        parser
            .with_dtd_tables(|parser| {
                assert_eq!(parser.tables.entities.hasher().salt(), parser.hash_salt());
                assert_eq!(
                    parser.tables.parameter_entities.hasher().salt(),
                    parser.hash_salt()
                );
                assert_eq!(parser.tables.defaults.hasher().salt(), parser.hash_salt());
                assert_eq!(
                    parser.namespaces.get("xml").unwrap(),
                    "http://www.w3.org/XML/1998/namespace"
                );
                assert!(parser.tables.entities.contains_key("e"));
                assert!(parser.tables.parameter_entities.contains_key("p"));
                for attributes in parser.tables.defaults.values() {
                    assert_eq!(attributes.by_name.hasher().salt(), parser.hash_salt());
                    for attribute in &attributes.ordered {
                        assert!(attributes.get(&attribute.name).is_some());
                    }
                }
                Ok(())
            })
            .unwrap();
        for (context, uri) in [
            ("", Some("urn:default")),
            ("=urn:child", Some("urn:child")),
            ("=", None),
        ] {
            let mut child = parser.external_child(Some(context), None).unwrap();
            assert_eq!(child.hash_salt(), parser.hash_salt());
            assert_eq!(child.default_namespace.as_deref(), uri);
            assert!(!child.namespaces.contains_key(""));
            let mut parameter = child.external_child(None, None).unwrap();
            assert_eq!(parameter.default_namespace.as_deref(), uri);
            parameter.feed(b"", true).unwrap();
            while parameter.next_event().unwrap().is_some() {}
            child.feed(b"<n>&e;</n>", true).unwrap();
            let mut start = false;
            while let Some(event) = child.next_event().unwrap() {
                if let EventKind::StartElement { name, attributes } = event.kind {
                    assert_eq!(
                        name,
                        match uri {
                            Some("urn:default") => "urn:default|n",
                            Some(_) => "urn:child|n",
                            None => "n",
                        }
                    );
                    assert_eq!(attributes[0].name, "b");
                    start = true;
                }
            }
            assert!(start);
            assert_eq!(child.default_namespace.as_deref(), uri);
        }
        assert_eq!(
            parser.default_namespace.as_ref().unwrap().as_ptr(),
            default_owner
        );
        parser.feed(b"<n>&e;</n></r>", true).unwrap();
        while parser.next_event().unwrap().is_some() {}
        assert!(parser.default_namespace.is_none());
    }

    #[test]
    fn nonnamespace_parsers_keep_salted_empty_bindings() {
        let mut parser = Parser::new(Config::default());
        assert!(parser.namespaces.is_empty());
        let previous = parser.namespaces.hasher().hash_one("xml");
        parser.set_hash_salt(*b"0123456789abcdef").unwrap();
        assert_ne!(parser.namespaces.hasher().hash_one("xml"), previous);
        for context in [None, Some("")] {
            let child = parser.external_child(context, None).unwrap();
            assert!(child.namespaces.is_empty());
            assert_eq!(child.hash_salt(), parser.hash_salt());
        }
        let mut child = parser
            .external_child(Some("=urn:hidden\u{c}xml=urn:custom"), None)
            .unwrap();
        assert_eq!(child.namespaces.get("xml").unwrap(), "urn:custom");
        assert_eq!(child.default_namespace.as_deref(), Some("urn:hidden"));
        assert!(!child.namespaces.contains_key(""));
        let inherited = child.external_child(Some(""), None).unwrap();
        assert_eq!(inherited.default_namespace.as_deref(), Some("urn:hidden"));
        let removed = child.external_child(Some("="), None).unwrap();
        assert!(removed.default_namespace.is_none());
        child.feed(b"<xml:r/>", true).unwrap();
        while let Some(event) = child.next_event().unwrap() {
            if let EventKind::StartElement { name, .. } = event.kind {
                assert_eq!(name, "xml:r");
            }
        }
    }
}

#[cfg(test)]
mod parameter_state_tests {
    use super::*;

    fn drain(parser: &mut Parser) -> Vec<Event> {
        let mut events = Vec::new_in(Allocator::System);
        while let Some(event) = parser.next_event().unwrap() {
            events.push(event);
        }
        events
    }

    #[test]
    fn ordinary_parsing_and_general_children_leave_parameter_state_unallocated() {
        let mut parser = Parser::new(Config::default());
        parser
            .feed(b"<!DOCTYPE r [<!ENTITY e 'v'>]><r>&e;</r>", true)
            .unwrap();
        drain(&mut parser);
        assert!(parser.parameter_state.get().is_none());
        let child = parser.external_child(Some(""), None).unwrap();
        assert!(parser.parameter_state.get().is_none());
        assert!(child.parameter_state.get().is_none());
    }

    #[test]
    fn precreated_parameter_children_share_skip_state_but_general_children_do_not() {
        let mut parent = Parser::new(Config::default());
        parent.set_param_entity_parsing(2);
        let general = parent.external_child(Some(""), None).unwrap();
        let mut first = parent.external_child(None, None).unwrap();
        let mut second = parent.external_child(None, None).unwrap();
        first.feed(b"%missing;", false).unwrap();
        drain(&mut first);
        assert!(parent.declarations_skipped());
        assert!(second.declarations_skipped());
        assert!(!general.declarations_skipped());
        let after_skip = parent.external_child(Some(""), None).unwrap();
        assert!(after_skip.declarations_skipped());
        assert!(after_skip.parameter_state.get().is_none());
        let mut separate = general.external_child(None, None).unwrap();
        separate.feed(b"<!ENTITY present 'P'>", true).unwrap();
        assert!(
            drain(&mut separate)
                .iter()
                .any(|event| matches!(event.kind, EventKind::EntityDeclaration(_)))
        );
        drop(parent);
        drop(first);
        second.feed(b"<!ENTITY ignored 'I'>", true).unwrap();
        assert!(
            !drain(&mut second)
                .iter()
                .any(|event| matches!(event.kind, EventKind::EntityDeclaration(_)))
        );
    }

    #[test]
    fn lazy_parameter_state_work_is_charged_once_and_limit_failure_keeps_it_uninitialized() {
        let mut config = Config::default();
        config.limits.max_entity_expansion_bytes = size_of::<ParameterState>();
        let parser = Parser::new(config.clone());
        parser.shared_parameter_state().unwrap();
        parser.shared_parameter_state().unwrap();
        assert_eq!(
            parser.expanded.expanded.load(Ordering::Relaxed),
            size_of::<ParameterState>()
        );
        config.limits.max_entity_expansion_bytes -= 1;
        let parser = Parser::new(config);
        assert_eq!(
            parser.shared_parameter_state().unwrap_err().kind,
            ErrorKind::LimitExceeded
        );
        assert!(parser.parameter_state.get().is_none());
    }
}

#[cfg(test)]
mod attribute_literal_run_tests {
    use super::*;

    #[test]
    fn ordinary_runs_match_scalar_whitespace_normalization_at_boundaries() {
        let alphabet = ['a', 'é', '😀', ' ', '\t', '\r', '\n'];
        for length in 0..=5 {
            for mut number in 0..7_usize.pow(length) {
                let mut input = std::string::String::new();
                for _ in 0..length {
                    input.push(alphabet[number % alphabet.len()]);
                    number /= alphabet.len();
                }
                for normalize in [false, true] {
                    let mut expected = std::string::String::from("prior\r");
                    let mut previous_cr = false;
                    for character in input.chars() {
                        if character == '\n' && previous_cr {
                            previous_cr = false;
                            continue;
                        }
                        previous_cr = normalize && character == '\r';
                        expected.push(if whitespace(character) {
                            ' '
                        } else {
                            character
                        });
                    }
                    let mut actual = String::try_from_str_in("prior\r", Allocator::System).unwrap();
                    append_lexical_attribute(&mut actual, lexical::Slice::plain(&input), normalize)
                        .unwrap();
                    assert_eq!(
                        actual,
                        expected.as_str(),
                        "normalize={normalize}, input={input:?}"
                    );
                }
            }
        }
        for width in [63, 64, 65, 127, 128, 129, 4095, 4096, 4097] {
            let prefix = "é".repeat(width);
            let input = format!("{prefix}\r\n😀\tend\r");
            let mut actual = String::new_in(Allocator::System);
            append_lexical_attribute(&mut actual, lexical::Slice::plain(&input), true).unwrap();
            assert_eq!(actual, format!("{prefix} 😀 end ").as_str());
        }
    }
}

#[cfg(test)]
mod coalesced_text_search_tests {
    use super::coalesced_text_end;

    // The original complete-line selector, including its root/internal CR rule.
    fn scalar_end(text: &str, internal: bool) -> usize {
        let coalesce = true;
        let mut boundary = 0;
        text.bytes()
            .enumerate()
            .find_map(|(index, byte)| {
                if matches!(byte, b'<' | b'&') {
                    return Some(index);
                }
                if byte == b'\n' || (!internal && byte == b'\r') {
                    if !coalesce || index >= 65_536 {
                        return Some(if boundary > 0 { boundary } else { index });
                    }
                    boundary = index
                        + if byte == b'\r' && text.as_bytes().get(index + 1) == Some(&b'\n') {
                            2
                        } else {
                            1
                        };
                }
                // Bound merging across lines, preserving the existing span of
                // an individual long line and its malformed-input prefix.
                (index >= 65_536 && boundary > 0).then_some(boundary)
            })
            .map_or(text.len(), |index| {
                if index == 0 && matches!(text.as_bytes()[0], b'\r' | b'\n') {
                    if text.starts_with("\r\n") { 2 } else { 1 }
                } else {
                    index
                }
            })
    }

    fn compare(text: &str) {
        for internal in [false, true] {
            let expected = scalar_end(text, internal);
            let actual = coalesced_text_end(text).unwrap_or_else(|| scalar_end(text, internal));
            assert_eq!(
                actual,
                expected,
                "internal={internal}, text length={}",
                text.len()
            );
            assert!(text.is_char_boundary(actual));
        }
    }

    #[test]
    fn bounded_search_matches_scalar_selector_for_all_short_sequences() {
        let alphabet = ['<', '&', '\r', '\n', 'x', ']', 'é', '\0'];
        for length in 0..=6 {
            for mut number in 0..alphabet.len().pow(length) {
                let mut text = std::string::String::new();
                for _ in 0..length {
                    text.push(alphabet[number % alphabet.len()]);
                    number /= alphabet.len();
                }
                compare(&text);
            }
        }
    }

    #[test]
    fn markup_and_crlf_keep_precedence_at_the_complete_line_cutoff() {
        for length in [65_535, 65_536, 65_537, 65_538, 131_075] {
            for first in [0, 1, 65_534, 65_535, 65_536, 65_537] {
                for second in [0, 1, 65_534, 65_535, 65_536, 65_537] {
                    if first >= length || second >= length {
                        continue;
                    }
                    for left in *b"<&\r\n" {
                        for right in *b"<&\r\n" {
                            let mut text = vec![b'x'; length];
                            text[first] = left;
                            text[second] = right;
                            compare(std::str::from_utf8(&text).unwrap());
                        }
                    }
                }
            }
        }
        for prefix in ["é".repeat(32_768), "aé".repeat(21_845), "\r".repeat(65_537)] {
            for tail in ["", "<", "&", "é<", "\r\n<", "]]>\0"] {
                compare(&format!("{prefix}{tail}"));
            }
        }
        let crossing = format!("{}\r\n<", "x".repeat(65_535));
        assert_eq!(coalesced_text_end(&crossing), None);
        assert_eq!(scalar_end(&crossing, false), 65_537);
        assert_eq!(scalar_end(&crossing, true), 65_536);
        let cutoff = format!("\n{}<", "x".repeat(65_535));
        assert_eq!(coalesced_text_end(&cutoff), Some(65_536));
    }
}

#[cfg(test)]
mod native_raw_context_tests {
    use super::*;

    fn text_parser(input: &[u8], final_input: bool) -> (Parser, AdapterFrame) {
        let mut parser = Parser::new(Config::default());
        parser.enable_input_context();
        parser.feed(input, final_input).unwrap();
        parser.next_event().unwrap().unwrap();
        let mut frame = parser.adapter_frame();
        let mut event = None;
        parser
            .next_event_for_c_text_context_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        assert!(parser.native_raw.is_some());
        (parser, frame)
    }

    #[test]
    fn direct_start_raw_views_keep_copied_fallbacks_and_owner_handoffs() {
        for (prefix, tag, namespace, enabled, c_mode, foreign, direct) in [
            ("<r><w a='v'/>", "<n a='x'>", false, true, true, false, true),
            ("<r><w a='v'/>", "<n a='x'>", true, true, true, false, true),
            (
                "<r xmlns='urn'><w a='v'/>",
                "<n a='x'>",
                true,
                true,
                true,
                false,
                false,
            ),
            (
                "<r><w a='v'/>",
                "<n a='x'/>",
                false,
                true,
                true,
                false,
                false,
            ),
            (
                "<r><w a='v'/>",
                "<n a='&amp;'>",
                false,
                true,
                true,
                false,
                false,
            ),
            (
                "<r><w a='v'/>",
                "<n a='x'>",
                false,
                false,
                true,
                false,
                false,
            ),
            (
                "<r><w a='v'/>",
                "<n a='x'>",
                false,
                true,
                false,
                false,
                false,
            ),
            ("<r><w a='v'/>", "<n a='x'>", false, true, true, true, false),
        ] {
            let mut parser = Parser::new(Config {
                namespace_separator: namespace.then_some('|'),
                ..Config::default()
            });
            if enabled {
                parser.enable_input_context();
            }
            parser.feed(prefix.as_bytes(), false).unwrap();
            while parser.next_event().unwrap().is_some() {}
            parser.feed(tag.as_bytes(), false).unwrap();
            let old_raw = parser.current_raw.as_str().to_owned();
            let raw_pointer = parser.current_raw.as_ptr();
            let raw_capacity = parser.current_raw.capacity();
            let scratch = parser.token_scratch.as_bytes().as_ptr();
            let scratch_len = parser.token_scratch.len();
            let mut other = Parser::new(Config::default());
            let mut frame = if foreign {
                other.adapter_frame()
            } else {
                parser.adapter_frame()
            };
            let mut event = None;
            if c_mode {
                parser.next_event_for_c_text_context_into(&mut event, &mut frame)
            } else {
                parser.next_event_for_adapter_into(&mut event, &mut frame)
            }
            .unwrap()
            .unwrap();
            assert_eq!(parser.current_raw(), Some(tag));
            assert_eq!(parser.native_raw.is_some(), direct);
            if direct {
                assert_eq!(parser.current_raw.as_str(), old_raw);
                assert_eq!(parser.current_raw.as_ptr(), raw_pointer);
                assert_eq!(parser.current_raw.capacity(), raw_capacity);
                assert_eq!(parser.token_scratch.as_bytes().as_ptr(), scratch);
                assert_eq!(parser.token_scratch.len(), scratch_len);
                assert_eq!(frame.name_bytes(), b"n\0");
                assert_eq!(
                    frame.attributes().collect::<std::vec::Vec<_>>(),
                    [(b"a\0".as_slice(), b"x\0".as_slice())]
                );
                let raw = parser.native_raw.unwrap();
                let (context, start) = parser.input_context();
                assert_eq!(
                    parser.current_raw().unwrap().as_ptr(),
                    context[raw.start - start..].as_ptr()
                );
                parser.feed(b"</n></r>", true).unwrap();
                assert_eq!(parser.current_raw(), Some(tag));
                while parser.next_event().unwrap().is_some() {
                    assert!(parser.native_raw.is_none());
                }
            }
            if foreign {
                other.finish_adapter_frame(frame);
            } else {
                parser.finish_adapter_frame(frame);
            }
        }
        for bytes in [arena::MAX_ARENA_BYTES, arena::MAX_ARENA_BYTES + 1] {
            let tag = format!("<n{}>", " ".repeat(bytes - 3));
            let mut parser = Parser::new(Config::default());
            parser.enable_input_context();
            parser.feed(tag.as_bytes(), false).unwrap();
            let mut frame = parser.adapter_frame();
            let mut event = None;
            parser
                .next_event_for_c_text_context_into(&mut event, &mut frame)
                .unwrap()
                .unwrap();
            assert_eq!(parser.native_raw.is_some(), bytes <= arena::MAX_ARENA_BYTES);
            assert_eq!(parser.current_raw(), Some(tag.as_str()));
            parser.finish_adapter_frame(frame);
        }
        for count in [arena::MAX_ARENA_ATTRIBUTES, arena::MAX_ARENA_ATTRIBUTES + 1] {
            let attributes = (0..count)
                .map(|i| format!(" a{i}='v'"))
                .collect::<std::string::String>();
            let mut parser = Parser::new(Config::default());
            parser.enable_input_context();
            parser
                .feed(format!("<r><w{attributes}/>").as_bytes(), false)
                .unwrap();
            while parser.next_event().unwrap().is_some() {}
            let tag = format!("<n{attributes}>");
            parser.feed(tag.as_bytes(), false).unwrap();
            let mut frame = parser.adapter_frame();
            let mut event = None;
            parser
                .next_event_for_c_text_context_into(&mut event, &mut frame)
                .unwrap()
                .unwrap();
            assert_eq!(
                parser.native_raw.is_some(),
                count <= arena::MAX_ARENA_ATTRIBUTES
            );
            assert_eq!(parser.current_raw(), Some(tag.as_str()));
            parser.finish_adapter_frame(frame);
        }
    }

    #[test]
    fn native_raw_getters_share_context_and_allow_ordinary_handoffs() {
        for count in [1, 23, 24, 4096, 4097] {
            let text = format!("{}{}", "é".repeat(count / 2), "x".repeat(count % 2));
            let input = format!("\u{feff}<r>{text}");
            for enabled in [false, true] {
                let mut parser = Parser::new(Config::default());
                if enabled {
                    parser.enable_input_context();
                }
                parser.feed(input.as_bytes(), false).unwrap();
                parser.next_event().unwrap().unwrap();
                let capacity = parser.current_raw.capacity();
                let mut frame = parser.adapter_frame();
                let mut event = None;
                parser
                    .next_event_for_c_text_context_into(&mut event, &mut frame)
                    .unwrap()
                    .unwrap();
                assert_eq!(parser.current_raw(), Some(text.as_str()));
                assert_eq!(parser.native_raw.is_some(), enabled && count <= 4096);
                if let Some(raw) = parser.native_raw {
                    // These assertions distinguish a real raw view from eager
                    // copying even when the owned String already had capacity.
                    assert_eq!(parser.current_raw.as_str(), "<r>");
                    assert_eq!(parser.current_raw.capacity(), capacity);
                    let (context, start) = parser.input_context();
                    assert_eq!(
                        parser.current_raw().unwrap().as_ptr(),
                        context[raw.start - start..].as_ptr()
                    );
                    assert_eq!(frame.native_text_range_for_c(), Some((6, count)));
                }
                parser.finish_adapter_frame(frame);
                parser.feed(b"", false).unwrap();
                assert_eq!(parser.current_raw(), Some(text.as_str()));
                parser.feed(b"<n/>tail</r>", true).unwrap();
                assert_eq!(parser.current_raw(), Some(text.as_str()));
                while parser.next_event().unwrap().is_some() {
                    assert!(parser.native_raw.is_none());
                }
            }
        }
    }

    #[test]
    fn matched_native_end_raw_keeps_owned_names_and_mode_boundaries() {
        for (opening, closing, separator, scoped) in [
            ("<é>", "</é>", None, false),
            ("<é>", "</é >", None, false),
            ("<é>", "</é>", Some('|'), false),
            ("<é xmlns='urn'>", "</é>", Some('|'), true),
        ] {
            for enabled in [false, true] {
                // Generic, C-context, and a foreign frame's owned fallback.
                for mode in 0..3 {
                    let mut parser = Parser::new(Config {
                        namespace_separator: separator,
                        ..Config::default()
                    });
                    if enabled {
                        parser.enable_input_context();
                    }
                    let input = format!("\u{feff}<r>{opening}\r{closing}\n<z/></r>");
                    parser.feed(input.as_bytes(), true).unwrap();
                    while let Some(event) = parser.next_event().unwrap() {
                        if matches!(event.kind, EventKind::Text(_)) {
                            break;
                        }
                    }
                    assert_eq!(parser.current_raw(), Some("\r"));
                    let raw_owner = (parser.current_raw.as_ptr(), parser.current_raw.capacity());
                    let scratch = (parser.token_scratch.as_ptr(), parser.token_scratch.len());
                    let name_owner = parser.stack.last().unwrap().name.value.as_ptr();
                    let mut foreign = Parser::new(Config::default());
                    let mut frame = if mode == 2 {
                        foreign.adapter_frame()
                    } else {
                        parser.adapter_frame()
                    };
                    let mut event = None;
                    if mode == 0 {
                        parser.next_event_for_adapter_into(&mut event, &mut frame)
                    } else {
                        parser.next_event_for_c_text_context_into(&mut event, &mut frame)
                    }
                    .unwrap()
                    .unwrap();
                    let (name, position) = if let Some(name) = frame.take_end_name() {
                        assert!(event.is_none());
                        (name, frame.position())
                    } else {
                        let event = event.take().unwrap();
                        let EventKind::EndElement { name } = event.kind else {
                            panic!("closing token must deliver End first");
                        };
                        (name, event.position)
                    };
                    assert_eq!(name, if scoped { "urn|é" } else { "é" });
                    assert_eq!(name.as_ptr(), name_owner);
                    assert_eq!(
                        (position.byte_index, position.byte_count),
                        (input.find(closing).unwrap(), closing.len())
                    );
                    assert_eq!((position.line, position.column), (2, 0));
                    let consumed = parser.source().position(0);
                    assert_eq!(
                        (consumed.line, consumed.column),
                        (2, closing.chars().count())
                    );
                    let viewed = enabled && mode == 1 && !scoped && closing == "</é>";
                    assert_eq!(parser.native_raw.is_some(), viewed);
                    assert_eq!(parser.current_raw(), Some(closing));
                    if viewed {
                        assert_eq!(parser.current_raw.as_str(), "\r");
                        assert_eq!(
                            (parser.current_raw.as_ptr(), parser.current_raw.capacity()),
                            raw_owner
                        );
                        assert_eq!(
                            (parser.token_scratch.as_ptr(), parser.token_scratch.len()),
                            scratch
                        );
                        let (context, start) = parser.input_context();
                        assert_eq!(
                            parser.current_raw().unwrap().as_ptr(),
                            context[position.byte_index - start..].as_ptr()
                        );
                    }
                    // Ordinary handoff drains namespace undo and the empty tag,
                    // then replaces the view with the final owned root End.
                    while parser.next_event().unwrap().is_some() {}
                    assert!(parser.native_raw.is_none());
                    assert_eq!(parser.current_raw(), Some("</r>"));
                    assert_eq!(parser.source().position(0).line, 3);
                    if mode == 2 {
                        foreign.finish_adapter_frame(frame);
                    } else {
                        parser.finish_adapter_frame(frame);
                    }
                }
            }
        }
    }

    #[test]
    fn native_raw_retention_survives_compaction_and_invalid_context_suffixes() {
        let text = format!("{}x", "€".repeat(1333));
        let end_name = "é".repeat(2050);
        let end_tag = format!("</{end_name}>");
        let mut unaligned = false;
        for case in 0..6 {
            let padding = case % 3;
            let tail = if case < 3 {
                text.clone()
            } else {
                format!("<{end_name}>{end_tag}")
            };
            let raw_text = if case < 3 {
                text.as_str()
            } else {
                end_tag.as_str()
            };
            let input = format!("<r>{}{}<n/>{tail}", "€".repeat(23_000), "x".repeat(padding));
            for suffix in [b"\xff".as_slice(), b"\xc3"] {
                let mut parser = Parser::new(Config::default());
                parser.enable_input_context();
                parser.feed(input.as_bytes(), false).unwrap();
                let mut frame = parser.adapter_frame();
                let mut event = None;
                while parser
                    .next_event_for_c_text_context_into(&mut event, &mut frame)
                    .unwrap()
                    .is_some()
                {}
                let raw = parser.native_raw.unwrap();
                assert_eq!(parser.current_raw(), Some(raw_text));
                // A completed frame can be cleared without discarding raw.
                assert!(!frame.is_active());
                parser.feed(b"", false).unwrap();
                let (context, start) = parser.input_context();
                assert_eq!(start, raw.start - 1024);
                assert_eq!(context, &input.as_bytes()[start..]);
                let physical_extra = parser.sources[0].text.len() - context.len();
                assert!(physical_extra <= 3);
                unaligned |= physical_extra != 0;
                parser.feed(suffix, false).unwrap();
                assert!(std::str::from_utf8(parser.input_context().0).is_err());
                assert_eq!(parser.current_raw(), Some(raw_text));
                assert_ne!(
                    parser.input_context().0.as_ptr(),
                    parser.sources[0].text.as_bytes().as_ptr()
                );
                if suffix == b"\xc3" {
                    parser.feed(b"\xa9</r>", true).unwrap();
                    assert_eq!(parser.current_raw(), Some(raw_text));
                    parser
                        .next_event_for_c_text_context_into(&mut event, &mut frame)
                        .unwrap()
                        .unwrap();
                    assert_eq!(parser.current_raw(), Some("é"));
                    assert!(parser.native_raw.is_some());
                    while parser.next_event().unwrap().is_some() {}
                } else {
                    assert!(parser.next_event().is_err());
                    assert_eq!(parser.current_raw(), Some(raw_text));
                }
                parser.finish_adapter_frame(frame);
            }
        }
        assert!(unaligned);
    }

    #[test]
    fn native_raw_overrides_keep_owned_empty_and_failure_publication_order() {
        for replacement in [None, Some(""), Some("owned")] {
            let (mut parser, frame) = text_parser(b"<r>text", false);
            let error = parser.save_current_raw(usize::MAX).unwrap_err();
            assert_eq!(error.kind, ErrorKind::NoMemory);
            assert_eq!(parser.current_raw(), Some("text"));
            parser.emit(EventKind::Default, parser.position()).unwrap();
            if let Some(raw) = replacement {
                parser.event_raw(raw).unwrap();
            }
            assert_eq!(parser.current_raw(), Some("text"));
            parser.pop_event().unwrap();
            assert_eq!(
                parser.current_raw(),
                match replacement {
                    None => Some("text"),
                    Some("") => None,
                    Some(raw) => Some(raw),
                }
            );
            assert_eq!(parser.native_raw.is_some(), replacement.is_none());
            parser.finish_adapter_frame(frame);
        }
        let (mut parser, mut frame) = text_parser(b"<r>text&amp;after</wrong>", true);
        let mut event = None;
        parser
            .next_event_for_c_text_context_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        assert_eq!(parser.current_raw(), Some("&amp;"));
        assert!(parser.native_raw.is_none());
        parser
            .next_event_for_c_text_context_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        assert_eq!(parser.current_raw(), Some("after"));
        assert!(parser.native_raw.is_some());
        assert_eq!(
            parser.next_event().unwrap_err().kind,
            ErrorKind::TagMismatch
        );
        assert_eq!(parser.current_raw(), Some("</wrong>"));
        assert!(parser.native_raw.is_none());
        parser.finish_adapter_frame(frame);

        let (mut parser, frame) = text_parser(b"<r>text", false);
        // Model the delivered writer state without popping its empty-raw
        // event: popping would itself replace the retained raw view.
        parser.parameter_mode = 2;
        parser.start_foreign_dtd(Position::default()).unwrap();
        parser.foreign_dtd_pending.as_mut().unwrap().delivered = true;
        parser
            .shared_parameter_state()
            .unwrap()
            .read
            .store(true, Ordering::Relaxed);
        assert!(parser.native_raw.is_some());
        assert_eq!(parser.current_raw(), Some("text"));
        assert!(matches!(
            parser.finish_foreign_dtd().unwrap().kind,
            EventKind::NotStandalone
        ));
        assert!(parser.native_raw.is_none());
        assert_eq!(parser.current_raw(), None);
        parser.finish_adapter_frame(frame);
    }

    #[test]
    fn native_raw_prefix_and_early_feed_errors_remain_readable() {
        let mut parser = Parser::new(Config::default());
        parser.enable_input_context();
        parser.feed(b"<r>abc</r>", true).unwrap();
        parser.next_event().unwrap().unwrap();
        assert!(parser.expanded.account(100, true, false));
        assert!(parser.set_entity_maximum_amplification(1.0));
        parser.set_entity_activation_threshold(0);
        let mut frame = parser.adapter_frame();
        let mut event = None;
        parser
            .next_event_for_c_text_context_into(&mut event, &mut frame)
            .unwrap()
            .unwrap();
        assert!(parser.native_raw.is_some() && frame.is_text());
        assert_eq!(parser.current_raw(), Some("abc"));
        let error = parser
            .next_event_for_c_text_context_into(&mut event, &mut frame)
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::LimitExceeded);
        assert!(!frame.is_active());
        assert_eq!(parser.feed(b"ignored", false).unwrap_err(), error);
        assert_eq!(parser.current_raw(), Some("abc"));
        parser.finish_adapter_frame(frame);

        for final_input in [false, true] {
            let (mut parser, frame) = text_parser(b"<r>text", final_input);
            parser.config.limits.max_total_bytes = parser.received;
            let expected = if final_input {
                ErrorKind::Finished
            } else {
                ErrorKind::LimitExceeded
            };
            assert_eq!(parser.feed(b"x", false).unwrap_err().kind, expected);
            assert_eq!(parser.current_raw(), Some("text"));
            parser.finish_adapter_frame(frame);
        }
    }
}
