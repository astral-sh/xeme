mod attlist;
mod composition;
mod grammar;
mod semantic;

use crate::lexical::{Buffer, Slice};
use crate::{
    DefaultAttribute, DefaultAttributes, Entity, Error, ErrorKind, EventKind, Parser, Position,
    character_reference, string, take_name, whitespace,
};
use oriole_storage::{Allocator, String, TryClone, Vec, try_insert, try_push};
use std::sync::atomic::Ordering;

/// A foreign subset may be declined without making unknown entities skippable.
#[derive(Debug)]
pub(crate) struct ForeignDtd {
    previous_subset: bool,
    position: Position,
    pub(crate) delivered: bool,
}

#[derive(Debug)]
pub(crate) struct ConditionalState {
    included_sources: Vec<usize>,
    ignored_depth: usize,
    ignored_source: usize,
    ignored_bytes: usize,
    header: Option<oriole_storage::Box<composition::Header>>,
    declaration: Option<oriole_storage::Box<composition::Declaration>>,
    attlist: Option<oriole_storage::Box<attlist::State>>,
}

impl ConditionalState {
    pub(crate) fn new(allocator: Allocator) -> Self {
        Self {
            included_sources: Vec::new_in(allocator),
            ignored_depth: 0,
            ignored_source: 0,
            ignored_bytes: 0,
            header: None,
            declaration: None,
            attlist: None,
        }
    }
}

/// A replacement is a distinct lexical source: references cannot span frames,
/// and only the original physical source has literal line endings normalized.
struct EntityValueFrame<'a> {
    rest: Slice<'a>,
    name: Option<&'a str>,
    normalize: bool,
}

#[derive(Debug)]
struct DeclarationLiteral {
    offset: usize,
    normalize: bool,
    parameters: Vec<String>,
}

#[derive(Debug)]
struct DeclarationExpansion {
    text: Buffer,
    raw: Buffer,
    literals: Vec<DeclarationLiteral>,
    parameters: Vec<DeclarationParameter>,
    capture_raw: bool,
    raw_offsets: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DeclarationParameter {
    offset: usize,
    position: Position,
    raw_start: usize,
    raw_end: usize,
    disabled: bool,
}

/// Semantic attribute types are always complete. Expat builds enumeration
/// callback payloads only while its ATTLIST handler is present.
enum AttributeCallback<'a> {
    Complete,
    Enumeration(Option<&'a str>),
    Captured(Option<String>),
    Silent,
}

impl Parser {
    /// Inherit active parameter names even though Expat's C context is null.
    /// A pending header prefix has not begun its external callback yet.
    pub(crate) fn inherit_parameter_context(&self, child: &mut Self) -> Result<(), Error> {
        let active = self
            .active_parameter_reference
            .as_ref()
            .and_then(|(name, _)| name.strip_prefix('%'));
        let Some(active) = active else {
            return Ok(());
        };
        let sources = self.sources.iter().filter_map(|source| {
            source
                .entity_name
                .as_deref()
                .and_then(|name| name.strip_prefix('%'))
        });
        for name in sources.chain(std::iter::once(active)) {
            self.charge_expansion(size_of::<String>() + name.len() + 1)?;
            let mut active = string("%", self.allocator)?;
            active.push_str(name)?;
            child.inherit_entity_name(active)?;
        }
        child.inherited_parameter_depth = self.inherited_parameter_depth + self.sources.len() - 1;
        Ok(())
    }

    pub(crate) fn finish_conditional_source(&self) -> Result<(), Error> {
        if self.conditional.ignored_depth != 0
            && self.conditional.ignored_source == self.sources.len()
        {
            return Err(self.err(ErrorKind::Syntax, "unclosed ignored conditional section"));
        }
        if (self.sources.len() == 1 && !self.conditional.included_sources.is_empty())
            || (!self.source().dtd_fragment
                && self.conditional.included_sources.last() == Some(&self.sources.len()))
        {
            return Err(self.err(
                ErrorKind::IncompleteParameterEntity,
                "unclosed conditional section",
            ));
        }
        Ok(())
    }

    fn conditional_raw(&mut self, count: usize) -> Result<(), Error> {
        self.account_source(count)?;
        if self.default_events {
            let position = self.source().position(count);
            self.save_current_raw(count)?;
            self.emit(EventKind::Default, position)?;
        }
        self.declaration_allowed = false;
        self.consume(count)?;
        Ok(())
    }

    fn start_conditional_section(&mut self) -> Result<bool, Error> {
        self.start_header_composition()
    }

    fn parse_ignored_section(&mut self) -> Result<bool, Error> {
        let text = self.source().remaining();
        let mut offset = 0;
        let mut depth = self.conditional.ignored_depth;
        while offset < text.len() {
            let rest = &text[offset..];
            let width = if rest.starts_with("<![") {
                if depth + self.conditional.included_sources.len() >= self.config.limits.max_depth {
                    return Err(self.err_at(
                        ErrorKind::LimitExceeded,
                        "conditional section nesting limit exceeded",
                        offset,
                    ));
                }
                depth += 1;
                3
            } else if rest.starts_with("]]>") {
                depth -= 1;
                3
            } else if !self.is_source_final()
                && ("<![".starts_with(rest) || "]]>".starts_with(rest))
            {
                break;
            } else {
                let character = rest.chars().next().expect("nonempty ignored input");
                if !crate::names::is_xml_char(character) {
                    return Err(self.err_at(
                        ErrorKind::InvalidToken,
                        "invalid XML character in ignored section",
                        offset,
                    ));
                }
                character.len_utf8()
            };
            offset += width;
            if self.conditional.ignored_bytes.saturating_add(offset)
                > self.config.limits.max_token_bytes
            {
                return Err(self.err_at(
                    ErrorKind::LimitExceeded,
                    "ignored conditional section limit exceeded",
                    offset - width,
                ));
            }
            if depth == 0 || offset >= 64 * 1024 {
                break;
            }
        }
        if offset == 0 {
            return Ok(false);
        }
        self.conditional.ignored_depth = depth;
        self.conditional.ignored_bytes += offset;
        self.conditional_raw(offset)?;
        Ok(true)
    }
}

impl Parser {
    pub(crate) fn parse_doctype(
        &mut self,
        token: Slice<'_>,
        position: Position,
    ) -> Result<(), Error> {
        if self.seen_root || self.seen_doctype || self.sources.len() > 1 || self.fragment {
            return Err(self.err(ErrorKind::Syntax, "misplaced document type declaration"));
        }
        let has_internal_subset = token.ends_with('[');
        if !token[9..].starts_with(whitespace) {
            let offset = 2 + take_name(&token[2..], self.config.name_rules)
                .map_or(0, |(name, _)| name.len());
            return Err(self.err_at(
                ErrorKind::InvalidToken,
                "whitespace required after DOCTYPE",
                offset,
            ));
        }
        let mut cursor = Cursor::new(
            token.for_slice(&token[9..token.len() - 1]),
            self.config.namespace_separator.is_some(),
            self.config.name_rules,
        );
        cursor
            .require_space()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        self.check_dtd_token(&cursor)?;
        let raw_name = cursor
            .name()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let name = cursor.lexical.for_slice(raw_name).decode(self.allocator)?;
        let spaced = cursor.space();
        let (system_id, public_id) = if cursor.starts("SYSTEM") || cursor.starts("PUBLIC") {
            if !spaced {
                return Err(self.err(ErrorKind::Syntax, "external identifier requires whitespace"));
            }
            external_id(&mut cursor, false, self.allocator, true)?
        } else {
            (None, None)
        };
        cursor.space();
        if !cursor.rest().is_empty() {
            return Err(self.err(ErrorKind::Syntax, "invalid document type declaration"));
        }
        self.seen_doctype = true;
        self.declaration_allowed = false;
        self.has_external_subset = system_id.is_some();
        self.doctype_external = Some((system_id.try_clone()?, public_id.try_clone()?));
        self.in_doctype = has_internal_subset;
        if system_id.is_some() && !self.standalone {
            self.emit(EventKind::NotStandalone, position)?;
            self.event_raw("")?;
        }
        self.emit(
            EventKind::StartDoctype(oriole_storage::try_box(
                crate::DoctypeDeclaration {
                    name,
                    system_id,
                    public_id,
                    has_internal_subset,
                },
                self.allocator,
            )?),
            position,
        )?;
        self.event_raw(
            &token
                .for_slice(if has_internal_subset {
                    &token
                } else {
                    &token[..token.len() - 1]
                })
                .decoded(self.allocator)?,
        )?;
        if !has_internal_subset {
            self.finish_doctype(position, ">")?;
        }
        Ok(())
    }

