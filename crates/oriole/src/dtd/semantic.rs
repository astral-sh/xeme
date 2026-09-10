//! Semantic commits around ordinary external DTD children. Grammar and raw
//! cursors only advance; no declaration is replayed after a callback yields.

use super::*;

#[derive(Debug)]
pub(super) struct Request {
    pub(super) source_name: String,
    pub(super) system_id: Option<String>,
    pub(super) public_id: Option<String>,
    pub(super) position: Position,
}

#[derive(Debug)]
pub(super) struct State {
    grammar: grammar::Cursor,
    raw_offset: usize,
    raw_started: bool,
    literal_index: usize,
    parameter_index: usize,
    element: Option<String>,
    first_attribute: bool,
    enumeration_seen: bool,
    enumeration_type: Option<String>,
    entity_committed: bool,
    reserved: Option<(String, bool)>,
    reservation_checked: bool,
    name_prepared: bool,
    public_prepared: bool,
    system_prepared: bool,
    duplicate: Option<(bool, bool)>,
    notation_capture: Option<bool>,
    notation_committed: bool,
    notation_final_default: bool,
    previously_skipped: bool,
    pub(super) waited_prefix: usize,
}

impl State {
    pub(super) fn new(parser: &Parser) -> Self {
        Self {
            grammar: grammar::Cursor::new(
                parser.allocator,
                parser.config.namespace_separator.is_some(),
                parser.config.limits.max_depth,
            ),
            raw_offset: 0,
            raw_started: false,
            literal_index: 0,
            parameter_index: 0,
            element: None,
            first_attribute: true,
            enumeration_seen: false,
            enumeration_type: None,
            entity_committed: false,
            reserved: None,
            reservation_checked: false,
            name_prepared: false,
            public_prepared: false,
            system_prepared: false,
            duplicate: None,
            notation_capture: None,
            notation_committed: false,
            notation_final_default: false,
            previously_skipped: parser.declarations_skipped,
            waited_prefix: 0,
        }
    }
}

impl DeclarationExpansion {
    fn raw_at(&self, offset: usize) -> usize {
        let index = self
            .raw_offsets
            .partition_point(|&(grammar, _)| grammar <= offset);
        if index == 0 {
            offset
        } else {
            let (grammar, raw) = self.raw_offsets[index - 1];
            raw + offset - grammar
        }
    }
}

impl Parser {
    /// Return true only after the closing delimiter's final projection. Each
    /// intermediate return lets queued callbacks run before more grammar work.
    pub(super) fn advance_declaration_semantics(
        &mut self,
        declaration: &mut composition::Declaration,
    ) -> Result<bool, Error> {
        let mut state = declaration.semantic.take().expect("semantic declaration");
        let progress = state
            .grammar
            .advance(
                &declaration.expansion.text[..declaration.ready],
                declaration.complete,
            )
            .map_err(|(kind, _offset)| self.err(kind, "invalid declaration grammar"))?;
        if state.notation_capture.is_none() && state.grammar.notation_name().is_some() {
            state.notation_capture = Some(self.notation_handler_enabled);
        }
        self.prepare_declaration_reservation(declaration, &mut state)?;
        let done = match progress {
            grammar::Progress::EnumerationMember {
                start,
                end,
                notation,
            } => {
                state.enumeration_seen = true;
                if self.attlist_handler_enabled && !self.declarations_skipped {
                    self.charge_expansion(end - start + 10)?;
                    if let Some(kind) = &mut state.enumeration_type {
                        kind.push('|')?;
                    } else {
                        state.enumeration_type = Some(string(
                            if notation { "NOTATION(" } else { "(" },
                            self.allocator,
                        )?);
                    }
                    state
                        .enumeration_type
                        .as_mut()
                        .expect("enumeration callback")
                        .push_str(&declaration.expansion.text[start..end])?;
                }
                false
            }
            grammar::Progress::Commit(commit) => {
                self.commit_declaration_unit(declaration, &mut state, commit)?;
                false
            }
            grammar::Progress::NeedMore => {
                state.waited_prefix = declaration.ready;
                self.classify_declaration_duplicate(declaration, &mut state);
                self.project_declaration_prefix(declaration, &mut state, false)?;
                if self.pending.is_empty() && declaration.request.is_some() {
                    self.reserve_declaration_name(declaration, &mut state)?;
                    let request = declaration
                        .request
                        .take()
                        .expect("checked external grammar request");
                    self.active_parameter_reference = Some((request.source_name, request.position));
                    if self.parameter_read.is_none() {
                        self.charge_expansion(size_of::<AtomicBool>())?;
                        self.parameter_read =
                            Some(Shared::try_new_in(AtomicBool::new(false), self.allocator)?);
                    }
                    self.parameter_read
                        .as_ref()
                        .expect("parameter read marker")
                        .store(false, Ordering::Relaxed);
                    let mut raw = self
                        .active_parameter_reference
                        .as_ref()
                        .expect("active reference")
                        .0
                        .try_clone()?;
                    raw.push(';')?;
                    self.has_external_subset = true;
                    self.emit(
                        EventKind::ExternalEntityReference(oriole_storage::try_box(
                            crate::ExternalEntityReference {
                                context: None,
                                system_id: request.system_id,
                                public_id: request.public_id,
                            },
                            self.allocator,
                        )?),
                        request.position,
                    )?;
                    self.pending
                        .back_mut()
                        .expect("external grammar reference")
                        .raw = Some(raw);
                }
                false
            }
            grammar::Progress::Done => {
                self.project_declaration_prefix(declaration, &mut state, true)?;
                true
            }
        };
        declaration.semantic = Some(state);
        Ok(done)
    }

