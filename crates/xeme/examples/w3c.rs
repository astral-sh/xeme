//! Native Fifth Edition catalog adapter. The caller resolves and supplies all files.

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use xeme::{Config, ErrorKind, EventKind, NameRules, Parser};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Parse {
        data: Vec<u8>,
        base: String,
        chunk: usize,
        namespaces: bool,
    },
}

#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Resolution {
    File { base: String, data: Vec<u8> },
    Error { error: String },
}

#[derive(Serialize)]
struct Outcome {
    status: u8,
    error: Option<String>,
    index: usize,
}

impl Outcome {
    fn failure(kind: ErrorKind, index: usize) -> Self {
        Self {
            status: 0,
            error: Some(format!("{kind:?}")),
            index,
        }
    }
}

#[derive(Serialize)]
struct Child {
    base: String,
    status: u8,
    error: Option<String>,
}

struct Case<'a, R, W> {
    input: &'a mut R,
    output: &'a mut W,
    requests: usize,
    children: Vec<Child>,
    resolver_errors: Vec<String>,
}

impl<R: BufRead, W: Write> Case<'_, R, W> {
    fn parse(
        &mut self,
        parser: &mut Parser,
        data: &[u8],
        base: &str,
        chunk: usize,
        depth: usize,
    ) -> Result<Outcome> {
        if let Err(error) = parser.set_base(Some(base.as_bytes())) {
            return Ok(Outcome::failure(error.kind, error.position.byte_index));
        }
        if !parser.set_param_entity_parsing(2) {
            return Err("cannot enable parameter entities before parsing".into());
        }
        // An empty source must still be finalized and drained.
        let mut parts = data.chunks(chunk).peekable();
        loop {
            let part = parts.next().unwrap_or_default();
            let final_input = parts.peek().is_none();
            if let Err(error) = parser.feed(part, final_input) {
                return Ok(Outcome::failure(error.kind, error.position.byte_index));
            }
            loop {
                let event = match parser.next_event() {
                    Ok(Some(event)) => event,
                    Ok(None) => break,
                    Err(error) => {
                        return Ok(Outcome::failure(error.kind, error.position.byte_index));
                    }
                };
                let EventKind::ExternalEntityReference(reference) = event.kind else {
                    continue;
                };
                let failure = || {
                    Outcome::failure(ErrorKind::ExternalEntityHandling, event.position.byte_index)
                };
                self.requests += 1;
                if depth >= 32 || self.requests > 1024 {
                    self.resolver_errors.push("external family budget".into());
                    return Ok(failure());
                }
                let Some(system) = reference.system_id.as_deref() else {
                    self.resolver_errors
                        .push("external reference has no system identifier".into());
                    return Ok(failure());
                };
                let declaration_base = reference
                    .base
                    .as_deref()
                    .map(std::str::from_utf8)
                    .transpose()?
                    .unwrap_or(base);
                send(
                    self.output,
                    &serde_json::json!({
                        "op": "resolve",
                        "base": declaration_base,
                        "system": system,
                    }),
                )?;
                let (base, data) =
                    match receive(self.input)?.ok_or("EOF while resolving an entity")? {
                        Resolution::File { base, data } => (base, data),
                        Resolution::Error { error } => {
                            self.resolver_errors.push(error);
                            return Ok(failure());
                        }
                    };
                let mut child = match parser.external_child(reference.context.as_deref(), None) {
                    Ok(child) => child,
                    Err(error) => {
                        return Ok(Outcome::failure(error.kind, error.position.byte_index));
                    }
                };
                let outcome = self.parse(&mut child, &data, &base, chunk, depth + 1)?;
                self.children.push(Child {
                    base,
                    status: outcome.status,
                    error: outcome.error,
                });
                if outcome.status != 1 {
                    return Ok(failure());
                }
                // DTD and value children publish into their parent's pending
                // declarations; general-content children need no DTD merge.
                if reference.context.is_none()
                    && let Err(error) = parser.merge_external_subset(&child)
                {
                    return Ok(Outcome::failure(error.kind, error.position.byte_index));
                }
            }
            if final_input {
                break;
            }
        }
        if !parser.is_finished() {
            return Err("parser did not finish after final input".into());
        }
        Ok(Outcome {
            status: 1,
            error: None,
            index: parser.position_between_callbacks(false).byte_index,
        })
    }
}

fn receive<T: DeserializeOwned>(input: &mut impl BufRead) -> Result<Option<T>> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&line)?))
}