    pub(crate) fn parameter_entities_enabled(&self) -> bool {
        self.parameter_mode != 0
    }

    /// Schedule the implicit subset and retain the state to restore if a handler
    /// declines to read it. Disabled processing still records a possible subset.
    pub(crate) fn start_foreign_dtd(&mut self, position: Position) -> Result<(), Error> {
        let previous_subset = self.has_external_subset;
        self.has_external_subset = true;
        if !self.parameter_entities_enabled() {
            return Ok(());
        }
        self.shared_parameter_state()?;
        self.foreign_dtd_pending = Some(ForeignDtd {
            previous_subset,
            position,
            delivered: false,
        });
        self.emit(
            EventKind::ExternalEntityReference(oriole_storage::try_box(
                crate::ExternalEntityReference {
                    context: None,
                    system_id: None,
                    public_id: None,
                },
                self.allocator,
            )?),
            position,
        )?;
        self.event_raw("")
    }

    /// Report that no external-entity handler was installed for the last event.
    ///
    /// An absent handler leaves a possible foreign subset unresolved. A handler
    /// that runs but does not read a child instead confirms that no foreign DTD
    /// exists. C adapters call this only for the absent-handler case.
    pub fn external_entity_handler_absent(&mut self) {
        if self
            .foreign_dtd_pending
            .as_ref()
            .is_some_and(|foreign| foreign.delivered)
        {
            self.foreign_dtd_pending = None;
        }
    }

    /// Complete a foreign callback before an already queued end-doctype event.
    /// The read marker belongs to the parser family and is set by child input,
    /// including a successful empty nonfinal feed, rather than child creation.
    pub(crate) fn finish_foreign_dtd(&mut self) -> Option<crate::Event> {
        if !self
            .foreign_dtd_pending
            .as_ref()
            .is_some_and(|foreign| foreign.delivered)
        {
            return None;
        }
        let foreign = self
            .foreign_dtd_pending
            .take()
            .expect("delivered foreign DTD");
        if self
            .parameter_state
            .get()
            .is_some_and(|state| state.read.load(Ordering::Relaxed))
        {
            if !self.standalone {
                self.current_raw.clear();
                return Some(crate::Event {
                    kind: EventKind::NotStandalone,
                    position: foreign.position,
                });
            }
        } else {
            self.has_external_subset = foreign.previous_subset;
        }
        None
    }

    fn finish_doctype(&mut self, position: Position, raw: &str) -> Result<(), Error> {
        self.in_doctype = false;
        if let Some((system_id, public_id)) = self.doctype_external.take() {
            if system_id.is_none() && self.foreign_dtd {
                self.start_foreign_dtd(position)?;
            } else if system_id.is_some() && self.parameter_entities_enabled() {
                self.has_external_subset = true;
                self.emit(
                    EventKind::ExternalEntityReference(oriole_storage::try_box(
                        crate::ExternalEntityReference {
                            context: None,
                            system_id,
                            public_id,
                        },
                        self.allocator,
                    )?),
                    position,
                )?;
                self.event_raw("")?;
            }
        }
        self.foreign_dtd = false;
        self.emit(EventKind::EndDoctype, position)?;
        self.event_raw(raw)?;
        Ok(())
    }

    pub(crate) fn parse_dtd_step(&mut self) -> Result<bool, Error> {
        // An active ordinary declaration retains its complete, unconsumed
        // source token and DTD context until every callback has been drained.
        // Resume it before inspecting or consuming any bytes from that source.
        if self.has_attlist() {
            return self.continue_attlist();
        }
        if self.conditional.ignored_depth != 0 {
            return self.parse_ignored_section();
        }
        let text = self.source().remaining();
        let whitespace_len = text
            .char_indices()
            .find(|(_, character)| !whitespace(*character))
            .map_or(text.len(), |(index, _)| index);
        if whitespace_len > 0 {
            self.account_source(whitespace_len)?;
            if self.default_events {
                let position = self.source().position(whitespace_len);
                self.save_current_raw(whitespace_len)?;
                self.emit(EventKind::Default, position)?;
            }
            self.declaration_allowed = false;
            self.consume(whitespace_len)?;
            return Ok(true);
        }
        if text
            .chars()
            .next()
            .is_some_and(|character| !crate::names::is_xml_char(character))
        {
            return Err(self.err(ErrorKind::InvalidToken, "invalid XML character in DTD"));
        }
        if text.starts_with("<![") {
            return self.start_conditional_section();
        }
        if self.external_subset && text.starts_with(']') {
            if "]]>".starts_with(text) && text.len() < 3 && !self.is_source_final() {
                return Ok(false);
            }
            if !text.starts_with("]]>") || self.conditional.included_sources.is_empty() {
                return Err(self.err(
                    ErrorKind::Syntax,
                    "unexpected conditional section closing delimiter",
                ));
            }
            self.conditional.included_sources.pop();
            self.conditional_raw(3)?;
            return Ok(true);
        }
        if text.starts_with("<!")
            && text != "<!"
            && !text.starts_with("<!-")
            && text[2..]
                .chars()
                .next()
                .is_some_and(|character| !self.config.name_rules.is_name_start(character))
        {
            return Err(self.err_at(ErrorKind::InvalidToken, "invalid DTD declaration name", 2));
        }
        if text.starts_with('%') {
            return self.parse_parameter_reference();
        }
        if self.in_doctype && text.starts_with(']') {
            if self.sources.len() > 1 {
                return Err(self.err(
                    ErrorKind::InvalidToken,
                    "parameter entity closes its containing subset",
                ));
            }
            let limit = self.config.limits.max_token_bytes;
            let end = self
                .source_mut()
                .scan_token(crate::ScanMode::Tag, limit)
                .map_err(|(kind, offset)| {
                    self.err_at(kind, "invalid or oversized DTD token", offset)
                })?;
            let Some(end) = end else {
                if self.is_source_final() {
                    return Err(self.err(
                        ErrorKind::UnclosedToken,
                        "unclosed document type declaration",
                    ));
                }
                return Ok(false);
            };
            let text = self.source().remaining();
            if !text[1..end - 1].chars().all(whitespace) {
                return Err(self.err(
                    ErrorKind::Syntax,
                    "invalid internal subset closing delimiter",
                ));
            }
            let position = self.source().position(end);
            let raw = self
                .source()
                .lexical_remaining()
                .for_slice(&text[..end])
                .decode(self.allocator)?;
            self.account_source(end)?;
            self.emit(EventKind::DoctypeClosingPrefix, self.source().position(1))?;
            self.event_raw(&raw[..1])?;
            if end > 2 {
                self.emit(
                    EventKind::DoctypeClosingPrefix,
                    self.source().position_at(1, end - 2),
                )?;
                self.event_raw(&raw[1..raw.len() - 1])?;
            }
            self.finish_doctype(position, ">")?;
            self.consume(end)?;
            return Ok(true);
        }
        if ["<", "<!", "<!-"].contains(&text) && !self.is_source_final() {
            return Ok(false);
        }
        let mode = if text.starts_with("<!--") {
            crate::ScanMode::Comment
        } else if text.starts_with("<?") {
            crate::ScanMode::Pi
        } else if text.starts_with("<!") {
            crate::ScanMode::DtdDeclaration
        } else if text == "<" && !self.is_source_final() {
            return Ok(false);
        } else if text.starts_with(['\'', '"']) {
            return self.parse_prolog_literal();
        } else {
            if invalid_dtd_token(
                text,
                self.config.namespace_separator.is_some(),
                self.config.name_rules,
            )
            .is_some()
            {
                return Err(self.err(ErrorKind::InvalidToken, "invalid token in DTD"));
            }
            return Err(self.err(ErrorKind::Syntax, "unexpected text in DTD"));
        };
        let limit = self.config.limits.max_token_bytes;
        if mode == crate::ScanMode::Pi
            && text[2..]
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
        let final_input = self.is_source_final();
        if self.reparse_deferral && !final_input && self.source().should_defer(limit) {
            return Ok(false);
        }
        let end = self
            .source_mut()
            .scan_token(mode, limit)
            .map_err(|(kind, offset)| {
                self.err_at(kind, "invalid or oversized DTD token", offset)
            })?;
        let Some(end) = end else {
            if mode == crate::ScanMode::DtdDeclaration && self.sources.len() > 1 {
                if ["<!ELEMENT", "<!ATTLIST", "<!ENTITY", "<!NOTATION"]
                    .iter()
                    .any(|opener| opener.starts_with(self.source().remaining()))
                {
                    return Err(self.err(
                        ErrorKind::UnclosedToken,
                        "declaration opener crosses a parameter boundary",
                    ));
                }
                return self.start_declaration_composition();
            }
            if final_input {
                return Err(self.err(ErrorKind::UnclosedToken, "unclosed DTD declaration"));
            }
            if self.sources.len() == 1
                && self.source().position(0).byte_index == self.feed_start_byte
            {
                self.source_mut().mark_deferred();
            }
            return Ok(false);
        };
        if mode == crate::ScanMode::DtdDeclaration
            && self.source().remaining().as_bytes()[end - 1] == b'%'
        {
            return self.start_declaration_composition();
        }
        self.account_source(end)?;
        let token = self
            .source()
            .lexical_remaining()
            .for_slice(&self.source().remaining()[..end])
            .to_owned(self.allocator)?;
        if let Some(offset) = crate::names::invalid_xml_char(&token) {
            return Err(self.err_at(
                ErrorKind::InvalidToken,
                "invalid XML character in DTD",
                offset,
            ));
        }
        if token.starts_with("<!ATTLIST") && token[9..].starts_with(whitespace) {
            return self.start_attlist(token, end);
        }
        self.save_current_raw(end)?;
        self.parse_subset(token.view(), 0)?;
        self.consume(end)?;
        Ok(true)
    }