    /// Apply missing-reference state in token order before publishing the
    /// declaration's name or each external identifier to a child snapshot.
    fn prepare_declaration_reservation(
        &mut self,
        declaration: &composition::Declaration,
        state: &mut State,
    ) -> Result<(), Error> {
        if !state.name_prepared
            && let Some((_, end, _)) = state.grammar.entity_name()
        {
            self.classify_declaration_duplicate(declaration, state);
            self.semantic_parameters_through(declaration, state, end)?;
            self.reserve_declaration_name(declaration, state)?;
            state.name_prepared = true;
        }
        let (system, public) = state.grammar.entity_ids();
        for (span, is_public) in [(public, true), (system, false)] {
            let Some((start, end)) = span else {
                continue;
            };
            if if is_public {
                state.public_prepared
            } else {
                state.system_prepared
            } {
                continue;
            }
            self.semantic_parameters_through(declaration, state, end)?;
            if is_public {
                state.public_prepared = true;
            } else {
                state.system_prepared = true;
            }
            if self.declarations_skipped {
                continue;
            }
            let Some((name, parameter)) = &state.reserved else {
                continue;
            };
            self.charge_expansion(end - start)?;
            let value = &declaration.expansion.text[start + 1..end - 1];
            let value = if is_public {
                let mut normalized = String::new_in(self.allocator);
                for part in value.split_ascii_whitespace() {
                    if !normalized.is_empty() {
                        normalized.push(' ')?;
                    }
                    normalized.push_str(part)?;
                }
                normalized
            } else {
                let index = declaration
                    .expansion
                    .literals
                    .partition_point(|literal| literal.offset < start);
                let normalize = declaration
                    .expansion
                    .literals
                    .get(index)
                    .filter(|literal| literal.offset == start)
                    .map_or(self.sources.len() == 1, |literal| literal.normalize);
                if normalize {
                    normalize_newlines(value, self.allocator)?
                } else {
                    string(value, self.allocator)?
                }
            };
            let table = if *parameter {
                &mut self.parameter_entities
            } else {
                &mut self.entities
            };
            let entity = table.get_mut(name).expect("owned reserved entity");
            if is_public {
                entity.public_id = Some(value);
            } else {
                entity.system_id = Some(value);
            }
        }
        Ok(())
    }

    fn semantic_parameters_through(
        &mut self,
        declaration: &composition::Declaration,
        state: &mut State,
        offset: usize,
    ) -> Result<(), Error> {
        let mut cursor = self.semantic_cursor(&declaration.expansion, state, offset, offset);
        self.declaration_parameters(&mut cursor)?;
        Self::save_semantic_cursor(state, &cursor);
        Ok(())
    }

    fn classify_declaration_duplicate(
        &self,
        declaration: &composition::Declaration,
        state: &mut State,
    ) {
        if state.entity_committed || state.reservation_checked {
            return;
        }
        if let Some((start, end, parameter)) = state.grammar.entity_name() {
            let table = if parameter {
                &self.parameter_entities
            } else {
                &self.entities
            };
            if table.contains_key(&declaration.expansion.text[start..end]) {
                state.reservation_checked = true;
                state.duplicate = Some((false, false));
            }
        }
    }

