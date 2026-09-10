//! A safe, incremental XML 1.0 parser with explicit resource limits.
//!
//! Feed bytes with [`Parser::feed`], then drain owned events with
//! [`Parser::next_event`]. No borrowed parser state crosses an event boundary.
#![forbid(unsafe_code)]

mod accounting;
mod dtd;
mod encoding;
mod lexical;
mod names;
mod recycling;
mod value;
mod value_lexer;

use oriole_storage::{
    AllocError, Allocator, HashMap, HashSet, Queue, Shared, String, TryClone, Vec, hash_map,
    try_insert, try_push, try_set_insert,
};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use accounting::EntityBudget;

pub use oriole_storage::Text;
pub use recycling::RecyclingToken;

use recycling::{AttributeRecycling, copy_attribute_string};

use encoding::{Decoder, Source};
pub use names::NameRules;
use names::{is_uri_char, is_xml_char, whitespace};

/// Bounds applied independently of input chunking.
#[derive(Clone, Debug)]
pub struct Limits {
    pub max_depth: usize,
    pub max_token_bytes: usize,
    pub max_total_bytes: usize,
    /// Total indirect bytes from entities, reused defaults, namespace URI expansion,
    /// repeated declaration callback names, skipped conditional-reference callback
    /// storage, and external reference identifiers and namespace contexts. Child construction also charges inherited declaration,
    /// namespace, encoding, and context storage, including per-entry structural
    /// work. Shared with external entity children.
    pub max_entity_expansion_bytes: usize,
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
    ExternalEntityReference(oriole_storage::Box<ExternalEntityReference>),
    StartDoctype(oriole_storage::Box<DoctypeDeclaration>),
    EndDoctype,
    NotStandalone,
    StartNamespace {
        prefix: Option<String>,
        uri: Option<String>,
    },
    EndNamespace {
        prefix: Option<String>,
    },
    EntityDeclaration(oriole_storage::Box<EntityDeclaration>),
    AttlistDeclaration(oriole_storage::Box<AttributeDeclaration>),
    NotationDeclaration(oriole_storage::Box<NotationDeclaration>),
    ElementDeclaration {
        name: String,
        model: String,
    },
    SkippedEntity {
        name: String,
        parameter: bool,
    },
}

#[derive(Debug)]
struct Element {
    raw_name: String,
    raw_encoding: Option<oriole_storage::Box<Vec<lexical::NameEncoding>>>,
    expanded_name: Option<String>,
    bindings: Vec<(String, Option<String>)>,
}

#[derive(Debug)]
struct Entity {
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
}

impl DefaultAttributes {
    fn new(allocator: Allocator, salt: [u8; 16]) -> Self {
        let map: HashMap<String, usize> = hash_map(allocator);
        Self {
            ordered: Vec::new_in(allocator),
            by_name: HashMap::with_hasher_in(map.hasher().with_salt(salt), allocator),
        }
    }

    fn get(&self, name: &str) -> Option<&DefaultAttribute> {
        self.by_name.get(name).map(|&index| &self.ordered[index])
    }

