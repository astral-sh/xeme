use crate::{
    DefaultAttribute, DefaultAttributes, Entity, Error, ErrorKind, EventKind, Parser, Position,
    character_reference, collapse_spaces, normalize_newlines, string, take_name, whitespace,
};
use oriole_storage::{Allocator, Shared, String, TryClone, Vec, try_insert, try_push};
use std::sync::atomic::{AtomicBool, Ordering};

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
    header_checked: usize,
    header: Option<HeaderContinuation>,
}

impl ConditionalState {
    pub(crate) fn new(allocator: Allocator) -> Self {
        Self {
            included_sources: Vec::new_in(allocator),
            ignored_depth: 0,
            ignored_source: 0,
            ignored_bytes: 0,
            header_checked: 0,
            header: None,
        }
    }
}

#[derive(Debug)]
struct HeaderFrame {
    text: String,
    offset: usize,
    name: Option<String>,
    position: Option<Position>,
}

#[derive(Debug)]
struct HeaderExternal {
    name: String,
    system_id: Option<String>,
    public_id: Option<String>,
    position: Position,
    read: Shared<AtomicBool>,
    delivered: bool,
}

#[derive(Debug)]
struct HeaderContinuation {
    frames: Vec<HeaderFrame>,
    selected: Option<bool>,
    skipped: bool,
    raw: String,
    raw_position: Position,
    expanded_bytes: usize,
    end: usize,
    external: Option<HeaderExternal>,
}

/// A replacement is a distinct lexical source: references cannot span frames,
/// and only the original physical source has literal line endings normalized.
struct EntityValueFrame<'a> {
    rest: &'a str,
    name: Option<&'a str>,
    normalize: bool,
}

/// A declaration replacement ends at a lexical boundary, even when its grammar
/// (for example a content-model group) continues in the containing entity.
struct DeclarationFrame<'a> {
    rest: &'a str,
    name: Option<&'a str>,
    source_offset: usize,
    normalize: bool,
}

struct DeclarationLiteral {
    offset: usize,
    normalize: bool,
    parameters: Vec<String>,
}