    /// A parent declaration owns the first table slot before its child parses.
    /// A reserved parameter has no replacement yet; its value-context reading
    /// is empty, whereas a grammar reference still requests an external child.
    fn reserve_declaration_name(
        &mut self,
        declaration: &composition::Declaration,
        state: &mut State,
    ) -> Result<(), Error> {
        if state.entity_committed || state.reservation_checked || self.declarations_skipped {
            return Ok(());
        }
        let Some((start, end, parameter)) = state.grammar.entity_name() else {
            return Ok(());
        };
        state.reservation_checked = true;
        let name = &declaration.expansion.text[start..end];
        let table = if parameter {
            &self.parameter_entities
        } else {
            &self.entities
        };
        if table.contains_key(name) {
            state.duplicate = Some((false, false));
            return Ok(());
        }
        if self.entities.len() + self.parameter_entities.len() >= self.config.limits.max_entities {
            return Err(self.err(
                ErrorKind::LimitExceeded,
                "entity declaration count limit exceeded",
            ));
        }
        self.charge_expansion(2 * name.len() + size_of::<Entity>())?;
        let name = string(name, self.allocator)?;
        let table = if parameter {
            &mut self.parameter_entities
        } else {
            &mut self.entities
        };
        try_insert(
            table,
            name.try_clone()?,
            Entity {
                value: None,
                system_id: None,
                public_id: None,
                notation: None,
                declared_in_parameter_entity: self.external_subset || self.sources.len() > 1,
                value_open: None,
            },
        )?;
        state.reserved = Some((name, parameter));
        Ok(())
    }

