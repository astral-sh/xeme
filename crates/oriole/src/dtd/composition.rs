//! Incremental declaration grammar over distinct parameter-entity sources.
//!
//! XML's proper declaration/PE nesting rules are validity constraints. A
//! nonvalidating parser accepts a closing delimiter from a replacement, while
//! names, quoted literals and references still have one lexical source.

use super::*;

#[derive(Debug)]
pub(super) struct Declaration {
    pub(super) expansion: DeclarationExpansion,
    pub(super) position: Position,
    quote_checked: usize,
    word_checked: usize,
    pub(super) semantic: Option<semantic::State>,
    pub(super) request: Option<semantic::Request>,
    pub(super) complete: bool,
    pub(super) ready: usize,
}

#[derive(Debug)]
pub(super) struct Header {
    selected: Option<bool>,
    word: Buffer,
    raw: Buffer,
    position: Position,
    bytes: usize,
}

impl Parser {
    pub(crate) fn declaration_context_byte_index(&self) -> Option<usize> {
        self.conditional
            .header
            .as_ref()
            .map(|header| header.position.byte_index)
            .into_iter()
            .chain(
                self.conditional
                    .declaration
                    .as_ref()
                    .map(|declaration| declaration.position.byte_index),
            )
            .chain(
                self.conditional
                    .declaration
                    .as_ref()
                    .and_then(|declaration| {
                        declaration
                            .request
                            .as_ref()
                            .map(|request| request.position.byte_index)
                    }),
            )
            .chain(
                self.foreign_dtd_pending
                    .as_ref()
                    .map(|foreign| foreign.position.byte_index),
            )
            .min()
    }

    pub(crate) fn has_header_composition(&self) -> bool {
        self.conditional.header.is_some()
    }

    pub(super) fn start_header_composition(&mut self) -> Result<bool, Error> {
        if !self.external_subset {
            return Err(self.err(
                ErrorKind::Syntax,
                "conditional section in the internal subset",
            ));
        }
        self.charge_expansion(size_of::<Header>())?;
        self.conditional.header = Some(oriole_storage::try_box(
            Header {
                selected: None,
                word: Buffer::new_in(self.allocator),
                raw: Buffer::plain(string("<![", self.allocator)?),
                position: self.here(),
                bytes: 3,
            },
            self.allocator,
        )?);
        self.consume(3)?;
        self.continue_header_composition()
    }