    fn parse_parameter_reference(&mut self) -> Result<bool, Error> {
        let limit = self.config.limits.max_token_bytes;
        let end = self
            .source_mut()
            .scan_reference(limit)
            .map_err(|(kind, offset)| {
                self.err_at(kind, "invalid parameter entity reference", offset)
            })?;
        let Some(end) = end else {
            if self.is_source_final() {
                return Err(self.err(
                    ErrorKind::UnclosedToken,
                    "unclosed parameter entity reference",
                ));
            }
            return Ok(false);
        };
        let raw_name = &self.source().remaining()[1..end];
        let name = self
            .source()
            .lexical_remaining()
            .for_slice(raw_name)
            .decode(self.allocator)?;
        if !self.config.name_rules.is_name(raw_name) {
            return Err(self.err(ErrorKind::InvalidToken, "invalid parameter entity name"));
        }
        if self.config.namespace_separator.is_some()
            && let Some(colon) = raw_name.find(':')
        {
            return Err(self.err_at(
                ErrorKind::InvalidToken,
                "parameter entity names cannot contain colons with namespaces enabled",
                colon + 1,
            ));
        }
        let position = self.source().position(end + 1);
        self.has_external_subset = true;
        if !self.parameter_entities_enabled() {
            self.set_declarations_skipped(!self.standalone);
            let raw = self
                .default_events
                .then(|| {
                    self.source()
                        .lexical_remaining()
                        .for_slice(&self.source().remaining()[..end + 1])
                        .decode(self.allocator)
                })
                .transpose()?;
            self.consume(end + 1)?;
            if !self.standalone {
                self.emit(EventKind::NotStandalone, position)?;
                self.event_raw("")?;
            }
            if let Some(raw) = raw {
                self.emit(EventKind::Default, position)?;
                self.pending.back_mut().expect("disabled parameter").raw = Some(raw);
            }
            return Ok(true);
        }
        let Some(entity) = self.tables.parameter_entities.get(&name) else {
            // Expat checks declaration existence only for a standalone document
            // reference outside an internal parameter-entity replacement.
            if self.standalone && !self.external_subset && self.sources.len() == 1 {
                return Err(self.err(ErrorKind::UndefinedEntity, "undefined parameter entity"));
            }
            self.set_declarations_skipped(self.declarations_skipped() || !self.standalone);
            let raw = self
                .source()
                .lexical_remaining()
                .for_slice(&self.source().remaining()[..end + 1])
                .decode(self.allocator)?;
            self.consume(end + 1)?;
            self.emit(
                EventKind::SkippedEntity {
                    name,
                    parameter: true,
                },
                position,
            )?;
            self.pending.back_mut().expect("skipped parameter").raw = Some(raw);
            return Ok(true);
        };
        let mut source_name = String::new_in(self.allocator);
        source_name.push('%')?;
        source_name.push_str(&name)?;
        if entity.is_value_open() || self.active_entities.contains(&name, true) {
            return Err(self.err(
                ErrorKind::RecursiveEntityReference,
                "recursive parameter entity",
            ));
        }
        if self.sources.len() + self.external_depth + self.inherited_parameter_depth
            > self.config.limits.max_entity_depth
        {
            return Err(self.err(
                ErrorKind::LimitExceeded,
                "parameter entity nesting limit exceeded",
            ));
        }
        if let Some(value) = &entity.value {
            self.charge_expansion(value.len())?;
            let value = value.try_clone()?;
            self.consume(end + 1)?;
            self.push_entity_source(crate::encoding::Source::entity(
                value,
                source_name,
                position,
                self.stack.len(),
                self.config.name_rules,
            ))?;
        } else {
            self.charge_external_identifiers(entity)?;
            let system_id = entity.system_id.try_clone()?;
            let public_id = entity.public_id.try_clone()?;
            self.consume(end + 1)?;
            let mut raw = source_name.try_clone()?;
            raw.push(';')?;
            self.active_parameter_reference = Some((source_name, position));
            self.shared_parameter_state()?;
            self.parameter_state
                .get()
                .expect("parameter read marker")
                .read
                .store(false, Ordering::Relaxed);
            self.emit(
                EventKind::ExternalEntityReference(oriole_storage::try_box(
                    crate::ExternalEntityReference {
                        context: None,
                        system_id,
                        public_id,
                    },
                    self.allocator,
                )?),
                position,
            )?;
            self.pending.back_mut().expect("external parameter").raw = Some(raw);
        }
        Ok(true)
    }

    fn append_declaration_token(
        &self,
        result: &mut DeclarationExpansion,
        text: Slice<'_>,
        boundary: bool,
    ) -> Result<(), Error> {
        if result.text.len().saturating_add(text.len()) > self.config.limits.max_token_bytes {
            return Err(self.err(
                ErrorKind::LimitExceeded,
                "expanded declaration token limit exceeded",
            ));
        }
        if text
            .chars()
            .any(|character| !crate::names::is_xml_char(character))
        {
            return Err(self.err(
                ErrorKind::InvalidToken,
                "invalid declaration replacement character",
            ));
        }
        result.text.append(text)?;
        if !boundary && result.capture_raw {
            result.raw.append(text)?;
        } else if boundary {
            self.declaration_raw_boundary(result)?;
        }
        Ok(())
    }

    /// Record only changes in grammar/raw displacement, never one entry per feed.
    fn declaration_raw_boundary(&self, expansion: &mut DeclarationExpansion) -> Result<(), Error> {
        let pair = (expansion.text.len(), expansion.raw.len());
        if let Some(last) = expansion.raw_offsets.last_mut()
            && last.0 == pair.0
        {
            *last = pair;
        } else {
            self.charge_expansion(size_of::<(usize, usize)>())?;
            try_push(&mut expansion.raw_offsets, pair)?;
        }
        Ok(())
    }

    fn declaration_parameters(&mut self, cursor: &mut Cursor<'_>) -> Result<(), Error> {
        let offset = cursor.offset();
        self.declaration_parameters_through(cursor, offset)
    }

    fn declaration_parameters_through(
        &mut self,
        cursor: &mut Cursor<'_>,
        offset: usize,
    ) -> Result<(), Error> {
        while let Some(&parameter) = cursor.parameters.get(cursor.parameter_index) {
            if parameter.offset > offset {
                break;
            }
            cursor.parameter_index += 1;
            if self.default_events && cursor.parameter_defaults {
                self.declaration_default_segment(
                    cursor,
                    parameter.raw_start,
                    false,
                    self.declarations_skipped(),
                    parameter.position,
                )?;
            }
            if parameter.disabled && !self.standalone {
                self.emit(EventKind::NotStandalone, parameter.position)?;
                self.event_raw("")?;
            }
            if self.default_events && cursor.parameter_defaults {
                let end = if self.standalone {
                    parameter.raw_end
                } else {
                    cursor
                        .parameters
                        .get(cursor.parameter_index)
                        .map_or(cursor.raw.len(), |next| next.raw_start)
                };
                self.declaration_default_segment(
                    cursor,
                    end,
                    cursor.closes_declaration
                        && !self.standalone
                        && cursor.parameter_index == cursor.parameters.len(),
                    true,
                    parameter.position,
                )?;
            }
            self.has_external_subset = true;
            self.set_declarations_skipped(self.declarations_skipped() || !self.standalone);
        }
        Ok(())
    }