    fn semantic_cursor<'a>(
        &self,
        expansion: &'a DeclarationExpansion,
        state: &State,
        start: usize,
        end: usize,
    ) -> Cursor<'a> {
        let mut cursor = Cursor::new(
            &expansion.text[start..end],
            self.config.namespace_separator.is_some(),
        );
        cursor.initial_len = end;
        cursor.literals = &expansion.literals;
        cursor.literal_index = state.literal_index;
        let parameter_end = expansion
            .parameters
            .partition_point(|parameter| parameter.offset <= end);
        cursor.parameters = &expansion.parameters[..parameter_end];
        cursor.parameter_index = state.parameter_index;
        cursor.raw = &expansion.raw[..expansion.raw_at(end)];
        cursor.raw_offset = state.raw_offset;
        cursor.raw_started = state.raw_started;
        cursor.raw_event = self.pending.len();
        cursor.closes_declaration = false;
        cursor.silent_defaults = !self.default_events;
        cursor.parameter_defaults = !state.previously_skipped;
        cursor.entity_defaults = state.grammar.kind() == Some(grammar::Kind::Entity);
        cursor.projection = state.grammar.kind();
        cursor.duplicate_defaults = state.duplicate;
        cursor
    }

    fn save_semantic_cursor(state: &mut State, cursor: &Cursor<'_>) {
        state.raw_offset = cursor.raw_offset;
        state.raw_started = cursor.raw_started;
        state.literal_index = cursor.literal_index;
        state.parameter_index = cursor.parameter_index;
        state.duplicate = cursor.duplicate_defaults;
    }

    fn project_declaration_prefix(
        &mut self,
        declaration: &composition::Declaration,
        state: &mut State,
        closing: bool,
    ) -> Result<(), Error> {
        let offset = state.grammar.offset();
        let mut cursor = self.semantic_cursor(&declaration.expansion, state, offset, offset);
        self.declaration_parameters(&mut cursor)?;
        {
            let end = cursor.raw.len();
            self.charge_expansion(end.saturating_sub(cursor.raw_offset) + 3)?;
            let unconditional = self.declarations_skipped
                && matches!(
                    state.grammar.kind(),
                    Some(grammar::Kind::Entity | grammar::Kind::Attlist)
                )
                || (state.notation_capture == Some(false)
                    && (!state.notation_committed || (closing && state.notation_final_default)));
            self.declaration_default_segment(
                &mut cursor,
                end,
                closing,
                unconditional,
                declaration.position,
            )?;
        }
        Self::save_semantic_cursor(state, &cursor);
        Ok(())
    }

    fn commit_declaration_unit(
        &mut self,
        declaration: &mut composition::Declaration,
        state: &mut State,
        commit: grammar::Commit,
    ) -> Result<(), Error> {
        let first_event = self.pending.len();
        // Ended replacement names stay attached to literal provenance. Active
        // names are checked via sources instead, so depth is not counted twice.
        for literal in declaration
            .expansion
            .literals
            .iter_mut()
            .skip(state.literal_index)
        {
            if literal.offset >= commit.end {
                break;
            }
            literal.parameters.retain(|name| {
                !self.sources.iter().any(|source| {
                    source
                        .entity_name
                        .as_deref()
                        .and_then(|value| value.strip_prefix('%'))
                        == Some(name.as_str())
                })
            });
        }
        if commit.kind == grammar::Kind::Entity {
            self.semantic_parameters_through(declaration, state, commit.end)?;
        }
        if commit.kind == grammar::Kind::Entity
            && !self.declarations_skipped
            && let Some((name, parameter)) = state.reserved.take()
        {
            // Child table merges preserve this first slot. Remove only our own
            // reservation before the ordinary declaration commit fills it.
            let table = if parameter {
                &mut self.parameter_entities
            } else {
                &mut self.entities
            };
            table.remove(&name);
        }
        let skipped_before = self.declarations_skipped;
        let mut cursor =
            self.semantic_cursor(&declaration.expansion, state, commit.start, commit.end);
        let position = declaration.position;
        if commit.kind == grammar::Kind::Attlist {
            if state.element.is_none() {
                let (start, end) = state.grammar.element_span().expect("ATTLIST element");
                self.charge_expansion(end - start)?;
                state.element = Some(string(
                    &declaration.expansion.text[start..end],
                    self.allocator,
                )?);
            }
            if let Some(kind) = &mut state.enumeration_type {
                self.charge_expansion(1)?;
                kind.push(')')?;
            }
            let callback = if state.enumeration_seen {
                AttributeCallback::Enumeration(state.enumeration_type.as_deref())
            } else {
                AttributeCallback::Complete
            };
            self.attlist_attribute(
                &mut cursor,
                position,
                state.element.as_ref().expect("ATTLIST element"),
                &mut state.first_attribute,
                callback,
            )?;
        } else {
            cursor
                .name()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            cursor
                .require_space()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            match commit.kind {
                grammar::Kind::Entity => {
                    self.entity_declaration(&mut cursor, position)?;
                    state.entity_committed = true;
                }
                grammar::Kind::Element => self.element_declaration(&mut cursor, position, 2)?,
                grammar::Kind::Notation => self.notation_declaration(
                    &mut cursor,
                    position,
                    state.notation_capture != Some(false),
                )?,
                grammar::Kind::Attlist => unreachable!("attribute handled above"),
            }
        }
        if self.value_state.is_some() {
            self.charge_expansion(declaration.expansion.raw_at(commit.end) + 2)?;
        }
        if let Some(value) = &mut self.value_state {
            let pending = value
                .declaration
                .as_mut()
                .expect("pending declaration value");
            let raw_end = declaration.expansion.raw_at(commit.end);
            let mut raw = string("<!", self.allocator)?;
            raw.push_str(&declaration.expansion.raw[..raw_end])?;
            let quote = raw.find(['\'', '"']).expect("entity value quote");
            pending.raw = raw;
            pending.quote = quote;
            pending.prefix_start = if cursor.raw_started {
                cursor.raw_offset + 2
            } else {
                0
            };
            pending.closes_declaration = false;
            cursor.raw_offset = raw_end;
            cursor.raw_started = true;
        } else {
            let raw_end = cursor.raw.len();
            self.charge_expansion(raw_end.saturating_sub(cursor.raw_offset) + 2)?;
            let unconditional = skipped_before
                && matches!(commit.kind, grammar::Kind::Entity | grammar::Kind::Attlist)
                || state.notation_capture == Some(false)
                || (commit.kind == grammar::Kind::Attlist
                    && state.enumeration_seen
                    && state.enumeration_type.is_none());
            self.declaration_default_segment(&mut cursor, raw_end, false, unconditional, position)?;
        }
        if commit.kind == grammar::Kind::Attlist {
            state.enumeration_seen = false;
            state.enumeration_type = None;
        }
        if commit.kind == grammar::Kind::Notation {
            state.notation_committed = true;
            state.notation_final_default =
                declaration.complete && state.notation_capture == Some(false);
        }
        // An already projected prefix can leave an empty semantic unit. Give
        // that callback explicit empty raw bytes rather than reusing the prior
        // external reference's current_raw fallback.
        for pending in self.pending.iter_mut().skip(first_event) {
            if pending.raw.is_none() {
                pending.raw = Some(String::new_in(self.allocator));
            }
        }
        Self::save_semantic_cursor(state, &cursor);
        Ok(())
    }
}
