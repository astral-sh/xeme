//! Incremental lexical validation for external parameter entities in values.

use crate::ErrorKind;
use crate::names::{NameRules, is_xml_char, whitespace};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValueScan {
    NeedMore,
    Complete,
    XmlDeclaration { start: usize, end: usize },
}

#[derive(Clone, Copy, Debug)]
enum NameKind {
    Name,
    Prefixed,
    Token,
    AfterColon,
}

#[derive(Clone, Copy, Debug)]
enum State {
    Token,
    LessThan,
    DeclarationStart,
    DeclarationName,
    DeclarationPercent,
    CommentOpen,
    Comment(u8),
    PiStart,
    PiName,
    PiBody { declaration: bool, question: bool },
    PiClose { declaration: bool },
    Literal(char),
    AfterLiteral,
    PercentStart,
    PercentName,
    PoundStart,
    PoundName,
    Name(NameKind),
    RightParen,
    RightBracket,
    DoubleBracket,
}

/// Scans an append-only decoded buffer without retaining references to it.
///
/// The caller owns decoding and size limits. A completed first XML declaration
/// is returned once for separate interpretation; later ones remain value text.
#[derive(Debug)]
pub(crate) struct ValueScanner {
    cursor: usize,
    name_rules: NameRules,
    token_start: usize,
    state: State,
    declaration_seen: bool,
    #[cfg(test)]
    steps: usize,
}

impl ValueScanner {
    pub(crate) fn new(name_rules: NameRules) -> Self {
        Self {
            cursor: 0,
            name_rules,
            token_start: 0,
            state: State::Token,
            declaration_seen: false,
            #[cfg(test)]
            steps: 0,
        }
    }

