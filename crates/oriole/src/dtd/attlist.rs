//! Owned continuation for complete ATTLIST tokens without grammar replacements.
//! Lexical prefixes and attribute definitions are published before callbacks.

use super::*;

#[derive(Debug)]
pub(super) struct State {
    token: Buffer,
    grammar: grammar::Cursor,
    position: crate::encoding::PositionCursor,
    element: Option<String>,
    ready: usize,
    fragment_start: usize,
    source_bytes: usize,
    first_attribute: bool,
    enumeration_seen: bool,
    enumeration_type: Option<String>,
    finish_fragment: bool,
}

impl Parser {
    pub(super) fn start_attlist(
        &mut self,
        token: Buffer,
        source_bytes: usize,
    ) -> Result<bool, Error> {
        self.declaration_allowed = false;
        self.conditional.attlist = Some(oriole_storage::try_box(
            State {
                token,
                grammar: grammar::Cursor::new(
                    self.allocator,
                    self.config.namespace_separator.is_some(),
                    self.config.limits.max_depth,
                    self.config.name_rules,
                ),
                position: self.source().position_cursor(),
                element: None,
                ready: 0,
                fragment_start: 0,
                source_bytes,
                first_attribute: true,
                enumeration_seen: false,
                enumeration_type: None,
                finish_fragment: false,
            },
            self.allocator,
        )?);
        self.continue_attlist()
    }

    pub(crate) fn has_attlist(&self) -> bool {
        self.conditional.attlist.is_some()
    }

    /// Borrow only owned token data within a core operation. The table scope
    /// ends before callbacks, and input feeds cannot invalidate this buffer.
    pub(crate) fn continue_attlist(&mut self) -> Result<bool, Error> {
        debug_assert!(self.in_doctype || self.external_subset);
        debug_assert!(!self.source().remaining().is_empty());
        let mut owner = self.conditional.attlist.take().expect("pending ATTLIST");
        let state = &mut *owner;
        loop {
            let length = state.token.len() - 3;
            if state.finish_fragment {
                state.finish_fragment = false;
                if self.default_events
                    && (!self.attlist_handler_enabled || self.declarations_skipped())
                {
                    self.attlist_raw(state, state.ready + 2, self.declarations_skipped())?;
                    self.conditional.attlist = Some(owner);
                    return Ok(true);
                }
            }
            if state.ready == 0 || state.fragment_start == state.ready + 2 {
                if state.ready == length {
                    let grammar = state
                        .token
                        .view()
                        .for_slice(&state.token[2..state.token.len() - 1]);
                    match state.grammar.advance_lexical(grammar, true) {
                        Ok(grammar::Progress::Done) => {}
                        Ok(_) => unreachable!("complete ATTLIST prefix has drained all commits"),
                        Err((kind, offset)) => {
                            return Err(self.err_at(
                                kind,
                                "invalid ATTLIST declaration",
                                offset + 2,
                            ));
                        }
                    }
                    if self.default_events
                        && (!self.attlist_handler_enabled || self.declarations_skipped())
                    {
                        self.attlist_raw(state, state.token.len(), self.declarations_skipped())?;
                    }
                    self.consume(state.source_bytes)?;
                    return Ok(true);
                }
                let start = state.ready + 2;
                state.fragment_start = if state.ready == 0 { 0 } else { start };
                state.ready += fragment_length(&state.token[start..state.token.len() - 1]);
            }
            let lexical = state
                .token
                .view()
                .for_slice(&state.token[2..state.ready + 2]);
            match state.grammar.advance_lexical(lexical, false) {
                Err((kind, offset)) => {
                    return Err(self.err_at(kind, "invalid ATTLIST declaration", offset + 2));
                }
                Ok(grammar::Progress::EnumerationMember {
                    start,
                    end,
                    notation,
                }) => {
                    state.enumeration_seen = true;
                    if self.attlist_handler_enabled && !self.declarations_skipped() {
                        if let Some(kind) = &mut state.enumeration_type {
                            kind.push('|')?;
                        } else {
                            state.enumeration_type = Some(enumeration_capture(
                                &state.token[start + 2..],
                                notation,
                                self.allocator,
                            )?);
                        }
                        state
                            .enumeration_type
                            .as_mut()
                            .expect("enumeration callback")
                            .push_str(
                                &state
                                    .token
                                    .view()
                                    .for_slice(&state.token[start + 2..end + 2])
                                    .decoded(self.allocator)?,
                            )?;
                    }
                }
                Ok(grammar::Progress::Commit(commit)) => {
                    self.commit_attlist(state, commit)?;
                    state.fragment_start = state.ready + 2;
                    self.conditional.attlist = Some(owner);
                    return Ok(true);
                }
                Ok(grammar::Progress::NeedMore) => {
                    state.finish_fragment = true;
                    if !self.default_events
                        || (self.attlist_handler_enabled && !self.declarations_skipped())
                    {
                        state.finish_fragment = false;
                        state.fragment_start = state.ready + 2;
                    }
                }
                Ok(grammar::Progress::Done) => {
                    unreachable!("the closing delimiter is not supplied yet")
                }
            }
        }
    }