    /// Assign each raw byte once, keeping skipped references between the
    /// declaration callbacks before and after them. Handler-dependent prefixes
    /// are evaluated by the consumer when their events are dispatched.
    fn declaration_default_segment(
        &mut self,
        cursor: &mut Cursor<'_>,
        end: usize,
        closing: bool,
        unconditional: bool,
        position: Position,
    ) -> Result<(), Error> {
        if end < cursor.raw_offset {
            return Ok(());
        }
        let mut raw = String::new_in(self.allocator);
        if !cursor.raw_started {
            raw.try_push_str("<!")?;
            cursor.raw_started = true;
        }
        raw.try_push_str(
            &cursor
                .raw
                .for_slice(&cursor.raw[cursor.raw_offset..end])
                .decoded(self.allocator)?,
        )?;
        cursor.raw_offset = end;
        if closing && !cursor.raw_closed {
            raw.try_push('>')?;
            cursor.raw_closed = true;
        }
        if raw.is_empty() {
            return Ok(());
        }
        let mut assigned = false;
        if !unconditional {
            for pending in self.pending.iter_mut().skip(cursor.raw_event) {
                if pending.raw.is_none()
                    && matches!(
                        pending.event.kind,
                        EventKind::EntityDeclaration(_)
                            | EventKind::AttlistDeclaration(_)
                            | EventKind::EntityDeclarationDuplicate { .. }
                            | EventKind::ElementDeclaration { .. }
                            | EventKind::NotationDeclaration(_)
                    )
                {
                    pending.raw = Some(if assigned {
                        String::new_in(self.allocator)
                    } else {
                        assigned = true;
                        std::mem::replace(&mut raw, String::new_in(self.allocator))
                    });
                }
            }
        }
        if !assigned && !cursor.silent_defaults {
            self.charge_expansion(size_of::<crate::PendingEvent>())?;
            self.emit(
                if unconditional {
                    EventKind::Default
                } else if let Some((external, unparsed)) = cursor.duplicate_defaults {
                    EventKind::EntityDeclarationDuplicate { external, unparsed }
                } else {
                    match cursor.projection {
                        Some(grammar::Kind::Element) => EventKind::ElementDeclarationPrefix,
                        Some(grammar::Kind::Notation) => EventKind::NotationDeclarationPrefix,
                        _ if cursor.entity_defaults => EventKind::EntityDeclarationPrefix,
                        _ => EventKind::AttlistDeclarationPrefix,
                    }
                },
                position,
            )?;
            self.pending.back_mut().expect("default fragment").raw = Some(raw);
        }
        cursor.raw_event = self.pending.len();
        Ok(())
    }

    fn parse_subset(&mut self, text: Slice<'_>, base_offset: usize) -> Result<(), Error> {
        self.parse_subset_expanded(text, base_offset, None)
    }

    fn parse_subset_expanded(
        &mut self,
        lexical: Slice<'_>,
        base_offset: usize,
        mut prepared: Option<DeclarationExpansion>,
    ) -> Result<(), Error> {
        let mut text = &*lexical;
        let initial_len = text.len();
        while !text.is_empty() {
            text = text.trim_start_matches(whitespace);
            if text.is_empty() {
                break;
            }
            let offset = base_offset + initial_len - text.len();
            let position = self.source().position_at(offset, 0);
            if text.starts_with('%') {
                return Err(self.err(
                    ErrorKind::ExternalEntityHandling,
                    "parameter entity references are not supported",
                ));
            }
            if let Some(rest) = text.strip_prefix("<!--") {
                let end = rest
                    .find("-->")
                    .ok_or_else(|| self.err(ErrorKind::UnclosedToken, "unclosed DTD comment"))?;
                let value = &rest[..end];
                if value.contains("--") || value.ends_with('-') {
                    return Err(self.err(ErrorKind::InvalidToken, "double hyphen in DTD comment"));
                }
                self.emit(
                    EventKind::Comment(self.markup_text(lexical.for_slice(value))?),
                    position,
                )?;
                self.event_raw(
                    &lexical
                        .for_slice(&text[..end + 7])
                        .decoded(self.allocator)?,
                )?;
                text = &rest[end + 3..];
                continue;
            }
            if text.starts_with("<?") {
                let end = text[2..].find("?>").map(|index| index + 2).ok_or_else(|| {
                    self.err(
                        ErrorKind::UnclosedToken,
                        "unclosed DTD processing instruction",
                    )
                })?;
                self.parse_pi(lexical.for_slice(&text[..end + 2]), position)?;
                self.event_raw(
                    &lexical
                        .for_slice(&text[..end + 2])
                        .decoded(self.allocator)?,
                )?;
                text = &text[end + 2..];
                continue;
            }
            if !text.starts_with("<!") {
                return Err(self.err(ErrorKind::Syntax, "unexpected text in internal subset"));
            }
            let mut quote = None;
            let mut end = None;
            for (index, character) in text.char_indices() {
                if let Some(quoted) = quote {
                    if character == quoted {
                        quote = None;
                    }
                } else if matches!(character, '\'' | '"') {
                    quote = Some(character);
                } else if character == '>' {
                    end = Some(index);
                    break;
                }
            }
            let end =
                end.ok_or_else(|| self.err(ErrorKind::UnclosedToken, "unclosed DTD declaration"))?;
            self.declaration_allowed = false;
            let first_event = self.pending.len();
            let previously_skipped = self.declarations_skipped();
            let expansion = prepared.take();
            let grammar = expansion
                .as_ref()
                .map_or(lexical.for_slice(&text[2..end]), |value| value.text.view());
            let mut cursor = Cursor::new(
                grammar,
                self.config.namespace_separator.is_some(),
                self.config.name_rules,
            );
            cursor.raw = grammar;
            cursor.raw_event = first_event;
            if let Some(expansion) = &expansion {
                cursor.literals = &expansion.literals;
                cursor.parameters = &expansion.parameters;
                cursor.raw = expansion.raw.view();
            }
            let declaration = cursor
                .name()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            cursor.parameter_defaults =
                !previously_skipped && matches!(declaration, "ENTITY" | "ATTLIST");
            cursor.entity_defaults = declaration == "ENTITY";
            cursor
                .require_space()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            match declaration {
                "ENTITY" => self.entity_declaration(&mut cursor, position)?,
                "ATTLIST" => self.attlist_declaration(&mut cursor, position)?,
                "ELEMENT" => self.element_declaration(&mut cursor, position, offset + 2)?,
                "NOTATION" => self.notation_declaration(&mut cursor, position, true)?,
                _ => return Err(self.err(ErrorKind::Syntax, "unsupported DTD declaration")),
            }
            if let Some(mut state) = self.value_state.take() {
                cursor.space();
                let suffix_error = (!cursor.rest().is_empty())
                    .then(|| self.err(ErrorKind::Syntax, "unexpected text in entity declaration"));
                let raw = if let Some(expansion) = &expansion {
                    self.charge_expansion(expansion.raw.len() + 3)?;
                    let mut raw = Buffer::plain(string("<!", self.allocator)?);
                    raw.append(expansion.raw.view())?;
                    raw.push('>')?;
                    raw
                } else {
                    self.charge_expansion(end + 1)?;
                    lexical
                        .for_slice(&text[..end + 1])
                        .to_owned(self.allocator)?
                };
                let quote = raw.find(['\u{27}', '\u{22}']).expect("entity value quote");
                let declaration = state.declaration.as_mut().expect("pending declaration");
                declaration.raw = raw;
                declaration.quote = quote;
                declaration.prefix_start = if cursor.raw_started {
                    cursor.raw_offset + 2
                } else {
                    0
                };
                for parameter in &cursor.parameters[cursor.parameter_index..] {
                    if parameter.offset > cursor.offset() {
                        break;
                    }
                    self.charge_expansion(size_of::<DeclarationParameter>())?;
                    try_push(&mut declaration.tail_parameters, *parameter)?;
                }
                declaration.suffix_error = suffix_error;
                self.value_state = Some(state);
                return Ok(());
            }
            cursor.space();
            self.declaration_parameters(&mut cursor)?;
            if !cursor.rest().is_empty() {
                let rest_offset = cursor.offset();
                if cursor.rest().starts_with(['?', '*', '+'])
                    && grammar[..rest_offset].ends_with(whitespace)
                {
                    return Err(self.err_at(
                        ErrorKind::InvalidToken,
                        "misplaced DTD repetition marker",
                        offset + 2 + rest_offset.min(end - 2),
                    ));
                }
                return Err(self.err(ErrorKind::Syntax, "unexpected text in DTD declaration"));
            }
            if self.default_events && matches!(declaration, "ENTITY" | "ATTLIST") {
                let closing_default = !previously_skipped
                    && self.declarations_skipped()
                    && (cursor.duplicate_defaults.is_some()
                        || self.pending.iter().skip(first_event).any(|pending| {
                            matches!(pending.event.kind, EventKind::EntityDeclaration(_))
                        }));
                let raw_end = cursor.raw.len();
                self.declaration_default_segment(
                    &mut cursor,
                    raw_end,
                    !closing_default,
                    previously_skipped,
                    position,
                )?;
                if closing_default {
                    self.declaration_default_segment(
                        &mut cursor,
                        raw_end,
                        true,
                        true,
                        self.source().position_at(offset + end, 1),
                    )?;
                }
                text = &text[end + 1..];
                continue;
            }
            if self.declarations_skipped()
                && matches!(declaration, "ENTITY" | "ATTLIST")
                && self.default_events
                && self
                    .pending
                    .iter()
                    .skip(first_event)
                    .all(|pending| matches!(pending.event.kind, EventKind::NotStandalone))
            {
                self.emit(EventKind::Default, position)?;
            }
            let closing_default =
                !previously_skipped
                    && self.declarations_skipped()
                    && self.default_events
                    && self.pending.iter().skip(first_event).any(|pending| {
                        matches!(pending.event.kind, EventKind::EntityDeclaration(_))
                    });
            let mut first_raw = true;
            for pending in self.pending.iter_mut().skip(first_event) {
                if pending.raw.is_some() {
                    continue;
                }
                pending.raw = Some(if first_raw {
                    first_raw = false;
                    if let Some(expansion) = &expansion {
                        let mut raw = string("<!", self.allocator)?;
                        raw.try_push_str(&expansion.raw.view().decoded(self.allocator)?)?;
                        if !closing_default {
                            raw.try_push('>')?;
                        }
                        raw
                    } else {
                        lexical
                            .for_slice(&text[..end + usize::from(!closing_default)])
                            .decode(self.allocator)?
                    }
                } else {
                    String::new_in(self.allocator)
                });
            }
            if closing_default {
                self.emit(
                    EventKind::Default,
                    self.source().position_at(offset + end, 1),
                )?;
                self.event_raw(">")?;
            }
            text = &text[end + 1..];
        }
        Ok(())
    }

