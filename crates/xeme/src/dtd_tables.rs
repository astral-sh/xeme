//! Parameter children share committed definitions; general children own snapshots.
//!
//! Parsing helpers access the inline working slot during one public core call.
//! Owned events leave that call only after the tables have returned to their owner.

use xeme_storage::{Allocator, HashMap, Shared, String, TryLock, TryLockGuard, hash_map};

use crate::{DefaultAttributes, Entity, Error, ErrorKind, Limits, Parser};

#[derive(Debug)]
pub(crate) struct DtdTables {
    pub(crate) entities: HashMap<String, Entity>,
    pub(crate) parameter_entities: HashMap<String, Entity>,
    pub(crate) defaults: HashMap<String, DefaultAttributes>,
    /// DTD hash identity is independent of each parser's namespace/active indexes.
    pub(crate) salt: [u8; 16],
    pub(crate) max_entities: usize,
    pub(crate) max_attributes: usize,
    pub(crate) max_default_attributes: usize,
}

impl DtdTables {
    pub(crate) fn new(allocator: Allocator, limits: &Limits) -> Self {
        let entities = hash_map(allocator);
        let salt = entities.hasher().salt();
        Self {
            entities,
            parameter_entities: hash_map(allocator),
            defaults: hash_map(allocator),
            salt,
            max_entities: limits.max_entities,
            max_attributes: limits.max_attributes,
            max_default_attributes: 0,
        }
    }

    pub(crate) fn empty_like(&self) -> Self {
        Self {
            entities: HashMap::with_hasher_in(
                self.entities.hasher().clone(),
                *self.entities.allocator(),
            ),
            parameter_entities: HashMap::with_hasher_in(
                self.parameter_entities.hasher().clone(),
                *self.parameter_entities.allocator(),
            ),
            defaults: HashMap::with_hasher_in(
                self.defaults.hasher().clone(),
                *self.defaults.allocator(),
            ),
            salt: self.salt,
            max_entities: self.max_entities,
            max_attributes: self.max_attributes,
            max_default_attributes: 0,
        }
    }

    fn is_empty(&self) -> bool {
        self.entities.is_empty() && self.parameter_entities.is_empty() && self.defaults.is_empty()
    }
}

/// The guard borrows a separate local owner, never a field of this scope.
struct Scope<'parser, 'owner> {
    parser: &'parser mut Parser,
    tables: TryLockGuard<'owner, DtdTables>,
}

impl<'parser, 'owner> Scope<'parser, 'owner> {
    fn new(parser: &'parser mut Parser, mut tables: TryLockGuard<'owner, DtdTables>) -> Self {
        debug_assert!(parser.tables.is_empty());
        std::mem::swap(&mut parser.tables, &mut tables);
        Self { parser, tables }
    }
}

impl Drop for Scope<'_, '_> {
    fn drop(&mut self) {
        // Publish container headers only: no allocation, destruction, assertions,
        // callbacks, or owner replacement. The guard then releases its lock.
        std::mem::swap(&mut self.parser.tables, &mut self.tables);
    }
}

impl Parser {
    /// Allocate before OnceLock::set so an allocator callback cannot run under
    /// the initialization lock. The working slot is still empty at this boundary.
    /// The suite accounts for this fixed allocation; moving existing containers
    /// adds no XML expansion or inherited-copy work.
    pub(crate) fn ensure_shared_tables(&self) -> Result<&Shared<TryLock<DtdTables>>, Error> {
        if let Some(owner) = self.shared_tables.get() {
            return Ok(owner);
        }
        debug_assert!(self.tables.is_empty());
        let owner = Shared::try_new_in(TryLock::new(self.tables.empty_like()), self.allocator)?;
        let _ = self.shared_tables.set(owner);
        Ok(self.shared_tables.get().expect("published DTD owner"))
    }

