//! Python's streaming event interface to the safe Xeme parser.
#![forbid(unsafe_code)]

use pyo3::exceptions::{PyException, PyMemoryError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use pyo3::{IntoPyObjectExt, PyTraverseError, PyVisit, create_exception};
use xeme::{Config, Error, ErrorKind, EventKind};

create_exception!(
    xeme,
    ParseError,
    PyException,
    "An XML parse or resource-limit error."
);

/// Convert an engine failure without retaining a Python traceback in the parser.
fn python_error(py: Python<'_>, error: Error) -> PyErr {
    if error.kind == ErrorKind::NoMemory {
        return PyMemoryError::new_err(error.message);
    }
    let exception = ParseError::new_err(format!(
        "{} at line {}, column {} (byte {})",
        error.message, error.position.line, error.position.column, error.position.byte_index
    ));
    let value = exception.value(py);
    let attributes = (|| -> PyResult<()> {
        value.setattr("kind", format!("{:?}", error.kind))?;
        value.setattr("line", error.position.line)?;
        value.setattr("column", error.position.column)?;
        value.setattr("byte_index", error.position.byte_index)?;
        Ok(())
    })();
    match attributes {
        Ok(()) => exception,
        Err(error) => error,
    }
}

/// Limits are immutable snapshots; each parser owns its configuration.
#[pyclass(name = "Limits", module = "xeme", frozen, get_all)]
struct PyLimits {
    max_depth: usize,
    max_token_bytes: usize,
    max_total_bytes: usize,
    max_entity_expansion_bytes: usize,
    max_entity_depth: usize,
    max_attributes: usize,
    max_entities: usize,
}

#[pymethods]
impl PyLimits {
    #[new]
    #[pyo3(signature = (*, max_depth=256, max_token_bytes=16*1024*1024,
        max_total_bytes=256*1024*1024, max_entity_expansion_bytes=8*1024*1024,
        max_entity_depth=32, max_attributes=10_000, max_entities=10_000))]
    fn new(
        max_depth: usize,
        max_token_bytes: usize,
        max_total_bytes: usize,
        max_entity_expansion_bytes: usize,
        max_entity_depth: usize,
        max_attributes: usize,
        max_entities: usize,
    ) -> Self {
        Self {
            max_depth,
            max_token_bytes,
            max_total_bytes,
            max_entity_expansion_bytes,
            max_entity_depth,
            max_attributes,
            max_entities,
        }
    }
}

impl From<&PyLimits> for xeme::Limits {
    fn from(value: &PyLimits) -> Self {
        Self {
            max_depth: value.max_depth,
            max_token_bytes: value.max_token_bytes,
            max_total_bytes: value.max_total_bytes,
            max_entity_expansion_bytes: value.max_entity_expansion_bytes,
            max_entity_depth: value.max_entity_depth,
            max_attributes: value.max_attributes,
            max_entities: value.max_entities,
            max_work_amplification: None,
        }
    }
}

/// Source coordinates refer to the original encoded bytes.
#[pyclass(
    name = "Position",
    module = "xeme",
    frozen,
    get_all,
    skip_from_py_object
)]
#[derive(Clone, Copy)]
struct PyPosition {
    line: usize,
    column: usize,
    byte_index: usize,
    byte_count: usize,
}

impl From<xeme::Position> for PyPosition {
    fn from(position: xeme::Position) -> Self {
        Self {
            line: position.line,
            column: position.column,
            byte_index: position.byte_index,
            byte_count: position.byte_count,
        }
    }
}

#[pymethods]
impl PyPosition {
    fn __repr__(&self) -> String {
        format!(
            "Position(line={}, column={}, byte_index={}, byte_count={})",
            self.line, self.column, self.byte_index, self.byte_count
        )
    }
}

/// An event's Python payload remains valid independently of later parser calls.
#[pyclass(name = "Event", module = "xeme", frozen, get_all)]
struct PyEvent {
    kind: &'static str,
    data: Py<PyAny>,
    position: PyPosition,
}

#[pymethods]
impl PyEvent {
    /// Attribute dictionaries can contain application objects, including cycles.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.data)
    }
}

impl PyEvent {
    fn from_event(py: Python<'_>, event: xeme::Event) -> PyResult<Option<Py<Self>>> {
        let (kind, data) = match event.kind {
            EventKind::StartElement { name, attributes } => {
                let attrs = PyDict::new(py);
                for attribute in attributes {
                    attrs.set_item(attribute.name.as_str(), attribute.value.as_str())?;
                }
                ("start", (name.as_str(), attrs).into_py_any(py)?)
            }
            EventKind::EndElement { name } => ("end", name.as_str().into_py_any(py)?),
            EventKind::Text(text) => ("text", text.as_ref().into_py_any(py)?),
            EventKind::Comment(text) => ("comment", text.as_str().into_py_any(py)?),
            EventKind::ProcessingInstruction { target, data } => {
                ("pi", (target.as_str(), data.as_str()).into_py_any(py)?)
            }
            EventKind::StartNamespace { prefix, uri } => (
                "start_ns",
                (prefix.as_deref(), uri.as_deref()).into_py_any(py)?,
            ),
            EventKind::EndNamespace { prefix } => ("end_ns", prefix.as_deref().into_py_any(py)?),
            EventKind::StartCdata => ("start_cdata", py.None()),
            EventKind::EndCdata => ("end_cdata", py.None()),
            EventKind::XmlDeclaration {
                version,
                encoding,
                standalone,
            } => (
                "xml_decl",
                (version.as_str(), encoding.as_deref(), standalone).into_py_any(py)?,
            ),
            EventKind::StartDoctype(declaration) => (
                "start_doctype",
                (
                    declaration.name.as_str(),
                    declaration.system_id.as_deref(),
                    declaration.public_id.as_deref(),
                    declaration.has_internal_subset,
                )
                    .into_py_any(py)?,
            ),
            EventKind::EndDoctype => ("end_doctype", py.None()),
            // Declaration details and C-adapter raw events are not part of this API.
            EventKind::Default
            | EventKind::EntityDeclarationPrefix
            | EventKind::AttlistDeclarationPrefix
            | EventKind::ElementDeclarationPrefix
            | EventKind::NotationDeclarationPrefix
            | EventKind::EntityDeclarationDuplicate { .. }
            | EventKind::DoctypeClosingPrefix
            | EventKind::EntityDeclaration(_)
            | EventKind::AttlistDeclaration(_)
            | EventKind::NotationDeclaration(_)
            | EventKind::ElementDeclaration { .. }
            | EventKind::NotStandalone
            | EventKind::TextDeclaration { .. }
            | EventKind::SkippedEntity { .. }
            | EventKind::ExternalEntityReference(_) => return Ok(None),
        };
        Ok(Some(Py::new(
            py,
            Self {
                kind,
                data,
                position: event.position.into(),
            },
        )?))
    }
}