    /// Resume the declaration's raw projection after its value callbacks. The
    /// remaining grammar references must observe the child's standalone state.
    pub(crate) fn finish_value_raw(
        &mut self,
        raw: Slice<'_>,
        quote: usize,
        parameters: &[DeclarationParameter],
        first_event: usize,
        position: Position,
        closes_declaration: bool,
    ) -> Result<(), Error> {
        let mut cursor = Cursor::new(
            Slice::plain(""),
            self.config.namespace_separator.is_some(),
            self.config.name_rules,
        );
        cursor.raw = raw.for_slice(&raw[2..raw.len() - usize::from(closes_declaration)]);
        cursor.closes_declaration = closes_declaration;
        cursor.raw_offset = quote - 2;
        cursor.raw_started = true;
        cursor.raw_event = first_event;
        cursor.parameters = parameters;
        cursor.parameter_defaults = true;
        cursor.entity_defaults = true;
        let value_skipped = self.declarations_skipped();
        if self.default_events && value_skipped {
            let delimiter = raw.as_bytes()[quote] as char;
            let quoted_end = quote
                + 2
                + raw[quote + 1..]
                    .find(delimiter)
                    .expect("closed entity value");
            self.declaration_default_segment(&mut cursor, quoted_end - 2, false, false, position)?;
        }
        self.declaration_parameters_through(&mut cursor, usize::MAX)?;
        if self.default_events {
            let end = cursor.raw.len();
            let closing_default = self.declarations_skipped();
            self.declaration_default_segment(
                &mut cursor,
                end,
                closes_declaration && !closing_default,
                value_skipped,
                position,
            )?;
            if closes_declaration && closing_default {
                self.declaration_default_segment(&mut cursor, end, true, true, position)?;
            }
        }
        Ok(())
    }

    fn element_declaration(
        &mut self,
        cursor: &mut Cursor<'_>,
        position: Position,
        error_offset: usize,
    ) -> Result<(), Error> {
        self.check_dtd_token(cursor)?;
        let name = cursor
            .name()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let name = cursor.lexical.for_slice(name).decode(self.allocator)?;
        cursor
            .require_space()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let model_start = cursor.rest();
        parse_content_model(cursor, 0, self.config.limits.max_depth).map_err(|message| {
            if cursor.rest().starts_with(['?', '*', '+']) {
                self.err_at(
                    ErrorKind::InvalidToken,
                    message,
                    error_offset + cursor.offset(),
                )
            } else {
                self.dtd_grammar_error(cursor, message)
            }
        })?;
        let model = cursor
            .lexical
            .for_slice(&model_start[..model_start.len() - cursor.rest().len()])
            .decode(self.allocator)?;
        self.declaration_parameters(cursor)?;
        self.emit(EventKind::ElementDeclaration { name, model }, position)
    }

    fn notation_declaration(
        &mut self,
        cursor: &mut Cursor<'_>,
        position: Position,
        emit: bool,
    ) -> Result<(), Error> {
        self.check_dtd_token(cursor)?;
        let name = cursor
            .ncname()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let name = cursor.lexical.for_slice(name).decode(self.allocator)?;
        cursor
            .require_space()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let (system_id, public_id) =
            external_id(cursor, true, self.allocator, self.sources.len() == 1)
                .map_err(|error| self.err(error.kind, error.message))?;
        self.declaration_parameters(cursor)?;
        if emit {
            self.emit(
                EventKind::NotationDeclaration(oriole_storage::try_box(
                    crate::NotationDeclaration {
                        name,
                        system_id,
                        public_id,
                    },
                    self.allocator,
                )?),
                position,
            )?;
        }
        Ok(())
    }

