//! A safe, incremental XML 1.0 parser with explicit resource limits.
//!
//! Feed bytes with [`Parser::feed`], then drain owned events with
//! [`Parser::next_event`]. No borrowed parser state crosses an event boundary.
#![forbid(unsafe_code)]

mod dtd;
mod encoding;
mod names;

use oriole_storage::{
    AllocError, Allocator, HashMap, Queue, Shared, String, TryClone, Vec, hash_map, hash_set,
    try_insert, try_push, try_set_insert,
};
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

use encoding::{Decoder, Source};
use names::{is_name, is_xml_char, whitespace};

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

#[derive(Debug, Eq, PartialEq)]
pub enum EventKind {
    /// Opt-in whitespace callback outside element content. Read [`Parser::current_raw`]
    /// before advancing the parser; this event does not own the raw token.
    Default,
    StartElement {
        name: String,
        attributes: Vec<Attribute>,
    },
    EndElement {
        name: String,
    },
    Text(String),
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
    ExternalEntityReference {
        context: Option<String>,
        system_id: Option<String>,
        public_id: Option<String>,
    },
    StartDoctype {
        name: String,
        system_id: Option<String>,
        public_id: Option<String>,
        has_internal_subset: bool,
    },
    EndDoctype,
    NotStandalone,
    StartNamespace {
        prefix: Option<String>,
        uri: Option<String>,
    },
    EndNamespace {
        prefix: Option<String>,
    },
    EntityDeclaration {
        name: String,
        value: Option<String>,
        parameter: bool,
        system_id: Option<String>,
        public_id: Option<String>,
        notation: Option<String>,
    },
    AttlistDeclaration {
        element: String,
        name: String,
        attribute_type: String,
        default: Option<String>,
        required: bool,
    },
    NotationDeclaration {
        name: String,
        system_id: Option<String>,
        public_id: Option<String>,
    },
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
    expanded_name: String,
    bindings: Vec<(String, Option<String>)>,
}

#[derive(Debug)]
struct Entity {
    value: Option<String>,
    system_id: Option<String>,
    public_id: Option<String>,
    notation: Option<String>,
    declared_in_parameter_entity: bool,
}

