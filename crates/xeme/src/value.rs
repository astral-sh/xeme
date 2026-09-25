//! Owned continuations for external parameter entities in entity values.

use std::sync::OnceLock;
use std::sync::atomic::Ordering;

use xeme_storage::{Shared, String, TryClone, TryLock, TryLockGuard, Vec, try_box, try_push};

use crate::lexical::{Buffer, Slice};
use crate::value_lexer::{ValueScan, ValueScanner};
use crate::{Entity, Error, ErrorKind, EventKind, Parser, Position, character_reference, string};

pub(crate) enum Build {
    Complete(String, bool),
    Pending(xeme_storage::Box<State>),
}

#[derive(Debug)]
pub(crate) struct Frame {
    pub(crate) text: Buffer,
    pub(crate) offset: usize,
    pub(crate) name: Option<String>,
    pub(crate) normalize: bool,
}

#[derive(Debug)]
pub(crate) struct Declaration {
    pub(crate) name: String,
    pub(crate) parameter: bool,
    pub(crate) origin: bool,
    pub(crate) position: Position,
    pub(crate) raw: Buffer,
    pub(crate) quote: usize,
    pub(crate) prefix_start: usize,
    pub(crate) tail_parameters: Vec<crate::dtd::DeclarationParameter>,
    pub(crate) prefix_sent: bool,
    pub(crate) closes_declaration: bool,
    pub(crate) suffix_error: Option<Error>,
}

#[derive(Debug)]
struct Output {
    text: String,
    standalone: bool,
    skipped: bool,
}

#[derive(Debug)]
struct Request {
    base: Option<Vec<u8>>,
    name: String,
    system_id: Option<String>,
    public_id: Option<String>,
    position: Position,
    delivered: bool,
}

#[derive(Debug)]
pub(crate) struct State {
    pub(crate) frames: Vec<Frame>,
    pub(crate) active: crate::active::ActiveEntities,
    named_frames: usize,
    pub(crate) declaration: Option<Declaration>,
    pub(crate) declaring: Option<String>,
    pub(crate) parameters: Vec<String>,
    output: Shared<TryLock<Output>>,
    read: Shared<crate::ParameterState>,
    request: Option<Request>,
    scanner: Option<ValueScanner>,
    content_start: usize,
    child: bool,
}

impl State {
    fn push_frame(&mut self, frame: Frame) -> Result<(), Error> {
        self.frames
            .try_reserve(1)
            .map_err(xeme_storage::AllocError::from)?;
        if let Some(name) = frame.name.as_deref() {
            self.active.insert(name, true, true)?;
            self.named_frames += 1;
        }
        self.frames.push(frame);
        Ok(())
    }

    fn pop_frame(&mut self) {
        let frame = self.frames.pop().expect("value frame");
        if let Some(name) = frame.name.as_deref() {
            self.active.remove_source(name, true);
            self.named_frames -= 1;
        }
    }

    fn clear_frames(&mut self) {
        while !self.frames.is_empty() {
            self.pop_frame();
        }
    }
}

impl Parser {
    pub(crate) fn value_context_byte_index(&self) -> Option<usize> {
        let state = self.value_state.as_ref()?;
        state
            .declaration
            .as_ref()
            .map(|declaration| declaration.position.byte_index)
            .into_iter()
            .chain(
                state
                    .request
                    .as_ref()
                    .map(|request| request.position.byte_index),
            )
            .min()
    }

    pub(crate) fn begin_external_value(
        &self,
        ready: (String, bool),
        frames: Vec<Frame>,
        declaring: Option<&str>,
        parameters: &[String],
        name: &str,
        entity: &Entity,
    ) -> Result<xeme_storage::Box<State>, Error> {
        self.charge_expansion(size_of::<State>() + size_of::<TryLock<Output>>())?;
        let mut inherited = Vec::new_in(self.allocator);
        for parameter in parameters {
            self.charge_expansion(size_of::<String>() + parameter.len())?;
            try_push(&mut inherited, parameter.try_clone()?)?;
        }
        let mut active = crate::active::ActiveEntities::new(
            self.allocator,
            self.tables.parameter_entities.hasher(),
        );
        for name in parameters {
            active.insert(name, true, false)?;
        }
        let mut named_frames = 0;
        for frame in &frames {
            if let Some(name) = frame.name.as_deref() {
                active.insert(name, true, true)?;
                named_frames += 1;
            }
        }
        let output = Shared::try_new_in(
            TryLock::new(Output {
                text: ready.0,
                standalone: self.standalone,
                skipped: ready.1 && !self.standalone,
            }),
            self.allocator,
        )?;
        let read = self.shared_parameter_state()?.clone();
        Ok(try_box(
            State {
                frames,
                active,
                named_frames,
                declaration: None,
                declaring: declaring
                    .map(|name| string(name, self.allocator))
                    .transpose()?,
                parameters: inherited,
                output,
                read,
                request: Some(self.value_request(name, entity)?),
                scanner: None,
                content_start: 0,
                child: false,
            },
            self.allocator,
        )?)
    }