    fn entity_declaration(
        &mut self,
        cursor: &mut Cursor<'_>,
        position: Position,
    ) -> Result<(), Error> {
        let parameter = cursor.eat("%");
        if parameter {
            cursor
                .require_space()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        }
        self.check_dtd_token(cursor)?;
        let name = cursor
            .ncname()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let name = cursor.lexical.for_slice(name).decode(self.allocator)?;
        cursor
            .require_space()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        if self.default_events
            && !self.declarations_skipped()
            && if parameter {
                self.tables.parameter_entities.contains_key(&name)
            } else {
                self.tables.entities.contains_key(&name)
            }
        {
            cursor.duplicate_defaults = Some((!cursor.starts("\"") && !cursor.starts("'"), false));
        }
        let mut skipped_parameter = false;
        let (value, system_id, public_id, notation) = if cursor.starts("\"") || cursor.starts("'") {
            let raw = cursor
                .quoted()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            self.declaration_parameters(cursor)?;
            if self.declarations_skipped() {
                return Ok(());
            }
            let declaring = (parameter && !self.tables.parameter_entities.contains_key(&name))
                .then_some(name.as_str());
            match self.entity_value(
                cursor.lexical.for_slice(raw),
                declaring,
                cursor
                    .last_literal_normalize
                    .unwrap_or(self.sources.len() == 1),
                cursor.last_literal_parameters,
            )? {
                crate::value::Build::Complete(value, skipped) => {
                    skipped_parameter = skipped;
                    (Some(value), None, None, None)
                }
                crate::value::Build::Pending(mut state) => {
                    self.charge_expansion(size_of::<crate::value::Declaration>())?;
                    state.declaration = Some(crate::value::Declaration {
                        name,
                        parameter,
                        position,
                        origin: self.external_subset || self.sources.len() > 1,
                        raw: Buffer::new_in(self.allocator),
                        quote: 0,
                        prefix_start: 0,
                        tail_parameters: Vec::new_in(self.allocator),
                        prefix_sent: false,
                        closes_declaration: true,
                        suffix_error: None,
                    });
                    self.value_state = Some(state);
                    return Ok(());
                }
            }
        } else {
            let (system_id, public_id) =
                external_id(cursor, false, self.allocator, self.sources.len() == 1)
                    .map_err(|error| self.err(error.kind, error.message))?;
            let spaced = cursor.space();
            let notation = if cursor.eat("NDATA") {
                if parameter || !spaced {
                    return Err(self.err(ErrorKind::Syntax, "invalid unparsed entity declaration"));
                }
                cursor
                    .require_space()
                    .map_err(|message| self.err(ErrorKind::Syntax, message))?;
                let notation = cursor
                    .ncname()
                    .map_err(|message| self.err(ErrorKind::Syntax, message))?;
                Some(cursor.lexical.for_slice(notation).decode(self.allocator)?)
            } else {
                None
            };
            if let Some((_, unparsed)) = &mut cursor.duplicate_defaults {
                *unparsed = notation.is_some();
            }
            (None, system_id, public_id, notation)
        };
        self.declaration_parameters(cursor)?;
        if self.declarations_skipped() {
            return Ok(());
        }
        let declaration_count = self.tables.entities.len() + self.tables.parameter_entities.len();
        let declarations = if parameter {
            &self.tables.parameter_entities
        } else {
            &self.tables.entities
        };
        if !declarations.contains_key(&name) {
            if declaration_count >= self.entity_limit() {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "entity declaration count limit exceeded",
                ));
            }
            let value_open = self.new_parameter_value_open(parameter && value.is_some())?;
            let declarations = if parameter {
                &mut self.tables.parameter_entities
            } else {
                &mut self.tables.entities
            };
            try_insert(
                declarations,
                name.try_clone()?,
                Entity {
                    value: value.try_clone()?,
                    system_id: system_id.try_clone()?,
                    public_id: public_id.try_clone()?,
                    notation: notation.try_clone()?,
                    declared_in_parameter_entity: self.external_subset || self.sources.len() > 1,
                    value_open,
                },
            )?;
            self.emit(
                EventKind::EntityDeclaration(oriole_storage::try_box(
                    crate::EntityDeclaration {
                        name,
                        value,
                        parameter,
                        system_id,
                        public_id,
                        notation,
                    },
                    self.allocator,
                )?),
                position,
            )?;
        }
        // A missing parameter stops subsequent declarations, but the current
        // value (including its suffix) is still stored and reported.
        if skipped_parameter {
            self.has_external_subset = true;
            self.set_declarations_skipped(!self.standalone);
        }
        Ok(())
    }

    /// Build an entity value without letting replacement quotes or unfinished
    /// references become part of the surrounding declaration's grammar.
    fn entity_value(
        &self,
        raw: Slice<'_>,
        declaring_parameter: Option<&str>,
        normalize: bool,
        declaration_parameters: &[String],
    ) -> Result<crate::value::Build, Error> {
        let mut value = String::try_with_capacity_in(raw.len(), self.allocator)?;
        let mut parents = Vec::new_in(self.allocator);
        let mut active = oriole_storage::HashSet::with_hasher_in(
            self.tables.parameter_entities.hasher().clone(),
            self.allocator,
        );
        let mut blockers = oriole_storage::HashSet::with_hasher_in(
            self.tables.parameter_entities.hasher().clone(),
            self.allocator,
        );
        // Allocate only when a parameter reference is actually visited. These
        // lexical provenance names can be deep even when no replacement runs.
        let mut blockers_initialized = false;
        let mut current = EntityValueFrame {
            rest: raw,
            name: None,
            normalize,
        };
        let mut skipped = false;
        loop {
            let start = current.rest.find(['&', '%']).unwrap_or(current.rest.len());
            if current.name.is_some() {
                self.account_entity_bytes(start, true)?;
            }
            self.append_entity_value(
                &mut value,
                current.rest.for_slice(&current.rest.as_str()[..start]),
                current.normalize,
            )?;
            current.rest = current.rest.for_slice(&current.rest.as_str()[start..]);
            if current.rest.is_empty() {
                if let Some(name) = current.name {
                    active.remove(name);
                }
                if let Some(parent) = parents.pop() {
                    current = parent;
                    continue;
                }
                return Ok(crate::value::Build::Complete(value, skipped));
            }
            let end = current.rest.find(';').ok_or_else(|| {
                self.err(
                    ErrorKind::InvalidToken,
                    "unclosed reference in entity value",
                )
            })?;
            if current.name.is_some() {
                self.account_entity_bytes(end + 1, true)?;
            }
            let reference = &current.rest.as_str()[1..end];
            let parameter = current.rest.starts_with('%');
            if !parameter && reference.starts_with('#') {
                let character = character_reference(reference)
                    .map_err(|(kind, _)| {
                        self.err(kind, "invalid character reference in entity value")
                    })?
                    .expect("numeric reference");
                let mut bytes = [0; 4];
                self.append_entity_value(
                    &mut value,
                    Slice::plain(character.encode_utf8(&mut bytes)),
                    false,
                )?;
                current.rest = current.rest.for_slice(&current.rest.as_str()[end + 1..]);
                continue;
            }
            if !self.config.name_rules.is_name(reference)
                || (self.config.namespace_separator.is_some() && reference.contains(':'))
            {
                return Err(self.err(ErrorKind::InvalidToken, "invalid reference in entity value"));
            }
            if !parameter {
                self.append_entity_value(
                    &mut value,
                    current.rest.for_slice(&current.rest.as_str()[..end + 1]),
                    false,
                )?;
                current.rest = current.rest.for_slice(&current.rest.as_str()[end + 1..]);
                continue;
            }
            if !self.external_subset && self.sources.len() == 1 {
                return Err(self.err(
                    ErrorKind::ParameterEntityReference,
                    "parameter reference in internal subset entity value",
                ));
            }
            let decoded_reference = current.rest.for_slice(reference).decoded(self.allocator)?;
            let reference = &*decoded_reference;
            // Charge reference work even for empty or missing replacements.
            self.charge_expansion(end + 1 + size_of::<EntityValueFrame<'_>>())?;
            if !blockers_initialized {
                for name in declaration_parameters {
                    oriole_storage::try_set_insert(&mut blockers, name.as_str())?;
                }
                blockers_initialized = true;
            }
            let entity = self.tables.parameter_entities.get(reference);
            if entity.is_some_and(Entity::is_value_open)
                || declaring_parameter == Some(reference)
                || blockers.contains(reference)
                || active.contains(reference)
                || self.active_entities.contains(reference, true)
            {
                return Err(self.err(
                    ErrorKind::RecursiveEntityReference,
                    "recursive parameter entity value",
                ));
            }
            current.rest = current.rest.for_slice(&current.rest.as_str()[end + 1..]);
            let Some((name, entity)) = self.tables.parameter_entities.get_key_value(reference)
            else {
                skipped = true;
                continue;
            };
            if parents.len()
                + self.sources.len()
                + self.external_depth
                + self.inherited_parameter_depth
                + declaration_parameters.len()
                > self.config.limits.max_entity_depth
            {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "parameter entity value nesting limit exceeded",
                ));
            }
            let Some(replacement) = entity.value.as_deref() else {
                // An entity name reserved by an unfinished declaration has no
                // replacement or external identifier yet. Value reads are empty.
                if entity.system_id.is_none() {
                    continue;
                }
                let mut frames = Vec::new_in(self.allocator);
                for frame in parents.into_iter().chain(std::iter::once(current)) {
                    self.charge_expansion(
                        size_of::<crate::value::Frame>()
                            + frame.rest.len()
                            + frame.name.map_or(0, str::len),
                    )?;
                    try_push(
                        &mut frames,
                        crate::value::Frame {
                            text: frame.rest.to_owned(self.allocator)?,
                            offset: 0,
                            name: frame
                                .name
                                .map(|name| string(name, self.allocator))
                                .transpose()?,
                            normalize: frame.normalize,
                        },
                    )?;
                }
                return self
                    .begin_external_value(
                        (value, skipped),
                        frames,
                        declaring_parameter,
                        declaration_parameters,
                        reference,
                        entity,
                    )
                    .map(crate::value::Build::Pending);
            };
            self.charge_expansion(replacement.len())?;
            oriole_storage::try_set_insert(&mut active, name.as_str())?;
            try_push(&mut parents, current)?;
            current = EntityValueFrame {
                rest: Slice::plain(replacement),
                name: Some(name.as_str()),
                normalize: false,
            };
        }
    }

    /// Append with a bound before allocating, retaining character-reference CRs
    /// in replacement frames and normalizing physical CR/CRLF exactly once.
    pub(crate) fn append_entity_value(
        &self,
        value: &mut String,
        text: Slice<'_>,
        normalize: bool,
    ) -> Result<(), Error> {
        if value.len().saturating_add(text.len()) > self.config.limits.max_token_bytes {
            return Err(self.err(
                ErrorKind::LimitExceeded,
                "expanded entity value limit exceeded",
            ));
        }
        if normalize && text.contains('\r') {
            value.push_str(&text.normalized(self.allocator)?)?;
        } else {
            value.push_str(&text.decoded(self.allocator)?)?;
        }
        Ok(())
    }

    fn attlist_declaration(
        &mut self,
        cursor: &mut Cursor<'_>,
        position: Position,
    ) -> Result<(), Error> {
        self.check_dtd_token(cursor)?;
        let element = cursor
            .name()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let element = cursor.lexical.for_slice(element).decode(self.allocator)?;
        let mut first_attribute = true;
        loop {
            let spaced = cursor.space();
            self.declaration_parameters(cursor)?;
            if cursor.rest().is_empty() {
                break;
            }
            if !spaced {
                return Err(self.err(
                    ErrorKind::Syntax,
                    "attribute declarations require whitespace",
                ));
            }
            self.attlist_attribute(
                cursor,
                position,
                &element,
                &mut first_attribute,
                AttributeCallback::Complete,
            )?;
        }
        Ok(())
    }

    /// Commit one complete attribute, preserving the same definition and callback
    /// accounting when an ATTLIST pauses at an external parameter reference.
    fn attlist_attribute(
        &mut self,
        cursor: &mut Cursor<'_>,
        position: Position,
        element: &String,
        first_attribute: &mut bool,
        callback: AttributeCallback<'_>,
    ) -> Result<(), Error> {
        self.check_dtd_token(cursor)?;
        let name = cursor
            .name()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let name = cursor.lexical.for_slice(name).decode(self.allocator)?;
        cursor
            .require_space()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let start = cursor.rest();
        if cursor.eat("NOTATION") {
            cursor
                .require_space()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            enumeration(cursor, true).map_err(|message| self.dtd_grammar_error(cursor, message))?;
        } else if cursor.starts("(") {
            enumeration(cursor, false)
                .map_err(|message| self.dtd_grammar_error(cursor, message))?;
        } else {
            self.check_dtd_token(cursor)?;
            let attribute_type = cursor
                .name()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            if !matches!(
                attribute_type,
                "CDATA"
                    | "ID"
                    | "IDREF"
                    | "IDREFS"
                    | "ENTITY"
                    | "ENTITIES"
                    | "NMTOKEN"
                    | "NMTOKENS"
            ) {
                return Err(self.err(ErrorKind::Syntax, "invalid attribute type"));
            }
        }
        let raw_type = &start[..start.len() - cursor.rest().len()];
        let mut attribute_type = String::try_with_capacity_in(raw_type.len(), self.allocator)?;
        for part in raw_type.split(whitespace) {
            attribute_type.push_str(&cursor.lexical.for_slice(part).decoded(self.allocator)?)?;
        }
        cursor
            .require_space()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        self.check_dtd_token(cursor)?;
        let mut required = cursor.eat("#REQUIRED");
        let value = if required || cursor.eat("#IMPLIED") {
            None
        } else {
            if cursor.eat("#FIXED") {
                required = true;
                cursor
                    .require_space()
                    .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            }
            let raw = cursor
                .quoted()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            self.declaration_parameters(cursor)?;
            if self.declarations_skipped() {
                return Ok(());
            }
            // Keep the lexical view: materializing proxy text here would lose
            // converted ASCII provenance. Replacement CR/LF is independent data.
            let normalize_line_endings = cursor
                .last_literal_normalize
                .unwrap_or(self.sources.len() == 1);
            let value = self.expand_attribute(
                cursor.lexical.for_slice(raw),
                attribute_type != "CDATA",
                normalize_line_endings,
            )?;
            Some(value)
        };
        self.declaration_parameters(cursor)?;
        if self.declarations_skipped() {
            return Ok(());
        }
        if !self.tables.defaults.contains_key(element) {
            try_insert(
                &mut self.tables.defaults,
                element.try_clone()?,
                DefaultAttributes::new(self.allocator, self.tables.salt),
            )?;
        }
        let attribute_limit = self.default_attribute_limit();
        let declarations = self
            .tables
            .defaults
            .get_mut(element)
            .expect("default list was inserted");
        if declarations.get(&name).is_none() {
            if declarations.ordered.len() >= attribute_limit {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "default attribute count limit exceeded",
                ));
            }
            declarations.try_insert(DefaultAttribute {
                name: name.try_clone()?,
                attribute_type: attribute_type.try_clone()?,
                value: value.try_clone()?,
            })?;
            self.tables.max_default_attributes = self
                .tables
                .max_default_attributes
                .max(declarations.ordered.len());
        }
        let silent = matches!(callback, AttributeCallback::Silent);
        let uncaptured = matches!(callback, AttributeCallback::Captured(None));
        let attribute_type = match callback {
            AttributeCallback::Complete => attribute_type,
            AttributeCallback::Silent => attribute_type,
            AttributeCallback::Enumeration(Some(value)) => {
                self.charge_expansion(value.len())?;
                string(value, self.allocator)?
            }
            AttributeCallback::Enumeration(None) => return Ok(()),
            AttributeCallback::Captured(Some(value)) => value,
            AttributeCallback::Captured(None) => attribute_type,
        };
        if *first_attribute {
            *first_attribute = false;
        } else {
            // Every later callback reuses the same input name. Charging before
            // cloning prevents a long name and many tiny declarations from
            // creating an unbounded queue of repeated callback payloads.
            self.charge_expansion(element.len())?;
        }
        if uncaptured {
            // Ordinary declarations retain Complete's repeated-name work even
            // when no enumeration members were captured for a callback.
            return Ok(());
        }
        if silent {
            // The adapter omits this unused payload, but position getters still
            // observe the same completed attribute as an emitted declaration.
            self.last_position = crate::AdapterLocation::Position(position);
            return Ok(());
        }
        self.emit(
            EventKind::AttlistDeclaration(oriole_storage::try_box(
                crate::AttributeDeclaration {
                    element: element.try_clone()?,
                    name,
                    attribute_type,
                    default: value,
                    required,
                },
                self.allocator,
            )?),
            position,
        )?;
        Ok(())
    }

    /// Keep malformed name bytes distinct from a valid token in the wrong role.
    fn dtd_grammar_error(&self, cursor: &Cursor<'_>, message: &'static str) -> Error {
        let kind =
            if invalid_dtd_token(cursor.rest(), cursor.namespaces, cursor.name_rules).is_some() {
                ErrorKind::InvalidToken
            } else {
                ErrorKind::Syntax
            };
        self.err(kind, message)
    }

    fn check_dtd_token(&self, cursor: &Cursor<'_>) -> Result<(), Error> {
        if invalid_dtd_token(cursor.rest(), cursor.namespaces, cursor.name_rules).is_some() {
            return Err(self.err(ErrorKind::InvalidToken, "invalid DTD token"));
        }
        Ok(())
    }
}

