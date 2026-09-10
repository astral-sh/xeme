//! Conversion of DTD content models into a single C-owned allocation.

use std::ffi::c_char;
use std::mem::{align_of, size_of};
use std::ptr;

use super::{XML_Content, malloc};

struct Model<'a> {
    kind: i32,
    quant: i32,
    name: Option<&'a str>,
    children: Vec<Model<'a>>,
}

struct Reader<'a> {
    text: &'a str,
    nodes: usize,
}

impl<'a> Reader<'a> {
    fn trim(&mut self) {
        self.text = self.text.trim_start_matches([' ', '\r', '\n', '\t']);
    }

    fn take(&mut self, expected: &str) -> bool {
        self.trim();
        if let Some(rest) = self.text.strip_prefix(expected) {
            self.text = rest;
            true
        } else {
            false
        }
    }

    fn quant(&mut self) -> i32 {
        // Quantifiers must immediately follow the name/closing parenthesis.
        let quant = match self.text.as_bytes().first() {
            Some(b'?') => 1,
            Some(b'*') => 2,
            Some(b'+') => 3,
            _ => return 0,
        };
        self.text = &self.text[1..];
        quant
    }

    fn node(&mut self, depth: usize) -> Result<Model<'a>, i32> {
        if depth > 128 || self.nodes >= 10_000 {
            return Err(43);
        }
        self.nodes += 1;
        self.trim();
        if self.take("(") {
            if self.take("#PCDATA") {
                let mut children = Vec::new();
                while self.take("|") {
                    let child = self.node(depth + 1)?;
                    if child.kind != 4 || child.quant != 0 {
                        return Err(2);
                    }
                    children.push(child);
                }
                if !self.take(")") {
                    return Err(2);
                }
                let quant = self.quant();
                if (children.is_empty() && quant != 0 && quant != 2)
                    || (!children.is_empty() && quant != 2)
                {
                    return Err(2);
                }
                return Ok(Model {
                    kind: 3,
                    quant,
                    name: None,
                    children,
                });
            }
            let mut children = vec![self.node(depth + 1)?];
            self.trim();
            let separator = match self.text.as_bytes().first() {
                Some(b',') => ",",
                Some(b'|') => "|",
                Some(b')') => ",", // A one-child sequence is legal.
                _ => return Err(2),
            };
            while self.take(separator) {
                children.push(self.node(depth + 1)?);
            }
            if !self.take(")") {
                return Err(2);
            }
            Ok(Model {
                kind: if separator == "," { 6 } else { 5 },
                quant: self.quant(),
                name: None,
                children,
            })
        } else {
            let end = self
                .text
                .find([' ', '\r', '\n', '\t', ',', '|', ')', '(', '?', '*', '+'])
                .unwrap_or(self.text.len());
            if end == 0 {
                return Err(2);
            }
            let name = &self.text[..end];
            self.text = &self.text[end..];
            Ok(Model {
                kind: 4,
                quant: self.quant(),
                name: Some(name),
                children: Vec::new(),
            })
        }
    }
}

fn count(model: &Model<'_>) -> (usize, usize) {
    model.children.iter().fold(
        (1, model.name.map_or(0, |name| name.len() + 1)),
        |(nodes, bytes), child| {
            let (child_nodes, child_bytes) = count(child);
            (nodes + child_nodes, bytes + child_bytes)
        },
    )
}

/// Allocate the full node tree and strings in one block, matching Expat's free API.
pub(super) fn allocate(text: &str) -> Result<*mut XML_Content, i32> {
    let text = text.trim();
    let model = match text {
        "EMPTY" => Model {
            kind: 1,
            quant: 0,
            name: None,
            children: Vec::new(),
        },
        "ANY" => Model {
            kind: 2,
            quant: 0,
            name: None,
            children: Vec::new(),
        },
        _ => {
            let mut reader = Reader { text, nodes: 0 };
            let model = reader.node(0)?;
            reader.trim();
            if !reader.text.is_empty() || ![3, 5, 6].contains(&model.kind) {
                return Err(2);
            }
            model
        }
    };
    let (nodes, string_bytes) = count(&model);
    let node_bytes = nodes.checked_mul(size_of::<XML_Content>()).ok_or(1)?;
    let size = node_bytes.checked_add(string_bytes).ok_or(1)?;
    assert!(align_of::<XML_Content>() <= align_of::<u128>());
    // SAFETY: malloc is aligned for XML_Content and the checked size covers every
    // node and name. The returned block transfers ownership to the C callback.
    unsafe {
        let block = malloc(size).cast::<XML_Content>();
        if block.is_null() {
            return Err(1);
        }
        let mut next_node = 1;
        let mut next_string = block.cast::<u8>().add(node_bytes).cast::<c_char>();
        write_node(&model, block, block, &mut next_node, &mut next_string);
        Ok(block)
    }
}

unsafe fn write_node(
    model: &Model<'_>,
    output: *mut XML_Content,
    block: *mut XML_Content,
    next_node: &mut usize,
    next_string: &mut *mut c_char,
) {
    // SAFETY: allocate reserved exactly the recursively counted number of nodes
    // and string bytes. Each child receives a distinct slot; strings do not overlap.
    unsafe {
        let name = if let Some(name) = model.name {
            let pointer = *next_string;
            ptr::copy_nonoverlapping(name.as_ptr().cast::<c_char>(), pointer, name.len());
            pointer.add(name.len()).write(0);
            *next_string = pointer.add(name.len() + 1);
            pointer
        } else {
            ptr::null_mut()
        };
        let children = if model.children.is_empty() {
            ptr::null_mut()
        } else {
            block.add(*next_node)
        };
        *next_node += model.children.len();
        ptr::write(
            output,
            XML_Content {
                kind: model.kind,
                quant: model.quant,
                name,
                numchildren: model.children.len() as u32,
                children,
            },
        );
        for (index, child) in model.children.iter().enumerate() {
            write_node(child, children.add(index), block, next_node, next_string);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn children_are_contiguous_and_owned_by_one_allocation() {
        let model = allocate("(head,(body|section)+,foot?)").unwrap();
        // SAFETY: The allocated model is live, and indices follow checked counts.
        unsafe {
            assert_eq!((*model).kind, 6);
            assert_eq!((*model).numchildren, 3);
            let children = std::slice::from_raw_parts((*model).children, 3);
            assert_eq!(CStr::from_ptr(children[0].name), c"head");
            assert_eq!(children[1].kind, 5);
            assert_eq!(children[1].quant, 3);
            assert_eq!(children[1].numchildren, 2);
            assert_eq!(children[2].quant, 1);
            super::super::XML_FreeContentModel(ptr::null_mut(), model);
        }
    }

    #[test]
    fn malformed_and_deep_models_are_bounded() {
        assert!(allocate("(a,b|c)").is_err());
        assert!(allocate("(#PCDATA|x)").is_err());
        assert!(allocate(&format!("{}a{}", "(".repeat(1000), ")".repeat(1000))).is_err());
    }
}
