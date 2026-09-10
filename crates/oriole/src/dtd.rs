use crate::{
    DefaultAttribute, DefaultAttributes, Entity, Error, ErrorKind, EventKind, Parser, Position,
    character_reference, collapse_spaces, normalize_newlines, string, take_name, whitespace,
};
use oriole_storage::{Allocator, Shared, String, TryClone, Vec, try_insert, try_push};
use std::sync::atomic::{AtomicBool, Ordering};

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
        self.parameter_mode == 2
            || (self.parameter_mode == 1 && (!self.standalone || self.external_subset))
    }

    fn finish_doctype(&mut self, position: Position, raw: &str) -> Result<(), Error> {
        self.in_doctype = false;
        if let Some((system_id, public_id)) = self.doctype_external.take()
            && self.parameter_entities_enabled()
            && (system_id.is_some() || self.foreign_dtd)
        {
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
            let mut cursor = Cursor::new(&text[2..end], self.config.namespace_separator.is_some());
            let declaration = cursor
                .name()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
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
                                    offset + end - cursor.rest().len(),
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
            cursor.space();
            if !cursor.rest().is_empty() {
                let rest_offset = end - cursor.rest().len();
                if cursor.rest().starts_with(['?', '*', '+'])
                    && text[..rest_offset].ends_with(whitespace)
                {
                    return Err(self.err_at(
                        ErrorKind::InvalidToken,
                        "misplaced DTD repetition marker",
                        offset + rest_offset,
                    ));
                }
                return Err(self.err(ErrorKind::Syntax, "unexpected text in DTD declaration"));
            }
            if self.declarations_skipped
                && matches!(declaration, "ENTITY" | "ATTLIST")
                && self.default_events
                && self.pending.len() == first_event
            {
                self.emit(EventKind::Default, position)?;
            }
            let closing_default = !previously_skipped
                && self.declarations_skipped
                && self.default_events
                && self.pending.iter().skip(first_event).any(|pending| {
                    matches!(pending.event.kind, EventKind::EntityDeclaration { .. })
                });
            for (index, pending) in self.pending.iter_mut().enumerate().skip(first_event) {
                pending.raw = Some(if index == first_event {
                    string(&text[..end + usize::from(!closing_default)], self.allocator)?
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
        let mut skipped_parameter = false;
        let (value, system_id, public_id, notation) = if cursor.starts("\"") || cursor.starts("'") {
            let raw = cursor
                .quoted()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            if self.declarations_skipped {
                return Ok(());
            }
            let declaring = (parameter && !self.parameter_entities.contains_key(&name))
                .then_some(name.as_str());
            let (value, skipped) = self.entity_value(raw, declaring)?;
            skipped_parameter = skipped;
            (Some(value), None, None, None)
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
            (None, system_id, public_id, notation)
        };
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
    ) -> Result<(String, bool), Error> {
        let mut value = String::try_with_capacity_in(raw.len(), self.allocator)?;
        let mut parents = Vec::new_in(self.allocator);
        let mut current = EntityValueFrame {
            rest: raw,
            name: None,
            normalize: self.sources.len() == 1,
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
                return Ok((value, skipped));
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
                > self.config.limits.max_entity_depth
            {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "parameter entity value nesting limit exceeded",
                ));
            }
            let replacement = entity.value.as_deref().ok_or_else(|| {
                self.err(
                    ErrorKind::ExternalEntityHandling,
                    "external parameter reference in entity value is unsupported",
                )
            })?;
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
    fn append_entity_value(
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
                if self.declarations_skipped {
                    continue;
                }
                let mut value = self.expand_attribute(raw, &mut Vec::new_in(self.allocator))?;
                if attribute_type != "CDATA" {
                    value = collapse_spaces(&value, self.allocator)?;
                }
                Some(value)
            };
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
}
impl<'a> Cursor<'a> {
    fn new(text: &'a str, namespaces: bool) -> Self {
        Self { text, namespaces }
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
    let system_literal = |value| {
        if normalize {
            normalize_newlines(value, allocator)
        } else {
            string(value, allocator)
        }
    };
    if cursor.eat("SYSTEM") {
        cursor.require_space().map_err(syntax)?;
        let value = cursor.quoted().map_err(syntax)?;
        Ok((Some(system_literal(value)?), None))
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
        Ok((Some(system_literal(system)?), Some(normalized)))
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