/// Distinguish a malformed prolog token from a valid token in the wrong role.
fn invalid_dtd_token(text: &str, namespaces: bool, name_rules: crate::NameRules) -> Option<usize> {
    let first = text.chars().next()?;
    let pound = first == '#';
    let start = usize::from(pound);
    let rest = &text[start..];
    if pound {
        if rest.chars().next().is_none_or(|character| {
            !name_rules.is_name_start(character) || (namespaces && character == ':')
        }) {
            return Some(start);
        }
    } else if !name_rules.is_name_char(first) {
        return (!matches!(
            first,
            '\'' | '"' | '(' | ')' | '[' | ']' | ',' | '|' | '%' | '<' | '>'
        ))
        .then_some(0);
    }
    let (end, next) = rest
        .char_indices()
        .find(|(_, character)| !name_rules.is_name_char(*character))?;
    if whitespace(next)
        || matches!(next, ')' | '>' | '%' | '|')
        || (!pound && matches!(next, ',' | '['))
    {
        return None;
    }
    if !pound
        && matches!(next, '?' | '*' | '+')
        && name_rules.is_name(&rest[..end])
        && (!namespaces || name_rules.is_qname(&rest[..end]))
    {
        return None;
    }
    Some(start + end)
}

struct Cursor<'a> {
    lexical: Slice<'a>,
    text: &'a str,
    namespaces: bool,
    name_rules: crate::NameRules,
    initial_len: usize,
    literals: &'a [DeclarationLiteral],
    literal_index: usize,
    last_literal_normalize: Option<bool>,
    last_literal_parameters: &'a [String],
    parameters: &'a [DeclarationParameter],
    parameter_index: usize,
    parameter_defaults: bool,
    entity_defaults: bool,
    projection: Option<grammar::Kind>,
    duplicate_defaults: Option<(bool, bool)>,
    raw_offset: usize,
    raw_event: usize,
    raw_started: bool,
    raw_closed: bool,
    closes_declaration: bool,
    silent_defaults: bool,
    raw: Slice<'a>,
}
impl<'a> Cursor<'a> {
    fn new(lexical: Slice<'a>, namespaces: bool, name_rules: crate::NameRules) -> Self {
        let text = lexical.as_str();
        Self {
            lexical,
            text,
            namespaces,
            name_rules,
            initial_len: text.len(),
            literals: &[],
            literal_index: 0,
            last_literal_normalize: None,
            last_literal_parameters: &[],
            parameters: &[],
            parameter_index: 0,
            parameter_defaults: false,
            entity_defaults: false,
            projection: None,
            duplicate_defaults: None,
            raw_offset: 0,
            raw_event: 0,
            raw_started: false,
            raw_closed: false,
            closes_declaration: true,
            silent_defaults: false,
            raw: Slice::plain(""),
        }
    }
    fn offset(&self) -> usize {
        self.initial_len - self.text.len()
    }
    fn rest(&self) -> &'a str {
        self.text
    }
    fn starts(&self, text: &str) -> bool {
        self.text.starts_with(text)
    }
    fn eat(&mut self, text: &str) -> bool {
        if let Some(rest) = self.text.strip_prefix(text) {
            self.text = rest;
            true
        } else {
            false
        }
    }
    fn space(&mut self) -> bool {
        let rest = self.text.trim_start_matches(whitespace);
        let changed = rest.len() != self.text.len();
        self.text = rest;
        changed
    }
    fn require_space(&mut self) -> Result<(), &'static str> {
        if self.space() {
            Ok(())
        } else {
            Err("whitespace required in DTD declaration")
        }
    }
    fn name(&mut self) -> Result<&'a str, &'static str> {
        let (name, rest) =
            take_name(self.text, self.name_rules).ok_or("name required in DTD declaration")?;
        if self.namespaces && !self.name_rules.is_qname(name) {
            return Err("invalid namespace-qualified name in DTD declaration");
        }
        self.text = rest;
        Ok(name)
    }
    fn ncname(&mut self) -> Result<&'a str, &'static str> {
        let name = self.name()?;
        if self.namespaces && name.contains(':') {
            return Err("this DTD name cannot contain a colon with namespaces enabled");
        }
        Ok(name)
    }
    fn quoted(&mut self) -> Result<&'a str, &'static str> {
        let offset = self.offset();
        while self
            .literals
            .get(self.literal_index)
            .is_some_and(|literal| literal.offset < offset)
        {
            self.literal_index += 1;
        }
        self.last_literal_normalize = self
            .literals
            .get(self.literal_index)
            .filter(|literal| literal.offset == offset)
            .map(|literal| literal.normalize);
        self.last_literal_parameters = self
            .literals
            .get(self.literal_index)
            .filter(|literal| literal.offset == offset)
            .map_or(&[], |literal| &literal.parameters);
        let quote = self
            .text
            .chars()
            .next()
            .filter(|c| matches!(c, '\'' | '"'))
            .ok_or("quoted value required")?;
        let rest = &self.text[1..];
        let end = rest.find(quote).ok_or("unclosed quoted value")?;
        self.text = &rest[end + 1..];
        Ok(&rest[..end])
    }
}