    fn attlist_raw(
        &mut self,
        state: &mut State,
        end: usize,
        unconditional: bool,
    ) -> Result<(), Error> {
        let start = state.fragment_start;
        let raw = state
            .token
            .view()
            .for_slice(&state.token[start..end])
            .decoded(self.allocator)?;
        // A token prefix is the only event from this step. Retain its capacity
        // rather than allocating a second owned token for the pending queue.
        debug_assert!(self.pending.is_empty());
        self.current_raw
            .try_reserve(raw.len().saturating_sub(self.current_raw.len()))?;
        self.current_raw.clear();
        self.current_raw.try_push_str(&raw)?;
        self.emit(
            if unconditional {
                EventKind::Default
            } else {
                EventKind::AttlistDeclarationPrefix
            },
            self.source()
                .position_from_cursor(&mut state.position, start, end - start),
        )?;
        state.fragment_start = end;
        Ok(())
    }

    fn commit_attlist(&mut self, state: &mut State, commit: grammar::Commit) -> Result<(), Error> {
        if state.element.is_none() {
            let (start, end) = state.grammar.element_span().expect("ATTLIST element");
            state.element = Some(
                state
                    .token
                    .view()
                    .for_slice(&state.token[start + 2..end + 2])
                    .decode(self.allocator)?,
            );
        }
        let lexical = state
            .token
            .view()
            .for_slice(&state.token[commit.start + 2..commit.end + 2]);
        let mut cursor = Cursor::new(
            lexical,
            self.config.namespace_separator.is_some(),
            self.config.name_rules,
        );
        let callback = if state.enumeration_seen {
            if let Some(kind) = &mut state.enumeration_type {
                kind.push(')')?;
            }
            let captured = state.enumeration_type.take();
            if captured.is_some() && !self.attlist_handler_enabled {
                AttributeCallback::Silent
            } else {
                AttributeCallback::Captured(captured)
            }
        } else if !self.attlist_handler_enabled {
            AttributeCallback::Silent
        } else {
            AttributeCallback::Complete
        };
        let first_event = self.pending.len();
        self.attlist_attribute(
            &mut cursor,
            self.source()
                .position_from_cursor(&mut state.position, state.fragment_start, 0),
            state.element.as_ref().expect("ATTLIST element"),
            &mut state.first_attribute,
            callback,
        )?;
        if self.pending.len() > first_event {
            self.pending.back_mut().expect("ATTLIST event").raw =
                Some(String::new_in(self.allocator));
        }
        if self.default_events
            && (!self.attlist_handler_enabled
                || self.declarations_skipped()
                || (state.enumeration_seen && self.pending.len() == first_event))
        {
            self.attlist_raw(state, state.ready + 2, true)?;
        }
        state.enumeration_seen = false;
        Ok(())
    }
}

/// Reserve the remaining enumeration once, when its first callback member is
/// captured. Whitespace and later attribute defaults do not inflate capacity;
/// converted ASCII representatives only overestimate their decoded byte length.
fn enumeration_capture(
    text: &str,
    notation: bool,
    allocator: Allocator,
) -> Result<String, oriole_storage::AllocError> {
    let prefix = if notation { "NOTATION(" } else { "(" };
    let remaining = text
        .bytes()
        .take_while(|&byte| byte != b')')
        .filter(|byte| !matches!(byte, b' ' | b'\r' | b'\n' | b'\t'))
        .count();
    // Include the closing parenthesis and the adapter's C string terminator.
    let capacity = remaining
        .checked_add(prefix.len() + 2)
        .ok_or(oriole_storage::AllocError::CapacityOverflow)?;
    let mut result = String::try_with_capacity_in(capacity, allocator)?;
    result.push_str(prefix)?;
    Ok(result)
}

