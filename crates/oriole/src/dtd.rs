use crate::{
    DefaultAttribute, Entity, Error, ErrorKind, EventKind, Parser, Position, character_reference,
    collapse_spaces, normalize_newlines, string, take_name, whitespace,
};
use oriole_storage::{Allocator, String, TryClone, Vec, try_insert, try_push};

impl Parser {
    pub(crate) fn parse_doctype(&mut self, token: &str, position: Position) -> Result<(), Error> {
        if self.seen_root || self.seen_doctype || self.sources.len() > 1 || self.fragment {
            return Err(self.err(ErrorKind::Syntax, "misplaced document type declaration"));
        }
        let mut cursor = Cursor::new(&token[9..token.len() - 1]);
        cursor
            .require_space()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let name = cursor
            .name()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let name = string(name, self.allocator)?;
        let had_space = cursor.space();
        let (system_id, public_id) = if cursor.starts("SYSTEM") || cursor.starts("PUBLIC") {
            if !had_space {
                return Err(self.err(
                    ErrorKind::Syntax,
                    "document type identifiers require whitespace",
                ));
            }
            external_id(&mut cursor, false, self.allocator)
                .map_err(|error| self.err(error.kind, error.message))?
        } else {
            (None, None)
        };
        cursor.space();
        let subset = if cursor.eat("[") {
            let rest = cursor.rest();
            let end = rest
                .rfind(']')
                .ok_or_else(|| self.err(ErrorKind::Syntax, "unclosed internal subset"))?;
            if !rest[end + 1..].chars().all(whitespace) {
                return Err(self.err(ErrorKind::Syntax, "trailing text after internal subset"));
            }
            Some(&rest[..end])
        } else {
            if !cursor.rest().is_empty() {
                return Err(self.err(ErrorKind::Syntax, "invalid document type declaration"));
            }
            None
        };
        self.has_external_subset = system_id.is_some();
        self.seen_doctype = true;
        self.declaration_allowed = false;
        self.emit(
            EventKind::StartDoctype {
                name,
                system_id,
                public_id,
                has_internal_subset: subset.is_some(),
            },
            position,
        )?;
        let header_end = subset.map_or(token.len() - 1, |_| token.len() - 1 - cursor.rest().len());
        self.event_raw(&token[..header_end])?;
        if let Some(subset) = subset {
            self.parse_subset(subset, header_end)?;
        }
        self.emit(EventKind::EndDoctype, position)?;
        self.event_raw(if subset.is_some() { "]>" } else { ">" })?;
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
                self.emit(
                    EventKind::Comment(normalize_newlines(value, self.allocator)?),
                    position,
                )?;
                self.event_raw(&text[..end + 7])?;
                text = &rest[end + 3..];
                continue;
            }
            if text.starts_with("<?") {
                let end = text.find("?>").ok_or_else(|| {
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
            let first_event = self.pending.len();
            let mut cursor = Cursor::new(&text[2..end]);
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
                    parse_content_model(&mut cursor, 0, self.config.limits.max_depth)
                        .map_err(|message| self.err(ErrorKind::Syntax, message))?;
                    let model = string(
                        &model_start[..model_start.len() - cursor.rest().len()],
                        self.allocator,
                    )?;
                    self.emit(EventKind::ElementDeclaration { name, model }, position)?;
                }
                "NOTATION" => {
                    let name = cursor
                        .name()
                        .map_err(|message| self.err(ErrorKind::Syntax, message))?;
                    let name = string(name, self.allocator)?;
                    cursor
                        .require_space()
                        .map_err(|message| self.err(ErrorKind::Syntax, message))?;
                    let (system_id, public_id) = external_id(&mut cursor, true, self.allocator)
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
                return Err(self.err(ErrorKind::Syntax, "unexpected text in DTD declaration"));
            }
            for (index, pending) in self.pending.iter_mut().enumerate().skip(first_event) {
                pending.raw = Some(if index == first_event {
                    string(&text[..end + 1], self.allocator)?
                } else {
                    String::new_in(self.allocator)
                });
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
            .name()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let name = string(name, self.allocator)?;
        cursor
            .require_space()
            .map_err(|message| self.err(ErrorKind::Syntax, message))?;
        let (value, system_id, public_id, notation) = if cursor.starts("\"") || cursor.starts("'") {
            let raw = cursor
                .quoted()
                .map_err(|message| self.err(ErrorKind::Syntax, message))?;
            if raw.contains('%') {
                return Err(self.err(
                    ErrorKind::ExternalEntityHandling,
                    "parameter entities in entity values are unsupported",
                ));
            }
            let mut value = String::try_with_capacity_in(raw.len(), self.allocator)?;
            let mut rest = raw;
            while let Some(start) = rest.find('&') {
                value.push_str(&normalize_newlines(&rest[..start], self.allocator)?)?;
                rest = &rest[start..];
                let end = rest.find(';').ok_or_else(|| {
                    self.err(
                        ErrorKind::InvalidToken,
                        "unclosed entity reference in entity value",
                    )
                })?;
                let reference = &rest[1..end];
                if reference.starts_with('#') {
                    value.push(
                        character_reference(reference)
                            .map_err(|kind| {
                                self.err(kind, "invalid character reference in entity value")
                            })?
                            .expect("numeric reference"),
                    )?;
                } else {
                    if !crate::names::is_name(reference) {
                        return Err(self.err(
                            ErrorKind::InvalidToken,
                            "invalid entity reference in entity value",
                        ));
                    }
                    value.push_str(&rest[..end + 1])?;
                }
                rest = &rest[end + 1..];
            }
            value.push_str(&normalize_newlines(rest, self.allocator)?)?;
            (Some(value), None, None, None)
        } else {
            let (system_id, public_id) = external_id(cursor, false, self.allocator)
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
                        .name()
                        .map_err(|message| self.err(ErrorKind::Syntax, message))?,
                    self.allocator,
                )?)
            } else {
                None
            };
            (None, system_id, public_id, notation)
        };
        if !parameter && !self.entities.contains_key(&name) {
            if self.entities.len() >= self.config.limits.max_entities {
                return Err(self.err(
                    ErrorKind::LimitExceeded,
                    "entity declaration count limit exceeded",
                ));
            }
            try_insert(
                &mut self.entities,
                name.try_clone()?,
                Entity {
                    value: value.try_clone()?,
                    system_id: system_id.try_clone()?,
                    public_id: public_id.try_clone()?,
                    notation: notation.try_clone()?,
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
        } else if parameter {
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
            let attribute_type =
                string(&start[..start.len() - cursor.rest().len()], self.allocator)?;
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
                let mut value = self.expand_attribute(raw, &mut Vec::new_in(self.allocator))?;
                if attribute_type != "CDATA" {
                    value = collapse_spaces(&value, self.allocator)?;
                }
                Some(value)
            };
            if !self.defaults.contains_key(&element) {
                try_insert(
                    &mut self.defaults,
                    element.try_clone()?,
                    Vec::new_in(self.allocator),
                )?;
            }
            let declarations = self
                .defaults
                .get_mut(&element)
                .expect("default list was inserted");
            if !declarations
                .iter()
                .any(|declaration| declaration.name == name)
            {
                if declarations.len() >= self.config.limits.max_attributes {
                    return Err(self.err(
                        ErrorKind::LimitExceeded,
                        "default attribute count limit exceeded",
                    ));
                }
                try_push(
                    declarations,
                    DefaultAttribute {
                        name: name.try_clone()?,
                        attribute_type: attribute_type.try_clone()?,
                        value: value.try_clone()?,
                    },
                )?;
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
}
impl<'a> Cursor<'a> {
    fn new(text: &'a str) -> Self {
        Self { text }
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
        self.text = rest;
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
) -> Result<(Option<String>, Option<String>), Error> {
    let syntax = |message| Error::bare(ErrorKind::Syntax, message);
    if cursor.eat("SYSTEM") {
        cursor.require_space().map_err(syntax)?;
        let value = cursor.quoted().map_err(syntax)?;
        Ok((Some(normalize_newlines(value, allocator)?), None))
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
            Some(normalize_newlines(system, allocator)?),
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
            cursor.name()?;
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