fn external_id(
    cursor: &mut Cursor<'_>,
    public_only: bool,
    allocator: Allocator,
    normalize: bool,
) -> Result<(Option<String>, Option<String>), Error> {
    let syntax = |message| Error::bare(ErrorKind::Syntax, message);
    let system_literal = |value: Slice<'_>, normalize| -> Result<String, Error> {
        if normalize {
            Ok(value.normalized(allocator)?)
        } else {
            Ok(value.decode(allocator)?)
        }
    };
    if cursor.eat("SYSTEM") {
        cursor.require_space().map_err(syntax)?;
        let value = cursor.quoted().map_err(syntax)?;
        Ok((
            Some(system_literal(
                cursor.lexical.for_slice(value),
                cursor.last_literal_normalize.unwrap_or(normalize),
            )?),
            None,
        ))
    } else if cursor.eat("PUBLIC") {
        cursor.require_space().map_err(syntax)?;
        let public = cursor.quoted().map_err(syntax)?;
        if cursor
            .lexical
            .for_slice(public)
            .invalid_public_character()
            .is_some()
        {
            return Err(Error::bare(
                ErrorKind::PublicId,
                "invalid public identifier character",
            ));
        }
        let public = cursor.lexical.for_slice(public).decoded(allocator)?;
        let mut normalized = String::new_in(allocator);
        for part in public.split_ascii_whitespace() {
            if !normalized.is_empty() {
                normalized.push(' ')?;
            }
            normalized.push_str(part)?;
        }
        let spaced = cursor.space();
        if public_only && !cursor.starts("\"") && !cursor.starts("'") {
            return Ok((None, Some(normalized)));
        }
        if !spaced {
            return Err(syntax("system identifier requires whitespace"));
        }
        let system = cursor.quoted().map_err(syntax)?;
        Ok((
            Some(system_literal(
                cursor.lexical.for_slice(system),
                cursor.last_literal_normalize.unwrap_or(normalize),
            )?),
            Some(normalized),
        ))
    } else {
        Err(syntax("external identifier requires SYSTEM or PUBLIC"))
    }
}

fn enumeration(cursor: &mut Cursor<'_>, names: bool) -> Result<(), &'static str> {
    if !cursor.eat("(") {
        return Err("enumeration requires an opening parenthesis");
    }
    loop {
        cursor.space();
        if names {
            cursor.ncname()?;
        } else {
            let end = cursor
                .rest()
                .char_indices()
                .find(|(_, c)| !cursor.name_rules.is_name_char(*c))
                .map_or(cursor.rest().len(), |(index, _)| index);
            if end == 0 {
                return Err("empty enumeration value");
            }
            cursor.text = &cursor.text[end..];
        }
        cursor.space();
        if cursor.eat(")") {
            return Ok(());
        }
        if !cursor.eat("|") {
            return Err("enumeration values require a separator");
        }
    }
}

fn parse_content_model(
    cursor: &mut Cursor<'_>,
    depth: usize,
    limit: usize,
) -> Result<(), &'static str> {
    if depth > limit.min(256) {
        return Err("DTD content model nesting limit exceeded");
    }
    if depth == 0 && (cursor.eat("EMPTY") || cursor.eat("ANY")) {
        return Ok(());
    }
    if !cursor.eat("(") {
        if depth == 0 {
            return Err("content model requires a group, EMPTY, or ANY");
        }
        cursor.name()?;
    } else {
        cursor.space();
        if cursor.eat("#PCDATA") {
            if depth != 0 {
                return Err("mixed content must appear at the top level");
            }
            cursor.space();
            let mut names = false;
            while cursor.eat("|") {
                names = true;
                cursor.space();
                cursor.name()?;
                cursor.space();
            }
            if !cursor.eat(")") {
                return Err("unclosed mixed content model");
            }
            if names && !cursor.eat("*") {
                return Err("mixed content model requires a repetition marker");
            }
            if !names {
                cursor.eat("*");
            }
            return Ok(());
        }
        parse_content_model(cursor, depth + 1, limit)?;
        cursor.space();
        let separator = if cursor.eat(",") {
            Some(",")
        } else if cursor.eat("|") {
            Some("|")
        } else {
            None
        };
        if let Some(separator) = separator {
            loop {
                cursor.space();
                parse_content_model(cursor, depth + 1, limit)?;
                cursor.space();
                if !cursor.eat(separator) {
                    break;
                }
            }
        }
        if !cursor.eat(")") {
            return Err("unclosed or inconsistent content model group");
        }
    }
    if !cursor.eat("?") && !cursor.eat("+") {
        cursor.eat("*");
    }
    Ok(())
}