/// Split lexical fragments without crossing a quote or rescanning a prefix.
fn fragment_length(text: &str) -> usize {
    let first = text.chars().next().expect("nonempty ATTLIST suffix");
    match first {
        '\'' | '"' => text[1..].find(first).map_or(text.len(), |end| end + 2),
        '(' | ')' | '|' | ',' | '%' | '?' | '*' | '+' | '=' => first.len_utf8(),
        ' ' | '\r' | '\n' | '\t' => text.find(|c| !whitespace(c)).unwrap_or(text.len()),
        _ => text
            .find([
                ' ', '\r', '\n', '\t', '\'', '"', '(', ')', '|', ',', '%', '?', '*', '+', '=',
            ])
            .unwrap_or(text.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_types_do_not_retain_whitespace_or_later_default_capacity() {
        let padding = " \t\r\n".repeat(262_144);
        let later_default = "v".repeat(1_048_576);
        for late_handler in [false, true] {
            let mut parser = Parser::new(crate::Config::default());
            parser.set_default_events(late_handler);
            parser.set_attlist_handler_enabled(!late_handler);
            let xml = format!(
                "<!DOCTYPE r [<!ATTLIST r a (first|{padding}second) 'first' b CDATA '{later_default}'>]><r/>"
            );
            parser.feed(xml.as_bytes(), true).unwrap();
            let mut captured = false;
            while let Some(event) = parser.next_event().unwrap() {
                if late_handler && parser.current_raw() == Some("first") {
                    parser.set_attlist_handler_enabled(true);
                }
                if let EventKind::AttlistDeclaration(value) = event.kind
                    && value.name == "a"
                {
                    captured = true;
                    assert_eq!(
                        value.attribute_type,
                        if late_handler {
                            "(second)"
                        } else {
                            "(first|second)"
                        }
                    );
                    assert!(value.attribute_type.capacity() <= 32);
                }
            }
            assert!(captured && parser.is_finished());
        }
    }

    #[test]
    fn uncaptured_enumerations_keep_ordinary_repeated_name_work() {
        for attributes in [
            "a (x|y) 'x' b CDATA 'B' c CDATA 'C'",
            "a CDATA 'A' b (x|y) 'x' c CDATA 'C'",
        ] {
            for handlers in 0..4 {
                for sufficient in [false, true] {
                    let expected_work = 2 * "root".len();
                    let mut config = crate::Config::default();
                    config.limits.max_entity_expansion_bytes =
                        expected_work - usize::from(!sufficient);
                    let mut parser = Parser::new(config);
                    parser.set_default_events(true);
                    parser.set_attlist_handler_enabled(matches!(handlers, 0 | 3));
                    let xml = format!("<!DOCTYPE root [<!ATTLIST root {attributes}>");
                    parser.feed(xml.as_bytes(), false).unwrap();
                    let result = loop {
                        match parser.next_event() {
                            Ok(Some(event)) => {
                                // Installing after the enumeration has ended
                                // leaves its callback uncaptured; removing the
                                // first callback also leaves later types silent.
                                if handlers == 2 && parser.current_raw() == Some("'x'") {
                                    parser.set_attlist_handler_enabled(true);
                                }
                                if handlers == 3
                                    && let EventKind::AttlistDeclaration(value) = event.kind
                                    && value.name == "a"
                                {
                                    parser.set_attlist_handler_enabled(false);
                                }
                            }
                            Ok(None) => break Ok(()),
                            Err(error) => break Err(error.kind),
                        }
                    };
                    assert_eq!(
                        result,
                        if sufficient {
                            Ok(())
                        } else {
                            Err(ErrorKind::LimitExceeded)
                        }
                    );
                    assert_eq!(
                        parser.expanded.expanded.load(Ordering::Relaxed),
                        if sufficient {
                            expected_work
                        } else {
                            expected_work / 2
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn silent_commits_preserve_positions_and_reused_name_work() {
        let xml = b"<!DOCTYPE root [<!ATTLIST root a CDATA 'A' b CDATA 'B'>";
        let mut observations = std::vec::Vec::new();
        for enabled in [false, true] {
            let mut parser = Parser::new(crate::Config::default());
            // Sample committed work at the closing delimiter, before final input.
            parser.set_reparse_deferral_enabled(false);
            parser.set_attlist_handler_enabled(enabled);
            for byte in xml {
                parser.feed(std::slice::from_ref(byte), false).unwrap();
                while parser.next_event().unwrap().is_some() {}
            }
            observations.push((
                parser.last_position,
                parser.expanded.expanded.load(Ordering::Relaxed),
            ));
            let mut limits = parser.config.limits.clone();
            limits.max_entity_expansion_bytes = observations.last().unwrap().1;
            let mut parser = Parser::new(crate::Config {
                limits,
                ..crate::Config::default()
            });
            parser.set_reparse_deferral_enabled(false);
            parser.set_attlist_handler_enabled(enabled);
            parser.feed(xml, false).unwrap();
            while parser.next_event().unwrap().is_some() {}
            parser
                .feed(b"<!ATTLIST root c CDATA 'C' d CDATA 'D'>", false)
                .unwrap();
            let error = loop {
                match parser.next_event() {
                    Ok(Some(_)) => {}
                    Ok(None) => panic!("reused element name bypassed the work limit"),
                    Err(error) => break error,
                }
            };
            assert_eq!(error.kind, ErrorKind::LimitExceeded);
        }
        assert_eq!(observations[0], observations[1]);
        assert_eq!(observations[0].1, "root".len());
    }

    #[test]
    fn completed_attributes_keep_only_the_current_events() {
        for defaults in [false, true] {
            let mut parser = Parser::new(crate::Config::default());
            parser.set_default_events(defaults);
            parser.set_attlist_handler_enabled(!defaults);
            let mut xml = std::string::String::from("<!DOCTYPE r [<!ATTLIST r");
            for index in 0..1000 {
                xml.push_str(&format!(" a{index} CDATA 'v'"));
            }
            xml.push_str(">]><r/>");
            parser.feed(xml.as_bytes(), true).unwrap();
            let mut attributes = 0;
            while let Some(event) = parser.next_event().unwrap() {
                assert!(
                    parser.pending.len() <= 1,
                    "queued more than one following event"
                );
                if matches!(event.kind, EventKind::AttlistDeclaration(_)) {
                    attributes += 1;
                }
                if let EventKind::StartElement { attributes, .. } = event.kind {
                    assert_eq!(attributes.len(), 1000);
                }
            }
            assert_eq!(attributes, if defaults { 0 } else { 1000 });
            assert!(parser.conditional.attlist.is_none());
        }
    }

    #[test]
    fn committed_prefix_is_published_before_child_updates() {
        let mut root = Parser::new(crate::Config::default());
        let mut parser = root.external_child(None, None).unwrap();
        parser
            .feed(b"<!ATTLIST r a CDATA 'A' b CDATA '&e;'>", true)
            .unwrap();
        let mut declarations = std::vec::Vec::new();
        while let Some(event) = parser.next_event().unwrap() {
            if let EventKind::AttlistDeclaration(value) = event.kind {
                declarations.push((
                    value.name.to_string(),
                    value.default.as_deref().unwrap().to_owned(),
                ));
                if value.name == "a" {
                    let mut child = parser.external_child(None, None).unwrap();
                    child.feed(b"<!ENTITY e 'child'><!ATTLIST r a CDATA 'ignored' b CDATA 'child-first'>", true).unwrap();
                    while child.next_event().unwrap().is_some() {}
                }
            }
        }
        root.feed(b"<r/>", true).unwrap();
        while let Some(event) = root.next_event().unwrap() {
            if let EventKind::StartElement { attributes, .. } = event.kind {
                assert_eq!(attributes[0].value, "A");
                assert_eq!(attributes[1].value, "child-first");
            }
        }
        assert_eq!(
            declarations,
            [
                ("a".to_owned(), "A".to_owned()),
                ("b".to_owned(), "child".to_owned())
            ]
        );
    }

    #[test]
    fn earlier_attribute_precedes_later_grammar_error() {
        let mut parser = Parser::new(crate::Config::default());
        parser
            .feed(
                b"<!DOCTYPE r [<!ATTLIST r a CDATA 'A' b INVALID 'B'>]><r/>",
                true,
            )
            .unwrap();
        let mut names = std::vec::Vec::new();
        let error = loop {
            match parser.next_event() {
                Ok(Some(event)) => {
                    if let EventKind::AttlistDeclaration(value) = event.kind {
                        names.push(value.name.to_string());
                    }
                }
                Ok(None) => panic!("malformed declaration accepted"),
                Err(error) => break error,
            }
        };
        assert_eq!(names, ["a"]);
        assert_eq!(error.kind, ErrorKind::Syntax);
        assert!(parser.conditional.attlist.is_none());
    }

    #[test]
    fn owned_token_survives_large_feeds_and_source_compaction() {
        let prefix = format!(
            "<!--{}--><!DOCTYPE r [<!ATTLIST r a CDATA 'A' b CDATA 'B'>]>",
            "x".repeat(70000)
        );
        let suffix = format!("{}<r/>", " ".repeat(70000));
        let mut parser = Parser::new(crate::Config::default());
        parser.feed(prefix.as_bytes(), false).unwrap();
        let mut appended = false;
        let mut names = std::vec::Vec::new();
        while let Some(event) = parser.next_event().unwrap() {
            match event.kind {
                EventKind::AttlistDeclaration(value) => {
                    names.push(value.name.to_string());
                    if !appended {
                        assert_eq!(value.name, "a");
                        parser.feed(suffix.as_bytes(), true).unwrap();
                        appended = true;
                    }
                }
                EventKind::StartElement { attributes, .. } => {
                    assert_eq!(attributes.len(), 2);
                    assert_eq!(event.position.byte_index, prefix.len() + 70000);
                }
                _ => {}
            }
        }
        assert!(appended && parser.is_finished());
        assert_eq!(names, ["a", "b"]);
    }
}