#[derive(Debug)]
struct PendingEvent {
    event: Event,
    raw: Option<String>,
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
    fn new(allocator: Allocator) -> Self {
        Self {
            ordered: Vec::new_in(allocator),
            by_name: hash_map(allocator),
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
        let mut cloned = Self::new(*self.ordered.allocator());
        for attribute in &self.ordered {
            cloned.try_insert(attribute.try_clone()?)?;
        }
        Ok(cloned)
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
/// DTDs. Internal parameter entities may select a conditional keyword. Other inline
/// parameter references remain unsupported.
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
    in_doctype: bool,
    conditional: dtd::ConditionalState,
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
    expanded: Shared<AtomicUsize>,
    fragment: bool,
    external_subset: bool,
    external_depth: usize,
    entity_chain: Vec<String>,
    has_external_subset: bool,
    standalone: bool,
    reparse_deferral: bool,
    last_position: Position,
    current_raw: String,
    expand_internal_entities: bool,
    default_events: bool,
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
        try_push(&mut sources, Source::new(allocator))?;
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
            in_doctype: false,
            conditional: dtd::ConditionalState::new(allocator),
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
            expanded: Shared::try_new_in(AtomicUsize::new(0), allocator)?,
            fragment: false,
            external_subset: false,
            external_depth: 0,
            entity_chain: Vec::new_in(allocator),
            has_external_subset: false,
            standalone: false,
            reparse_deferral: true,
            last_position: Position {
                line: 1,
                ..Position::default()
            },
            current_raw: String::new_in(allocator),
            expand_internal_entities: true,
            default_events: false,
            decoding_error: None,
            id_attribute_index: None,
        })
    }

    #[must_use]
    pub fn allocator(&self) -> Allocator {
        self.allocator
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

    /// Construct an independent parser for an application-provided external entity.
    ///
    /// Pass the context from [`EventKind::ExternalEntityReference`]. Namespace
    /// bindings, declarations, recursion tracking, and the expansion budget are
    /// inherited. The parser never opens a path or performs a network request.
    /// A `None` context creates an external DTD parser. After its input has parsed
    /// successfully, use [`Self::merge_external_subset`] to import its declarations.
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
        child
            .decoder
            .inherit_map(&self.decoder, &mut child.sources[0])?;
        for (name, entity) in &self.entities {
            try_insert(&mut child.entities, name.try_clone()?, entity.try_clone()?)?;
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
                entity.try_clone()?,
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
        child.expand_internal_entities = self.expand_internal_entities;
        child.default_events = self.default_events;
        child.parameter_mode = self.parameter_mode;
        child.has_external_subset = self.has_external_subset;
        child.standalone = self.standalone;
        child.reparse_deferral = self.reparse_deferral;
        for name in &self.entity_chain {
            try_push(&mut child.entity_chain, name.try_clone()?)?;
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

    /// Bound inherited copies before allocating a child, including work for empty
    /// declarations. Repeated empty external references must not repeatedly clone
    /// an otherwise unused large declaration environment for free.
    fn charge_external_child_storage(
        &self,
        context: Option<&str>,
        encoding: Option<&str>,
    ) -> Result<(), Error> {
        for (name, entity) in self.entities.iter().chain(&self.parameter_entities) {
            self.charge_expansion(size_of::<(String, Entity)>())?;
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

    /// Select parameter entity processing: 0 never, 1 unless standalone, 2 always.
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
                    DefaultAttributes::new(self.allocator),
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
        ) {
            self.decoding_error = Some((error.kind, error.message));
        }
        Ok(())
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
    /// Converted ASCII aliases are unsupported; ASCII must use direct map entries.
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
        self.error = None;
        self.decoding_error = None;
        if let Err(error) = self.decoder.feed(
            &[],
            self.final_input,
            &mut self.sources[0],
            self.config.limits.max_token_bytes,
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

    /// Resolve the pending sequence to a non-ASCII BMP scalar, or pass `-1` for invalid data.
    ///
    /// ASCII aliases are rejected so conversion cannot change XML lexical syntax.
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
        ) {
            self.decoding_error = Some((error.kind, error.message));
        }
        Ok(())
    }

    /// Return the next event, or `None` when input or a custom conversion is needed,
    /// or parsing is done. Inspect [`Self::encoding_conversion`] before feeding
    /// more input when using a multibyte custom map.
    pub fn next_event(&mut self) -> Result<Option<Event>, Error> {
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
                position: if kind == ErrorKind::UnknownEncoding {
                    self.decoder
                        .unknown_encoding_position()
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
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
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
    fn emit(&mut self, kind: EventKind, position: Position) -> Result<(), Error> {
        self.pending.try_push_back(PendingEvent {
            event: Event { kind, position },
            raw: None,
        })?;
        Ok(())
    }
    fn pop_event(&mut self) -> Option<Event> {
        let pending = self.pending.pop_front()?;
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
        self.current_raw
            .try_push_str(&source.remaining()[..count])?;
        Ok(())
    }
    fn event_raw(&mut self, raw: &str) -> Result<(), Error> {
        if let Some(pending) = self.pending.back_mut() {
            pending.raw = Some(string(raw, self.allocator)?);
        }
        Ok(())
    }
    fn consume(&mut self, count: usize) {
        self.source_mut().consume(count);
    }
    fn is_source_final(&self) -> bool {
        self.sources.len() > 1
            || (self.final_input && self.decoder.conversion().is_none())
            || self.decoding_error.is_some()
    }

    fn next_event_inner(&mut self) -> Result<Option<Event>, Error> {
        loop {
            if let Some(event) = self.pop_event() {
                return Ok(Some(event));
            }
            if self.finished {
                return Ok(None);
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
                    if kind == ErrorKind::UnknownEncoding
                        && let Some(position) = self.decoder.unknown_encoding_position()
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
                self.consume(9);
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
                    .is_some_and(|character| !names::is_name_start(character))
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
                    && !names::is_name_start(character)
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
                self.has_external_subset = true;
                if self.parameter_entities_enabled() {
                    self.emit(
                        EventKind::ExternalEntityReference {
                            context: None,
                            system_id: None,
                            public_id: None,
                        },
                        self.here(),
                    )?;
                    continue;
                }
            }
            let position = self.source().position(end);
            let token = string(&self.source().remaining()[..end], self.allocator)?;
            self.save_current_raw(end)?;
            if let Some((offset, _)) = token
                .char_indices()
                .find(|(_, character)| !is_xml_char(*character))
            {
                return Err(self.err_at(ErrorKind::InvalidToken, "invalid XML character", offset));
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
                    self.emit(EventKind::Comment(self.source_text(text)?), position)?;
                }
                ScanMode::Pi => self.parse_pi(&token, position)?,
                ScanMode::Doctype => self.parse_doctype(&token, position)?,
                ScanMode::Tag if token.starts_with("</") => self.parse_end(&token, position)?,
                ScanMode::Tag => self.parse_start(&token, position)?,
            }
            self.consume(end);
        }
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
        let limit = self.source().converted_text_limit();
        let text = &self.source().remaining()[..limit];
        let final_text = self.is_source_final() && limit == self.source().remaining().len();
        let mut end = text
            .bytes()
            .position(|byte| matches!(byte, b'<' | b'&' | b'\n') || (!internal && byte == b'\r'))
            .map_or(text.len(), |index| {
                if index == 0 && matches!(text.as_bytes()[0], b'\r' | b'\n') {
                    if text.starts_with("\r\n") { 2 } else { 1 }
                } else {
                    index
                }
            });
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
            return Err(self.err_at(
                ErrorKind::InvalidToken,
                "CDATA terminator in character data",
                forbidden + 2,
            ));
        }
        if let Some(stop) = invalid {
            if stop == 0 {
                return Err(self.err_at(ErrorKind::InvalidToken, "invalid XML character data", 0));
            }
            end = stop;
        }
        let text = &text[..end];
        if self.stack.is_empty() && !self.fragment && !text.chars().all(whitespace) {
            if !self.seen_root && is_name(text) && end == self.source().remaining().len() {
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
                && is_name(text)
                && end == self.source().remaining().len()
                && let Some((kind, message)) = self.decoding_error
            {
                return Err(self.err_at(kind, message, end));
            }
            if !self.seen_root
                && is_name(text)
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
            Some(self.source_text(text)?)
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
        self.consume(end);
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
            self.consume(3);
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
        let value = self.source_text(text)?;
        self.save_current_raw(end)?;
        self.consume(end);
        self.emit(EventKind::Text(value), position)?;
        Ok(true)
    }

    fn parse_reference(&mut self) -> Result<bool, Error> {
        let limit = self.config.limits.max_token_bytes;
        let end = self.source_mut().scan_reference(limit).map_err(|kind| {
            let offset = self
                .source()
                .remaining()
                .char_indices()
                .skip(1)
                .find(|(_, character)| {
                    whitespace(*character) || matches!(*character, '<' | '&' | '\'' | '"' | '\0')
                })
                .map_or(0, |(index, _)| index);
            self.err_at(kind, "invalid entity reference", offset)
        })?;
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
            Some(string(&text[1..end], self.allocator)?)
        } else {
            None
        };
        let position = self.source().position(end + 1);
        self.save_current_raw(end + 1)?;
        if let Some(character) = character
            .map_err(|(kind, offset)| self.err_at(kind, "invalid character reference", offset))?
        {
            self.consume(end + 1);
            self.emit(
                EventKind::Text(string(character.encode_utf8(&mut [0; 4]), self.allocator)?),
                position,
            )?;
            return Ok(true);
        }
        let name = name.expect("general entity references have an owned name");
        if !is_name(&name) {
            return Err(self.err(ErrorKind::InvalidToken, "invalid entity name"));
        }
        if self.config.namespace_separator.is_some()
            && let Some(colon) = name.find(':')
        {
            return Err(self.err_at(
                ErrorKind::InvalidToken,
                "entity names cannot contain colons with namespaces enabled",
                colon + 1,
            ));
        }
        let Some(entity) = self.entities.get(&name) else {
            if self.has_external_subset && !self.standalone {
                self.consume(end + 1);
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
            let system_id = entity
                .system_id
                .try_clone()?
                .expect("external entities have a system identifier");
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
            self.consume(end + 1);
            self.emit(
                EventKind::ExternalEntityReference {
                    context: Some(context),
                    system_id: Some(system_id),
                    public_id,
                },
                position,
            )?;
            return Ok(true);
        }
        if !self.expand_internal_entities {
            self.consume(end + 1);
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
        self.consume(end + 1);
        try_push(
            &mut self.sources,
            Source::entity(value, name, position, self.stack.len()),
        )?;
        Ok(true)
    }

    fn parse_pi(&mut self, token: &str, position: Position) -> Result<(), Error> {
        let body = token
            .strip_prefix("<?")
            .and_then(|body| body.strip_suffix("?>"))
            .ok_or_else(|| self.err(ErrorKind::InvalidToken, "invalid processing instruction"))?;
        let (target, rest) = take_name(body).ok_or_else(|| {
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
            let attrs = parse_raw_attributes(rest, false, self.allocator, 3).map_err(|error| {
                self.err_at(
                    if error.kind == ErrorKind::NoMemory {
                        ErrorKind::NoMemory
                    } else {
                        ErrorKind::XmlDeclaration
                    },
                    error.message,
                    2 + target.len() + error.position.byte_index,
                )
            })?;
            if self.fragment {
                let mut attrs = attrs.into_iter();
                let first = attrs
                    .next()
                    .ok_or_else(|| self.err(ErrorKind::XmlDeclaration, "empty text declaration"))?;
                let (version, encoding_attr) = if first.0 == "version" {
                    (Some(string(first.1, self.allocator)?), attrs.next())
                } else {
                    (None, Some(first))
                };
                let (name, encoding, _, _) = encoding_attr.ok_or_else(|| {
                    self.err(
                        ErrorKind::XmlDeclaration,
                        "text declaration requires an encoding",
                    )
                })?;
                if name != "encoding" || !valid_encoding_name(encoding) || attrs.next().is_some() {
                    return Err(self.err(ErrorKind::XmlDeclaration, "invalid text declaration"));
                }
                self.decoder
                    .check_declaration(encoding)
                    .map_err(|error| self.err(error.kind, error.message))?;
                self.declaration_allowed = false;
                self.emit(
                    EventKind::TextDeclaration {
                        version,
                        encoding: string(encoding, self.allocator)?,
                    },
                    position,
                )?;
                return Ok(());
            }
            if attrs.is_empty() || attrs[0].0 != "version" || !matches!(attrs[0].1, "1.0" | "1.1") {
                return Err(self.err_at(
                    ErrorKind::XmlDeclaration,
                    "XML declaration must begin with a version",
                    2 + target.len() + attrs.first().map_or(0, |attribute| attribute.2),
                ));
            }
            let version = string(attrs[0].1, self.allocator)?;
            if version != "1.0" {
                return Err(self.err(ErrorKind::XmlDeclaration, "only XML 1.0 is supported"));
            }
            let mut encoding = None;
            let mut standalone = None;
            for (name, value, _, _) in attrs.into_iter().skip(1) {
                match name {
                    "encoding" if encoding.is_none() && standalone.is_none() => {
                        if !valid_encoding_name(value) {
                            return Err(
                                self.err(ErrorKind::XmlDeclaration, "invalid encoding name")
                            );
                        }
                        self.decoder
                            .check_declaration(value)
                            .map_err(|error| self.err(error.kind, error.message))?;
                        encoding = Some(string(value, self.allocator)?);
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
            self.standalone = standalone == Some(true);
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
                    target: string(target, self.allocator)?,
                    data: self.source_text(rest.trim_start_matches(whitespace))?,
                },
                position,
            )?;
        }
        self.declaration_allowed = false;
        Ok(())
    }

    fn parse_start(&mut self, token: &str, position: Position) -> Result<(), Error> {
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
        let (name, rest) = take_name(body)
            .ok_or_else(|| self.err_at(ErrorKind::InvalidToken, "invalid element name", 1))?;
        let raw_attrs = parse_raw_attributes(
            rest,
            true,
            self.allocator,
            self.config.limits.max_attributes,
        )
        .map_err(|error| {
            self.err_at(
                error.kind,
                error.message,
                1 + name.len() + error.position.byte_index,
            )
        })?;
        if raw_attrs.len() > self.config.limits.max_attributes {
            return Err(self.err(ErrorKind::LimitExceeded, "attribute count limit exceeded"));
        }
        let mut attrs = Vec::new_in(self.allocator);
        attrs
            .try_reserve(raw_attrs.len())
            .map_err(|_| AllocError::OutOfMemory)?;
        let mut names = hash_set(self.allocator);
        for (attr_name, value, attribute_offset, _) in raw_attrs {
            if !try_set_insert(&mut names, attr_name)? {
                return Err(self.err_at(
                    ErrorKind::DuplicateAttribute,
                    "duplicate attribute",
                    1 + name.len() + attribute_offset,
                ));
            }
            let mut value = self.expand_attribute(value, &mut Vec::new_in(self.allocator))?;
            if self
                .defaults
                .get(name)
                .and_then(|decls| decls.get(attr_name))
                .is_some_and(|decl| decl.attribute_type != "CDATA")
            {
                value = collapse_spaces(&value, self.allocator)?;
            }
            try_push(
                &mut attrs,
                Attribute {
                    name: string(attr_name, self.allocator)?,
                    value,
                    specified: true,
                },
            )?;
        }
        if let Some(defaults) = self.defaults.get(name) {
            for default in &defaults.ordered {
                if !names.contains(default.name.as_str())
                    && let Some(value) = &default.value
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
                    if !prefix.is_empty() && (!is_name(prefix) || prefix.contains(':')) {
                        return Err(self.err(ErrorKind::InvalidToken, "invalid namespace prefix"));
                    }
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
                    if self
                        .config
                        .namespace_separator
                        .is_some_and(|separator| separator != '\0' && uri.contains(separator))
                    {
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
            let mut expanded = hash_set(self.allocator);
            for attr in &mut attrs {
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
        self.seen_root = true;
        self.declaration_allowed = false;
        try_push(
            &mut self.stack,
            Element {
                raw_name: string(name, self.allocator)?,
                expanded_name: expanded_name.try_clone()?,
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
        Ok(())
    }

    fn parse_end(&mut self, token: &str, position: Position) -> Result<(), Error> {
        let body = &token[2..token.len() - 1];
        let (name, rest) =
            take_name(body).ok_or_else(|| self.err(ErrorKind::InvalidToken, "invalid end tag"))?;
        if !rest.chars().all(whitespace) {
            return Err(self.err(ErrorKind::InvalidToken, "unexpected text in end tag"));
        }
        if self.sources.len() > 1 && self.stack.len() <= self.source().initial_depth {
            return Err(self.err(
                ErrorKind::AsynchronousEntity,
                "entity closes an element outside its replacement text",
            ));
        }
        if self
            .stack
            .last()
            .is_none_or(|element| element.raw_name != name)
        {
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
                name: element.expanded_name,
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
            Some((prefix, local)) => {
                if prefix.is_empty()
                    || local.is_empty()
                    || local.contains(':')
                    || !is_name(prefix)
                    || !is_name(local)
                {
                    return Err(self.err(ErrorKind::InvalidToken, "invalid qualified name"));
                }
                (Some(prefix), local)
            }
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
        if triplets && let Some(prefix) = prefix {
            if separator != '\0' {
                result.try_push(separator)?;
            }
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

    fn source_text(&self, text: &str) -> Result<String, Error> {
        if self.sources.len() > 1 {
            // Literal line endings were normalized when the entity was declared.
            // Any CR left in replacement text came from a character reference.
            string(text, self.allocator)
        } else {
            normalize_newlines(text, self.allocator)
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

    fn expand_attribute(&mut self, value: &str, chain: &mut Vec<String>) -> Result<String, Error> {
        if !value.bytes().any(|byte| matches!(byte, b'&' | b'<')) {
            return normalize_attribute_whitespace(value, self.allocator);
        }
        let mut output = String::try_with_capacity_in(value.len(), self.allocator)?;
        let mut rest = value;
        while !rest.is_empty() {
            let end = rest.find(['&', '<']).unwrap_or(rest.len());
            output.try_push_str(&normalize_attribute_whitespace(
                &rest[..end],
                self.allocator,
            )?)?;
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
            let name = &rest[1..end];
            if let Some(character) = character_reference(name)
                .map_err(|(kind, _)| self.err(kind, "invalid character reference"))?
            {
                output.try_push(character)?;
            } else {
                if !is_name(name)
                    || (self.config.namespace_separator.is_some() && name.contains(':'))
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
                    if self.has_external_subset && !self.standalone {
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
                output.try_push_str(&self.expand_attribute(&value, chain)?)?;
                chain.pop();
            }
            rest = &rest[end + 1..];
        }
        Ok(output)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanMode {
    Tag,
    Comment,
    Pi,
    Doctype,
}

fn take_name(input: &str) -> Option<(&str, &str)> {
    let end = input
        .char_indices()
        .find(|(_, c)| !names::is_name_char(*c))
        .map_or(input.len(), |(index, _)| index);
    let name = &input[..end];
    is_name(name).then_some((name, &input[end..]))
}

fn parse_raw_attributes(
    text: &str,
    allow_refs: bool,
    allocator: Allocator,
    limit: usize,
) -> Result<Vec<(&str, &str, usize, usize)>, Error> {
    let original_len = text.len();
    let mut text = text;
    let mut result = Vec::new_in(allocator);
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
        let (name, rest) = take_name(text).ok_or(error_at(
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
        try_push(&mut result, (name, value, name_offset, value_offset))?;
        text = &rest[end + 1..];
    }
    Ok(result)
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
fn collapse_spaces(text: &str, allocator: Allocator) -> Result<String, Error> {
    let mut output = String::new_in(allocator);
    for part in text.split(' ').filter(|part| !part.is_empty()) {
        if !output.is_empty() {
            output.try_push(' ')?;
        }
        output.try_push_str(part)?;
    }
    Ok(output)
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
