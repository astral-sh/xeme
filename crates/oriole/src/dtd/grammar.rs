//! Declaration roles over append-only, lexically complete DTD prefixes.
//!
//! The caller supplies the text between `<!` and `>`, inserting whitespace at
//! parameter-entity boundaries. Calls may end after whitespace or a complete
//! literal; `final_input` marks the closing `>`. Tokens and completed model
//! particles are never scanned twice, including across empty replacements.

use crate::{ErrorKind, names};
use oriole_storage::{Allocator, Vec, try_push};

type Result<T> = std::result::Result<T, (ErrorKind, usize)>;
pub(crate) type Span = (usize, usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Entity,
    Attlist,
    Element,
    Notation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Commit {
    pub(crate) kind: Kind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Progress {
    Commit(Commit),
    EnumerationMember {
        start: usize,
        end: usize,
        notation: bool,
    },
    NeedMore,
    Done,
}

#[derive(Clone, Copy, Debug)]
enum State {
    Keyword,
    EntityName,
    ParameterName,
    EntityValue,
    EntitySystem,
    EntityPublic,
    EntityPublicSystem,
    EntityTail,
    EntityNotation,
    NotationName,
    NotationId,
    NotationSystem,
    NotationPublic,
    NotationPublicTail,
    ElementName,
    ModelRoot,
    ModelParticle,
    ModelAfterParticle,
    ModelSeparator,
    MixedSeparator,
    MixedName,
    MixedQuantifier(bool),
    AttlistElement,
    AttributeName,
    AttributeType,
    NotationEnumeration,
    EnumerationValue(bool),
    EnumerationSeparator(bool),
    AttributeDefault,
    FixedDefault,
    End,
}

#[derive(Clone, Copy, Debug)]
enum TokenKind {
    Word,
    Hash,
    Literal,
    Mark(u8),
}

#[derive(Clone, Copy, Debug)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
    spaced: bool,
}

#[derive(Clone, Copy, Debug)]
struct Group {
    separator: Option<u8>,
    mixed_names: bool,
}

/// A monotonic declaration-role cursor. Semantic expansion belongs to the caller.
#[derive(Debug)]
pub(crate) struct Cursor {
    state: State,
    kind: Option<Kind>,
    offset: usize,
    spaced: bool,
    token_end: usize,
    lookahead: Option<Token>,
    previous_word: bool,
    namespaces: bool,
    max_depth: usize,
    groups: Vec<Group>,
    element: Option<(usize, usize)>,
    entity: Option<(usize, usize, bool)>,
    entity_system: Option<(usize, usize)>,
    entity_public: Option<(usize, usize)>,
    notation: Option<(usize, usize)>,
    attribute_start: usize,
}

impl Cursor {
    /// Bound both retained stack storage and its geometric growth history.
    /// The caller charges this once before constructing the cursor; the stack
    /// never shrinks, so repeated model groups reuse the same capacity.
    pub(crate) fn maximum_stack_bytes(max_depth: usize) -> usize {
        let capacity = (max_depth.min(256) + 1).next_power_of_two().max(8);
        2 * capacity * size_of::<Group>()
    }

    pub(crate) fn new(allocator: Allocator, namespaces: bool, max_depth: usize) -> Self {
        Self {
            state: State::Keyword,
            kind: None,
            offset: 0,
            spaced: false,
            token_end: 0,
            lookahead: None,
            previous_word: false,
            namespaces,
            max_depth: max_depth.min(256),
            groups: Vec::new_in(allocator),
            element: None,
            entity: None,
            entity_system: None,
            entity_public: None,
            notation: None,
            attribute_start: 0,
        }
    }

    pub(crate) fn kind(&self) -> Option<Kind> {
        self.kind
    }

    pub(crate) fn offset(&self) -> usize {
        self.offset
    }

    pub(crate) fn element_span(&self) -> Option<(usize, usize)> {
        self.element
    }

    pub(crate) fn entity_name(&self) -> Option<(usize, usize, bool)> {
        self.entity
    }

    pub(crate) fn notation_name(&self) -> Option<(usize, usize)> {
        self.notation
    }

    /// Literal spans include their quotes; normalization uses caller provenance.
    pub(crate) fn entity_ids(&self) -> (Option<Span>, Option<Span>) {
        (self.entity_system, self.entity_public)
    }

    /// Produce one callback boundary, or await another complete lexical prefix.
    /// Previously supplied bytes must not change or be removed.
    pub(crate) fn advance(&mut self, text: &str, final_input: bool) -> Result<Progress> {
        loop {
            let token = self.peek(text)?;
            match self.state {
                State::ModelAfterParticle => {
                    if let Some(token) = token
                        && !token.spaced
                        && matches!(token.kind, TokenKind::Mark(b'?' | b'+' | b'*'))
                    {
                        self.consume(token);
                    } else if token.is_none() && !final_input && !self.spaced {
                        return Ok(Progress::NeedMore);
                    }
                    if self.groups.is_empty() {
                        return Ok(self.commit(Kind::Element, 0));
                    }
                    self.state = State::ModelSeparator;
                    continue;
                }
                State::MixedQuantifier(required) => {
                    if let Some(token) = token
                        && !token.spaced
                        && matches!(token.kind, TokenKind::Mark(b'*'))
                    {
                        self.consume(token);
                    } else if token.is_none() && !final_input && !self.spaced {
                        return Ok(Progress::NeedMore);
                    } else if required
                        || token.is_some_and(|token| {
                            !token.spaced && matches!(token.kind, TokenKind::Mark(b'?' | b'+'))
                        })
                    {
                        return Err((ErrorKind::Syntax, self.offset));
                    }
                    return Ok(self.commit(Kind::Element, 0));
                }
                State::End => {
                    return if let Some(token) = token {
                        let kind = if matches!(token.kind, TokenKind::Mark(b'?' | b'+' | b'*'))
                            && (token.spaced || !self.previous_word)
                        {
                            ErrorKind::InvalidToken
                        } else {
                            ErrorKind::Syntax
                        };
                        Err((kind, token.start))
                    } else if final_input {
                        Ok(Progress::Done)
                    } else {
                        Ok(Progress::NeedMore)
                    };
                }
                State::EntityTail | State::NotationPublicTail if token.is_none() && final_input => {
                    let kind = self.kind.expect("declaration kind has been read");
                    return Ok(self.commit(kind, 0));
                }
                State::AttributeName if token.is_none() && final_input => {
                    return Ok(Progress::Done);
                }
                _ => {}
            }
            let Some(token) = token else {
                return if final_input {
                    Err((ErrorKind::Syntax, self.offset))
                } else {
                    Ok(Progress::NeedMore)
                };
            };
            let value = &text[token.start..token.end];
            let syntax = || (ErrorKind::Syntax, token.start);
            match self.state {
                State::Keyword => {
                    if token.start != 0 {
                        return Err(syntax());
                    }
                    let (kind, state) = match value {
                        "ENTITY" => (Kind::Entity, State::EntityName),
                        "ATTLIST" => (Kind::Attlist, State::AttlistElement),
                        "ELEMENT" => (Kind::Element, State::ElementName),
                        "NOTATION" => (Kind::Notation, State::NotationName),
                        _ => return Err(syntax()),
                    };
                    self.kind = Some(kind);
                    self.state = state;
                }
                State::EntityName | State::ParameterName => {
                    Self::require_space(token)?;
                    if matches!(self.state, State::EntityName) && value == "%" {
                        self.state = State::ParameterName;
                    } else {
                        self.name(text, token, true)?;
                        self.entity = Some((
                            token.start,
                            token.end,
                            matches!(self.state, State::ParameterName),
                        ));
                        self.state = State::EntityValue;
                    }
                }
                State::EntityValue => {
                    Self::require_space(token)?;
                    match value {
                        "SYSTEM" => self.state = State::EntitySystem,
                        "PUBLIC" => self.state = State::EntityPublic,
                        _ if matches!(token.kind, TokenKind::Literal) => {
                            self.consume(token);
                            return Ok(self.commit(Kind::Entity, 0));
                        }
                        _ => return Err(syntax()),
                    }
                }
                State::EntitySystem | State::EntityPublicSystem => {
                    Self::literal(token)?;
                    self.entity_system = Some((token.start, token.end));
                    self.state = State::EntityTail;
                }
                State::EntityPublic => {
                    Self::public_literal(text, token)?;
                    self.entity_public = Some((token.start, token.end));
                    self.state = State::EntityPublicSystem;
                }
                State::EntityTail => {
                    Self::require_space(token)?;
                    if value != "NDATA" || self.entity.is_some_and(|(_, _, parameter)| parameter) {
                        return Err(syntax());
                    }
                    self.state = State::EntityNotation;
                }
                State::EntityNotation => {
                    Self::require_space(token)?;
                    self.name(text, token, true)?;
                    self.consume(token);
                    return Ok(self.commit(Kind::Entity, 0));
                }
                State::NotationName => {
                    Self::require_space(token)?;
                    self.name(text, token, true)?;
                    self.notation = Some((token.start, token.end));
                    self.state = State::NotationId;
                }
                State::NotationId => {
                    Self::require_space(token)?;
                    self.state = match value {
                        "SYSTEM" => State::NotationSystem,
                        "PUBLIC" => State::NotationPublic,
                        _ => return Err(syntax()),
                    };
                }
                State::NotationSystem | State::NotationPublicTail => {
                    Self::literal(token)?;
                    self.consume(token);
                    return Ok(self.commit(Kind::Notation, 0));
                }
                State::NotationPublic => {
                    Self::public_literal(text, token)?;
                    self.state = State::NotationPublicTail;
                }
                State::ElementName | State::AttlistElement => {
                    Self::require_space(token)?;
                    self.name(text, token, false)?;
                    self.element = Some((token.start, token.end));
                    self.state = if matches!(self.state, State::ElementName) {
                        State::ModelRoot
                    } else {
                        State::AttributeName
                    };
                }
                State::ModelRoot => {
                    Self::require_space(token)?;
                    if matches!(value, "EMPTY" | "ANY") {
                        self.consume(token);
                        return Ok(self.commit(Kind::Element, 0));
                    }
                    if value != "(" {
                        return Err(syntax());
                    }
                    self.open_group(token)?;
                }
                State::ModelParticle => {
                    if value == "#PCDATA"
                        && self.groups.len() == 1
                        && self.groups[0].separator.is_none()
                    {
                        self.state = State::MixedSeparator;
                    } else {
                        if self.groups.len() > self.max_depth {
                            return Err((ErrorKind::LimitExceeded, token.start));
                        }
                        if value == "(" {
                            self.open_group(token)?;
                        } else {
                            self.name(text, token, false)?;
                            self.state = State::ModelAfterParticle;
                        }
                    }
                }
                State::ModelSeparator => {
                    if value == ")" {
                        self.groups.pop().expect("content model has an open group");
                        self.state = State::ModelAfterParticle;
                    } else if matches!(value, "," | "|") {
                        let separator = value.as_bytes()[0];
                        let group = self
                            .groups
                            .last_mut()
                            .expect("content model has an open group");
                        if group
                            .separator
                            .is_some_and(|previous| previous != separator)
                        {
                            return Err(syntax());
                        }
                        group.separator = Some(separator);
                        self.state = State::ModelParticle;
                    } else {
                        return Err(syntax());
                    }
                }
                State::MixedSeparator => {
                    if value == "|" {
                        self.state = State::MixedName;
                    } else if value == ")" {
                        let group = self.groups.pop().expect("mixed content has an open group");
                        self.state = State::MixedQuantifier(group.mixed_names);
                    } else {
                        return Err(syntax());
                    }
                }
                State::MixedName => {
                    self.name(text, token, false)?;
                    self.groups
                        .last_mut()
                        .expect("mixed content has an open group")
                        .mixed_names = true;
                    self.state = State::MixedSeparator;
                }
                State::AttributeName => {
                    Self::require_space(token)?;
                    self.name(text, token, false)?;
                    self.attribute_start = token.start;
                    self.state = State::AttributeType;
                }
                State::AttributeType => {
                    Self::require_space(token)?;
                    self.state = match value {
                        "CDATA" | "ID" | "IDREF" | "IDREFS" | "ENTITY" | "ENTITIES" | "NMTOKEN"
                        | "NMTOKENS" => State::AttributeDefault,
                        "NOTATION" => State::NotationEnumeration,
                        "(" => State::EnumerationValue(false),
                        _ => return Err(syntax()),
                    };
                }
                State::NotationEnumeration => {
                    Self::require_space(token)?;
                    if value != "(" {
                        return Err(syntax());
                    }
                    self.state = State::EnumerationValue(true);
                }
                State::EnumerationValue(names) => {
                    if names {
                        self.name(text, token, true)?;
                    } else if !matches!(token.kind, TokenKind::Word) {
                        return Err(syntax());
                    }
                    self.state = State::EnumerationSeparator(names);
                    self.consume(token);
                    return Ok(Progress::EnumerationMember {
                        start: token.start,
                        end: token.end,
                        notation: names,
                    });
                }
                State::EnumerationSeparator(names) => {
                    self.state = match value {
                        "|" => State::EnumerationValue(names),
                        ")" => State::AttributeDefault,
                        _ => return Err(syntax()),
                    };
                }
                State::AttributeDefault => {
                    Self::require_space(token)?;
                    if value == "#FIXED" {
                        self.state = State::FixedDefault;
                    } else if matches!(value, "#IMPLIED" | "#REQUIRED")
                        || matches!(token.kind, TokenKind::Literal)
                    {
                        self.consume(token);
                        return Ok(self.commit(Kind::Attlist, self.attribute_start));
                    } else {
                        return Err(syntax());
                    }
                }
                State::FixedDefault => {
                    Self::literal(token)?;
                    self.consume(token);
                    return Ok(self.commit(Kind::Attlist, self.attribute_start));
                }
                State::ModelAfterParticle | State::MixedQuantifier(_) | State::End => {
                    unreachable!("handled before token dispatch")
                }
            }
            self.consume(token);
        }
    }

    fn commit(&mut self, kind: Kind, start: usize) -> Progress {
        self.state = if kind == Kind::Attlist {
            State::AttributeName
        } else {
            State::End
        };
        Progress::Commit(Commit {
            kind,
            start,
            end: self.token_end,
        })
    }

    fn open_group(&mut self, token: Token) -> Result<()> {
        try_push(
            &mut self.groups,
            Group {
                separator: None,
                mixed_names: false,
            },
        )
        .map_err(|_| (ErrorKind::NoMemory, token.start))?;
        self.state = State::ModelParticle;
        Ok(())
    }

    fn consume(&mut self, token: Token) {
        self.offset = token.end;
        self.token_end = token.end;
        self.previous_word = matches!(token.kind, TokenKind::Word);
        self.spaced = false;
        self.lookahead = None;
    }

    fn name(&self, text: &str, token: Token, ncname: bool) -> Result<()> {
        let name = &text[token.start..token.end];
        if !matches!(token.kind, TokenKind::Word)
            || !names::is_name(name)
            || (self.namespaces && (!names::is_qname(name) || (ncname && name.contains(':'))))
        {
            return Err((ErrorKind::Syntax, token.start));
        }
        Ok(())
    }

    fn require_space(token: Token) -> Result<()> {
        if token.spaced {
            Ok(())
        } else {
            Err((ErrorKind::Syntax, token.start))
        }
    }

    fn literal(token: Token) -> Result<()> {
        Self::require_space(token)?;
        if matches!(token.kind, TokenKind::Literal) {
            Ok(())
        } else {
            Err((ErrorKind::Syntax, token.start))
        }
    }

    fn public_literal(text: &str, token: Token) -> Result<()> {
        Self::literal(token)?;
        if let Some((offset, _)) = text[token.start + 1..token.end - 1]
            .char_indices()
            .find(|(_, c)| !c.is_ascii_alphanumeric() && !" \r\n-'()+,./:=?;!*#@$_%".contains(*c))
        {
            return Err((ErrorKind::Syntax, token.start + 1 + offset));
        }
        Ok(())
    }

    fn peek(&mut self, text: &str) -> Result<Option<Token>> {
        if let Some(token) = self.lookahead {
            return Ok(Some(token));
        }
        let Some(rest) = text.get(self.offset..) else {
            return Err((ErrorKind::Syntax, self.offset));
        };
        let whitespace = rest.len() - rest.trim_start_matches(names::whitespace).len();
        self.offset += whitespace;
        self.spaced |= whitespace != 0;
        let rest = &text[self.offset..];
        let Some(first) = rest.chars().next() else {
            return Ok(None);
        };
        let (kind, length) = if matches!(first, '\'' | '"') {
            let end = rest[1..]
                .find(first)
                .ok_or((ErrorKind::Syntax, self.offset))?;
            (TokenKind::Literal, end + 2)
        } else if names::is_name_char(first) || first == '#' {
            let prefix = usize::from(first == '#');
            let end = rest[prefix..]
                .char_indices()
                .find(|(_, c)| !names::is_name_char(*c))
                .map_or(rest.len(), |(offset, _)| prefix + offset);
            (
                if prefix == 0 {
                    TokenKind::Word
                } else {
                    TokenKind::Hash
                },
                end,
            )
        } else if first.is_ascii() {
            (TokenKind::Mark(first as u8), 1)
        } else {
            return Err((ErrorKind::Syntax, self.offset));
        };
        self.lookahead = Some(Token {
            kind,
            start: self.offset,
            end: self.offset + length,
            spaced: self.spaced,
        });
        Ok(self.lookahead)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(
        cursor: &mut Cursor,
        text: &str,
        final_input: bool,
    ) -> Result<std::vec::Vec<Commit>> {
        let mut commits = std::vec::Vec::new();
        loop {
            match cursor.advance(text, final_input)? {
                Progress::Commit(commit) => commits.push(commit),
                Progress::EnumerationMember { .. } => {}
                Progress::NeedMore | Progress::Done => break,
            }
        }
        Ok(commits)
    }

    fn boundaries(text: &str) -> std::vec::Vec<usize> {
        let mut quote = None;
        let mut boundaries = std::vec::Vec::new();
        for (offset, c) in text.char_indices() {
            if quote == Some(c) {
                quote = None;
                boundaries.push(offset + 1);
            } else if quote.is_none() {
                if matches!(c, '\'' | '"') {
                    quote = Some(c);
                } else if names::whitespace(c) {
                    boundaries.push(offset + 1);
                }
            }
        }
        boundaries
    }

    #[test]
    fn declarations_resume_at_each_complete_lexical_boundary() {
        for text in [
            "ENTITY e 'a b'",
            "ENTITY % e PUBLIC 'public id' 'system'",
            "ENTITY e SYSTEM 'system' NDATA n",
            "NOTATION n SYSTEM 's'",
            "NOTATION n PUBLIC 'p'",
            "NOTATION n PUBLIC 'p' 's'",
            "ELEMENT r EMPTY",
            "ELEMENT r ANY",
            "ELEMENT r ( a , (b|c)* , d? )+",
            "ELEMENT r (#PCDATA)",
            "ELEMENT r (#PCDATA)*",
            "ELEMENT r (#PCDATA | a | b)*",
            "ATTLIST r",
            "ATTLIST r a CDATA 'A' b ID #REQUIRED c NMTOKENS #IMPLIED",
            "ATTLIST r a NOTATION (n|m) #FIXED 'n' b (a|12|:x) 'a'",
            "ATTLIST π:根 π:名 CDATA 'ü'",
        ] {
            let mut whole = Cursor::new(Allocator::System, true, 64);
            let expected =
                collect(&mut whole, text, true).unwrap_or_else(|error| panic!("{text}: {error:?}"));
            let mut cursor = Cursor::new(Allocator::System, true, 64);
            let mut actual = std::vec::Vec::new();
            for end in boundaries(text) {
                actual.extend(
                    collect(&mut cursor, &text[..end], false)
                        .unwrap_or_else(|error| panic!("{}: {error:?}", &text[..end])),
                );
            }
            actual.extend(collect(&mut cursor, text, true).unwrap());
            assert_eq!(actual, expected, "{text}");
            assert_eq!(cursor.advance(text, true).unwrap(), Progress::Done);
        }
    }

    #[test]
    fn commit_timing_and_ranges_preserve_role_boundaries() {
        for (text, early) in [
            ("ENTITY e 'V' ", true),
            ("ENTITY e SYSTEM 's' ", false),
            ("ENTITY e PUBLIC 'p' 's' ", false),
            ("ENTITY e SYSTEM 's' NDATA n ", true),
            ("NOTATION n SYSTEM 's' ", true),
            ("NOTATION n PUBLIC 'p' ", false),
            ("NOTATION n PUBLIC 'p' 's' ", true),
            ("ELEMENT r (a,b)* ", true),
        ] {
            let mut cursor = Cursor::new(Allocator::System, false, 64);
            let before = collect(&mut cursor, text, false).unwrap();
            assert_eq!(!before.is_empty(), early, "{text}");
            let after = collect(&mut cursor, text, true).unwrap();
            let commit = before.first().or(after.first()).unwrap();
            assert_eq!(commit.end, text.trim_end().len(), "{text}");
            assert_eq!(cursor.kind(), Some(commit.kind));
        }
        let text = "ATTLIST root a CDATA 'A' b ID #IMPLIED";
        let mut cursor = Cursor::new(Allocator::System, false, 64);
        let commits = collect(&mut cursor, text, true).unwrap();
        assert_eq!(cursor.element_span(), Some((8, 12)));
        assert_eq!(&text[commits[0].start..commits[0].end], "a CDATA 'A'");
        assert_eq!(&text[commits[1].start..commits[1].end], "b ID #IMPLIED");
        for (text, span) in [("ENTITY e ", (7, 8, false)), ("ENTITY % e ", (9, 10, true))] {
            let mut cursor = Cursor::new(Allocator::System, false, 64);
            assert_eq!(cursor.advance(text, false), Ok(Progress::NeedMore));
            assert_eq!(cursor.entity_name(), Some(span));
        }
        let mut cursor = Cursor::new(Allocator::System, false, 64);
        assert_eq!(cursor.advance("NOTATION n ", false), Ok(Progress::NeedMore));
        assert_eq!(cursor.notation_name(), Some((9, 10)));
        let mut cursor = Cursor::new(Allocator::System, false, 64);
        assert_eq!(
            cursor.advance("ENTITY e PUBLIC 'p' ", false),
            Ok(Progress::NeedMore)
        );
        assert_eq!(cursor.entity_ids(), (None, Some((16, 19))));
        assert_eq!(
            cursor.advance("ENTITY e PUBLIC 'p' 's' ", false),
            Ok(Progress::NeedMore)
        );
        assert_eq!(cursor.entity_ids(), (Some((20, 23)), Some((16, 19))));
    }

    #[test]
    fn stray_quantifiers_differ_from_attached_name_or_mixed_model_tokens() {
        for text in [
            "ELEMENT r EMPTY *",
            "ELEMENT r ANY ?",
            "ELEMENT r (a) *",
            "ELEMENT r (a)**",
            "ENTITY e 'v' *",
            "NOTATION n SYSTEM 's'*",
        ] {
            let error =
                collect(&mut Cursor::new(Allocator::System, false, 64), text, true).unwrap_err();
            assert_eq!(error.0, ErrorKind::InvalidToken, "{text}");
        }
        for text in [
            "ELEMENT r EMPTY*",
            "ELEMENT r ANY?",
            "ENTITY e SYSTEM 's' NDATA n*",
            "ELEMENT r (#PCDATA)?",
            "ELEMENT r (#PCDATA|a)+",
            "ELEMENT r (#PCDATA|a) *",
        ] {
            let error =
                collect(&mut Cursor::new(Allocator::System, false, 64), text, true).unwrap_err();
            assert_eq!(error.0, ErrorKind::Syntax, "{text}");
        }
    }

    #[test]
    fn incomplete_or_invalid_roles_never_finish() {
        for text in [
            "",
            " ENTITY e 'v'",
            "ENTITY",
            "ENTITY e",
            "ENTITY %e 'v'",
            "ENTITY e'v'",
            "ENTITY e PUBLIC 'p'",
            "ENTITY % e SYSTEM 's' NDATA n",
            "ENTITY e 'v' tail",
            "NOTATION n PUBLIC 'p' 's' tail",
            "NOTATION n PUBLIC 'bad\tidentifier'",
            "NOTATION :n SYSTEM 's'",
            "ELEMENT r ()",
            "ELEMENT r (a|b,c)",
            "ELEMENT r ((#PCDATA))",
            "ELEMENT r (a|#PCDATA)",
            "ELEMENT r (#PCDATA|a)",
            "ELEMENT r (#PCDATA|a) *",
            "ELEMENT r (#PCDATA)?",
            "ELEMENT r (a) *",
            "ELEMENT r (a**)",
            "ELEMENT r (a,)",
            "ELEMENT r (a b)",
            "ELEMENT r EMPTY*",
            "ATTLIST r a",
            "ATTLIST r a CDATA",
            "ATTLIST r a CDATA #FIXED",
            "ATTLIST r a CDATA'v'",
            "ATTLIST r a () 'v'",
            "ATTLIST r a (x|) 'v'",
            "ATTLIST r a NOTATION (1) 'v'",
            "ATTLIST r a NOTATION (:n) 'v'",
            "ATTLIST r a CDATA 'x'b CDATA 'y'",
        ] {
            let mut cursor = Cursor::new(Allocator::System, true, 64);
            assert!(collect(&mut cursor, text, true).is_err(), "{text}");
        }
    }

    #[test]
    fn enumeration_members_are_reported_once_before_the_attribute_commit() {
        let text = "ATTLIST r a (x| 12 |z) 'x' b NOTATION (n|m) #IMPLIED";
        let mut cursor = Cursor::new(Allocator::System, false, 64);
        let mut members = std::vec::Vec::new();
        let mut commits = 0;
        for end in boundaries(text)
            .into_iter()
            .chain(std::iter::once(text.len()))
        {
            loop {
                match cursor.advance(&text[..end], end == text.len()).unwrap() {
                    Progress::EnumerationMember {
                        start,
                        end,
                        notation,
                    } => {
                        members.push((&text[start..end], notation, commits));
                    }
                    Progress::Commit(_) => commits += 1,
                    Progress::NeedMore | Progress::Done => break,
                }
            }
        }
        assert_eq!(
            members,
            [
                ("x", false, 0),
                ("12", false, 0),
                ("z", false, 0),
                ("n", true, 1),
                ("m", true, 1)
            ]
        );
        assert_eq!(commits, 2);
    }

    #[test]
    fn namespace_and_model_depth_rules_remain_explicit() {
        assert_eq!(
            Cursor::maximum_stack_bytes(usize::MAX),
            Cursor::maximum_stack_bytes(256)
        );
        assert!(Cursor::maximum_stack_bytes(0) >= size_of::<Group>());
        for text in [
            "ENTITY a:b 'v'",
            "NOTATION a:b SYSTEM 's'",
            "ATTLIST r a NOTATION (a:b) 'v'",
        ] {
            assert!(collect(&mut Cursor::new(Allocator::System, false, 64), text, true).is_ok());
            assert!(collect(&mut Cursor::new(Allocator::System, true, 64), text, true).is_err());
        }
        for (text, depth, valid) in [
            ("ELEMENT r EMPTY", 0, true),
            ("ELEMENT r (#PCDATA)", 0, true),
            ("ELEMENT r (a)", 0, false),
            ("ELEMENT r (a)", 1, true),
            ("ELEMENT r ((a))", 1, false),
            ("ELEMENT r ((a))", 2, true),
        ] {
            let result = collect(
                &mut Cursor::new(Allocator::System, false, depth),
                text,
                true,
            );
            assert_eq!(result.is_ok(), valid, "{text}/{depth}");
            if !valid {
                assert_eq!(result.unwrap_err().0, ErrorKind::LimitExceeded);
            }
        }
    }

    #[test]
    fn repeated_empty_replacements_and_many_attributes_advance_monotonically() {
        let mut text = std::string::String::from("ATTLIST r ");
        let mut cursor = Cursor::new(Allocator::System, false, 64);
        let mut commits = 0;
        for _ in 0..4096 {
            assert_eq!(cursor.advance(&text, false), Ok(Progress::NeedMore));
            assert_eq!(cursor.offset(), text.len());
            text.push(' ');
        }
        for i in 0..4096 {
            text.push_str(&format!("a{i} CDATA 'v' "));
            commits += collect(&mut cursor, &text, false).unwrap().len();
            assert_eq!(cursor.offset(), text.len());
        }
        assert_eq!(commits, 4096);
        assert_eq!(cursor.advance(&text, true), Ok(Progress::Done));
    }
}