    /// Install an independently copied child's tables before exposing that child.
    pub(crate) fn publish_copied_tables(&mut self) -> Result<(), Error> {
        debug_assert!(self.shared_tables.get().is_none());
        // Allocate the owner before moving any tables, so failure is transactional.
        let owner = Shared::try_new_in(TryLock::new(self.tables.empty_like()), self.allocator)?;
        {
            let mut tables = owner.try_lock().expect("unpublished DTD owner");
            std::mem::swap(&mut self.tables, &mut tables);
        }
        self.shared_tables = std::sync::OnceLock::from(owner);
        Ok(())
    }

    pub(crate) fn dtd_busy(&self) -> Error {
        self.err(
            ErrorKind::ExternalEntityHandling,
            "shared DTD is already in use",
        )
    }

    /// Run exactly one table scope. Internal helpers must never reacquire it.
    pub(crate) fn with_dtd_tables<R>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let Some(owner) = self.shared_tables.get().cloned() else {
            return operation(self);
        };
        let tables = owner.try_lock().ok_or_else(|| self.dtd_busy())?;
        let scope = Scope::new(self, tables);
        operation(scope.parser)
    }

    pub(crate) fn entity_limit(&self) -> usize {
        self.config
            .limits
            .max_entities
            .min(self.tables.max_entities)
    }

    pub(crate) fn default_attribute_limit(&self) -> usize {
        self.config
            .limits
            .max_attributes
            .min(self.tables.max_attributes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, Event, EventKind};

    fn drain(parser: &mut Parser) -> Result<std::vec::Vec<Event>, Error> {
        let mut events = std::vec::Vec::new();
        while let Some(event) = parser.next_event()? {
            events.push(event);
        }
        Ok(events)
    }

    fn text(parser: &mut Parser, xml: &[u8]) -> std::string::String {
        parser.feed(xml, true).unwrap();
        let mut text = std::string::String::new();
        for event in drain(parser).unwrap() {
            if let EventKind::Text(value) = event.kind {
                text.push_str(&value);
            }
        }
        text
    }

    #[test]
    fn parameter_commits_survive_nonfinal_input_error_and_parent_drop() {
        for malformed in [false, true] {
            let mut parent = Parser::new(Config::default());
            parent.has_external_subset = true;
            let mut first = parent.external_child(None, None).unwrap();
            let mut sibling = parent.external_child(None, None).unwrap();
            let mut independent = parent.external_child(Some(""), None).unwrap();
            first
                .feed(
                    b"<!ENTITY e 'first'><!ATTLIST r a NMTOKENS ' a  b '>",
                    false,
                )
                .unwrap();
            drain(&mut first).unwrap();
            if malformed {
                first.feed(b"<!", true).unwrap();
                assert!(drain(&mut first).is_err());
            }
            assert!(parent.tables.is_empty());
            sibling.feed(b"<!ENTITY e 'second'>", true).unwrap();
            assert!(
                !drain(&mut sibling)
                    .unwrap()
                    .iter()
                    .any(|event| matches!(event.kind, EventKind::EntityDeclaration(_)))
            );
            let general = parent.external_child(Some(""), None).unwrap();
            drop(first);
            drop(sibling);
            parent
                .feed(b"<!DOCTYPE r SYSTEM 'd'><r>&e;</r>", true)
                .unwrap();
            let events = drain(&mut parent).unwrap();
            assert!(events.iter().any(|event| matches!(&event.kind, EventKind::StartElement { attributes, .. } if attributes[0].value == "a b")));
            assert!(
                events.iter().any(
                    |event| matches!(&event.kind, EventKind::Text(value) if &**value == "first")
                )
            );
            drop(parent);
            let mut retained = general.external_child(None, None).unwrap();
            retained.feed(b"<!ENTITY x '&e;'>", true).unwrap();
            drain(&mut retained).unwrap();
            drop(general);
            let mut content = retained.external_child(Some(""), None).unwrap();
            assert_eq!(text(&mut content, b"&x;"), "first");
            independent.feed(b"&e;", true).unwrap();
            assert!(drain(&mut independent).unwrap().iter().any(
                |event| matches!(&event.kind, EventKind::SkippedEntity { name, .. } if name == "e")
            ));
        }
    }

    #[test]
    fn start_doctype_child_initializes_owner_before_internal_declarations() {
        let xml = b"<!DOCTYPE r SYSTEM 'd' [<!ENTITY e 'parent'>]><r>&e;</r>";
        for width in 1..=xml.len() {
            let mut parent = Parser::new(Config::default());
            let mut result = std::string::String::new();
            let mut names = 0;
            for (offset, chunk) in xml.chunks(width).enumerate() {
                parent
                    .feed(chunk, (offset + 1) * width >= xml.len())
                    .unwrap();
                while let Some(event) = parent.next_event().unwrap() {
                    match event.kind {
                        EventKind::StartDoctype(_) => {
                            assert!(parent.tables.is_empty());
                            let mut child = parent.external_child(None, None).unwrap();
                            child.feed(b"<!ENTITY e 'child'>", false).unwrap();
                            drain(&mut child).unwrap();
                        }
                        EventKind::EntityDeclaration(_) => names += 1,
                        EventKind::Text(value) => result.push_str(&value),
                        _ => {}
                    }
                }
            }
            assert_eq!(result, "child");
            assert_eq!(names, 0);
            assert!(parent.tables.is_empty());
        }
    }

    #[test]
    fn ordinary_documents_and_their_general_children_do_not_allocate_an_owner() {
        let mut parent = Parser::new(Config::default());
        assert_eq!(text(&mut parent, b"<r>plain</r>"), "plain");
        let mut child = parent.external_child(Some(""), None).unwrap();
        assert_eq!(text(&mut child, b"<r>child</r>"), "child");
        assert!(parent.shared_tables.get().is_none());
        assert!(child.shared_tables.get().is_none());
    }

    #[test]
    fn metadata_caps_cannot_be_raised_by_parameter_children() {
        let mut parent = Parser::new(Config {
            limits: Limits {
                max_entities: 1,
                max_attributes: 1,
                ..Limits::default()
            },
            ..Config::default()
        });
        let mut child = parent.external_child(None, None).unwrap();
        child.set_limits(Limits::default()).unwrap();
        child
            .feed(b"<!ENTITY e 'E'><!ATTLIST r a CDATA 'A'>", false)
            .unwrap();
        drain(&mut child).unwrap();
        let lower = Limits {
            max_entities: 0,
            ..Limits::default()
        };
        assert_eq!(
            parent.set_limits(lower).unwrap_err().kind,
            ErrorKind::LimitExceeded
        );
        assert_eq!(parent.config.limits.max_entities, 1);
        for xml in [
            b"<!ENTITY x 'X'>".as_slice(),
            b"<!ATTLIST r b CDATA 'B'>".as_slice(),
        ] {
            let mut sibling = parent.external_child(None, None).unwrap();
            sibling.set_limits(Limits::default()).unwrap();
            sibling.feed(xml, true).unwrap();
            assert_eq!(
                drain(&mut sibling).unwrap_err().kind,
                ErrorKind::LimitExceeded
            );
        }
        let mut higher = parent.config.limits.clone();
        higher.max_entities = 2;
        higher.max_attributes = 2;
        parent.set_limits(higher).unwrap();
        child
            .feed(b"<!ENTITY x 'X'><!ATTLIST r b CDATA 'B'>", true)
            .unwrap();
        drain(&mut child).unwrap();
        assert_eq!(
            text(&mut parent, b"<!DOCTYPE r SYSTEM 'd'><r>&e;&x;</r>"),
            "EX"
        );
    }

    #[test]
    fn salt_changes_rehash_family_tables_without_changing_general_snapshots() {
        let mut parent = Parser::new(Config::default());
        parent.has_external_subset = true;
        let mut first = parent.external_child(None, None).unwrap();
        let mut sibling = parent.external_child(None, None).unwrap();
        first
            .feed(b"<!ENTITY e 'E'><!ATTLIST r a CDATA 'A'>", false)
            .unwrap();
        drain(&mut first).unwrap();
        let mut general = parent.external_child(Some(""), None).unwrap();
        let original = general.hash_salt();
        let salt = *b"0123456789abcdef";
        parent.set_hash_salt(salt).unwrap();
        sibling
            .with_dtd_tables(|parser| {
                assert_eq!(parser.tables.salt, salt);
                assert_eq!(parser.tables.entities.hasher().salt(), salt);
                assert_eq!(parser.tables.defaults["r"].by_name.hasher().salt(), salt);
                assert_eq!(parser.hash_salt(), original);
                Ok(())
            })
            .unwrap();
        // Same local seed is not a no-op if another family member changed the DTD.
        sibling.set_hash_salt(original).unwrap();
        parent
            .with_dtd_tables(|parser| {
                assert_eq!(parser.tables.salt, original);
                Ok(())
            })
            .unwrap();
        assert_eq!(text(&mut general, b"<r>&e;</r>"), "E");
        let mut snapshot = general.external_child(None, None).unwrap();
        snapshot
            .feed(b"<!ENTITY e 'changed'><!ENTITY x 'X'>", true)
            .unwrap();
        drain(&mut snapshot).unwrap();
        assert_eq!(text(&mut parent, b"<!DOCTYPE r SYSTEM 'd'><r>&e;</r>"), "E");
        parent
            .with_dtd_tables(|parser| {
                assert!(!parser.tables.entities.contains_key("x"));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn scope_publishes_on_error_and_unwind_and_contention_is_nonblocking() {
        let mut parent = Parser::new(Config::default());
        let mut sibling = parent.external_child(None, None).unwrap();
        let mut separate = Parser::new(Config::default());
        let owner = parent.shared_tables.get().unwrap().clone();
        let failure = parent
            .with_dtd_tables::<()>(|parser| {
                parser.tables.max_entities = 7;
                Err(parser.err(ErrorKind::Syntax, "controlled error"))
            })
            .unwrap_err();
        assert_eq!(failure.kind, ErrorKind::Syntax);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = parent.with_dtd_tables::<()>(|parser| {
                parser.tables.max_entities = 8;
                panic!("controlled unwind");
            });
        }));
        assert!(panic.is_err());
        assert!(parent.tables.is_empty());
        let held = owner.try_lock().unwrap();
        assert_eq!(held.max_entities, 8);
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    assert_eq!(
                        sibling.next_event().unwrap_err().kind,
                        ErrorKind::ExternalEntityHandling
                    );
                    let mut output = Some(Event {
                        kind: EventKind::Default,
                        position: Default::default(),
                    });
                    assert_eq!(
                        sibling
                            .next_event_for_recycling_into(&mut output)
                            .unwrap_err()
                            .kind,
                        ErrorKind::ExternalEntityHandling
                    );
                    assert!(output.is_none());
                    assert_eq!(
                        sibling.set_hash_salt([1; 16]).unwrap_err().kind,
                        ErrorKind::ExternalEntityHandling
                    );
                    assert_eq!(
                        sibling.set_limits(Limits::default()).unwrap_err().kind,
                        ErrorKind::ExternalEntityHandling
                    );
                    assert_eq!(
                        sibling.external_child(Some(""), None).unwrap_err().kind,
                        ErrorKind::ExternalEntityHandling
                    );
                    assert_eq!(text(&mut separate, b"<r>independent</r>"), "independent");
                })
                .join()
                .unwrap();
        });
        drop(held);
        sibling.feed(b"<!ENTITY e 'E'>", true).unwrap();
        drain(&mut sibling).unwrap();
        assert_eq!(text(&mut parent, b"<!DOCTYPE r SYSTEM 'd'><r>&e;</r>"), "E");
    }
}
