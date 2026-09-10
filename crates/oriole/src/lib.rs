//! A safe, incremental XML 1.0 parser with explicit resource limits.
//!
//! Feed bytes with [`Parser::feed`], then drain owned events with
//! [`Parser::next_event`]. No borrowed parser state crosses an event boundary.
#![forbid(unsafe_code)]

mod dtd;
mod encoding;
mod names;

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

use encoding::{Decoder, Source};
use names::{is_name, is_xml_char, whitespace};

/// Bounds applied independently of input chunking.
#[derive(Clone, Debug)]
pub struct Limits {
    pub max_depth: usize,
    pub max_token_bytes: usize,
    pub max_total_bytes: usize,
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
    pub encoding: Option<String>,
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

/// An XML parse failure. A parser remains failed after returning an error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
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
    BadCharacterReference,
    BinaryEntityReference,
    ExternalEntityHandling,
    UnknownEncoding,
    IncorrectEncoding,
    UnclosedCdataSection,
    ExternalEntityInAttribute,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
    pub specified: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub kind: EventKind,
    pub position: Position,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventKind {
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
    StartDoctype {
        name: String,
        system_id: Option<String>,
        public_id: Option<String>,
        has_internal_subset: bool,
    },
    EndDoctype,
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

#[derive(Clone, Debug)]
struct Element {
    raw_name: String,
    expanded_name: String,
    bindings: Vec<(String, Option<String>)>,
}

#[derive(Clone, Debug)]
struct Entity {
    value: Option<String>,
    notation: Option<String>,
}

#[derive(Clone, Debug)]
struct DefaultAttribute {
    name: String,
    attribute_type: String,
    value: Option<String>,
}

/// An incremental, non-validating XML 1.0 parser.
///
/// External entities are never fetched. Referencing one produces an explicit error.
/// Parameter entity references are unsupported and rejected, including in DTDs.
#[derive(Debug)]
pub struct Parser {
    config: Config,
    decoder: Decoder,
    sources: Vec<Source>,
    pending: VecDeque<Event>,
    stack: Vec<Element>,
    namespaces: HashMap<String, String>,
    entities: HashMap<String, Entity>,
    defaults: HashMap<String, Vec<DefaultAttribute>>,
    seen_root: bool,
    closed_root: bool,
    seen_doctype: bool,
    declaration_allowed: bool,
    in_cdata: bool,
    final_input: bool,
    finished: bool,
    error: Option<Error>,
    received: usize,
    expanded: usize,
    last_position: Position,
    current_raw: String,
    expand_internal_entities: bool,
    decoding_error: Option<(ErrorKind, &'static str)>,
    id_attribute_index: Option<usize>,
}

impl Parser {
    #[must_use]
    pub fn new(config: Config) -> Self {
        let decoder = Decoder::new(config.encoding.as_deref());
        let mut namespaces = HashMap::new();
        namespaces.insert("xml".into(), "http://www.w3.org/XML/1998/namespace".into());
        Self {
            config,
            decoder,
            sources: vec![Source::new()],
            pending: VecDeque::new(),
            stack: Vec::new(),
            namespaces,
            entities: HashMap::new(),
            defaults: HashMap::new(),
            seen_root: false,
            closed_root: false,
            seen_doctype: false,
            declaration_allowed: true,
            in_cdata: false,
            final_input: false,
            finished: false,
            error: None,
            received: 0,
            expanded: 0,
            last_position: Position {
                line: 1,
                ..Position::default()
            },
            current_raw: String::new(),
            expand_internal_entities: true,
            decoding_error: None,
            id_attribute_index: None,
        }
    }

    /// Append input without calling user code. Drain events before feeding more data.
    pub fn feed(&mut self, bytes: &[u8], is_final: bool) -> Result<(), Error> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if self.final_input {
            return self.fail(ErrorKind::Finished, "input has already been finalized");
        }
        self.received = match self.received.checked_add(bytes.len()) {
            Some(size) if size <= self.config.limits.max_total_bytes => size,
            _ => return self.fail(ErrorKind::LimitExceeded, "input byte limit exceeded"),
        };
        self.final_input = is_final;
        if self.decoding_error.is_some() {
            return Ok(());
        }
        if let Err((kind, message)) = self.decoder.feed(
            bytes,
            is_final,
            &mut self.sources[0],
            self.config.limits.max_token_bytes,
        ) {
            self.decoding_error = Some((kind, message));
        }
        Ok(())
    }

    /// Return the next event, or `None` when more input is needed or parsing is done.
    pub fn next_event(&mut self) -> Result<Option<Event>, Error> {
        if let Some(event) = self.pending.pop_front() {
            self.last_position = event.position;
            return Ok(Some(event));
        }
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let mut result = self.next_event_inner();
        if let (Err(error), Some((kind, message))) = (&result, self.decoding_error)
            && matches!(
                error.kind,
                ErrorKind::UnclosedToken | ErrorKind::NoElements | ErrorKind::UnclosedCdataSection
            )
        {
            let source = &self.sources[0];
            result = Err(Error {
                kind,
                message: message.into(),
                position: source.end_position(),
            });
        }
        if let Err(error) = &result {
            self.error = Some(error.clone());
            let prefixes: Vec<_> = self
                .pending
                .iter()
                .filter_map(|event| match &event.kind {
                    EventKind::StartNamespace { prefix, .. } => Some(prefix.clone()),
                    _ => None,
                })
                .collect();
            let position = error.position;
            for prefix in prefixes.into_iter().rev() {
                self.emit(EventKind::EndNamespace { prefix }, position);
            }
            if let Some(event) = self.pending.pop_front() {
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
    pub fn set_expand_internal_entities(&mut self, enabled: bool) {
        self.expand_internal_entities = enabled;
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
    fn err(&self, kind: ErrorKind, message: impl Into<String>) -> Error {
        Error {
            kind,
            message: message.into(),
            position: self.here(),
        }
    }
    fn fail<T>(&mut self, kind: ErrorKind, message: impl Into<String>) -> Result<T, Error> {
        let error = self.err(kind, message);
        self.error = Some(error.clone());
        Err(error)
    }
    fn emit(&mut self, kind: EventKind, position: Position) {
        self.pending.push_back(Event { kind, position });
    }
    fn consume(&mut self, count: usize) {
        self.source_mut().consume(count);
    }
    fn is_source_final(&self) -> bool {
        self.sources.len() > 1 || self.final_input || self.decoding_error.is_some()
    }

    fn next_event_inner(&mut self) -> Result<Option<Event>, Error> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }
            if self.finished {
                return Ok(None);
            }
            if self.source().remaining().is_empty() {
                if self.sources.len() > 1 {
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
                    return Err(self.err(kind, message));
                }
                if !self.final_input {
                    return Ok(None);
                }
                self.last_position = self.here();
                if self.in_cdata {
                    return Err(self.err(ErrorKind::UnclosedCdataSection, "unclosed CDATA section"));
                }
                if !self.seen_root {
                    return Err(self.err(ErrorKind::NoElements, "document contains no element"));
                }
                if !self.stack.is_empty() {
                    return Err(self.err(ErrorKind::NoElements, "unclosed element"));
                }
                self.finished = true;
                return Ok(None);
            }
            if self.in_cdata {
                if !self.parse_cdata()? {
                    return Ok(None);
                }
                continue;
            }
            let first = self.source().remaining().as_bytes()[0];
            if first == b'&' {
                if self.stack.is_empty() {
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
            let mode = if remaining.starts_with("<!--") {
                ScanMode::Comment
            } else if remaining.starts_with("<![CDATA[") {
                if self.stack.is_empty() {
                    return Err(self.err(ErrorKind::Syntax, "CDATA outside the document element"));
                }
                let position = self.source().position(9);
                self.current_raw = "<![CDATA[".into();
                self.consume(9);
                self.in_cdata = true;
                self.emit(EventKind::StartCdata, position);
                continue;
            } else if remaining.starts_with("<?") {
                ScanMode::Pi
            } else if remaining.starts_with("<!DOCTYPE") {
                ScanMode::Doctype
            } else if remaining.starts_with("</") {
                ScanMode::Tag
            } else if remaining.len() == 1
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
            let final_input = self.is_source_final();
            let max_token = self.config.limits.max_token_bytes;
            let end = self
                .source_mut()
                .scan_token(mode, max_token)
                .map_err(|kind| self.err(kind, "XML token byte limit exceeded"))?;
            let Some(end) = end else {
                if final_input {
                    return Err(self.err(ErrorKind::UnclosedToken, "unclosed XML token"));
                }
                return Ok(None);
            };
            let position = self.source().position(end);
            let token = self.source().remaining()[..end].to_owned();
            self.current_raw.clone_from(&token);
            validate_chars(&token).map_err(|kind| self.err(kind, "invalid XML character"))?;
            match mode {
                ScanMode::Comment => {
                    let text = &token[4..token.len() - 3];
                    if text.contains("--") || text.ends_with('-') {
                        return Err(self.err(ErrorKind::InvalidToken, "double hyphen in comment"));
                    }
                    self.declaration_allowed = false;
                    self.emit(EventKind::Comment(normalize_newlines(text)), position);
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
        let text = self.source().remaining();
        let mut end = text
            .bytes()
            .position(|byte| byte == b'<' || byte == b'&')
            .unwrap_or(text.len());
        if end == text.len() && !self.is_source_final() {
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
        let text = &text[..end];
        validate_chars(text).map_err(|kind| self.err(kind, "invalid XML character"))?;
        if text.contains("]]>") {
            return Err(self.err(
                ErrorKind::InvalidToken,
                "CDATA terminator in character data",
            ));
        }
        if self.stack.is_empty() && !text.chars().all(whitespace) {
            return Err(self.err(
                if self.closed_root {
                    ErrorKind::JunkAfterDocumentElement
                } else {
                    ErrorKind::Syntax
                },
                "character data outside the document element",
            ));
        }
        let position = self.source().position(end);
        let value = normalize_newlines(text);
        self.current_raw = text.to_owned();
        self.declaration_allowed = false;
        if !self.stack.is_empty() {
            self.emit(EventKind::Text(value), position);
        }
        self.consume(end);
        Ok(true)
    }

    fn parse_cdata(&mut self) -> Result<bool, Error> {
        let text = self.source().remaining();
        if text.starts_with("]]>") {
            let position = self.source().position(3);
            self.current_raw = "]]>".into();
            self.consume(3);
            self.in_cdata = false;
            self.emit(EventKind::EndCdata, position);
            return Ok(true);
        }
        let end = if let Some(end) = text.find("]]>") {
            end
        } else if self.is_source_final() {
            text.len()
        } else {
            let mut end = text.len();
            if text.ends_with('\r') {
                end -= 1;
            }
            while end > 0 && end + 2 >= text.len() && text.as_bytes()[end - 1] == b']' {
                end -= 1;
            }
            end
        };
        if end == 0 {
            return Ok(false);
        }
        let text = &text[..end];
        validate_chars(text).map_err(|kind| self.err(kind, "invalid XML character"))?;
        let position = self.source().position(end);
        let value = normalize_newlines(text);
        self.current_raw = text.to_owned();
        self.consume(end);
        self.emit(EventKind::Text(value), position);
        Ok(true)
    }

    fn parse_reference(&mut self) -> Result<bool, Error> {
        let limit = self.config.limits.max_token_bytes;
        let end = self
            .source_mut()
            .scan_reference(limit)
            .map_err(|kind| self.err(kind, "entity reference byte limit exceeded"))?;
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
        let name = text[1..end].to_owned();
        let position = self.source().position(end + 1);
        self.current_raw = text[..end + 1].to_owned();
        if let Some(character) = character_reference(&name)
            .map_err(|kind| self.err(kind, "invalid character reference"))?
        {
            self.consume(end + 1);
            self.emit(EventKind::Text(character.to_string()), position);
            return Ok(true);
        }
        if !is_name(&name) {
            return Err(self.err(ErrorKind::InvalidToken, "invalid entity name"));
        }
        let entity = self.entities.get(&name).ok_or_else(|| {
            self.err(
                ErrorKind::UndefinedEntity,
                format!("undefined entity: {name}"),
            )
        })?;
        if entity.notation.is_some() {
            return Err(self.err(
                ErrorKind::BinaryEntityReference,
                "unparsed entity in content",
            ));
        }
        let value = entity
            .value
            .as_ref()
            .ok_or_else(|| {
                self.err(
                    ErrorKind::ExternalEntityHandling,
                    "external entity resolution is not supported",
                )
            })?
            .clone();
        if !self.expand_internal_entities {
            self.consume(end + 1);
            self.emit(
                EventKind::SkippedEntity {
                    name,
                    parameter: false,
                },
                position,
            );
            return Ok(true);
        }
        if self
            .sources
            .iter()
            .any(|source| source.entity_name.as_deref() == Some(&name))
        {
            return Err(self.err(
                ErrorKind::RecursiveEntityReference,
                "recursive entity reference",
            ));
        }
        if self.sources.len() > self.config.limits.max_entity_depth {
            return Err(self.err(ErrorKind::LimitExceeded, "entity nesting limit exceeded"));
        }
        self.charge_expansion(value.len())?;
        self.consume(end + 1);
        self.sources
            .push(Source::entity(value, name, position, self.stack.len()));
        Ok(true)
    }

    fn parse_pi(&mut self, token: &str, position: Position) -> Result<(), Error> {
        let body = &token[2..token.len() - 2];
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
            let attrs = parse_raw_attributes(rest, false)
                .map_err(|(kind, message)| self.err(kind, message))?;
            if attrs.is_empty()
                || attrs[0].0 != "version"
                || !matches!(attrs[0].1.as_str(), "1.0" | "1.1")
            {
                return Err(self.err(
                    ErrorKind::XmlDeclaration,
                    "XML declaration must begin with a version",
                ));
            }
            let version = attrs[0].1.clone();
            if version != "1.0" {
                return Err(self.err(ErrorKind::XmlDeclaration, "only XML 1.0 is supported"));
            }
            let mut encoding = None;
            let mut standalone = None;
            for (name, value) in attrs.into_iter().skip(1) {
                match name.as_str() {
                    "encoding" if encoding.is_none() && standalone.is_none() => {
                        if !valid_encoding_name(&value) {
                            return Err(
                                self.err(ErrorKind::XmlDeclaration, "invalid encoding name")
                            );
                        }
                        self.decoder
                            .check_declaration(&value)
                            .map_err(|(kind, message)| self.err(kind, message))?;
                        encoding = Some(value);
                    }
                    "standalone" if standalone.is_none() => {
                        standalone = Some(match value.as_str() {
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
            self.emit(
                EventKind::XmlDeclaration {
                    version,
                    encoding,
                    standalone,
                },
                position,
            );
        } else {
            if self.config.namespace_separator.is_some() && target.contains(':') {
                return Err(self.err(
                    ErrorKind::InvalidToken,
                    "colon in processing instruction target",
                ));
            }
            self.emit(
                EventKind::ProcessingInstruction {
                    target: target.to_owned(),
                    data: normalize_newlines(rest.trim_start_matches(whitespace)),
                },
                position,
            );
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
            .ok_or_else(|| self.err(ErrorKind::InvalidToken, "invalid element name"))?;
        let raw_attrs =
            parse_raw_attributes(rest, true).map_err(|(kind, message)| self.err(kind, message))?;
        if raw_attrs.len() > self.config.limits.max_attributes {
            return Err(self.err(ErrorKind::LimitExceeded, "attribute count limit exceeded"));
        }
        let mut attrs = Vec::with_capacity(raw_attrs.len());
        let mut names = HashSet::new();
        for (attr_name, value) in raw_attrs {
            if !names.insert(attr_name.clone()) {
                return Err(self.err(ErrorKind::DuplicateAttribute, "duplicate attribute"));
            }
            let mut value = self.expand_attribute(&value, &mut Vec::new())?;
            if self
                .defaults
                .get(name)
                .and_then(|decls| decls.iter().find(|decl| decl.name == attr_name))
                .is_some_and(|decl| decl.attribute_type != "CDATA")
            {
                value = collapse_spaces(&value);
            }
            attrs.push(Attribute {
                name: attr_name,
                value,
                specified: true,
            });
        }
        if let Some(defaults) = self.defaults.get(name).cloned() {
            for default in defaults {
                if !names.contains(&default.name)
                    && let Some(value) = default.value
                {
                    if attrs.len() >= self.config.limits.max_attributes {
                        return Err(
                            self.err(ErrorKind::LimitExceeded, "attribute count limit exceeded")
                        );
                    }
                    attrs.push(Attribute {
                        name: default.name,
                        value,
                        specified: false,
                    });
                }
            }
        }
        self.id_attribute_index = attrs.iter().position(|attribute| {
            self.defaults.get(name).is_some_and(|declarations| {
                declarations.iter().any(|declaration| {
                    declaration.name == attribute.name && declaration.attribute_type == "ID"
                })
            })
        });
        let mut bindings = Vec::new();
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
                        self.namespaces.insert(prefix.to_owned(), uri.clone())
                    };
                    bindings.push((prefix.to_owned(), previous));
                    self.emit(
                        EventKind::StartNamespace {
                            prefix: (!prefix.is_empty()).then(|| prefix.to_owned()),
                            uri: (!uri.is_empty()).then(|| uri.clone()),
                        },
                        position,
                    );
                }
            }
            let id_name = self
                .id_attribute_index
                .map(|index| attrs[index].name.clone());
            attrs.retain(|attr| attr.name != "xmlns" && !attr.name.starts_with("xmlns:"));
            self.id_attribute_index =
                id_name.and_then(|name| attrs.iter().position(|attribute| attribute.name == name));
            let mut expanded = HashSet::new();
            for attr in &mut attrs {
                let key = self.expand_name(&attr.name, true, false)?;
                if !expanded.insert(key) {
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
        self.stack.push(Element {
            raw_name: name.to_owned(),
            expanded_name: expanded_name.clone(),
            bindings,
        });
        self.emit(
            EventKind::StartElement {
                name: expanded_name,
                attributes: attrs,
            },
            position,
        );
        if empty {
            self.end_element(position)?;
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
            return Err(self.err(ErrorKind::TagMismatch, "mismatched end tag"));
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
        );
        for (prefix, previous) in element.bindings.into_iter().rev() {
            match previous {
                Some(uri) => {
                    self.namespaces.insert(prefix.clone(), uri);
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
            );
        }
        if self.stack.is_empty() {
            self.closed_root = true;
        }
        Ok(())
    }

    fn expand_name(&self, name: &str, attribute: bool, triplets: bool) -> Result<String, Error> {
        let Some(separator) = self.config.namespace_separator else {
            return Ok(name.to_owned());
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
        let uri = match prefix {
            Some("xmlns") => {
                return Err(self.err(
                    ErrorKind::ReservedPrefixXmlns,
                    "xmlns cannot prefix an element or ordinary attribute",
                ));
            }
            Some(prefix) => Some(self.namespaces.get(prefix).ok_or_else(|| {
                self.err(
                    ErrorKind::UndefinedPrefix,
                    format!("unbound namespace prefix: {prefix}"),
                )
            })?),
            None if !attribute => self.namespaces.get(""),
            None => None,
        };
        let Some(uri) = uri else {
            return Ok(local.to_owned());
        };
        let mut result =
            String::with_capacity(uri.len() + local.len() + prefix.map_or(2, |p| p.len() + 2));
        result.push_str(uri);
        if separator != '\0' {
            result.push(separator);
        }
        result.push_str(local);
        if triplets && let Some(prefix) = prefix {
            if separator != '\0' {
                result.push(separator);
            }
            result.push_str(prefix);
        }
        Ok(result)
    }

    fn charge_expansion(&mut self, size: usize) -> Result<(), Error> {
        self.expanded = self
            .expanded
            .checked_add(size)
            .filter(|size| *size <= self.config.limits.max_entity_expansion_bytes)
            .ok_or_else(|| {
                self.err(
                    ErrorKind::LimitExceeded,
                    "entity expansion byte limit exceeded",
                )
            })?;
        Ok(())
    }

    fn expand_attribute(&mut self, value: &str, chain: &mut Vec<String>) -> Result<String, Error> {
        let mut output = String::with_capacity(value.len());
        let mut rest = value;
        while !rest.is_empty() {
            let end = rest.find(['&', '<']).unwrap_or(rest.len());
            output.push_str(&normalize_attribute_whitespace(&rest[..end]));
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
                .map_err(|kind| self.err(kind, "invalid character reference"))?
            {
                output.push(character);
            } else {
                if !is_name(name) {
                    return Err(self.err(ErrorKind::InvalidToken, "invalid entity name"));
                }
                if chain.iter().any(|item| item == name) {
                    return Err(self.err(
                        ErrorKind::RecursiveEntityReference,
                        "recursive entity in attribute",
                    ));
                }
                if chain.len() >= self.config.limits.max_entity_depth {
                    return Err(self.err(ErrorKind::LimitExceeded, "entity nesting limit exceeded"));
                }
                let entity = self.entities.get(name).ok_or_else(|| {
                    self.err(ErrorKind::UndefinedEntity, "undefined entity in attribute")
                })?;
                let value = entity
                    .value
                    .as_ref()
                    .ok_or_else(|| {
                        self.err(
                            ErrorKind::ExternalEntityInAttribute,
                            "external entity in attribute",
                        )
                    })?
                    .clone();
                self.charge_expansion(value.len())?;
                chain.push(name.to_owned());
                output.push_str(&self.expand_attribute(&value, chain)?);
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
    mut text: &str,
    allow_refs: bool,
) -> Result<Vec<(String, String)>, (ErrorKind, &'static str)> {
    let mut result = Vec::new();
    while !text.is_empty() {
        let trimmed = text.trim_start_matches(whitespace);
        if trimmed.len() == text.len() {
            return Err((
                ErrorKind::InvalidToken,
                "attributes must be separated by whitespace",
            ));
        }
        text = trimmed;
        if text.is_empty() {
            break;
        }
        let (name, rest) =
            take_name(text).ok_or((ErrorKind::InvalidToken, "invalid attribute name"))?;
        let rest = rest.trim_start_matches(whitespace);
        let rest = rest
            .strip_prefix('=')
            .ok_or((ErrorKind::InvalidToken, "attribute is missing equals sign"))?
            .trim_start_matches(whitespace);
        let quote = rest
            .chars()
            .next()
            .filter(|c| *c == '\'' || *c == '"')
            .ok_or((ErrorKind::InvalidToken, "attribute value must be quoted"))?;
        let rest = &rest[1..];
        let end = rest
            .find(quote)
            .ok_or((ErrorKind::UnclosedToken, "unclosed attribute value"))?;
        let value = &rest[..end];
        if value.contains('<') || (!allow_refs && value.contains('&')) {
            return Err((
                ErrorKind::InvalidToken,
                "invalid character in attribute value",
            ));
        }
        result.push((name.to_owned(), value.to_owned()));
        text = &rest[end + 1..];
    }
    Ok(result)
}

fn validate_chars(text: &str) -> Result<(), ErrorKind> {
    if text.chars().all(is_xml_char) {
        Ok(())
    } else {
        Err(ErrorKind::InvalidToken)
    }
}
fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}
fn normalize_attribute_whitespace(text: &str) -> String {
    normalize_newlines(text)
        .chars()
        .map(|c| if whitespace(c) { ' ' } else { c })
        .collect()
}
fn collapse_spaces(text: &str) -> String {
    text.split(' ')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn valid_encoding_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
fn character_reference(name: &str) -> Result<Option<char>, ErrorKind> {
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
            if digits.is_empty()
                || !digits.bytes().all(|c| {
                    if radix == 16 {
                        c.is_ascii_hexdigit()
                    } else {
                        c.is_ascii_digit()
                    }
                })
            {
                return Err(ErrorKind::BadCharacterReference);
            }
            let value = u32::from_str_radix(digits, radix)
                .ok()
                .and_then(char::from_u32)
                .filter(|c| is_xml_char(*c))
                .ok_or(ErrorKind::BadCharacterReference)?;
            Ok(Some(value))
        }
        _ => Ok(None),
    }
}