/// Input must be drained to the next input boundary before another feed.
#[pyclass(name = "Parser", module = "xeme._native")]
struct PyParser {
    parser: xeme::Parser,
    needs_drain: bool,
    final_input: bool,
    error: Option<Error>,
}

impl PyParser {
    fn fail(&mut self, py: Python<'_>, error: Error) -> PyErr {
        self.error = Some(error);
        python_error(py, error)
    }
}

#[pymethods]
impl PyParser {
    #[new]
    #[pyo3(signature = (*, encoding=None, namespace_separator=None,
        namespace_triplets=false, limits=None))]
    fn new(
        py: Python<'_>,
        encoding: Option<String>,
        namespace_separator: Option<&str>,
        namespace_triplets: bool,
        limits: Option<PyRef<'_, PyLimits>>,
    ) -> PyResult<Self> {
        let separator = namespace_separator
            .map(str::parse::<char>)
            .transpose()
            .map_err(|_| {
                PyValueError::new_err("namespace_separator must be exactly one character")
            })?;
        if namespace_triplets && separator.is_none() {
            return Err(PyValueError::new_err(
                "namespace_triplets requires namespace_separator",
            ));
        }
        let config = Config {
            encoding,
            namespace_separator: separator,
            namespace_triplets,
            limits: limits
                .as_deref()
                .map(xeme::Limits::from)
                .unwrap_or_default(),
            ..Config::default()
        };
        let mut parser = xeme::Parser::try_new_in(config, xeme_storage::Allocator::System)
            .map_err(|error| python_error(py, error))?;
        // Expand internal parameter entities and report external requests so the
        // Python policy can reject them instead of silently skipping declarations.
        parser.set_param_entity_parsing(2);
        Ok(Self {
            parser,
            needs_drain: false,
            final_input: false,
            error: None,
        })
    }

    /// Append encoded bytes. Syntax errors can also be raised by `next_event`.
    #[pyo3(signature = (data, r#final=false))]
    fn feed(&mut self, py: Python<'_>, data: &Bound<'_, PyBytes>, r#final: bool) -> PyResult<()> {
        if let Some(error) = self.error {
            return Err(python_error(py, error));
        }
        if self.final_input {
            return Err(PyValueError::new_err("cannot feed after final input"));
        }
        if self.needs_drain {
            return Err(PyRuntimeError::new_err(
                "drain read_events() before feeding more input",
            ));
        }
        self.parser
            .feed(data.as_bytes(), r#final)
            .map_err(|error| self.fail(py, error))?;
        self.needs_drain = true;
        self.final_input = r#final;
        Ok(())
    }

    /// Return one event, or `None` after fully draining the supplied input.
    fn next_event(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyEvent>>> {
        if let Some(error) = self.error {
            return Err(python_error(py, error));
        }
        loop {
            let Some(event) = self
                .parser
                .next_event()
                .map_err(|error| self.fail(py, error))?
            else {
                self.needs_drain = false;
                return Ok(None);
            };
            // This package cannot resolve external resources. Reject requests before
            // advancing the engine, including external subsets otherwise skipped by default.
            let external = match &event.kind {
                EventKind::StartDoctype(declaration) => {
                    declaration.system_id.is_some() || declaration.public_id.is_some()
                }
                EventKind::ExternalEntityReference(_) | EventKind::SkippedEntity { .. } => true,
                _ => false,
            };
            if external {
                let error = Error {
                    kind: ErrorKind::ExternalEntityHandling,
                    message: "external entities are not supported by the Python API",
                    position: event.position,
                };
                return Err(self.fail(py, error));
            }
            let position = event.position;
            match PyEvent::from_event(py, event) {
                Ok(Some(event)) => return Ok(Some(event)),
                Ok(None) => continue,
                Err(error) => {
                    // The engine consumed an event which Python could not retain.
                    // Fail permanently instead of resuming with missing output.
                    self.error = Some(Error {
                        kind: ErrorKind::NoMemory,
                        message: "could not allocate a Python event",
                        position,
                    });
                    return Err(error);
                }
            }
        }
    }
}

/// The extension deliberately retains the GIL while accessing its parser.
#[pymodule(gil_used = true)]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyParser>()?;
    module.add_class::<PyLimits>()?;
    module.add_class::<PyPosition>()?;
    module.add_class::<PyEvent>()?;
    module.add("ParseError", module.py().get_type::<ParseError>())?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