fn send(output: &mut impl Write, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

fn run(input: &mut impl BufRead, output: &mut impl Write) -> Result<()> {
    while let Some(Request::Parse {
        data,
        base,
        chunk,
        namespaces,
    }) = receive(input)?
    {
        if !matches!(chunk, 1 | 7 | 4096) {
            return Err("chunk must be 1, 7, or 4096".into());
        }
        let config = Config {
            namespace_separator: namespaces.then_some('|'),
            ..Config::default()
        };
        assert_eq!(config.name_rules, NameRules::FifthEdition);
        let mut parser = Parser::new(config);
        let mut case = Case {
            input,
            output,
            requests: 0,
            children: Vec::new(),
            resolver_errors: Vec::new(),
        };
        let outcome = case.parse(&mut parser, &data, &base, chunk, 0)?;
        send(
            case.output,
            &serde_json::json!({
                "op": "result",
                "name_rules": "FifthEdition",
                "status": outcome.status,
                "error": outcome.error,
                "index": outcome.index,
                "children": case.children,
                "resolver_errors": case.resolver_errors,
            }),
        )?;
    }
    Ok(())
}

fn main() -> Result<()> {
    run(&mut io::stdin().lock(), &mut io::stdout().lock())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn request(xml: &str, chunk: usize) -> Value {
        json!({
            "op": "parse", "base": "file:///suite/root.xml",
            "data": xml.as_bytes(), "chunk": chunk, "namespaces": true,
        })
    }

    fn transcript(messages: &[Value]) -> Result<Vec<Value>> {
        let mut input = Vec::new();
        for message in messages {
            send(&mut input, message)?;
        }
        let mut output = Vec::new();
        run(&mut input.as_slice(), &mut output)?;
        output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| Ok(serde_json::from_slice(line)?))
            .collect()
    }

    #[test]
    fn persistent_requests_use_fifth_edition_and_finalize_empty_input() {
        let output = transcript(&[request("<\u{10000}/>", 1), request("", 7)]).unwrap();
        assert_eq!(output[0]["status"], 1);
        assert_eq!(output[1]["status"], 0);
        assert_eq!(output[1]["error"], "NoElements");
    }

    #[test]
    fn nested_dtds_preserve_declaration_bases_and_general_child_context() {
        for chunk in [1, 7, 4096] {
            let output = transcript(&[
                request("<!DOCTYPE r SYSTEM 'dtd/main.dtd'><r>&e;</r>", chunk),
                json!({"base": "file:///suite/dtd/main.dtd", "data":
                    b"<!ENTITY % p SYSTEM 'definitions.dtd'>%p;".as_slice()}),
                json!({"base": "file:///suite/dtd/definitions.dtd", "data":
                    b"<!ENTITY e SYSTEM '../content.xml'>".as_slice()}),
                json!({"base": "file:///suite/content.xml", "data":
                    "<\u{10000}/>".as_bytes()}),
            ])
            .unwrap();
            assert_eq!(output[0]["base"], "file:///suite/root.xml");
            assert_eq!(output[0]["system"], "dtd/main.dtd");
            assert_eq!(output[1]["base"], "file:///suite/dtd/main.dtd");
            assert_eq!(output[1]["system"], "definitions.dtd");
            assert_eq!(output[2]["base"], "file:///suite/dtd/definitions.dtd");
            assert_eq!(output[2]["system"], "../content.xml");
            assert_eq!(output[3]["status"], 1);
            assert_eq!(output[3]["children"].as_array().unwrap().len(), 3);
            assert_eq!(output[3]["resolver_errors"], json!([]));
        }
    }

    #[test]
    fn child_xml_errors_and_resolver_failures_remain_distinct() {
        let root = "<!DOCTYPE r [<!ENTITY e SYSTEM 'child.xml'>]><r>&e;</r>";
        let output = transcript(&[
            request(root, 7),
            json!({"base": "file:///suite/child.xml", "data": b"<a>".as_slice()}),
        ])
        .unwrap();
        assert_eq!(output[1]["status"], 0);
        assert_eq!(output[1]["error"], "ExternalEntityHandling");
        assert_eq!(output[1]["children"][0]["status"], 0);
        assert_eq!(output[1]["resolver_errors"], json!([]));

        let output = transcript(&[request(root, 7), json!({"error": "missing file"})]).unwrap();
        assert_eq!(output[1]["status"], 0);
        assert_eq!(output[1]["children"], json!([]));
        assert_eq!(output[1]["resolver_errors"], json!(["missing file"]));
    }

    #[test]
    fn external_value_children_resume_the_pending_declaration() {
        for chunk in [1, 7, 4096] {
            let output = transcript(&[
                request("<!DOCTYPE r SYSTEM 'main.dtd'><r>&e;</r>", chunk),
                json!({"base": "file:///suite/main.dtd", "data":
                    b"<!ENTITY % p SYSTEM 'value.ent'><!ENTITY e 'L%p;R'>".as_slice()}),
                json!({"base": "file:///suite/value.ent", "data": b"M".as_slice()}),
            ])
            .unwrap();
            assert_eq!(output[1]["base"], "file:///suite/main.dtd");
            assert_eq!(output[1]["system"], "value.ent");
            assert_eq!(output[2]["status"], 1);
            assert_eq!(output[2]["children"][0]["status"], 1);
        }
    }

    #[test]
    fn invalid_protocol_fails_the_worker() {
        assert!(transcript(&[request("<r/>", 0)]).is_err());
        assert!(transcript(&[json!({"op": "unknown"})]).is_err());
        assert!(transcript(&[request("<!DOCTYPE r SYSTEM 'dtd'><r/>", 1)]).is_err());
    }
}