    fn try_insert(&mut self, attribute: DefaultAttribute) -> Result<(), AllocError> {
        debug_assert!(self.get(&attribute.name).is_none());
        let name = attribute.name.try_clone()?;
        // Reserve both containers before changing their logical contents so an
        // allocation failure cannot leave an index without its declaration.
        self.ordered
            .try_reserve(1)
            .map_err(|_| AllocError::OutOfMemory)?;
        self.by_name
            .try_reserve(1)
            .map_err(|_| AllocError::OutOfMemory)?;
        let index = self.ordered.len();
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

impl Entity {
    fn is_value_open(&self) -> bool {
        self.value_open
            .as_ref()
            .is_some_and(|open| open.load(Ordering::Relaxed))
    }

    /// General-content children copy Expat's DTD table. A reserved slot without
    /// a system ID becomes an empty internal value in that copy; parameter
    /// children retain the unfinished slot and its already parsed identifiers.
    fn clone_for_child(
        &self,
        parameter_context: bool,
        parameter_entity: bool,
        allocator: Allocator,
    ) -> Result<Self, AllocError> {
        if !parameter_context && self.value.is_none() && self.system_id.is_none() {
            Ok(Self {
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
            if !parameter_context && self.value_open.is_some() {
                entity.value_open = Some(Shared::try_new_in(AtomicBool::new(false), allocator)?);
            }
            Ok(entity)
        }
    }
}

impl TryClone for Entity {
    fn try_clone(&self) -> Result<Self, AllocError> {
        Ok(Self {
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

/// An incremental, non-validating XML 1.0 parser.
///
/// External entity references produce events for application-controlled resolution;
/// the parser never performs I/O. Parameter entity processing is opt-in and supports
/// references between declarations and nested INCLUDE/IGNORE sections in external
/// DTDs. Internal parameter entities may select a conditional keyword or expand
/// inside entity values in external DTDs and parameter entities, and supply
/// complete lexical tokens and grammar delimiters inside declarations. Names,
/// quoted literals, and references retain their original lexical boundaries;
/// references between declarations must contain complete declarations.
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
    sources: Vec<Source>,
    pending: Queue<PendingEvent>,
    stack: Vec<Element>,
    namespaces: HashMap<String, String>,
    entities: HashMap<String, Entity>,
    parameter_entities: HashMap<String, Entity>,
    parameter_mode: u8,
    foreign_dtd: bool,
    foreign_dtd_pending: Option<dtd::ForeignDtd>,
    in_doctype: bool,
    conditional: dtd::ConditionalState,
    value_state: Option<oriole_storage::Box<value::State>>,
    declarations_skipped: bool,
    doctype_external: Option<(Option<String>, Option<String>)>,
    defaults: HashMap<String, DefaultAttributes>,
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
    expanded: Shared<EntityBudget>,
    fragment: bool,
    external_subset: bool,
    external_depth: usize,
    entity_chain: Vec<String>,
    parameter_read: Option<Shared<AtomicBool>>,
    parameter_encoding_initialized: bool,
    active_parameter_reference: Option<(String, Position)>,
    inherited_parameter_depth: usize,
    has_external_subset: bool,
    standalone: bool,
    reparse_deferral: bool,
    last_position: Position,
    current_raw: String,
    token_scratch: lexical::Buffer,
    raw_attributes: Vec<RawAttribute>,
    attribute_recycling: AttributeRecycling,
    expand_internal_entities: bool,
    default_events: bool,
    notation_handler_enabled: bool,
    attlist_handler_enabled: bool,
    decoding_error: Option<(ErrorKind, &'static str)>,
    id_attribute_index: Option<usize>,
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
        let decoder = Decoder::new(encoding, allocator)?;
        let mut namespaces = hash_map(allocator);
        try_insert(
            &mut namespaces,
            string("xml", allocator)?,
            string("http://www.w3.org/XML/1998/namespace", allocator)?,
        )?;
        let mut sources = Vec::new_in(allocator);
        try_push(&mut sources, Source::new(allocator, config.name_rules))?;
        Ok(Self {
            config,
            allocator,
            decoder,
            sources,
            namespaces,
            pending: Queue::new_in(allocator),
            stack: Vec::new_in(allocator),
            entities: hash_map(allocator),
            parameter_entities: hash_map(allocator),
            parameter_mode: 0,
            foreign_dtd: false,
            foreign_dtd_pending: None,
            in_doctype: false,
            conditional: dtd::ConditionalState::new(allocator),
            value_state: None,
            declarations_skipped: false,
            doctype_external: None,
            defaults: hash_map(allocator),
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
            expanded: Shared::try_new_in(EntityBudget::new(), allocator)?,
            fragment: false,
            external_subset: false,
            external_depth: 0,
            entity_chain: Vec::new_in(allocator),
            parameter_read: None,
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
            token_scratch: lexical::Buffer::new_in(allocator),
            raw_attributes: Vec::new_in(allocator),
            attribute_recycling: AttributeRecycling::new(allocator)?,
            expand_internal_entities: true,
            default_events: false,
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

    /// Return the public caller salt, without exposing secret randomized keys.
    #[must_use]
    pub fn hash_salt(&self) -> [u8; 16] {
        self.namespaces.hasher().salt()
    }

    /// Replace the caller salt while retaining randomized hash protection.
    ///
    /// Every table is prepared before changing any table, so allocation failure
    /// preserves all contents and the previous salt. Owned external children
    /// retain their existing hash state; subsequently created children inherit
    /// the new salt. Adapters enforce their own configuration timing rules.
    #[doc(hidden)]
    pub fn set_hash_salt(&mut self, salt: [u8; 16]) -> Result<(), Error> {
        if self.hash_salt() == salt {
            return Ok(());
        }
        let namespaces = prepare_salted_map(&self.namespaces, salt)?;
        let entities = prepare_salted_map(&self.entities, salt)?;
        let parameters = prepare_salted_map(&self.parameter_entities, salt)?;
        let defaults = prepare_salted_map(&self.defaults, salt)?;
        let mut indexes = Vec::new_in(self.allocator);
        indexes
            .try_reserve_exact(self.defaults.len())
            .map_err(AllocError::from)?;
        for attributes in self.defaults.values() {
            indexes.push(prepare_salted_map(&attributes.by_name, salt)?);
        }
        // No growth can fail after preparation. Rebuild inner indexes before
        // changing the outer table's iteration order; drops return old table
        // storage through its original allocator.
        for (attributes, index) in self.defaults.values_mut().zip(indexes) {
            replace_hash_map(&mut attributes.by_name, index);
        }
        replace_hash_map(&mut self.namespaces, namespaces);
        replace_hash_map(&mut self.entities, entities);
        replace_hash_map(&mut self.parameter_entities, parameters);
        replace_hash_map(&mut self.defaults, defaults);
        Ok(())
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
        self.decoder = Decoder::new(encoding, self.allocator)?;
        Ok(())
    }

    /// Update protocol metadata for an adapter that has finished or irreversibly
    /// aborted parsing. This leaves the active decoder, custom map, buffered input,
    /// and positions intact; it does not make the parser accept further input.
    /// The adapter must have finished or irreversibly aborted and must never
    /// resume feeding this parser. The core cannot verify the adapter state.
    #[doc(hidden)]
    pub fn set_completed_encoding(&mut self, encoding: Option<&str>) -> Result<(), Error> {
        self.decoder.set_completed_encoding(encoding)
    }

    /// Construct an independent parser for an application-provided external entity.
    ///
    /// Pass the context from [`EventKind::ExternalEntityReference`]. Namespace
    /// bindings, declarations, recursion tracking, and the expansion budget are
    /// inherited. The parser never opens a path or performs a network request.
    /// A `None` context creates an external DTD parser, or a value parser when
    /// resolving a reference inside an entity value. DTD children import their
    /// declarations through [`Self::merge_external_subset`]; value children share
    /// an owned output channel with the suspended declaration. Every child must
    /// be processed before requesting the parent's next event.
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
        self.charge_external_child_storage(context, encoding)?;
        // The constructor consumes Config.encoding into the allocator-owned decoder.
        // The stored config contains only inline values, so this clone cannot allocate.
        debug_assert!(self.config.encoding.is_none());
        let mut child =
            Self::try_new_with_encoding_in(self.config.clone(), encoding, self.allocator)?;
        child.set_hash_salt(self.hash_salt())?;
        child
            .decoder
            .inherit_map(&self.decoder, &mut child.sources[0])?;
        for (name, entity) in &self.entities {
            try_insert(
                &mut child.entities,
                name.try_clone()?,
                entity.clone_for_child(context.is_none(), false, self.allocator)?,
            )?;
        }
        for (name, attributes) in &self.defaults {
            try_insert(
                &mut child.defaults,
                name.try_clone()?,
                attributes.try_clone()?,
            )?;
        }
        for (name, entity) in &self.parameter_entities {
            try_insert(
                &mut child.parameter_entities,
                name.try_clone()?,
                entity.clone_for_child(context.is_none(), true, self.allocator)?,
            )?;
        }
        child.namespaces.clear();
        for (prefix, uri) in &self.namespaces {
            try_insert(&mut child.namespaces, prefix.try_clone()?, uri.try_clone()?)?;
        }
        child.expanded = self.expanded.clone();
        child.fragment = true;
        child.external_subset = context.is_none();
        child.external_depth = self.external_depth + 1;
        child.inherited_parameter_depth = self.inherited_parameter_depth;
        if context.is_none() {
            child.parameter_read = self.parameter_read.clone();
            child.declarations_skipped = self.declarations_skipped;
        }
        child.expand_internal_entities = self.expand_internal_entities;
        child.default_events = self.default_events;
        child.notation_handler_enabled = self.notation_handler_enabled;
        child.attlist_handler_enabled = self.attlist_handler_enabled;
        child.parameter_mode = self.parameter_mode;
        child.has_external_subset = self.has_external_subset;
        child.standalone = self.standalone;
        child.reparse_deferral = self.reparse_deferral;
        for name in &self.entity_chain {
            try_push(&mut child.entity_chain, name.try_clone()?)?;
        }
        if context.is_none() && !self.inherit_value_context(&mut child)? {
            self.inherit_parameter_context(&mut child)?;
        }
        if let Some(context) = context {
            for part in context.split('\u{c}').filter(|part| !part.is_empty()) {
                if let Some((prefix, uri)) = part.split_once('=') {
                    if uri.is_empty() {
                        child.namespaces.remove(prefix);
                    } else {
                        try_insert(
                            &mut child.namespaces,
                            string(prefix, self.allocator)?,
                            string(uri, self.allocator)?,
                        )?;
                    }
                } else if !child.entity_chain.iter().any(|name| name == part) {
                    if child.entity_chain.len() >= child.config.limits.max_entity_depth {
                        return Err(self.err(
                            ErrorKind::LimitExceeded,
                            "external entity context nesting limit exceeded",
                        ));
                    }
                    try_push(&mut child.entity_chain, string(part, self.allocator)?)?;
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

    /// Bound inherited copies before allocating a child, including work for empty
    /// declarations. Repeated empty external references must not repeatedly clone
    /// an otherwise unused large declaration environment for free.
    fn charge_external_child_storage(
        &self,
        context: Option<&str>,
        encoding: Option<&str>,
    ) -> Result<(), Error> {
        if context.is_some() {
            for entity in self.parameter_entities.values() {
                if entity.value.is_none() && entity.system_id.is_none() {
                    self.charge_expansion(size_of::<AtomicBool>())?;
                }
            }
        }
        for (name, entity) in self.entities.iter().chain(&self.parameter_entities) {
            self.charge_expansion(size_of::<(String, Entity)>())?;
            if context.is_some() && entity.value_open.is_some() {
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
        for (element, attributes) in &self.defaults {
            self.charge_expansion(size_of::<(String, DefaultAttributes)>())?;
            self.charge_expansion(element.len())?;
            for attribute in &attributes.ordered {
                self.charge_expansion(size_of::<DefaultAttribute>())?;
                self.charge_expansion(attribute.name.len())?;
                self.charge_expansion(attribute.attribute_type.len())?;
                if let Some(value) = &attribute.value {
                    self.charge_expansion(value.len())?;
                }
                // The ordered declaration and its lookup index each own a name.
                self.charge_expansion(size_of::<(String, usize)>())?;
                self.charge_expansion(attribute.name.len())?;
            }
        }
        for (prefix, uri) in &self.namespaces {
            self.charge_expansion(size_of::<(String, String)>())?;
            self.charge_expansion(prefix.len())?;
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
    /// Import successfully parsed external DTD declarations. Existing declarations win.
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
        for (name, entity) in &child.entities {
            if !self.entities.contains_key(name) {
                if self.entities.len() + self.parameter_entities.len()
                    >= self.config.limits.max_entities
                {
                    return self.fail(
                        ErrorKind::LimitExceeded,
                        "entity declaration count limit exceeded",
                    );
                }
                try_insert(&mut self.entities, name.try_clone()?, entity.try_clone()?)?;
            }
        }
        for (name, entity) in &child.parameter_entities {
            if !self.parameter_entities.contains_key(name) {
                if self.entities.len() + self.parameter_entities.len()
                    >= self.config.limits.max_entities
                {
                    return self.fail(
                        ErrorKind::LimitExceeded,
                        "entity declaration count limit exceeded",
                    );
                }
                try_insert(
                    &mut self.parameter_entities,
                    name.try_clone()?,
                    entity.try_clone()?,
                )?;
            }
        }
        for (name, attributes) in &child.defaults {
            if !self.defaults.contains_key(name) {
                try_insert(
                    &mut self.defaults,
                    name.try_clone()?,
                    DefaultAttributes::new(self.allocator, self.namespaces.hasher().salt()),
                )?;
            }
            let target = self.defaults.get_mut(name).expect("default list exists");
            for attribute in &attributes.ordered {
                if target.get(&attribute.name).is_none() {
                    if target.ordered.len() >= self.config.limits.max_attributes {
                        return Err(self.err(
                            ErrorKind::LimitExceeded,
                            "default attribute count limit exceeded",
                        ));
                    }
                    target.try_insert(attribute.try_clone()?)?;
                }
            }
        }
        self.declarations_skipped |= child.declarations_skipped;
        self.standalone |= child.standalone;
        Ok(())
    }

    /// Append input without calling user code. Drain events before feeding more data.
    pub fn feed(&mut self, bytes: &[u8], is_final: bool) -> Result<(), Error> {
        if let Some(error) = &self.error {
            return Err(*error);
        }
        if self.final_input {
            return self.fail(ErrorKind::Finished, "input has already been finalized");
        }
        self.feed_start_byte = self.sources[0].position(0).byte_index;
        self.received = match self.received.checked_add(bytes.len()) {
            Some(size) if size <= self.config.limits.max_total_bytes => size,
            _ => return self.fail(ErrorKind::LimitExceeded, "input byte limit exceeded"),
        };
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
        if let Err(error) = self.decoder.feed(
            bytes,
            is_final,
            &mut self.sources[0],
            self.config.limits.max_token_bytes,
            self.fragment && !self.external_subset,
        ) {
            self.decoding_error = Some((error.kind, error.message));
        }
        // Encoding detection may retain a complete BOM while awaiting a text
        // declaration. These bytes already form a token, even on a nonfinal feed.
        let prefix = self
            .decoder
            .pending_bom_len(self.fragment && !self.external_subset);
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

    fn mark_parameter_read(&mut self) {
        if !self.parameter_encoding_initialized && self.decoder.protocol_encoding_ready() {
            self.parameter_encoding_initialized = true;
            if let Some(read) = &self.parameter_read {
                read.store(true, Ordering::Relaxed);
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
        if let Err(error) = self.decoder.feed(
            &[],
            self.final_input,
            &mut self.sources[0],
            self.config.limits.max_token_bytes,
            self.fragment && !self.external_subset,
        ) {
            self.decoding_error = Some((error.kind, error.message));
        }
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
        if let Err(error) = self.decoder.feed(
            &[],
            self.final_input,
            &mut self.sources[0],
            self.config.limits.max_token_bytes,
            self.fragment && !self.external_subset,
        ) {
            self.decoding_error = Some((error.kind, error.message));
        }
        Ok(())
    }

    /// Return an owned event and a token for an adapter that returns its storage.
    ///
    /// Ordinary [`Self::next_event`] consumers keep the existing owned-event API.
    /// The token is allocation-free, cannot be cloned, and identifies this parser
    /// generation without retaining a parser reference across a callback.
    #[doc(hidden)]
    #[inline(always)]
    pub fn next_event_for_recycling(&mut self) -> Result<Option<(Event, RecyclingToken)>, Error> {
        Ok(self
            .next_event()?
            .map(|event| (event, self.attribute_recycling.token())))
    }

    /// Return the original attribute storage after an adapter finishes its callback.
    ///
    /// The token rejects accidental returns to another parser, including a reset
    /// parser at the same address. The adapter must return the original buffers:
    /// substituting foreign storage can violate allocator routing and accounting.
    /// This is a host contract, not a security boundary against malicious Rust
    /// code, which can also change resource limits. Every allocation retains its
    /// original allocator regardless. Rejected or oversized storage is dropped.
    /// This operation never allocates and retains at most 64 KiB of capacities.
    #[doc(hidden)]
    pub fn recycle_attributes(&mut self, token: RecyclingToken, attributes: Vec<Attribute>) {
        self.attribute_recycling.recycle(token, attributes);
    }

    /// Return the next event, or `None` when input or a custom conversion is needed,
    /// or parsing is done. Inspect [`Self::encoding_conversion`] before feeding
    /// more input when using a multibyte custom map.
    pub fn next_event(&mut self) -> Result<Option<Event>, Error> {
        if let Some(event) = self.finish_foreign_dtd() {
            self.last_position = event.position;
            return Ok(Some(event));
        }
        if let Some(event) = self.pop_event() {
            self.last_position = event.position;
            return Ok(Some(event));
        }
        if let Some(error) = &self.error {
            return Err(*error);
        }
        let mut result = self.next_event_inner();
        if let (Err(error), Some((kind, message))) = (&result, self.decoding_error)
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
            result = Err(Error {
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
            });
        }
        if let Err(error) = &result {
            self.error = Some(*error);
            if error.kind == ErrorKind::NoMemory {
                self.pending.clear();
                return result;
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
                self.error = Some(error);
                self.pending.clear();
                return Err(error);
            }
            if let Some(event) = self.pop_event() {
                result = Ok(Some(event));
            }
        }
        if let Ok(Some(event)) = &result {
            self.last_position = event.position;
        }
        result
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
    /// Absolute input, expansion-work and nesting limits remain independent.
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
        (!self.current_raw.is_empty()).then_some(self.current_raw.as_str())
    }
    /// Replace limits before parsing begins.
    pub fn set_limits(&mut self, limits: Limits) -> Result<(), Error> {
        if self.received != 0 {
            return self.fail(ErrorKind::Syntax, "limits must be set before parsing");
        }
        self.config.limits = limits;
        Ok(())
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
            self.parameter_read
                .as_ref()
                .expect("foreign DTD read marker")
                .store(false, Ordering::Relaxed);
        }
        if let Some(raw) = pending.raw {
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
    fn account_source(&mut self, count: usize) -> Result<(), Error> {
        let bytes = self.source().accounting_bytes(count);
        if !self
            .expanded
            .account(bytes, self.fragment || self.sources.len() > 1, true)
        {
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

    fn next_event_inner(&mut self) -> Result<Option<Event>, Error> {
        // Encoding detection consumes a BOM without producing a text token.
        // Charge that prefix even for empty input or an incomplete next token.
        self.account_source(0)?;
        if let Some((_, position)) = self.active_parameter_reference.take() {
            if self
                .parameter_read
                .as_ref()
                .is_some_and(|read| read.load(Ordering::Relaxed))
            {
                if !self.standalone {
                    self.emit(EventKind::NotStandalone, position)?;
                    self.event_raw("")?;
                }
            } else {
                self.declarations_skipped |= !self.standalone;
            }
        }
        loop {
            if let Some(event) = self.pop_event() {
                return Ok(Some(event));
            }
            if self.finished {
                return Ok(None);
            }
            if self.value_state.is_some() {
                if !self.continue_value()? {
                    return Ok(None);
                }
                continue;
            }
            if self.has_header_composition() {
                if !self.continue_header_composition()? {
                    return Ok(None);
                }
                continue;
            }
            if self.has_declaration_composition() {
                if !self.continue_declaration_composition()? {
                    return Ok(None);
                }
                continue;
            }
            if self.source().remaining().is_empty() {
                if self.sources.len() > 1 {
                    self.finish_conditional_source()?;
                    let source = self.sources.pop().expect("entity source exists");
                    if self.stack.len() != source.initial_depth || self.in_cdata {
                        return Err(self.err(
                            ErrorKind::AsynchronousEntity,
                            "entity replacement is not balanced",
                        ));
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
                    return Ok(None);
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
                return Ok(None);
            }
            if self.in_doctype || self.external_subset {
                if !self.parse_dtd_step()? {
                    return Ok(None);
                }
                continue;
            }
            if self.in_cdata {
                if !self.parse_cdata()? {
                    return Ok(None);
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
                if !self.parse_reference()? {
                    return Ok(None);
                }
                continue;
            }
            if first != b'<' {
                if !self.parse_text()? {
                    return Ok(None);
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
                return Ok(None);
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
                return Ok(None);
            }
            let end = self
                .source_mut()
                .scan_token(mode, max_token)
                .map_err(|(kind, offset)| {
                    self.err_at(kind, "invalid or oversized XML token", offset)
                })?;
            let Some(end) = end else {
                if self.sources.len() == 1
                    && self.source().position(0).byte_index == self.feed_start_byte
                {
                    self.source_mut().mark_deferred();
                }
                if final_input {
                    return Err(self.err(ErrorKind::UnclosedToken, "unclosed XML token"));
                }
                return Ok(None);
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
                if let Some((offset, _)) = token
                    .char_indices()
                    .find(|(_, character)| !is_xml_char(*character))
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
                    ScanMode::Tag if token.starts_with("</") => {
                        self.parse_end(token.view(), position)?
                    }
                    ScanMode::Tag => self.parse_start(token.view(), position)?,
                    ScanMode::DtdDeclaration => {
                        unreachable!("DTD scanner only runs in DTD context")
                    }
                }
                Ok(())
            })();
            // Parsing only writes queued raw overrides. Publish the owned token
            // before returning either its events or its terminal error.
            token.swap_decoded(&mut self.current_raw)?;
            parsed?;
            self.token_scratch = token;
            self.consume(end)?;
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

    fn parse_text(&mut self) -> Result<bool, Error> {
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
        if !self.seen_root && !self.fragment {
            // Whitespace is a separate prolog token before a quoted literal.
            // Emit its default callback before diagnosing the following token.
            let remaining = self.source().remaining();
            let whitespace_end = remaining
                .char_indices()
                .find(|(_, character)| !whitespace(*character))
                .map_or(remaining.len(), |(offset, _)| offset);
            if whitespace_end > 0
                && matches!(remaining.as_bytes().get(whitespace_end), Some(b'\'' | b'"'))
            {
                limit = limit.min(whitespace_end);
            }
        }
        let text = &self.source().remaining()[..limit];
        let final_text = self.is_source_final() && limit == self.source().remaining().len();
        let coalesce = !self.stack.is_empty() || self.fragment;
        let mut boundary = 0;
        let mut end = text
            .bytes()
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
        let invalid = text[..end]
            .char_indices()
            .find(|(_, character)| !is_xml_char(*character))
            .map(|(index, _)| index);
        let forbidden = text[..end].find("]]>");
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
        if self.stack.is_empty() && !self.fragment && !text.chars().all(whitespace) {
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
        let value = if !self.stack.is_empty() || self.fragment {
            Some(self.character_data(text)?)
        } else {
            None
        };
        self.save_current_raw(end)?;
        self.declaration_allowed = false;
        if let Some(value) = value {
            self.emit(EventKind::Text(value), position)?;
        } else if self.default_events {
            self.emit(EventKind::Default, position)?;
        }
        self.consume(end)?;
        Ok(true)
    }

    fn parse_cdata(&mut self) -> Result<bool, Error> {
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
        if let Some((invalid, _)) = text[..end]
            .char_indices()
            .find(|(_, character)| !is_xml_char(*character))
        {
            if invalid == 0 {
                return Err(self.err(ErrorKind::InvalidToken, "invalid XML character"));
            }
            end = invalid;
        }
        let text = &text[..end];
        let position = self.source().position(end);
        let value = self.character_data(text)?;
        self.save_current_raw(end)?;
        self.consume(end)?;
        self.emit(EventKind::Text(value), position)?;
        Ok(true)
    }

    fn parse_reference(&mut self) -> Result<bool, Error> {
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
            self.emit(
                EventKind::Text(Text::try_from_str_in(
                    character.encode_utf8(&mut [0; 4]),
                    self.allocator,
                )?),
                position,
            )?;
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
        let Some(entity) = self.entities.get(&name) else {
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
        if self.entity_chain.iter().any(|item| item == &name)
            || self
                .sources
                .iter()
                .any(|source| source.entity_name.as_deref() == Some(&name))
        {
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
            let system_id = entity.system_id.try_clone()?;
            let public_id = entity.public_id.try_clone()?;
            let mut context = String::new_in(self.allocator);
            if self.config.namespace_separator.is_some() {
                let mut bindings = Vec::new_in(self.allocator);
                for binding in &self.namespaces {
                    try_push(&mut bindings, binding)?;
                }
                bindings.sort_unstable_by_key(|(prefix, _)| *prefix);
                for (prefix, uri) in bindings {
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
                EventKind::ExternalEntityReference(oriole_storage::try_box(
                    crate::ExternalEntityReference {
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
        try_push(
            &mut self.sources,
            Source::entity(
                value,
                name,
                position,
                self.stack.len(),
                self.config.name_rules,
            ),
        )?;
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
            if target != "xml" || !self.declaration_allowed || self.sources.len() > 1 {
                return Err(self.err(
                    ErrorKind::MisplacedXmlDeclaration,
                    "XML declaration is not at the beginning",
                ));
            }
            let mut attrs = Vec::new_in(self.allocator);
            parse_raw_attributes(rest, false, &mut attrs, 3, self.config.name_rules).map_err(
                |error| {
                    self.err_at(
                        if error.kind == ErrorKind::NoMemory {
                            ErrorKind::NoMemory
                        } else {
                            ErrorKind::XmlDeclaration
                        },
                        error.message,
                        2 + target.len() + error.position.byte_index,
                    )
                },
            )?;
            if self.fragment && !self.is_external_value() {
                let mut attrs = attrs.into_iter().map(|attribute| attribute.parts(rest));
                let first = attrs
                    .next()
                    .ok_or_else(|| self.err(ErrorKind::XmlDeclaration, "empty text declaration"))?;
                let (version, encoding_attr) = if first.0 == "version" {
                    (
                        Some(token.for_slice(first.1).decode(self.allocator)?),
                        attrs.next(),
                    )
                } else {
                    (None, Some(first))
                };
                let (name, encoding, _, _) = encoding_attr.ok_or_else(|| {
                    self.err(
                        ErrorKind::XmlDeclaration,
                        "text declaration requires an encoding",
                    )
                })?;
                let encoding = token.for_slice(encoding).decoded(self.allocator)?;
                if name != "encoding" || !valid_encoding_name(&encoding) || attrs.next().is_some() {
                    return Err(self.err(ErrorKind::XmlDeclaration, "invalid text declaration"));
                }
                self.decoder
                    .check_declaration(&encoding)
                    .map_err(|error| self.err(error.kind, error.message))?;
                self.declaration_allowed = false;
                self.emit(
                    EventKind::TextDeclaration {
                        version,
                        encoding: string(&encoding, self.allocator)?,
                    },
                    position,
                )?;
                return Ok(());
            }
            let version = attrs
                .first()
                .map(|attribute| {
                    token
                        .for_slice(attribute.value(rest))
                        .decoded(self.allocator)
                })
                .transpose()?;
            if attrs.is_empty()
                || attrs[0].name(rest) != "version"
                || !version.as_ref().is_some_and(|version| {
                    !version.is_empty()
                        && version.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
                        })
                })
            {
                return Err(self.err_at(
                    ErrorKind::XmlDeclaration,
                    "XML declaration must begin with a version",
                    2 + target.len() + attrs.first().map_or(0, |attribute| attribute.name_start),
                ));
            }
            let version = version
                .expect("validated declaration version")
                .into_owned(self.allocator)?;
            let mut encoding = None;
            let mut standalone = None;
            for attribute in attrs.into_iter().skip(1) {
                let (name, value, _, _) = attribute.parts(rest);
                match name {
                    "encoding" if encoding.is_none() && standalone.is_none() => {
                        let value = token.for_slice(value).decoded(self.allocator)?;
                        if !valid_encoding_name(&value) {
                            return Err(
                                self.err(ErrorKind::XmlDeclaration, "invalid encoding name")
                            );
                        }
                        self.decoder
                            .check_declaration(&value)
                            .map_err(|error| self.err(error.kind, error.message))?;
                        encoding = Some(value.into_owned(self.allocator)?);
                    }
                    "standalone" if standalone.is_none() => {
                        standalone = Some(match value {
                            "yes" => true,
                            "no" => false,
                            _ => {
                                return Err(self.err(
                                    ErrorKind::XmlDeclaration,
                                    "invalid standalone declaration",
                                ));
                            }
                        });
                    }
                    _ => {
                        return Err(self.err(
                            ErrorKind::XmlDeclaration,
                            "invalid XML declaration attribute",
                        ));
                    }
                }
            }
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

    fn parse_start(&mut self, token: lexical::Slice<'_>, position: Position) -> Result<(), Error> {
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
        let (raw_name, rest) = take_name(body, self.config.name_rules)
            .ok_or_else(|| self.err_at(ErrorKind::InvalidToken, "invalid element name", 1))?;
        if self.config.namespace_separator.is_some() && !self.config.name_rules.is_qname(raw_name) {
            return Err(self.err(ErrorKind::InvalidToken, "invalid qualified element name"));
        }
        let name_value = token.for_slice(raw_name).decoded(self.allocator)?;
        let name: &str = &name_value;
        // Offset records retain no references into the reusable lexical buffer.
        // Validate the complete tag before expanding values or emitting callbacks.
        let mut raw_attrs =
            std::mem::replace(&mut self.raw_attributes, Vec::new_in(self.allocator));
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
        if raw_attrs.len() > self.config.limits.max_attributes {
            return Err(self.err(ErrorKind::LimitExceeded, "attribute count limit exceeded"));
        }
        // Empty tags need no attribute buffer and must not evict a warm cache.
        let mut attrs = if raw_attrs.is_empty() {
            Vec::new_in(self.allocator)
        } else {
            self.attribute_recycling.take()
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
        let mut names = (raw_attrs.len() > 8)
            .then(|| HashSet::with_hasher_in(self.namespaces.hasher().clone(), self.allocator));
        for (index, attribute) in raw_attrs.iter().enumerate() {
            let (attr_name, value, attribute_offset, _) = attribute.parts(rest);
            if self.config.namespace_separator.is_some()
                && !self.config.name_rules.is_qname(attr_name)
            {
                return Err(self.err(ErrorKind::InvalidToken, "invalid qualified attribute name"));
            }
            let attr_name = decoded_names.get(index).map_or(attr_name, |name| &**name);
            let duplicate = if let Some(names) = &mut names {
                !try_set_insert(names, attr_name)?
            } else {
                raw_attrs[..index]
                    .iter()
                    .enumerate()
                    .any(|(prior, attribute)| {
                        decoded_names
                            .get(prior)
                            .map_or(attribute.name(rest), |name| &**name)
                            == attr_name
                    })
            };
            if duplicate {
                return Err(self.err_at(
                    ErrorKind::DuplicateAttribute,
                    "duplicate attribute",
                    1 + raw_name.len() + attribute_offset,
                ));
            }
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
                .defaults
                .get(name)
                .and_then(|decls| decls.get(attr_name))
                .is_some_and(|decl| decl.attribute_type != "CDATA")
                && attribute_needs_normalization(value);
            let attribute = &mut attrs[index];
            self.expand_attribute_into(token.for_slice(value), &mut attribute.value, tokenized)?;
            copy_attribute_string(&mut attribute.name, attr_name)?;
            attribute.specified = true;
        }
        if let Some(defaults) = self.defaults.get(name) {
            for default in &defaults.ordered {
                if !names.as_ref().map_or_else(
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
            self.defaults
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
                        self.namespaces.remove(prefix)
                    } else {
                        try_insert(
                            &mut self.namespaces,
                            string(prefix, self.allocator)?,
                            uri.try_clone()?,
                        )?
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
            let mut expanded =
                HashSet::with_hasher_in(self.namespaces.hasher().clone(), self.allocator);
            for attr in &mut attrs {
                // An unprefixed attribute has no namespace. Its raw name was
                // already checked for duplicates, and may legitimately equal
                // another attribute's serialized expanded name.
                if !attr.name.contains(':') {
                    continue;
                }
                let key = self.expand_name(&attr.name, true, false)?;
                if !try_set_insert(&mut expanded, key)? {
                    return Err(self.err(
                        ErrorKind::DuplicateAttribute,
                        "duplicate expanded attribute name",
                    ));
                }
                attr.name = self.expand_name(&attr.name, true, self.config.namespace_triplets)?;
            }
        }
        let expanded_name = self.expand_name(name, false, self.config.namespace_triplets)?;
        let raw_encoding = token.for_slice(raw_name).name_encoding(self.allocator)?;
        let raw_encoding = if raw_encoding.is_empty() {
            None
        } else {
            Some(oriole_storage::try_box(raw_encoding, self.allocator)?)
        };
        self.seen_root = true;
        self.declaration_allowed = false;
        try_push(
            &mut self.stack,
            Element {
                expanded_name: if expanded_name == name {
                    None
                } else {
                    Some(expanded_name.try_clone()?)
                },
                raw_name: name_value.into_owned(self.allocator)?,
                raw_encoding,
                bindings,
            },
        )?;
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
            element.raw_name != *decoded_name
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
        self.emit(
            EventKind::EndElement {
                name: element.expanded_name.unwrap_or(element.raw_name),
            },
            position,
        )?;
        for (prefix, previous) in element.bindings.into_iter().rev() {
            match previous {
                Some(uri) => {
                    try_insert(&mut self.namespaces, prefix.try_clone()?, uri)?;
                }
                None => {
                    self.namespaces.remove(&prefix);
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

    fn expand_name(&self, name: &str, attribute: bool, triplets: bool) -> Result<String, Error> {
        let Some(separator) = self.config.namespace_separator else {
            return string(name, self.allocator);
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
                Some(prefix) => Some(self.namespaces.get(prefix).ok_or_else(|| {
                    self.err(ErrorKind::UndefinedPrefix, "unbound namespace prefix")
                })?),
                None if !attribute => self.namespaces.get(""),
                None => None,
            };
        let Some(uri) = uri else {
            return string(local, self.allocator);
        };
        // A short qualified name can reuse an arbitrarily long URI on every
        // element or attribute. Bound that copied output independently of input.
        self.charge_expansion(uri.len())?;
        let mut result = String::try_with_capacity_in(
            uri.len() + local.len() + prefix.map_or(2, |p| p.len() + 2),
            self.allocator,
        )?;
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
        self.expanded
            .expanded
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |expanded| {
                expanded
                    .checked_add(size)
                    .filter(|expanded| *expanded <= self.config.limits.max_entity_expansion_bytes)
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
        &mut self,
        value: lexical::Slice<'_>,
        output: &mut String,
        tokenized: bool,
    ) -> Result<(), Error> {
        if tokenized
            || value.has_ascii_aliases()
            || value
                .bytes()
                .any(|byte| matches!(byte, b'&' | b'<' | b'\t' | b'\r' | b'\n'))
        {
            *output = self.expand_attribute(value, &mut Vec::new_in(self.allocator), tokenized)?;
        } else {
            copy_attribute_string(output, &value)?;
        }
        Ok(())
    }

    fn expand_attribute(
        &mut self,
        value: lexical::Slice<'_>,
        chain: &mut Vec<String>,
        tokenized: bool,
    ) -> Result<String, Error> {
        if !tokenized && !value.bytes().any(|byte| matches!(byte, b'&' | b'<')) {
            if !chain.is_empty() {
                self.account_entity_bytes(value.len(), true)?;
            }
            return normalize_lexical_attribute(value, self.allocator);
        }
        let mut output = String::try_with_capacity_in(value.len(), self.allocator)?;
        self.append_attribute(value, chain, &mut output, tokenized)?;
        if tokenized && output.ends_with(' ') {
            output.truncate(output.len() - 1);
        }
        Ok(output)
    }

    /// Expand into one output so normalization spans entity boundaries. Converted
    /// ASCII remains data while raw whitespace and numeric spaces are normalized.
    fn append_attribute(
        &mut self,
        value: lexical::Slice<'_>,
        chain: &mut Vec<String>,
        output: &mut String,
        tokenized: bool,
    ) -> Result<(), Error> {
        let mut rest: &str = &value;
        while !rest.is_empty() {
            let end = rest.find(['&', '<']).unwrap_or(rest.len());
            if !chain.is_empty() {
                self.account_entity_bytes(end, true)?;
            }
            let literal = value.for_slice(&rest[..end]);
            if tokenized {
                for (character, raw_ascii) in literal.decoded_chars() {
                    if raw_ascii && whitespace(character) {
                        append_attribute_space(output)?;
                    } else {
                        output.try_push(character)?;
                    }
                }
            } else {
                output.try_push_str(&normalize_lexical_attribute(literal, self.allocator)?)?;
            }
            rest = &rest[end..];
            if rest.is_empty() {
                break;
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
            if !chain.is_empty() {
                self.account_entity_bytes(end + 1, true)?;
            }
            let raw_name = &rest[1..end];
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
            } else {
                let decoded_name = value.for_slice(raw_name).decoded(self.allocator)?;
                let name: &str = &decoded_name;
                if !self.config.name_rules.is_name(raw_name)
                    || (self.config.namespace_separator.is_some() && raw_name.contains(':'))
                {
                    return Err(self.err(ErrorKind::InvalidToken, "invalid entity name"));
                }
                if chain.iter().any(|item| item == name)
                    || self.entity_chain.iter().any(|item| item == name)
                {
                    return Err(self.err(
                        ErrorKind::RecursiveEntityReference,
                        "recursive entity in attribute",
                    ));
                }
                if chain.len() >= self.config.limits.max_entity_depth {
                    return Err(self.err(ErrorKind::LimitExceeded, "entity nesting limit exceeded"));
                }
                let Some(entity) = self.entities.get(name) else {
                    if !self.requires_internal_entity_declaration() {
                        rest = &rest[end + 1..];
                        continue;
                    }
                    return Err(
                        self.err(ErrorKind::UndefinedEntity, "undefined entity in attribute")
                    );
                };
                if entity.declared_in_parameter_entity
                    && self.requires_internal_entity_declaration()
                {
                    return Err(self.err(
                        ErrorKind::EntityDeclaredInParameterEntity,
                        "entity was declared in a parameter entity",
                    ));
                }
                let value = entity.value.as_ref().ok_or_else(|| {
                    self.err(
                        ErrorKind::ExternalEntityInAttribute,
                        "external entity in attribute",
                    )
                })?;
                self.charge_expansion(value.len())?;
                let value = value.try_clone()?;
                try_push(chain, string(name, self.allocator)?)?;
                self.append_attribute(lexical::Slice::plain(&value), chain, output, tokenized)?;
                chain.pop();
            }
            rest = &rest[end + 1..];
        }
        Ok(())
    }
}

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

fn parse_raw_attributes(
    text: &str,
    allow_refs: bool,
    result: &mut Vec<RawAttribute>,
    limit: usize,
    name_rules: NameRules,
) -> Result<(), Error> {
    let original_len = text.len();
    let mut text = text;
    result.clear();
    let error_at = |kind, message, remaining: &str| Error {
        kind,
        message,
        position: Position {
            byte_index: original_len - remaining.len(),
            line: 1,
            column: 0,
            byte_count: 0,
        },
    };
    while !text.is_empty() {
        let trimmed = text.trim_start_matches(whitespace);
        if trimmed.len() == text.len() {
            return Err(error_at(
                ErrorKind::InvalidToken,
                "attributes must be separated by whitespace",
                text,
            ));
        }
        text = trimmed;
        if text.is_empty() {
            break;
        }
        if result.len() >= limit {
            return Err(error_at(
                ErrorKind::LimitExceeded,
                "attribute count limit exceeded",
                text,
            ));
        }
        let name_offset = original_len - text.len();
        let (name, rest) = take_name(text, name_rules).ok_or(error_at(
            ErrorKind::InvalidToken,
            "invalid attribute name",
            text,
        ))?;
        let rest = rest.trim_start_matches(whitespace);
        let rest = rest
            .strip_prefix('=')
            .ok_or(error_at(
                ErrorKind::InvalidToken,
                "attribute is missing equals sign",
                rest,
            ))?
            .trim_start_matches(whitespace);
        let quote = rest
            .chars()
            .next()
            .filter(|c| matches!(c, '\'' | '"'))
            .ok_or(error_at(
                ErrorKind::InvalidToken,
                "attribute value must be quoted",
                rest,
            ))?;
        let rest = &rest[1..];
        let end = rest.find(quote).ok_or(error_at(
            ErrorKind::UnclosedToken,
            "unclosed attribute value",
            rest,
        ))?;
        let value = &rest[..end];
        let value_offset = original_len - rest.len();
        if let Some(offset) = value
            .find('<')
            .or_else(|| (!allow_refs).then(|| value.find('&')).flatten())
        {
            return Err(error_at(
                ErrorKind::InvalidToken,
                "invalid character in attribute value",
                &rest[offset..],
            ));
        }
        try_push(
            result,
            RawAttribute {
                name_start: name_offset,
                name_end: name_offset + name.len(),
                value_start: value_offset,
                value_end: value_offset + value.len(),
            },
        )?;
        text = &rest[end + 1..];
    }
    Ok(())
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
fn normalize_lexical_attribute(
    text: lexical::Slice<'_>,
    allocator: Allocator,
) -> Result<String, Error> {
    if !text.has_ascii_aliases() {
        return normalize_attribute_whitespace(&text, allocator);
    }
    let mut output = String::try_with_capacity_in(text.len(), allocator)?;
    let mut previous_cr = false;
    for (character, ascii) in text.decoded_chars() {
        if ascii && character == '\n' && previous_cr {
            previous_cr = false;
            continue;
        }
        previous_cr = ascii && character == '\r';
        output.try_push(if ascii && whitespace(character) {
            ' '
        } else {
            character
        })?;
    }
    Ok(output)
}

fn normalize_attribute_whitespace(text: &str, allocator: Allocator) -> Result<String, Error> {
    if !text.contains(['\t', '\r', '\n']) {
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
        output.try_push(if whitespace(character) {
            ' '
        } else {
            character
        })?;
    }
    Ok(output)
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
mod hash_salt_tests {
    use super::*;
    use std::hash::BuildHasher;

    #[test]
    fn salt_rebuilds_every_table_and_owned_children() {
        let mut parser = Parser::new(Config::default());
        parser.set_param_entity_parsing(2);
        parser.feed(b"<!DOCTYPE r [<!ENTITY e 'ok'><!ENTITY % p \"<!ATTLIST n b CDATA 'v'>\">%p;<!ATTLIST r a CDATA 'v'>]><r>", false).unwrap();
        while parser.next_event().unwrap().is_some() {}
        let previous = parser.namespaces.hasher().hash_one("xml");
        parser.set_hash_salt(*b"0123456789abcdef").unwrap();
        assert_ne!(parser.namespaces.hasher().hash_one("xml"), previous);
        assert_eq!(parser.entities.hasher().salt(), parser.hash_salt());
        assert_eq!(
            parser.parameter_entities.hasher().salt(),
            parser.hash_salt()
        );
        assert_eq!(parser.defaults.hasher().salt(), parser.hash_salt());
        assert_eq!(
            parser.namespaces.get("xml").unwrap(),
            "http://www.w3.org/XML/1998/namespace"
        );
        assert!(parser.entities.contains_key("e"));
        assert!(parser.parameter_entities.contains_key("p"));
        for attributes in parser.defaults.values() {
            assert_eq!(attributes.by_name.hasher().salt(), parser.hash_salt());
            for attribute in &attributes.ordered {
                assert!(attributes.get(&attribute.name).is_some());
            }
        }
        let mut child = parser.external_child(Some(""), None).unwrap();
        assert_eq!(child.hash_salt(), parser.hash_salt());
        child.feed(b"<n>&e;</n>", true).unwrap();
        while child.next_event().unwrap().is_some() {}
        parser.feed(b"<n>&e;</n></r>", true).unwrap();
        while parser.next_event().unwrap().is_some() {}
    }
}