    fn value_request(&self, name: &str, entity: &Entity) -> Result<Request, Error> {
        self.charge_expansion(size_of::<Request>() + name.len())?;
        self.charge_external_identifiers(entity)?;
        Ok(Request {
            base: self.copy_external_base(entity.base.as_ref())?,
            name: string(name, self.allocator)?,
            system_id: entity.system_id.try_clone()?,
            public_id: entity.public_id.try_clone()?,
            position: self.here(),
            delivered: false,
        })
    }

    /// Borrow the output shared with value children without waiting, reporting
    /// contention at this parser's position. Drop the guard before callbacks or yields.
    fn value_output<'a>(&self, state: &'a State) -> Result<TryLockGuard<'a, Output>, Error> {
        state.output.try_lock().ok_or_else(|| {
            self.err(
                ErrorKind::ExternalEntityHandling,
                "entity value output is unavailable",
            )
        })
    }

    fn value_append(&self, state: &State, text: Slice<'_>, normalize: bool) -> Result<(), Error> {
        let mut output = self.value_output(state)?;
        self.append_entity_value(&mut output.text, text, normalize)
    }

    fn value_skip(&self, state: &State) -> Result<(), Error> {
        let mut output = self.value_output(state)?;
        output.skipped = !output.standalone;
        Ok(())
    }

    /// A value child shares only an output channel, never its lexical input.
    pub(crate) fn inherit_value_context(&self, child: &mut Self) -> Result<bool, Error> {
        let Some(state) = &self.value_state else {
            return Ok(false);
        };
        let Some(request) = state.request.as_ref().filter(|request| request.delivered) else {
            return Ok(false);
        };
        let source_names = self.sources.iter().filter_map(|source| {
            source
                .entity_name
                .as_deref()
                .and_then(|name| name.strip_prefix('%'))
        });
        for name in source_names
            .chain(
                state
                    .frames
                    .iter()
                    .filter_map(|frame| frame.name.as_deref()),
            )
            .chain(state.parameters.iter().map(String::as_str))
            .chain(state.declaring.as_deref())
            .chain(std::iter::once(request.name.as_str()))
        {
            self.charge_expansion(size_of::<String>() + name.len() + 1)?;
            let mut name_copy = string("%", self.allocator)?;
            name_copy.push_str(name)?;
            child.inherit_entity_name(name_copy)?;
        }
        child.inherited_parameter_depth = self.inherited_parameter_depth + self.sources.len() - 1
            + state.named_frames
            + state.parameters.len();
        child.parameter_state = OnceLock::from(state.read.clone());
        child.standalone = self.value_output(state)?.standalone;
        self.charge_expansion(size_of::<State>())?;
        child.value_state = Some(try_box(
            State {
                frames: Vec::new_in(self.allocator),
                active: crate::active::ActiveEntities::new(
                    self.allocator,
                    child.active_entities.names.hasher(),
                ),
                named_frames: 0,
                declaration: None,
                declaring: None,
                parameters: Vec::new_in(self.allocator),
                output: state.output.clone(),
                read: state.read.clone(),
                request: None,
                scanner: Some(ValueScanner::new(self.config.name_rules)),
                content_start: 0,
                child: true,
            },
            self.allocator,
        )?);
        Ok(true)
    }

    pub(crate) fn is_external_value(&self) -> bool {
        self.value_state.as_ref().is_some_and(|state| state.child)
    }

    /// Advance one continuation step, releasing every borrow before yielding.
    pub(crate) fn continue_value(&mut self) -> Result<bool, Error> {
        // The C adapter may resolve an unknown protocol encoding and retry.
        // Retain the child mode and channel until that recovery is complete.
        if let Some((ErrorKind::UnknownEncoding, message)) = self.decoding_error {
            return Err(self.err(ErrorKind::UnknownEncoding, message));
        }
        let mut state = self.value_state.take().expect("pending entity value");
        if let Some(scanner) = &mut state.scanner {
            let text = self.source().remaining();
            if text.len() > self.config.limits.max_token_bytes {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "external value byte limit exceeded",
                ));
            }
            match scanner
                .scan(
                    text,
                    self.is_source_final(),
                    self.config.namespace_separator.is_some(),
                )
                .map_err(|(kind, offset)| {
                    self.err_at(kind, "invalid external entity value", offset)
                })? {
                ValueScan::NeedMore => {
                    self.value_state = Some(state);
                    return Ok(false);
                }
                ValueScan::XmlDeclaration { start, end } => {
                    self.charge_expansion(end - start)?;
                    let token = self
                        .source()
                        .lexical_remaining()
                        .for_slice(&text[start..end])
                        .to_owned(self.allocator)?;
                    let position = self.source().position_at(start, end - start);
                    self.account_source(end)?;
                    state.content_start = end;
                    self.value_state = Some(state);
                    self.parse_pi(token.view(), position)?;
                    let state = self.value_state.as_ref().expect("value declaration");
                    let mut output = self.value_output(state)?;
                    output.standalone |= self.standalone;
                    self.standalone = output.standalone;
                    drop(output);
                    self.event_raw(&token.view().decoded(self.allocator)?)?;
                    return Ok(true);
                }
                ValueScan::Complete => {
                    if let Some((kind, message)) = self.decoding_error {
                        return Err(self.err(kind, message));
                    }
                    self.charge_expansion(text.len() - state.content_start + size_of::<Frame>())?;
                    let frame = Frame {
                        text: self
                            .source()
                            .lexical_remaining()
                            .for_slice(&text[state.content_start..])
                            .to_owned(self.allocator)?,
                        offset: 0,
                        name: None,
                        normalize: true,
                    };
                    state.push_frame(frame)?;
                    state.scanner = None;
                }
            }
        }
        if let Some(declaration) = &mut state.declaration
            && !declaration.prefix_sent
        {
            declaration.prefix_sent = true;
            if self.default_events && declaration.prefix_start < declaration.quote {
                let declarations = if declaration.parameter {
                    &self.tables.parameter_entities
                } else {
                    &self.tables.entities
                };
                let kind = if declarations.contains_key(&declaration.name) {
                    EventKind::EntityDeclarationDuplicate {
                        external: false,
                        unparsed: false,
                    }
                } else {
                    EventKind::EntityDeclarationPrefix
                };
                self.emit(kind, declaration.position)?;
                self.event_raw(
                    &declaration
                        .raw
                        .view()
                        .for_slice(&declaration.raw[declaration.prefix_start..declaration.quote])
                        .decoded(self.allocator)?,
                )?;
                self.value_state = Some(state);
                return Ok(true);
            }
        }
        if let Some(mut request) = state.request.take() {
            if !request.delivered {
                state.read.read.store(false, Ordering::Relaxed);
                self.emit(
                    EventKind::ExternalEntityReference(xeme_storage::try_box(
                        crate::ExternalEntityReference {
                            base: request.base.take(),
                            context: None,
                            system_id: request.system_id.try_clone()?,
                            public_id: request.public_id.try_clone()?,
                        },
                        self.allocator,
                    )?),
                    request.position,
                )?;
                self.event_raw("")?;
                request.delivered = true;
                state.request = Some(request);
                self.value_state = Some(state);
                return Ok(true);
            }
            if !state.read.read.load(Ordering::Relaxed) {
                self.value_skip(&state)?;
            }
        }
        self.store_value(&mut state)?;
        if state.request.is_some() {
            self.value_state = Some(state);
            return Ok(true);
        }
        if state.child {
            self.finished = true;
            self.value_state = Some(state);
            return Ok(true);
        }
        self.finish_value_declaration(&mut state)?;
        Ok(true)
    }

    fn store_value(&mut self, state: &mut State) -> Result<(), Error> {
        while let Some(frame) = state.frames.last() {
            let rest = &frame.text[frame.offset..];
            let start = rest.find(['&', '%']).unwrap_or(rest.len());
            if state.child && frame.name.is_none() {
                self.account_source(state.content_start + frame.offset + start)?;
            } else if frame.name.is_some() {
                self.account_entity_bytes(start, true)?;
            }
            self.value_append(
                state,
                frame.text.view().for_slice(&rest[..start]),
                frame.normalize,
            )?;
            state.frames.last_mut().expect("value frame").offset += start;
            let frame = state.frames.last().expect("value frame");
            let rest = &frame.text[frame.offset..];
            if rest.is_empty() {
                state.pop_frame();
                continue;
            }
            let end = rest.find(';').ok_or_else(|| {
                self.err(ErrorKind::InvalidToken, "unclosed entity value reference")
            })?;
            if state.child && frame.name.is_none() {
                self.account_source(state.content_start + frame.offset + end + 1)?;
            } else if frame.name.is_some() {
                self.account_entity_bytes(end + 1, true)?;
            }
            let name = &rest[1..end];
            if rest.starts_with('&') && name.starts_with('#') {
                let value = character_reference(name)
                    .map_err(|(kind, _)| self.err(kind, "invalid character reference"))?
                    .expect("numeric reference");
                self.value_append(state, Slice::plain(value.encode_utf8(&mut [0; 4])), false)?;
                state.frames.last_mut().expect("value frame").offset += end + 1;
                continue;
            }
            if !self.config.name_rules.is_name(name)
                || (self.config.namespace_separator.is_some() && name.contains(':'))
            {
                return Err(self.err(ErrorKind::InvalidToken, "invalid entity value reference"));
            }
            if rest.starts_with('&') {
                self.value_append(state, frame.text.view().for_slice(&rest[..end + 1]), false)?;
                state.frames.last_mut().expect("value frame").offset += end + 1;
                continue;
            }
            let decoded_name = frame.text.view().for_slice(name).decoded(self.allocator)?;
            let name: &str = &decoded_name;
            self.charge_expansion(end + 1 + size_of::<Frame>())?;
            let entity = self.tables.parameter_entities.get(name);
            if entity.is_some_and(Entity::is_value_open)
                || state.declaring.as_deref() == Some(name)
                || state.active.contains(name, true)
                || self.active_entities.contains(name, true)
            {
                return Err(self.err(
                    ErrorKind::RecursiveEntityReference,
                    "recursive entity value",
                ));
            }
            let Some(entity) = entity else {
                self.value_skip(state)?;
                state.frames.last_mut().expect("value frame").offset += end + 1;
                if state.child {
                    state.clear_frames();
                }
                continue;
            };
            if state.named_frames
                + self.sources.len()
                + self.external_depth
                + self.inherited_parameter_depth
                + state.parameters.len()
                > self.config.limits.max_entity_depth
            {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "entity value nesting limit exceeded",
                ));
            }
            if let Some(value) = &entity.value {
                // Expat's external value processor queues this internal entity
                // without draining its value stack. The shared DTD keeps it open
                // after this child finishes or is freed, so later reads recurse.
                if state.child && self.config.expat_external_value_compatibility {
                    entity
                        .value_open
                        .as_ref()
                        .expect("internal parameter entity open state")
                        .store(true, Ordering::Relaxed);
                    state.clear_frames();
                    continue;
                }
                self.charge_expansion(value.len())?;
                let frame = Frame {
                    text: Buffer::plain(value.try_clone()?),
                    offset: 0,
                    name: Some(string(name, self.allocator)?),
                    normalize: false,
                };
                state.frames.last_mut().expect("value frame").offset += end + 1;
                state.push_frame(frame)?;
            } else if entity.system_id.is_none() {
                state.frames.last_mut().expect("value frame").offset += end + 1;
                if state.child {
                    state.clear_frames();
                }
            } else {
                let request = self.value_request(name, entity)?;
                state.frames.last_mut().expect("value frame").offset += end + 1;
                state.request = Some(request);
                return Ok(());
            }
        }
        Ok(())
    }

    fn finish_value_declaration(&mut self, state: &mut State) -> Result<(), Error> {
        let declaration = state.declaration.take().expect("value declaration");
        let mut output = self.value_output(state)?;
        let value = std::mem::replace(&mut output.text, String::new_in(self.allocator));
        let skipped = output.skipped;
        self.standalone = output.standalone;
        drop(output);
        let first_event = self.pending.len();
        let count = self.tables.entities.len() + self.tables.parameter_entities.len();
        let declarations = if declaration.parameter {
            &self.tables.parameter_entities
        } else {
            &self.tables.entities
        };
        if !declarations.contains_key(&declaration.name) {
            if count >= self.entity_limit() {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "entity declaration count limit exceeded",
                ));
            }
            let value_open = self.new_parameter_value_open(declaration.parameter)?;
            let declarations = if declaration.parameter {
                &mut self.tables.parameter_entities
            } else {
                &mut self.tables.entities
            };
            xeme_storage::try_insert(
                declarations,
                declaration.name.try_clone()?,
                Entity {
                    base: None,
                    value: Some(value.try_clone()?),
                    system_id: None,
                    public_id: None,
                    notation: None,
                    declared_in_parameter_entity: declaration.origin,
                    value_open,
                },
            )?;
            self.emit(
                EventKind::EntityDeclaration(xeme_storage::try_box(
                    crate::EntityDeclaration {
                        base: None,
                        name: declaration.name,
                        parameter: declaration.parameter,
                        value: Some(value),
                        system_id: None,
                        public_id: None,
                        notation: None,
                    },
                    self.allocator,
                )?),
                declaration.position,
            )?;
        } else if self.default_events {
            self.emit(
                EventKind::EntityDeclarationDuplicate {
                    external: false,
                    unparsed: false,
                },
                declaration.position,
            )?;
        }
        self.set_declarations_skipped(skipped);
        self.has_external_subset = true;
        self.finish_value_raw(
            declaration.raw.view(),
            declaration.quote,
            &declaration.tail_parameters,
            first_event,
            declaration.position,
            declaration.closes_declaration,
        )?;
        if let Some(error) = declaration.suffix_error {
            return Err(error);
        }
        Ok(())
    }
}