    /// Validate newly appended characters, returning errors at token starts.
    pub(crate) fn scan(
        &mut self,
        text: &str,
        is_final: bool,
        namespaces: bool,
    ) -> Result<ValueScan, (ErrorKind, usize)> {
        let name_rules = self.name_rules;
        while let Some(character) = text[self.cursor..].chars().next() {
            #[cfg(test)]
            {
                self.steps += 1;
            }
            let next = self.cursor + character.len_utf8();
            if matches!(self.state, State::Token) {
                self.token_start = self.cursor;
            }
            if !is_xml_char(character) {
                if matches!(self.state, State::RightBracket | State::DoubleBracket) {
                    // A bracket has no required following delimiter. Diagnose
                    // this character as the next token, after closing it.
                    self.state = State::Token;
                    continue;
                }
                return Err((ErrorKind::InvalidToken, self.token_start));
            }
            match self.state {
                State::Token => {
                    self.state = match character {
                        c if whitespace(c) => State::Token,
                        '<' => State::LessThan,
                        '\'' | '"' => State::Literal(character),
                        '%' => State::PercentStart,
                        '#' => State::PoundStart,
                        ')' => State::RightParen,
                        ']' => State::RightBracket,
                        ',' | '[' | '(' | '|' | '>' => State::Token,
                        c if name_rules.is_name_char(c) => {
                            State::Name(if name_start(c, namespaces, name_rules) {
                                NameKind::Name
                            } else {
                                NameKind::Token
                            })
                        }
                        _ => return Err((ErrorKind::InvalidToken, self.token_start)),
                    };
                }
                State::LessThan => {
                    self.state = match character {
                        '!' => State::DeclarationStart,
                        '?' => State::PiStart,
                        c if name_start(c, namespaces, name_rules) || !c.is_ascii() => {
                            return Err((ErrorKind::Syntax, self.token_start));
                        }
                        _ => return Err((ErrorKind::InvalidToken, self.token_start)),
                    };
                }
                State::DeclarationStart => {
                    self.state = match character {
                        '-' => State::CommentOpen,
                        '[' => State::Token,
                        c if declaration_name(c, namespaces, name_rules) => State::DeclarationName,
                        _ => return Err((ErrorKind::InvalidToken, self.token_start)),
                    };
                }
                State::DeclarationName => {
                    if whitespace(character) {
                        self.state = State::Token;
                        continue;
                    }
                    if character == '%' {
                        self.state = State::DeclarationPercent;
                    } else if !declaration_name(character, namespaces, name_rules) {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                }
                State::DeclarationPercent => {
                    if whitespace(character) || character == '%' {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                    // The percent starts the next token; it was only lookahead
                    // for the declaration keyword's required separator.
                    self.token_start = self.cursor - 1;
                    self.state = State::PercentStart;
                    continue;
                }
                State::CommentOpen => {
                    if character != '-' {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                    self.state = State::Comment(0);
                }
                State::Comment(dashes) => {
                    self.state = match (dashes, character) {
                        (2, '>') => State::Token,
                        (2, _) => return Err((ErrorKind::InvalidToken, self.token_start)),
                        (_, '-') => State::Comment(dashes + 1),
                        _ => State::Comment(0),
                    };
                }
                State::PiStart => {
                    if !name_start(character, namespaces, name_rules) {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                    self.state = State::PiName;
                }
                State::PiName => {
                    if name_char(character, namespaces, name_rules) {
                        self.cursor = next;
                        continue;
                    }
                    let target = &text[self.token_start + 2..self.cursor];
                    let declaration = target == "xml";
                    if !declaration && target.eq_ignore_ascii_case("xml") {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                    self.state = match character {
                        c if whitespace(c) => State::PiBody {
                            declaration,
                            question: false,
                        },
                        '?' => State::PiClose { declaration },
                        _ => return Err((ErrorKind::InvalidToken, self.token_start)),
                    };
                }
                State::PiBody {
                    declaration,
                    question,
                } => {
                    if question && character == '>' {
                        self.state = State::Token;
                        self.cursor = next;
                        if declaration && !self.declaration_seen {
                            self.declaration_seen = true;
                            return Ok(ValueScan::XmlDeclaration {
                                start: self.token_start,
                                end: next,
                            });
                        }
                        continue;
                    }
                    self.state = State::PiBody {
                        declaration,
                        question: character == '?',
                    };
                }
                State::PiClose { declaration } => {
                    if character != '>' {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                    self.state = State::Token;
                    self.cursor = next;
                    if declaration && !self.declaration_seen {
                        self.declaration_seen = true;
                        return Ok(ValueScan::XmlDeclaration {
                            start: self.token_start,
                            end: next,
                        });
                    }
                    continue;
                }
                State::Literal(quote) => {
                    if character == quote {
                        self.state = State::AfterLiteral;
                    }
                }
                State::AfterLiteral => {
                    if !whitespace(character) && !matches!(character, '>' | '%' | '[') {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                    self.state = State::Token;
                    continue;
                }
                State::PercentStart => {
                    if whitespace(character) || character == '%' {
                        self.state = State::Token;
                        continue;
                    }
                    if !name_start(character, namespaces, name_rules) {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                    self.state = State::PercentName;
                }
                State::PercentName => {
                    if character == ';' {
                        self.state = State::Token;
                    } else if !name_char(character, namespaces, name_rules) {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                }
                State::PoundStart => {
                    if !name_start(character, namespaces, name_rules) {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                    self.state = State::PoundName;
                }
                State::PoundName => {
                    if whitespace(character) || matches!(character, ')' | '>' | '%' | '|') {
                        self.state = State::Token;
                        continue;
                    }
                    if !name_char(character, namespaces, name_rules) {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                }
                State::Name(mut kind) => {
                    if matches!(kind, NameKind::AfterColon) {
                        kind = if name_char(character, namespaces, name_rules) {
                            NameKind::Prefixed
                        } else {
                            NameKind::Token
                        };
                        self.state = State::Name(kind);
                    }
                    if namespaces && character == ':' {
                        self.state = State::Name(if matches!(kind, NameKind::Name) {
                            NameKind::AfterColon
                        } else {
                            NameKind::Token
                        });
                    } else if name_char(character, namespaces, name_rules) {
                        // Keep scanning this name or Nmtoken.
                    } else if whitespace(character)
                        || matches!(character, '>' | ')' | ',' | '|' | '[' | '%')
                    {
                        self.state = State::Token;
                        continue;
                    } else if matches!(character, '+' | '*' | '?')
                        && !matches!(kind, NameKind::Token)
                    {
                        self.state = State::Token;
                    } else {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                }
                State::RightParen => {
                    if matches!(character, '*' | '?' | '+') {
                        self.state = State::Token;
                    } else if whitespace(character) || matches!(character, '>' | ',' | '|' | ')') {
                        self.state = State::Token;
                        continue;
                    } else {
                        return Err((ErrorKind::InvalidToken, self.token_start));
                    }
                }
                State::RightBracket => {
                    if character == ']' {
                        self.state = State::DoubleBracket;
                    } else {
                        self.state = State::Token;
                        continue;
                    }
                }
                State::DoubleBracket => match character {
                    '>' => self.state = State::Token,
                    ']' => self.token_start += 1,
                    _ => {
                        self.state = State::Token;
                        continue;
                    }
                },
            }
            self.cursor = next;
        }
        if !is_final {
            return Ok(ValueScan::NeedMore);
        }
        match self.state {
            State::Token
            | State::AfterLiteral
            | State::PoundName
            | State::Name(NameKind::Name | NameKind::Prefixed | NameKind::Token)
            | State::RightParen
            | State::RightBracket => Ok(ValueScan::Complete),
            _ => Err((ErrorKind::UnclosedToken, self.token_start)),
        }
    }
}

fn name_start(character: char, namespaces: bool, name_rules: NameRules) -> bool {
    name_rules.is_name_start(character) && (!namespaces || character != ':')
}

fn name_char(character: char, namespaces: bool, name_rules: NameRules) -> bool {
    name_rules.is_name_char(character) && (!namespaces || character != ':')
}

fn declaration_name(character: char, namespaces: bool, name_rules: NameRules) -> bool {
    character.is_ascii() && name_start(character, namespaces, name_rules)
}

#[cfg(test)]
mod tests {
    use super::{NameRules, ValueScan, ValueScanner};
    use crate::ErrorKind;

    fn scan(text: &str, width: usize, namespaces: bool) -> Result<(), (ErrorKind, usize)> {
        let mut scanner = ValueScanner::new(NameRules::default());
        for end in (1..=text.len()).filter(|end| text.is_char_boundary(*end)) {
            if end % width != 0 && end != text.len() {
                continue;
            }
            while let ValueScan::XmlDeclaration { .. } =
                scanner.scan(&text[..end], end == text.len(), namespaces)?
            {}
        }
        assert!(scanner.steps <= text.len() * 3, "scanner rescanned input");
        Ok(())
    }

    #[test]
    fn complete_tokens_accept_every_chunk_boundary() {
        for text in [
            "X 名",
            "'x'",
            "\"&#13;&amp;\"",
            "<!ENTITY e 'x'>",
            "<!--a-->",
            "<?pi?>",
            "%p;",
            "#PCDATA",
            "a*",
            "0",
            ")",
            "]",
            "]]>",
            "]]]>",
            "[ ( a | b )* >",
            "X <?xml version='1.0'?><?xml?> X",
        ] {
            for width in 1..=text.len() {
                assert_eq!(scan(text, width, false), Ok(()), "{text:?}, {width}");
            }
        }
    }

    #[test]
    fn lexical_errors_cannot_join_tokens_across_feeds() {
        for (text, kind) in [
            ("<r>", ErrorKind::Syntax),
            ("</r>", ErrorKind::InvalidToken),
            ("'", ErrorKind::UnclosedToken),
            ("x'x'", ErrorKind::InvalidToken),
            ("'x'a", ErrorKind::InvalidToken),
            ("&amp;", ErrorKind::InvalidToken),
            ("<!--x--y>", ErrorKind::InvalidToken),
            ("%p", ErrorKind::UnclosedToken),
            ("%p x", ErrorKind::InvalidToken),
            ("*", ErrorKind::InvalidToken),
            ("0*", ErrorKind::InvalidToken),
            (")]", ErrorKind::InvalidToken),
            ("]]", ErrorKind::UnclosedToken),
            ("<!ENTITY% p", ErrorKind::InvalidToken),
            ("<?XML?>", ErrorKind::InvalidToken),
            ("<?pi?x>", ErrorKind::InvalidToken),
        ] {
            for width in 1..=text.len() {
                assert_eq!(
                    scan(text, width, false).map_err(|error| error.0),
                    Err(kind),
                    "{text:?}, {width}"
                );
            }
        }
    }

    #[test]
    fn first_xml_declaration_is_returned_once_with_original_offsets() {
        let text = "prefix <?xml version='1.0'?><?xml?>";
        let mut scanner = ValueScanner::new(NameRules::default());
        assert_eq!(
            scanner.scan(text, true, false),
            Ok(ValueScan::XmlDeclaration { start: 7, end: 28 })
        );
        assert_eq!(scanner.scan(text, true, false), Ok(ValueScan::Complete));
    }

    #[test]
    fn invalid_characters_after_brackets_start_a_new_token() {
        for text in ["]\0", "]]\0"] {
            for width in 1..=text.len() {
                assert_eq!(
                    scan(text, width, false),
                    Err((ErrorKind::InvalidToken, text.len() - 1))
                );
            }
        }
    }

    #[test]
    fn unfinished_tokens_have_linear_work() {
        for (prefix, suffix) in [("'", "'"), ("<!--", "-->"), ("<?pi ", "?>"), ("", " ")] {
            let text = format!("{prefix}{}{suffix}", "a".repeat(16 * 1024));
            assert_eq!(scan(&text, 1, false), Ok(()));
        }
    }
}