struct DeclarationExpansion {
    text: String,
    raw: String,
    literals: Vec<DeclarationLiteral>,
    parameters: Vec<DeclarationParameter>,
    capture_raw: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DeclarationParameter {
    offset: usize,
    position: Position,
    raw_start: usize,
    raw_end: usize,
    disabled: bool,
}

impl Parser {
    /// Continue a conditional header after an external DTD callback. Each frame
    /// owns its lexical boundary, so child input never becomes parent syntax.
    fn continue_conditional_header(&mut self) -> Result<bool, Error> {
        let mut header = self.conditional.header.take().expect("pending header");
        if let Some(mut external) = header.external.take() {
            if !external.delivered {
                external.read.store(false, Ordering::Relaxed);
                let kind = EventKind::ExternalEntityReference {
                    context: None,
                    system_id: external.system_id.try_clone()?,
                    public_id: external.public_id.try_clone()?,
                };
                self.emit(kind, external.position)?;
                let mut raw = string("%", self.allocator)?;
                raw.push_str(&external.name)?;
                raw.push(';')?;
                self.pending.back_mut().expect("external event").raw = Some(raw);
                external.delivered = true;
                header.external = Some(external);
                self.conditional.header = Some(header);
                return Ok(true);
            }
            if external.read.load(Ordering::Relaxed) {
                if !self.standalone {
                    self.emit(EventKind::NotStandalone, external.position)?;
                    self.event_raw("")?;
                    self.conditional.header = Some(header);
                    return Ok(true);
                }
            } else {
                header.skipped = true;
                self.declarations_skipped |= !self.standalone;
            }
        }
        while let Some(frame) = header.frames.last_mut() {
            let rest = &frame.text[frame.offset..];
            let literal = rest.find('%').unwrap_or(rest.len());
            let word = rest[..literal].trim_matches(whitespace);
            if !word.is_empty() {
                header.selected = Some(match (word, header.selected) {
                    ("INCLUDE", None) => true,
                    ("IGNORE", None) => false,
                    _ => {
                        let leading = rest[..literal].len()
                            - rest[..literal].trim_start_matches(whitespace).len();
                        header.raw.push_str(&rest[..leading])?;
                        self.flush_conditional_header_raw(&mut header)?;
                        return Err(
                            self.err(ErrorKind::Syntax, "invalid conditional section keyword")
                        );
                    }
                });
            }
            if header.expanded_bytes.saturating_add(literal) > self.config.limits.max_token_bytes {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "expanded conditional header limit exceeded",
                ));
            }
            header.expanded_bytes += literal;
            header.raw.push_str(&rest[..literal])?;
            frame.offset += literal;
            let rest = &frame.text[frame.offset..];
            if rest.is_empty() {
                header.frames.pop();
                continue;
            }
            let end = rest.find(';').ok_or_else(|| {
                self.err(
                    ErrorKind::Syntax,
                    "unclosed conditional parameter reference",
                )
            })?;
            let name = &rest[1..end];
            if !crate::names::is_name(name)
                || (self.config.namespace_separator.is_some() && name.contains(':'))
            {
                return Err(self.err(
                    ErrorKind::InvalidToken,
                    "invalid conditional parameter name",
                ));
            }
            let position = frame
                .position
                .unwrap_or_else(|| self.source().position_at(3 + frame.offset, end + 1));
            let entity = self.parameter_entities.get(name);
            if self.parameter_mode == 0 || entity.is_none() {
                if header.expanded_bytes.saturating_add(end + 1)
                    > self.config.limits.max_token_bytes
                {
                    return Err(self.err(
                        ErrorKind::LimitExceeded,
                        "expanded conditional header limit exceeded",
                    ));
                }
                header.expanded_bytes += end + 1;
                header.skipped = true;
                self.declarations_skipped |= !self.standalone;
                // Own the reference before releasing the frame borrow to emit.
                let reference = string(&rest[..end + 1], self.allocator)?;
                frame.offset += end + 1;
                if self.parameter_mode == 0 && !self.standalone {
                    self.charge_expansion(2 * size_of::<crate::PendingEvent>())?;
                    self.flush_conditional_header_raw(&mut header)?;
                    self.emit(EventKind::NotStandalone, position)?;
                    self.event_raw("")?;
                }
                header.raw.push_str(&reference)?;
                continue;
            }
            self.charge_expansion(end + 1 + size_of::<HeaderFrame>())?;
            let name = string(name, self.allocator)?;
            frame.offset += end + 1;
            if header
                .frames
                .iter()
                .any(|frame| frame.name.as_ref() == Some(&name))
                || self.sources.iter().any(|source| {
                    source
                        .entity_name
                        .as_deref()
                        .and_then(|name| name.strip_prefix('%'))
                        == Some(name.as_str())
                })
                || self
                    .entity_chain
                    .iter()
                    .any(|active| active.strip_prefix('%') == Some(name.as_str()))
            {
                return Err(self.err(
                    ErrorKind::RecursiveEntityReference,
                    "recursive conditional parameter entity",
                ));
            }
            if header.frames.len() - 1
                + self.sources.len()
                + self.external_depth
                + self.inherited_parameter_depth
                > self.config.limits.max_entity_depth
            {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "conditional parameter nesting limit exceeded",
                ));
            }
            let entity = entity.expect("enabled declared parameter");
            if let Some(value) = &entity.value {
                self.charge_expansion(value.len())?;
                let text = value.try_clone()?;
                try_push(
                    &mut header.frames,
                    HeaderFrame {
                        text,
                        offset: 0,
                        name: Some(name),
                        position: Some(position),
                    },
                )?;
                continue;
            }
            self.charge_external_identifiers(entity)?;
            self.charge_expansion(size_of::<HeaderExternal>() + size_of::<AtomicBool>())?;
            let external = HeaderExternal {
                name,
                system_id: entity.system_id.try_clone()?,
                public_id: entity.public_id.try_clone()?,
                position,
                read: match &self.parameter_read {
                    Some(read) => read.clone(),
                    None => Shared::try_new_in(AtomicBool::new(false), self.allocator)?,
                },
                delivered: false,
            };
            self.has_external_subset = true;
            self.flush_conditional_header_raw(&mut header)?;
            header.raw_position = self.source().position_at(3 + header.frames[0].offset, 0);
            header.external = Some(external);
            self.conditional.header = Some(header);
            return Ok(true);
        }
        let Some(included) = header.selected else {
            self.flush_conditional_header_raw(&mut header)?;
            return Err(self.err(ErrorKind::Syntax, "missing conditional section keyword"));
        };
        if self.conditional.included_sources.len() >= self.config.limits.max_depth {
            return Err(self.err(
                ErrorKind::LimitExceeded,
                "conditional section nesting limit exceeded",
            ));
        }
        if included {
            try_push(&mut self.conditional.included_sources, self.sources.len())?;
        } else {
            self.conditional.ignored_depth = 1;
            self.conditional.ignored_source = self.sources.len();
            self.conditional.ignored_bytes = header.end;
        }
        if header.skipped {
            self.has_external_subset = true;
            self.declarations_skipped |= !self.standalone;
        }
        header.raw.push('[')?;
        self.flush_conditional_header_raw(&mut header)?;
        self.conditional.header_checked = 0;
        self.declaration_allowed = false;
        self.consume(header.end);
        Ok(true)
    }

    /// Queue owned raw bytes before yielding to a handler that may change handlers.
    fn flush_conditional_header_raw(
        &mut self,
        header: &mut HeaderContinuation,
    ) -> Result<(), Error> {
        let raw = std::mem::replace(&mut header.raw, String::new_in(self.allocator));
        if self.default_events && !raw.is_empty() {
            self.emit(EventKind::Default, header.raw_position)?;
            self.pending.back_mut().expect("default event").raw = Some(raw);
        }
        Ok(())
    }

    /// Inherit active parameter names even though Expat's C context is null.
    /// A pending header prefix has not begun its external callback yet.
    pub(crate) fn inherit_parameter_context(&self, child: &mut Self) -> Result<(), Error> {
        let header = self.conditional.header.as_ref();
        let external = header
            .and_then(|header| header.external.as_ref())
            .filter(|external| external.delivered);
        let active = external.map(|external| external.name.as_str()).or_else(|| {
            self.active_parameter_reference
                .as_ref()
                .and_then(|(name, _)| name.strip_prefix('%'))
        });
        let Some(active) = active else {
            return Ok(());
        };
        let frames = if external.is_some() {
            header.map(|header| &header.frames[..]).unwrap_or(&[])
        } else {
            &[]
        };
        let sources = self.sources.iter().filter_map(|source| {
            source
                .entity_name
                .as_deref()
                .and_then(|name| name.strip_prefix('%'))
        });
        for name in sources
            .chain(frames.iter().filter_map(|frame| frame.name.as_deref()))
            .chain(std::iter::once(active))
        {
            self.charge_expansion(size_of::<String>() + name.len() + 1)?;
            let mut active = string("%", self.allocator)?;
            active.push_str(name)?;
            try_push(&mut child.entity_chain, active)?;
        }
        child.inherited_parameter_depth = self.inherited_parameter_depth + self.sources.len() - 1
            + frames.iter().filter(|frame| frame.name.is_some()).count();
        if let Some(external) = external {
            child.parameter_read = Some(external.read.clone());
        }
        Ok(())
    }

    pub(crate) fn finish_conditional_source(&self) -> Result<(), Error> {
        if self.conditional.included_sources.last() == Some(&self.sources.len())
            || (self.conditional.ignored_depth != 0
                && self.conditional.ignored_source == self.sources.len())
        {
            return Err(self.err(
                if self.sources.len() > 1 || self.conditional.ignored_depth == 0 {
                    ErrorKind::IncompleteParameterEntity
                } else {
                    ErrorKind::Syntax
                },
                "conditional section is not closed in its source entity",
            ));
        }
        Ok(())
    }

    fn conditional_raw(&mut self, count: usize) -> Result<(), Error> {
        if self.default_events {
            let position = self.source().position(count);
            self.save_current_raw(count)?;
            self.emit(EventKind::Default, position)?;
        }
        self.declaration_allowed = false;
        self.consume(count);
        Ok(())
    }

    fn start_conditional_section(&mut self) -> Result<bool, Error> {
        if self.conditional.header.is_some() {
            return self.continue_conditional_header();
        }
        if !self.external_subset {
            return Err(self.err(
                ErrorKind::Syntax,
                "conditional section in the internal subset",
            ));
        }
        let text = self.source().remaining();
        let checked = self.conditional.header_checked.max(3);
        let limit = self.config.limits.max_token_bytes;
        let mut end = None;
        for (offset, character) in text[checked..].char_indices() {
            let offset = checked + offset;
            if offset >= limit {
                return Err(self.err_at(
                    ErrorKind::LimitExceeded,
                    "conditional header limit exceeded",
                    offset,
                ));
            }
            if character == '[' {
                end = Some(offset + 1);
                break;
            }
            if !crate::names::is_xml_char(character) {
                return Err(self.err_at(
                    ErrorKind::InvalidToken,
                    "invalid conditional header character",
                    offset,
                ));
            }
        }
        let Some(end) = end else {
            self.conditional.header_checked = text.len();
            if self.is_source_final() {
                return Err(self.err(
                    ErrorKind::IncompleteParameterEntity,
                    "unclosed conditional header",
                ));
            }
            return Ok(false);
        };
        let header = &text[3..end - 1];
        if !header.contains('%') {
            let included = match header.trim_matches(whitespace) {
                "INCLUDE" => true,
                "IGNORE" => false,
                _ => return Err(self.err(ErrorKind::Syntax, "invalid conditional section keyword")),
            };
            if self.conditional.included_sources.len() >= self.config.limits.max_depth {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "conditional section nesting limit exceeded",
                ));
            }
            if included {
                try_push(&mut self.conditional.included_sources, self.sources.len())?;
            } else {
                self.conditional.ignored_depth = 1;
                self.conditional.ignored_source = self.sources.len();
                self.conditional.ignored_bytes = end;
            }
            self.conditional.header_checked = 0;
            self.conditional_raw(end)?;
            return Ok(true);
        }
        self.charge_expansion(
            size_of::<HeaderContinuation>() + size_of::<HeaderFrame>() + header.len(),
        )?;
        let mut frames = Vec::new_in(self.allocator);
        try_push(
            &mut frames,
            HeaderFrame {
                text: string(header, self.allocator)?,
                offset: 0,
                name: None,
                position: None,
            },
        )?;
        self.conditional.header = Some(HeaderContinuation {
            frames,
            selected: None,
            skipped: false,
            raw: string("<![", self.allocator)?,
            raw_position: self.source().position(end),
            expanded_bytes: 0,
            end,
            external: None,
        });
        self.continue_conditional_header()
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
    pub(crate) fn parse_doctype(&mut self, token: &str, position: Position) -> Result<(), Error> {
        if self.seen_root || self.seen_doctype || self.sources.len() > 1 || self.fragment {
            return Err(self.err(ErrorKind::Syntax, "misplaced document type declaration"));
        }
        let has_internal_subset = token.ends_with('[');
        if !token[9..].starts_with(whitespace) {
            let offset = 2 + take_name(&token[2..]).map_or(0, |(name, _)| name.len());
            return Err(self.err_at(
                ErrorKind::InvalidToken,
                "whitespace required after DOCTYPE",
                offset,
            ));
        }
        let mut cursor = Cursor::new(
            &token[9..token.len() - 1],
            self.config.namespace_separator.is_some(),
        );
        cursor
            .require_space()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let name = string(
            cursor
                .name()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?,
            self.allocator,
        )?;
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
            EventKind::StartDoctype {
                name,
                system_id,
                public_id,
                has_internal_subset,
            },
            position,
        )?;
        self.event_raw(if has_internal_subset {
            token
        } else {
            &token[..token.len() - 1]
        })?;
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
        if self.parameter_read.is_none() {
            self.charge_expansion(size_of::<AtomicBool>())?;
            self.parameter_read = Some(Shared::try_new_in(AtomicBool::new(false), self.allocator)?);
        }
        self.foreign_dtd_pending = Some(ForeignDtd {
            previous_subset,
            position,
            delivered: false,
        });
        self.emit(
            EventKind::ExternalEntityReference {
                context: None,
                system_id: None,
                public_id: None,
            },
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
            .parameter_read
            .as_ref()
            .is_some_and(|read| read.load(Ordering::Relaxed))
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
                    EventKind::ExternalEntityReference {
                        context: None,
                        system_id,
                        public_id,
                    },
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
        if self.conditional.ignored_depth != 0 {
            return self.parse_ignored_section();
        }
        let text = self.source().remaining();
        let whitespace_len = text
            .char_indices()
            .find(|(_, character)| !whitespace(*character))
            .map_or(text.len(), |(index, _)| index);
        if whitespace_len > 0 {
            if self.default_events {
                let position = self.source().position(whitespace_len);
                self.save_current_raw(whitespace_len)?;
                self.emit(EventKind::Default, position)?;
            }
            self.declaration_allowed = false;
            self.consume(whitespace_len);
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
                .is_some_and(|character| !crate::names::is_name_start(character))
        {
            return Err(self.err_at(ErrorKind::InvalidToken, "invalid DTD declaration name", 2));
        }
        if text.starts_with('%') {
            return self.parse_parameter_reference();
        }
        if self.in_doctype && text.starts_with(']') {
            if self.sources.len() > 1 {
                return Err(self.err(
                    ErrorKind::AsynchronousEntity,
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
            let raw = string(&text[..end], self.allocator)?;
            self.finish_doctype(position, &raw)?;
            self.consume(end);
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
            crate::ScanMode::Tag
        } else if text == "<" && !self.is_source_final() {
            return Ok(false);
        } else {
            return Err(self.err(ErrorKind::Syntax, "unexpected text in DTD"));
        };
        let limit = self.config.limits.max_token_bytes;
        if mode == crate::ScanMode::Pi
            && text[2..]
                .chars()
                .next()
                .is_some_and(|character| !crate::names::is_name_start(character))
        {
            return Err(self.err_at(
                ErrorKind::InvalidToken,
                "invalid processing instruction target",
                2,
            ));
        }
        let end = self
            .source_mut()
            .scan_token(mode, limit)
            .map_err(|(kind, offset)| {
                self.err_at(kind, "invalid or oversized DTD token", offset)
            })?;
        let Some(end) = end else {
            if self.is_source_final() {
                return Err(self.err(ErrorKind::UnclosedToken, "unclosed DTD declaration"));
            }
            return Ok(false);
        };
        let token = string(&self.source().remaining()[..end], self.allocator)?;
        if let Some((offset, _)) = token
            .char_indices()
            .find(|(_, character)| !crate::names::is_xml_char(*character))
        {
            return Err(self.err_at(
                ErrorKind::InvalidToken,
                "invalid XML character in DTD",
                offset,
            ));
        }
        self.save_current_raw(end)?;
        self.parse_subset(&token, 0)?;
        self.consume(end);
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
        let name = string(&self.source().remaining()[1..end], self.allocator)?;
        if !crate::names::is_name(&name) {
            return Err(self.err(ErrorKind::InvalidToken, "invalid parameter entity name"));
        }
        if self.config.namespace_separator.is_some()
            && let Some(colon) = name.find(':')
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
            self.declarations_skipped = !self.standalone;
            self.consume(end + 1);
            if !self.standalone {
                self.emit(EventKind::NotStandalone, position)?;
                self.event_raw("")?;
            }
            return Ok(true);
        }
        let Some(entity) = self.parameter_entities.get(&name) else {
            if !self.external_subset {
                return Err(self.err(ErrorKind::UndefinedEntity, "undefined parameter entity"));
            }
            self.declarations_skipped |= !self.standalone;
            let raw = string(&self.source().remaining()[..end + 1], self.allocator)?;
            self.consume(end + 1);
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
        if self
            .sources
            .iter()
            .any(|source| source.entity_name.as_ref() == Some(&source_name))
            || self.entity_chain.iter().any(|name| name == &source_name)
        {
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
            self.consume(end + 1);
            try_push(
                &mut self.sources,
                crate::encoding::Source::entity(value, source_name, position, self.stack.len()),
            )?;
        } else {
            self.charge_external_identifiers(entity)?;
            let system_id = entity.system_id.try_clone()?;
            let public_id = entity.public_id.try_clone()?;
            self.consume(end + 1);
            let mut raw = source_name.try_clone()?;
            raw.push(';')?;
            self.active_parameter_reference = Some((source_name, position));
            if self.parameter_read.is_none() {
                self.charge_expansion(size_of::<AtomicBool>())?;
                self.parameter_read =
                    Some(Shared::try_new_in(AtomicBool::new(false), self.allocator)?);
            }
            self.parameter_read
                .as_ref()
                .expect("parameter read marker")
                .store(false, Ordering::Relaxed);
            self.emit(
                EventKind::ExternalEntityReference {
                    context: None,
                    system_id,
                    public_id,
                },
                position,
            )?;
            self.pending.back_mut().expect("external parameter").raw = Some(raw);
        }
        Ok(true)
    }

    /// Expand complete declaration tokens, retaining literal provenance and
    /// separating replacement frames with grammar-only whitespace. Quoted
    /// values and references must finish in the frame where they begin.
    fn declaration_tokens(
        &self,
        text: &str,
        base_offset: usize,
    ) -> Result<Option<DeclarationExpansion>, Error> {
        // The overwhelmingly common declaration contains no parameter reference.
        if !text.contains('%') {
            return Ok(None);
        }
        let mut check = text;
        let mut has_reference = false;
        while let Some(offset) = check.find(['\'', '"', '%']) {
            check = &check[offset..];
            if check.starts_with('%') {
                if !check[1..].starts_with(whitespace) {
                    has_reference = true;
                    break;
                }
                check = &check[1..];
            } else {
                let quote = check.as_bytes()[0] as char;
                let Some(end) = check[1..].find(quote) else {
                    break;
                };
                check = &check[end + 2..];
            }
        }
        if !has_reference {
            return Ok(None);
        }
        let mut result = DeclarationExpansion {
            text: String::new_in(self.allocator),
            raw: String::new_in(self.allocator),
            literals: Vec::new_in(self.allocator),
            parameters: Vec::new_in(self.allocator),
            // An external value callback can install a Default handler while
            // an ENTITY declaration is suspended. Keep its future raw suffix.
            capture_raw: self.default_events || text.starts_with("ENTITY"),
        };
        let mut parents: Vec<DeclarationFrame<'_>> = Vec::new_in(self.allocator);
        let mut current = DeclarationFrame {
            rest: text,
            name: None,
            source_offset: base_offset,
            normalize: self.sources.len() == 1,
        };
        loop {
            let end = current
                .rest
                .find(['\'', '"', '%', '<', '>'])
                .unwrap_or(current.rest.len());
            if current.rest[..end].ends_with(')') && current.rest.as_bytes().get(end) == Some(&b'%')
            {
                return Err(self.err_at(
                    ErrorKind::InvalidToken,
                    "parameter reference cannot delimit a closing model group",
                    current.source_offset + if current.name.is_none() { end } else { 0 },
                ));
            }
            self.append_declaration_token(&mut result, &current.rest[..end], false)?;
            current.rest = &current.rest[end..];
            if current.name.is_none() {
                current.source_offset += end;
            }
            if current.rest.is_empty() {
                if let Some(parent) = parents.pop() {
                    self.append_declaration_token(&mut result, " ", true)?;
                    current = parent;
                    continue;
                }
                return Ok(Some(result));
            }
            if current.rest.starts_with(['<', '>']) {
                return Err(self.err_at(
                    ErrorKind::IncompleteParameterEntity,
                    "parameter replacement crosses a declaration boundary",
                    current.source_offset,
                ));
            }
            if current.rest.starts_with(['\'', '"']) {
                let quote = current.rest.as_bytes()[0] as char;
                let end = current.rest[1..].find(quote).ok_or_else(|| {
                    self.err_at(
                        ErrorKind::UnclosedToken,
                        "quoted declaration token crosses a parameter boundary",
                        current.source_offset,
                    )
                })? + 2;
                self.charge_expansion(size_of::<DeclarationLiteral>())?;
                let mut parameters = Vec::new_in(self.allocator);
                for name in parents
                    .iter()
                    .filter_map(|frame| frame.name)
                    .chain(current.name)
                {
                    self.charge_expansion(size_of::<String>() + name.len())?;
                    try_push(&mut parameters, string(name, self.allocator)?)?;
                }
                try_push(
                    &mut result.literals,
                    DeclarationLiteral {
                        offset: result.text.len(),
                        normalize: current.normalize,
                        parameters,
                    },
                )?;
                self.append_declaration_token(&mut result, &current.rest[..end], false)?;
                current.rest = &current.rest[end..];
                if current.name.is_none() {
                    current.source_offset += end;
                }
                continue;
            }
            // The percent sign in <!ENTITY % name ...> is a grammar marker.
            if current.rest[1..].starts_with(whitespace) {
                self.append_declaration_token(&mut result, "%", false)?;
                current.rest = &current.rest[1..];
                if current.name.is_none() {
                    current.source_offset += 1;
                }
                continue;
            }
            let end = current.rest.find(';').ok_or_else(|| {
                self.err_at(
                    ErrorKind::InvalidToken,
                    "unclosed declaration parameter reference",
                    current.source_offset,
                )
            })?;
            let name = &current.rest[1..end];
            if !crate::names::is_name(name)
                || (self.config.namespace_separator.is_some() && name.contains(':'))
            {
                return Err(self.err_at(
                    ErrorKind::InvalidToken,
                    "invalid declaration parameter name",
                    current.source_offset,
                ));
            }
            if !self.external_subset && self.sources.len() == 1 {
                return Err(self.err_at(
                    ErrorKind::ParameterEntityReference,
                    "parameter reference in an internal subset declaration",
                    current.source_offset,
                ));
            }
            self.charge_expansion(end + 1 + size_of::<DeclarationFrame<'_>>())?;
            let entity = self.parameter_entities.get(name);
            self.append_declaration_token(&mut result, " ", true)?;
            if self.parameter_mode == 0 || entity.is_none() {
                self.charge_expansion(
                    2 * size_of::<crate::PendingEvent>() + size_of::<DeclarationParameter>(),
                )?;
                try_push(
                    &mut result.parameters,
                    DeclarationParameter {
                        offset: result.text.len(),
                        position: self.source().position_at(current.source_offset, end + 1),
                        raw_start: result.raw.len(),
                        raw_end: result.raw.len() + if result.capture_raw { end + 1 } else { 0 },
                        disabled: self.parameter_mode == 0,
                    },
                )?;
                if result.capture_raw {
                    result.raw.try_push_str(&current.rest[..end + 1])?;
                }
                current.rest = &current.rest[end + 1..];
                if current.name.is_none() {
                    current.source_offset += end + 1;
                }
                continue;
            }
            if current.name == Some(name)
                || parents.iter().any(|frame| frame.name == Some(name))
                || self
                    .entity_chain
                    .iter()
                    .any(|entry| entry.strip_prefix('%') == Some(name))
                || self.sources.iter().any(|source| {
                    source
                        .entity_name
                        .as_deref()
                        .and_then(|name| name.strip_prefix('%'))
                        == Some(name)
                })
            {
                return Err(self.err_at(
                    ErrorKind::RecursiveEntityReference,
                    "recursive declaration parameter entity",
                    current.source_offset,
                ));
            }
            if parents.len()
                + self.sources.len()
                + self.external_depth
                + self.inherited_parameter_depth
                > self.config.limits.max_entity_depth
            {
                return Err(self.err_at(
                    ErrorKind::LimitExceeded,
                    "declaration parameter nesting limit exceeded",
                    current.source_offset,
                ));
            }
            let value = entity
                .expect("declared parameter")
                .value
                .as_deref()
                .ok_or_else(|| {
                    self.err_at(
                        ErrorKind::ExternalEntityHandling,
                        "external parameter reference inside a declaration is unsupported",
                        current.source_offset,
                    )
                })?;
            self.charge_expansion(value.len())?;
            let position = current.source_offset;
            current.rest = &current.rest[end + 1..];
            if current.name.is_none() {
                current.source_offset += end + 1;
            }
            try_push(&mut parents, current)?;
            current = DeclarationFrame {
                rest: value,
                name: Some(name),
                source_offset: position,
                normalize: false,
            };
        }
    }

    fn append_declaration_token(
        &self,
        result: &mut DeclarationExpansion,
        text: &str,
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
        result.text.try_push_str(text)?;
        if !boundary && result.capture_raw {
            result.raw.try_push_str(text)?;
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
                    self.declarations_skipped,
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
                    !self.standalone && cursor.parameter_index == cursor.parameters.len(),
                    true,
                    parameter.position,
                )?;
            }
            self.has_external_subset = true;
            self.declarations_skipped |= !self.standalone;
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
        raw.try_push_str(&cursor.raw[cursor.raw_offset..end])?;
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
                        EventKind::EntityDeclaration { .. }
                            | EventKind::AttlistDeclaration { .. }
                            | EventKind::EntityDeclarationDuplicate { .. }
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
        if !assigned {
            self.charge_expansion(size_of::<crate::PendingEvent>())?;
            self.emit(
                if unconditional {
                    EventKind::Default
                } else if let Some((external, unparsed)) = cursor.duplicate_defaults {
                    EventKind::EntityDeclarationDuplicate { external, unparsed }
                } else if cursor.entity_defaults {
                    EventKind::EntityDeclarationPrefix
                } else {
                    EventKind::AttlistDeclarationPrefix
                },
                position,
            )?;
            self.pending.back_mut().expect("default fragment").raw = Some(raw);
        }
        cursor.raw_event = self.pending.len();
        Ok(())
    }

    fn parse_subset(&mut self, mut text: &str, base_offset: usize) -> Result<(), Error> {
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
                self.emit(EventKind::Comment(self.source_text(value)?), position)?;
                self.event_raw(&text[..end + 7])?;
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
                self.parse_pi(&text[..end + 2], position)?;
                self.event_raw(&text[..end + 2])?;
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
            let previously_skipped = self.declarations_skipped;
            let expansion = self.declaration_tokens(&text[2..end], offset + 2)?;
            let grammar = expansion
                .as_ref()
                .map_or(&text[2..end], |value| value.text.as_str());
            let mut cursor = Cursor::new(grammar, self.config.namespace_separator.is_some());
            cursor.raw = grammar;
            cursor.raw_event = first_event;
            if let Some(expansion) = &expansion {
                cursor.literals = &expansion.literals;
                cursor.parameters = &expansion.parameters;
                cursor.raw = &expansion.raw;
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
                "ELEMENT" => {
                    let name = cursor
                        .name()
                        .map_err(|message| self.err(ErrorKind::Syntax, message))?;
                    let name = string(name, self.allocator)?;
                    cursor
                        .require_space()
                        .map_err(|message| self.err(ErrorKind::Syntax, message))?;
                    let model_start = cursor.rest();
                    parse_content_model(&mut cursor, 0, self.config.limits.max_depth).map_err(
                        |message| {
                            if cursor.rest().starts_with(['?', '*', '+']) {
                                self.err_at(
                                    ErrorKind::InvalidToken,
                                    message,
                                    offset + 2 + cursor.offset().min(end - 2),
                                )
                            } else {
                                self.err(ErrorKind::Syntax, message)
                            }
                        },
                    )?;
                    let model = string(
                        &model_start[..model_start.len() - cursor.rest().len()],
                        self.allocator,
                    )?;
                    self.declaration_parameters(&mut cursor)?;
                    self.emit(EventKind::ElementDeclaration { name, model }, position)?;
                }
                "NOTATION" => {
                    let name = cursor
                        .ncname()
                        .map_err(|message| self.err(ErrorKind::Syntax, message))?;
                    let name = string(name, self.allocator)?;
                    cursor
                        .require_space()
                        .map_err(|message| self.err(ErrorKind::Syntax, message))?;
                    let (system_id, public_id) =
                        external_id(&mut cursor, true, self.allocator, self.sources.len() == 1)
                            .map_err(|error| self.err(error.kind, error.message))?;
                    self.declaration_parameters(&mut cursor)?;
                    self.emit(
                        EventKind::NotationDeclaration {
                            name,
                            system_id,
                            public_id,
                        },
                        position,
                    )?;
                }
                _ => return Err(self.err(ErrorKind::Syntax, "unsupported DTD declaration")),
            }
            if let Some(mut state) = self.value_state.take() {
                cursor.space();
                let suffix_error = (!cursor.rest().is_empty())
                    .then(|| self.err(ErrorKind::Syntax, "unexpected text in entity declaration"));
                let raw = if let Some(expansion) = &expansion {
                    self.charge_expansion(expansion.raw.len() + 3)?;
                    let mut raw = string("<!", self.allocator)?;
                    raw.push_str(&expansion.raw)?;
                    raw.push('>')?;
                    raw
                } else {
                    self.charge_expansion(end + 1)?;
                    string(&text[..end + 1], self.allocator)?
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
                    && self.declarations_skipped
                    && (cursor.duplicate_defaults.is_some()
                        || self.pending.iter().skip(first_event).any(|pending| {
                            matches!(pending.event.kind, EventKind::EntityDeclaration { .. })
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
            if self.declarations_skipped
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
            let closing_default = !previously_skipped
                && self.declarations_skipped
                && self.default_events
                && self.pending.iter().skip(first_event).any(|pending| {
                    matches!(pending.event.kind, EventKind::EntityDeclaration { .. })
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
                        raw.try_push_str(&expansion.raw)?;
                        if !closing_default {
                            raw.try_push('>')?;
                        }
                        raw
                    } else {
                        string(&text[..end + usize::from(!closing_default)], self.allocator)?
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
        raw: &str,
        quote: usize,
        parameters: &[DeclarationParameter],
        first_event: usize,
        position: Position,
    ) -> Result<(), Error> {
        let mut cursor = Cursor::new("", self.config.namespace_separator.is_some());
        cursor.raw = &raw[2..raw.len() - 1];
        cursor.raw_offset = quote - 2;
        cursor.raw_started = true;
        cursor.raw_event = first_event;
        cursor.parameters = parameters;
        cursor.parameter_defaults = true;
        cursor.entity_defaults = true;
        let value_skipped = self.declarations_skipped;
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
            let closing_default = self.declarations_skipped;
            self.declaration_default_segment(
                &mut cursor,
                end,
                !closing_default,
                value_skipped,
                position,
            )?;
            if closing_default {
                self.declaration_default_segment(&mut cursor, end, true, true, position)?;
            }
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
        let name = cursor
            .ncname()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let name = string(name, self.allocator)?;
        cursor
            .require_space()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        if self.default_events
            && !self.declarations_skipped
            && if parameter {
                self.parameter_entities.contains_key(&name)
            } else {
                self.entities.contains_key(&name)
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
            if self.declarations_skipped {
                return Ok(());
            }
            let declaring = (parameter && !self.parameter_entities.contains_key(&name))
                .then_some(name.as_str());
            match self.entity_value(
                raw,
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
                        raw: String::new_in(self.allocator),
                        quote: 0,
                        prefix_start: 0,
                        tail_parameters: Vec::new_in(self.allocator),
                        prefix_sent: false,
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
                Some(string(
                    cursor
                        .ncname()
                        .map_err(|message| self.err(ErrorKind::Syntax, message))?,
                    self.allocator,
                )?)
            } else {
                None
            };
            if let Some((_, unparsed)) = &mut cursor.duplicate_defaults {
                *unparsed = notation.is_some();
            }
            (None, system_id, public_id, notation)
        };
        self.declaration_parameters(cursor)?;
        if self.declarations_skipped {
            return Ok(());
        }
        let declaration_count = self.entities.len() + self.parameter_entities.len();
        let declarations = if parameter {
            &mut self.parameter_entities
        } else {
            &mut self.entities
        };
        if !declarations.contains_key(&name) {
            if declaration_count >= self.config.limits.max_entities {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "entity declaration count limit exceeded",
                ));
            }
            try_insert(
                declarations,
                name.try_clone()?,
                Entity {
                    value: value.try_clone()?,
                    system_id: system_id.try_clone()?,
                    public_id: public_id.try_clone()?,
                    notation: notation.try_clone()?,
                    declared_in_parameter_entity: self.external_subset || self.sources.len() > 1,
                },
            )?;
            self.emit(
                EventKind::EntityDeclaration {
                    name,
                    value,
                    parameter,
                    system_id,
                    public_id,
                    notation,
                },
                position,
            )?;
        }
        // A missing parameter stops subsequent declarations, but the current
        // value (including its suffix) is still stored and reported.
        if skipped_parameter {
            self.has_external_subset = true;
            self.declarations_skipped = !self.standalone;
        }
        Ok(())
    }

    /// Build an entity value without letting replacement quotes or unfinished
    /// references become part of the surrounding declaration's grammar.
    fn entity_value(
        &self,
        raw: &str,
        declaring_parameter: Option<&str>,
        normalize: bool,
        declaration_parameters: &[String],
    ) -> Result<crate::value::Build, Error> {
        let mut value = String::try_with_capacity_in(raw.len(), self.allocator)?;
        let mut parents = Vec::new_in(self.allocator);
        let mut current = EntityValueFrame {
            rest: raw,
            name: None,
            normalize,
        };
        let mut skipped = false;
        loop {
            let start = current.rest.find(['&', '%']).unwrap_or(current.rest.len());
            self.append_entity_value(&mut value, &current.rest[..start], current.normalize)?;
            current.rest = &current.rest[start..];
            if current.rest.is_empty() {
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
            let reference = &current.rest[1..end];
            let parameter = current.rest.starts_with('%');
            if !parameter && reference.starts_with('#') {
                let character = character_reference(reference)
                    .map_err(|(kind, _)| {
                        self.err(kind, "invalid character reference in entity value")
                    })?
                    .expect("numeric reference");
                let mut bytes = [0; 4];
                self.append_entity_value(&mut value, character.encode_utf8(&mut bytes), false)?;
                current.rest = &current.rest[end + 1..];
                continue;
            }
            if !crate::names::is_name(reference)
                || (self.config.namespace_separator.is_some() && reference.contains(':'))
            {
                return Err(self.err(ErrorKind::InvalidToken, "invalid reference in entity value"));
            }
            if !parameter {
                self.append_entity_value(&mut value, &current.rest[..end + 1], false)?;
                current.rest = &current.rest[end + 1..];
                continue;
            }
            if !self.external_subset && self.sources.len() == 1 {
                return Err(self.err(
                    ErrorKind::ParameterEntityReference,
                    "parameter reference in internal subset entity value",
                ));
            }
            // Charge reference work even for empty or missing replacements.
            self.charge_expansion(end + 1 + size_of::<EntityValueFrame<'_>>())?;
            if declaring_parameter == Some(reference)
                || declaration_parameters.iter().any(|name| name == reference)
                || current.name == Some(reference)
                || parents
                    .iter()
                    .any(|frame: &EntityValueFrame<'_>| frame.name == Some(reference))
                || self
                    .entity_chain
                    .iter()
                    .any(|name| name.strip_prefix('%') == Some(reference))
                || self.sources.iter().any(|source| {
                    source
                        .entity_name
                        .as_deref()
                        .and_then(|name| name.strip_prefix('%'))
                        == Some(reference)
                })
            {
                return Err(self.err(
                    ErrorKind::RecursiveEntityReference,
                    "recursive parameter entity value",
                ));
            }
            current.rest = &current.rest[end + 1..];
            let Some(entity) = self.parameter_entities.get(reference) else {
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
                            text: string(frame.rest, self.allocator)?,
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
            try_push(&mut parents, current)?;
            current = EntityValueFrame {
                rest: replacement,
                name: Some(reference),
                normalize: false,
            };
        }
    }

    /// Append with a bound before allocating, retaining character-reference CRs
    /// in replacement frames and normalizing physical CR/CRLF exactly once.
    pub(crate) fn append_entity_value(
        &self,
        value: &mut String,
        text: &str,
        normalize: bool,
    ) -> Result<(), Error> {
        if value.len().saturating_add(text.len()) > self.config.limits.max_token_bytes {
            return Err(self.err(
                ErrorKind::LimitExceeded,
                "expanded entity value limit exceeded",
            ));
        }
        if normalize && text.contains('\r') {
            value.push_str(&normalize_newlines(text, self.allocator)?)?;
        } else {
            value.push_str(text)?;
        }
        Ok(())
    }

    fn attlist_declaration(
        &mut self,
        cursor: &mut Cursor<'_>,
        position: Position,
    ) -> Result<(), Error> {
        let element = cursor
            .name()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let element = string(element, self.allocator)?;
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
            let name = cursor
                .name()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            let name = string(name, self.allocator)?;
            cursor
                .require_space()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            let start = cursor.rest();
            if cursor.eat("NOTATION") {
                cursor
                    .require_space()
                    .map_err(|message| self.err(ErrorKind::Syntax, message))?;
                enumeration(cursor, true)
                    .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            } else if cursor.starts("(") {
                enumeration(cursor, false)
                    .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            } else {
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
                attribute_type.push_str(part)?;
            }
            cursor
                .require_space()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
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
                if self.declarations_skipped {
                    continue;
                }
                // A CR/LF pair in a replacement was produced by character
                // references, so it represents two attribute whitespace
                // characters rather than one physical line ending.
                let normalized =
                    if cursor.last_literal_normalize == Some(false) && raw.contains("\r\n") {
                        let mut value = String::try_with_capacity_in(raw.len(), self.allocator)?;
                        for character in raw.chars() {
                            value.try_push(if whitespace(character) {
                                ' '
                            } else {
                                character
                            })?;
                        }
                        Some(value)
                    } else {
                        None
                    };
                let mut value = self.expand_attribute(
                    normalized.as_deref().unwrap_or(raw),
                    &mut Vec::new_in(self.allocator),
                )?;
                if attribute_type != "CDATA" {
                    value = collapse_spaces(&value, self.allocator)?;
                }
                Some(value)
            };
            self.declaration_parameters(cursor)?;
            if self.declarations_skipped {
                continue;
            }
            if !self.defaults.contains_key(&element) {
                try_insert(
                    &mut self.defaults,
                    element.try_clone()?,
                    DefaultAttributes::new(self.allocator),
                )?;
            }
            let declarations = self
                .defaults
                .get_mut(&element)
                .expect("default list was inserted");
            if declarations.get(&name).is_none() {
                if declarations.ordered.len() >= self.config.limits.max_attributes {
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
            }
            if first_attribute {
                first_attribute = false;
            } else {
                // Every later callback reuses the same input name. Charging before
                // cloning prevents a long name and many tiny declarations from
                // creating an unbounded queue of repeated callback payloads.
                self.charge_expansion(element.len())?;
            }
            self.emit(
                EventKind::AttlistDeclaration {
                    element: element.try_clone()?,
                    name,
                    attribute_type,
                    default: value,
                    required,
                },
                position,
            )?;
        }
        Ok(())
    }
}

struct Cursor<'a> {
    text: &'a str,
    namespaces: bool,
    initial_len: usize,
    literals: &'a [DeclarationLiteral],
    literal_index: usize,
    last_literal_normalize: Option<bool>,
    last_literal_parameters: &'a [String],
    parameters: &'a [DeclarationParameter],
    parameter_index: usize,
    parameter_defaults: bool,
    entity_defaults: bool,
    duplicate_defaults: Option<(bool, bool)>,
    raw_offset: usize,
    raw_event: usize,
    raw_started: bool,
    raw_closed: bool,
    raw: &'a str,
}
impl<'a> Cursor<'a> {
    fn new(text: &'a str, namespaces: bool) -> Self {
        Self {
            text,
            namespaces,
            initial_len: text.len(),
            literals: &[],
            literal_index: 0,
            last_literal_normalize: None,
            last_literal_parameters: &[],
            parameters: &[],
            parameter_index: 0,
            parameter_defaults: false,
            entity_defaults: false,
            duplicate_defaults: None,
            raw_offset: 0,
            raw_event: 0,
            raw_started: false,
            raw_closed: false,
            raw: "",
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
        let (name, rest) = take_name(self.text).ok_or("name required in DTD declaration")?;
        if self.namespaces && !crate::names::is_qname(name) {
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
    let system_literal = |value, normalize| {
        if normalize {
            normalize_newlines(value, allocator)
        } else {
            string(value, allocator)
        }
    };
    if cursor.eat("SYSTEM") {
        cursor.require_space().map_err(syntax)?;
        let value = cursor.quoted().map_err(syntax)?;
        Ok((
            Some(system_literal(
                value,
                cursor.last_literal_normalize.unwrap_or(normalize),
            )?),
            None,
        ))
    } else if cursor.eat("PUBLIC") {
        cursor.require_space().map_err(syntax)?;
        let public = cursor.quoted().map_err(syntax)?;
        if !public
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " \r\n-'()+,./:=?;!*#@$_%".contains(c))
        {
            return Err(syntax("invalid public identifier character"));
        }
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
                system,
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
                .find(|(_, c)| !crate::names::is_name_char(*c))
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