    /// Select a complete keyword and opening bracket from lexical sources.
    /// Only ordinary external DTD parsers run during external callbacks.
    pub(crate) fn continue_header_composition(&mut self) -> Result<bool, Error> {
        let mut state = self.conditional.header.take().expect("pending header");
        loop {
            if self.source().remaining().is_empty() {
                if self.sources.len() > 1 {
                    if !self.source().dtd_fragment {
                        return Err(self.err(
                            ErrorKind::IncompleteParameterEntity,
                            "conditional header crosses a between-declaration parameter boundary",
                        ));
                    }
                    self.header_word(&mut state)?;
                    self.pop_entity_source();
                    continue;
                }
                if self.is_source_final() {
                    return Err(self.err(
                        ErrorKind::IncompleteParameterEntity,
                        "unclosed conditional header",
                    ));
                }
                self.conditional.header = Some(state);
                return Ok(false);
            }
            let text = self.source().remaining();
            let character = text.chars().next().expect("nonempty source");
            if !crate::names::is_xml_char(character) {
                return Err(self.err(
                    ErrorKind::InvalidToken,
                    "invalid conditional header character",
                ));
            }
            if character == '%' {
                self.header_word(&mut state)?;
                let limit = self.config.limits.max_token_bytes;
                let end = self
                    .source_mut()
                    .scan_reference(limit)
                    .map_err(|(kind, offset)| {
                        self.err_at(kind, "invalid conditional parameter reference", offset)
                    })?;
                let Some(end) = end else {
                    if self.is_source_final() {
                        return Err(self.err(
                            ErrorKind::Syntax,
                            "unclosed conditional parameter reference",
                        ));
                    }
                    self.conditional.header = Some(state);
                    return Ok(false);
                };
                let name = &self.source().remaining()[1..end];
                if !self.config.name_rules.is_name(name)
                    || (self.config.namespace_separator.is_some() && name.contains(':'))
                {
                    return Err(self.err(
                        ErrorKind::InvalidToken,
                        "invalid conditional parameter name",
                    ));
                }
                let decoded_name = self
                    .source()
                    .lexical_remaining()
                    .for_slice(name)
                    .decoded(self.allocator)?;
                let name: &str = &decoded_name;
                let entity = self.tables.parameter_entities.get(name);
                if self.parameter_mode == 0 || entity.is_none() {
                    state.bytes = state.bytes.saturating_add(end + 1);
                    if state.bytes > self.config.limits.max_token_bytes {
                        return Err(self.err(
                            ErrorKind::LimitExceeded,
                            "expanded conditional header limit exceeded",
                        ));
                    }
                    let position = self.source().position(end + 1);
                    let raw = self
                        .source()
                        .lexical_remaining()
                        .for_slice(&self.source().remaining()[..end + 1])
                        .to_owned(self.allocator)?;
                    self.set_declarations_skipped(self.declarations_skipped() || !self.standalone);
                    self.has_external_subset = true;
                    if self.parameter_mode == 0 && !self.standalone {
                        self.charge_expansion(2 * size_of::<crate::PendingEvent>())?;
                        self.flush_header_composition(&mut state)?;
                        self.emit(EventKind::NotStandalone, position)?;
                        self.event_raw("")?;
                    }
                    state.raw.append(raw.view())?;
                    self.consume(end + 1)?;
                    continue;
                }
                if entity.expect("declared parameter").value.is_none() {
                    self.flush_header_composition(&mut state)?;
                    if !self.pending.is_empty() {
                        self.conditional.header = Some(state);
                        return Ok(true);
                    }
                    // The normal reference path shares the read marker and
                    // inherits active source names into the external child.
                    self.parse_parameter_reference()?;
                    state.position = self.here();
                    self.conditional.header = Some(state);
                    return Ok(true);
                }
                if entity.is_some_and(Entity::is_value_open)
                    || self.active_entities.contains(name, true)
                {
                    return Err(self.err(
                        ErrorKind::RecursiveEntityReference,
                        "recursive conditional parameter entity",
                    ));
                }
                if self.sources.len() + self.external_depth + self.inherited_parameter_depth
                    > self.config.limits.max_entity_depth
                {
                    return Err(self.err(
                        ErrorKind::LimitExceeded,
                        "conditional parameter nesting limit exceeded",
                    ));
                }
                let value = entity
                    .expect("declared parameter")
                    .value
                    .as_ref()
                    .expect("internal parameter");
                self.charge_expansion(
                    value.len() + end + 1 + size_of::<crate::encoding::Source>(),
                )?;
                let value = value.try_clone()?;
                let mut source_name = string("%", self.allocator)?;
                source_name.push_str(name)?;
                let position = self.source().position(end + 1);
                self.consume(end + 1)?;
                let mut source = crate::encoding::Source::entity(
                    value,
                    source_name,
                    position,
                    self.stack.len(),
                    self.config.name_rules,
                );
                source.dtd_fragment = true;
                self.push_entity_source(source)?;
                continue;
            }
            state.bytes = state.bytes.saturating_add(character.len_utf8());
            if state.bytes > self.config.limits.max_token_bytes {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "expanded conditional header limit exceeded",
                ));
            }
            if character == '[' || whitespace(character) {
                self.header_word(&mut state)?;
            } else {
                state.word.append(
                    self.source()
                        .lexical_remaining()
                        .for_slice(&self.source().remaining()[..character.len_utf8()]),
                )?;
            }
            if character == '[' && state.selected.is_none() {
                self.flush_header_composition(&mut state)?;
                return Err(self.err(ErrorKind::Syntax, "missing conditional section keyword"));
            }
            state.raw.append(
                self.source()
                    .lexical_remaining()
                    .for_slice(&self.source().remaining()[..character.len_utf8()]),
            )?;
            self.consume(character.len_utf8())?;
            if character == '[' {
                let included = state.selected.expect("complete conditional keyword");
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
                    self.conditional.ignored_bytes = state.bytes;
                }
                self.flush_header_composition(&mut state)?;
                self.declaration_allowed = false;
                return Ok(true);
            }
        }
    }

    fn header_word(&mut self, state: &mut Header) -> Result<(), Error> {
        if !state.word.is_empty() {
            state.selected = Some(match (state.word.as_str(), state.selected) {
                ("INCLUDE", None) => true,
                ("IGNORE", None) => false,
                _ => {
                    state.raw.truncate(state.raw.len() - state.word.len());
                    self.flush_header_composition(state)?;
                    return Err(self.err(ErrorKind::Syntax, "invalid conditional section keyword"));
                }
            });
            state.word.clear();
        }
        Ok(())
    }

    fn flush_header_composition(&mut self, state: &mut Header) -> Result<(), Error> {
        let raw = std::mem::replace(&mut state.raw, Buffer::new_in(self.allocator));
        if self.default_events && !raw.is_empty() {
            self.emit(EventKind::Default, state.position)?;
            self.pending.back_mut().expect("header default").raw =
                Some(raw.view().decode(self.allocator)?);
        }
        state.position = self.here();
        Ok(())
    }

    pub(crate) fn has_declaration_composition(&self) -> bool {
        self.conditional.declaration.is_some()
    }

    pub(super) fn start_declaration_composition(
        &mut self,
        incremental: bool,
    ) -> Result<bool, Error> {
        if !incremental {
            self.charge_expansion(size_of::<Declaration>())?;
        }
        self.conditional.declaration = Some(oriole_storage::try_box(
            Declaration {
                expansion: DeclarationExpansion {
                    direct: incremental,
                    text: Buffer::new_in(self.allocator),
                    raw: Buffer::new_in(self.allocator),
                    literals: Vec::new_in(self.allocator),
                    parameters: Vec::new_in(self.allocator),
                    // Value callbacks may install a Default handler later.
                    capture_raw: true,
                    raw_offsets: Vec::new_in(self.allocator),
                },
                position: self.here(),
                quote_checked: 0,
                word_checked: 0,
                semantic: incremental.then(|| semantic::State::new(self)),
                request: None,
                complete: false,
                ready: 0,
            },
            self.allocator,
        )?);
        self.consume(2)?; // The declaration opener is one lexical token.
        self.continue_declaration_composition()
    }

    fn charge_declaration_composition(&self, state: &mut Declaration) -> Result<(), Error> {
        if state.expansion.direct {
            self.charge_expansion(
                size_of::<Declaration>()
                    + size_of::<semantic::State>()
                    + grammar::Cursor::maximum_stack_bytes(self.config.limits.max_depth),
            )?;
            state.expansion.direct = false;
        }
        Ok(())
    }

    /// Consume each physical byte once. Entity source boundaries add grammar
    /// whitespace, and quoted tokens remain in their originating source.
    pub(crate) fn continue_declaration_composition(&mut self) -> Result<bool, Error> {
        let mut state = self
            .conditional
            .declaration
            .take()
            .expect("pending declaration");
        loop {
            if state
                .semantic
                .as_ref()
                .is_some_and(|semantic| state.ready > semantic.waited_prefix)
                || (state.semantic.is_some() && (state.request.is_some() || state.complete))
            {
                let done = self.advance_declaration_semantics(&mut state)?;
                if !done {
                    self.conditional.declaration = Some(state);
                }
                return Ok(true);
            }
            if self.source().remaining().is_empty() {
                if self.sources.len() > 1 {
                    if !self.source().dtd_fragment {
                        return Err(self.err(
                            ErrorKind::IncompleteParameterEntity,
                            "declaration crosses a between-declaration parameter boundary",
                        ));
                    }
                    self.charge_declaration_composition(&mut state)?;
                    self.pop_entity_source();
                    self.append_declaration_token(&mut state.expansion, Slice::plain(" "), true)?;
                    state.ready = state.expansion.text.len();
                    continue;
                }
                if self.is_source_final() {
                    return Err(self.err(ErrorKind::UnclosedToken, "unclosed DTD declaration"));
                }
                self.conditional.declaration = Some(state);
                return Ok(false);
            }
            if state.expansion.direct
                && self.reparse_deferral
                && !self.is_source_final()
                && self.source().should_defer(
                    self.config
                        .limits
                        .max_token_bytes
                        .saturating_sub(state.expansion.token_bytes(0)),
                )
            {
                self.conditional.declaration = Some(state);
                return Ok(false);
            }
            let text = self.source().remaining();
            let end = if state.expansion.direct {
                let space = text.starts_with(whitespace);
                let end = text[state.word_checked..]
                    .char_indices()
                    .find(|(_, c)| {
                        whitespace(*c) != space || matches!(c, '\'' | '"' | '%' | '<' | '>')
                    })
                    .map_or(text.len(), |(offset, _)| state.word_checked + offset);
                if !space && end == text.len() && !self.is_source_final() {
                    if state.expansion.token_bytes(end) > self.config.limits.max_token_bytes {
                        return Err(self.err(
                            ErrorKind::LimitExceeded,
                            "expanded declaration token limit exceeded",
                        ));
                    }
                    state.word_checked = end;
                    self.source_mut().mark_deferred();
                    self.conditional.declaration = Some(state);
                    return Ok(false);
                }
                state.word_checked = 0;
                end
            } else {
                text.find(['\'', '"', '%', '<', '>']).unwrap_or(text.len())
            };
            if end != 0 {
                if let Some((offset, character)) =
                    text[..end].char_indices().rfind(|(_, c)| whitespace(*c))
                {
                    state.ready = state.expansion.text.len() + offset + character.len_utf8();
                }
                self.append_declaration_token(
                    &mut state.expansion,
                    self.source().lexical_remaining().for_slice(&text[..end]),
                    false,
                )?;
                self.consume(end)?;
                continue;
            }
            match text.as_bytes()[0] {
                b'>' => {
                    self.consume(1)?;
                    if state.semantic.is_some() {
                        state.complete = true;
                        state.ready = state.expansion.text.len();
                        continue;
                    }
                    return self
                        .finish_declaration_composition(oriole_storage::Box::into_inner(state));
                }
                b'<' => {
                    return Err(self.err(ErrorKind::InvalidToken, "markup inside a declaration"));
                }
                b'\'' | b'"' => {
                    if self.reparse_deferral
                        && !self.is_source_final()
                        && self.source().should_defer(
                            self.config
                                .limits
                                .max_token_bytes
                                .saturating_sub(state.expansion.token_bytes(0)),
                        )
                    {
                        self.conditional.declaration = Some(state);
                        return Ok(false);
                    }
                    let quote = text.as_bytes()[0];
                    let checked = state.quote_checked.max(1);
                    let Some(end) = text.as_bytes()[checked..]
                        .iter()
                        .position(|&b| b == quote)
                        .map(|offset| checked + offset + 1)
                    else {
                        if state.expansion.token_bytes(text.len())
                            > self.config.limits.max_token_bytes
                        {
                            return Err(self.err(
                                ErrorKind::LimitExceeded,
                                "expanded declaration token limit exceeded",
                            ));
                        }
                        if self.is_source_final() {
                            return Err(self.err(
                                ErrorKind::UnclosedToken,
                                "quoted declaration token crosses a parameter boundary",
                            ));
                        }
                        state.quote_checked = text.len();
                        self.source_mut().mark_deferred();
                        self.conditional.declaration = Some(state);
                        return Ok(false);
                    };
                    // A nonfinal literal needs a following byte before its token
                    // is complete, matching Expat's declaration tokenizer.
                    if end == text.len() && !self.is_source_final() {
                        if state.expansion.token_bytes(end) > self.config.limits.max_token_bytes {
                            return Err(self.err(
                                ErrorKind::LimitExceeded,
                                "expanded declaration token limit exceeded",
                            ));
                        }
                        state.quote_checked = end - 1;
                        self.source_mut().mark_deferred();
                        self.conditional.declaration = Some(state);
                        return Ok(false);
                    }
                    state
                        .expansion
                        .charge_work(self, size_of::<DeclarationLiteral>())?;
                    let mut parameters = Vec::new_in(self.allocator);
                    for source in &self.sources {
                        if let Some(name) = source
                            .entity_name
                            .as_deref()
                            .and_then(|name| name.strip_prefix('%'))
                        {
                            self.charge_expansion(size_of::<String>() + name.len())?;
                            try_push(&mut parameters, string(name, self.allocator)?)?;
                        }
                    }
                    let offset = state.expansion.text.len();
                    try_push(
                        &mut state.expansion.literals,
                        DeclarationLiteral {
                            offset,
                            normalize: self.sources.len() == 1,
                            parameters,
                        },
                    )?;
                    self.append_declaration_token(
                        &mut state.expansion,
                        self.source().lexical_remaining().for_slice(&text[..end]),
                        false,
                    )?;
                    self.consume(end)?;
                    state.quote_checked = 0;
                    state.ready = state.expansion.text.len();
                }
                b'%' => {
                    if text.len() == 1 && !self.is_source_final() {
                        self.conditional.declaration = Some(state);
                        return Ok(false);
                    }
                    if text[1..].starts_with(whitespace) {
                        self.append_declaration_token(
                            &mut state.expansion,
                            Slice::plain("%"),
                            false,
                        )?;
                        self.consume(1)?;
                        continue;
                    }
                    self.charge_declaration_composition(&mut state)?;
                    let limit = self.config.limits.max_token_bytes;
                    let end =
                        self.source_mut()
                            .scan_reference(limit)
                            .map_err(|(kind, offset)| {
                                self.err_at(kind, "invalid declaration parameter reference", offset)
                            })?;
                    let Some(end) = end else {
                        if self.is_source_final() {
                            return Err(self.err(
                                ErrorKind::InvalidToken,
                                "unclosed declaration parameter reference",
                            ));
                        }
                        self.conditional.declaration = Some(state);
                        return Ok(false);
                    };
                    let name = &self.source().remaining()[1..end];
                    if !self.config.name_rules.is_name(name)
                        || (self.config.namespace_separator.is_some() && name.contains(':'))
                    {
                        return Err(self.err(
                            ErrorKind::InvalidToken,
                            "invalid declaration parameter name",
                        ));
                    }
                    if !self.external_subset && self.sources.len() == 1 {
                        return Err(self.err(
                            ErrorKind::ParameterEntityReference,
                            "parameter reference in an internal subset declaration",
                        ));
                    }
                    if state.expansion.text.ends_with(')') {
                        return Err(self.err(
                            ErrorKind::InvalidToken,
                            "parameter reference cannot delimit a closing model group",
                        ));
                    }
                    self.charge_expansion(end + 1)?;
                    self.append_declaration_token(&mut state.expansion, Slice::plain(" "), true)?;
                    state.ready = state.expansion.text.len();
                    let decoded_name = self
                        .source()
                        .lexical_remaining()
                        .for_slice(name)
                        .decoded(self.allocator)?;
                    let name: &str = &decoded_name;
                    let entity = self.tables.parameter_entities.get(name);
                    if self.parameter_mode == 0 || entity.is_none() {
                        self.charge_expansion(
                            2 * size_of::<crate::PendingEvent>()
                                + size_of::<DeclarationParameter>(),
                        )?;
                        let offset = state.expansion.text.len();
                        let raw_start = state.expansion.raw.len();
                        try_push(
                            &mut state.expansion.parameters,
                            DeclarationParameter {
                                offset,
                                position: self.source().position(end + 1),
                                raw_start,
                                raw_end: raw_start + end + 1,
                                disabled: self.parameter_mode == 0,
                            },
                        )?;
                        state.expansion.raw.append(
                            self.source()
                                .lexical_remaining()
                                .for_slice(&self.source().remaining()[..end + 1]),
                        )?;
                        self.declaration_raw_boundary(&mut state.expansion)?;
                        self.consume(end + 1)?;
                        continue;
                    }
                    if entity.is_some_and(Entity::is_value_open)
                        || self.active_entities.contains(name, true)
                    {
                        return Err(self.err(
                            ErrorKind::RecursiveEntityReference,
                            "recursive declaration parameter entity",
                        ));
                    }
                    if self.sources.len() + self.external_depth + self.inherited_parameter_depth
                        > self.config.limits.max_entity_depth
                    {
                        return Err(self.err(
                            ErrorKind::LimitExceeded,
                            "declaration parameter nesting limit exceeded",
                        ));
                    }
                    let entity = entity.expect("declared parameter");
                    let Some(value) = entity.value.as_ref() else {
                        self.charge_external_identifiers(entity)?;
                        self.charge_expansion(
                            2 * name.len()
                                + 4
                                + size_of::<semantic::Request>()
                                + size_of::<crate::PendingEvent>(),
                        )?;
                        let base = self.copy_external_base(entity.base.as_ref())?;
                        let system_id = entity.system_id.try_clone()?;
                        let public_id = entity.public_id.try_clone()?;
                        let mut source_name = string("%", self.allocator)?;
                        source_name.push_str(name)?;
                        let position = self.source().position(end + 1);
                        self.consume(end + 1)?;
                        state.request = Some(semantic::Request {
                            base,
                            source_name,
                            system_id,
                            public_id,
                            position,
                        });
                        if state.semantic.is_none() {
                            self.charge_expansion(
                                size_of::<semantic::State>()
                                    + grammar::Cursor::maximum_stack_bytes(
                                        self.config.limits.max_depth,
                                    ),
                            )?;
                            state.semantic = Some(semantic::State::new(self));
                        }
                        continue;
                    };
                    self.charge_expansion(
                        value.len() + name.len() + 1 + size_of::<crate::encoding::Source>(),
                    )?;
                    let value = value.try_clone()?;
                    let mut source_name = string("%", self.allocator)?;
                    source_name.push_str(name)?;
                    let position = self.source().position(end + 1);
                    self.consume(end + 1)?;
                    let mut source = crate::encoding::Source::entity(
                        value,
                        source_name,
                        position,
                        self.stack.len(),
                        self.config.name_rules,
                    );
                    source.dtd_fragment = true;
                    self.push_entity_source(source)?;
                }
                _ => unreachable!("declaration delimiter"),
            }
        }
    }

    /// Parse the completed grammar with an anchored source for diagnostics. The
    /// original source, including an unconsumed replacement tail, is restored
    /// on both success and failure; no callback is dispatched in this scope.
    fn finish_declaration_composition(&mut self, mut state: Declaration) -> Result<bool, Error> {
        self.charge_expansion(state.expansion.text.len() + 3)?;
        let mut token = Buffer::plain(string("<!", self.allocator)?);
        token.append(state.expansion.text.view())?;
        token.push('>')?;
        for literal in &mut state.expansion.literals {
            literal
                .parameters
                .retain(|name| !self.active_entities.source_contains(name, true));
        }
        let source_name = self
            .source()
            .entity_name
            .as_ref()
            .map_or_else(|| Ok(String::new_in(self.allocator)), TryClone::try_clone)?;
        let anchored = crate::encoding::Source::entity(
            String::new_in(self.allocator),
            source_name,
            state.position,
            self.stack.len(),
            self.config.name_rules,
        );
        let original = std::mem::replace(self.sources.last_mut().expect("source"), anchored);
        let result = self.parse_subset_expanded(token.view(), 0, Some(state.expansion));
        *self.sources.last_mut().expect("source") = original;
        result.map(|()| true)
    }
}
