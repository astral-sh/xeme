#![no_main]

//! Custom encoding callbacks, partial wire characters, allocator faults, and re-entry.
//! Sixteen control bytes precede arbitrary XML in a two-, three-, and four-byte encoding.

use std::ffi::{c_char, c_int, c_void};
use std::ptr;

use libfuzzer_sys::fuzz_target;
use xeme_expat::*;
use xeme_fuzz::allocator::{CALLS, FAIL_AT, LIVE, SUITE};

struct State {
    parser: XML_Parser,
    handlers: usize,
    releases: usize,
    active: bool,
    conversions: usize,
    action: u8,
    mode: u8,
    value: i32,
    map_mode: u8,
}

impl State {
    fn new(data: &[u8], action: u8) -> Self {
        Self {
            parser: ptr::null_mut(),
            handlers: 0,
            releases: 0,
            active: false,
            conversions: 0,
            action,
            mode: data[4] & 7,
            value: i32::from(u16::from_le_bytes([data[6], data[7]])),
            map_mode: data[10] & 15,
        }
    }
}

unsafe extern "C" fn convert(data: *mut c_void, bytes: *const c_char) -> c_int {
    let state = data.cast::<State>();
    // Copy fields before entering the API. No Rust reference to callback state
    // crosses a call that could itself invoke another callback.
    unsafe {
        assert!((*state).active);
        (*state).conversions += 1;
        let parser = (*state).parser;
        let action = if (*state).conversions == 1 {
            (*state).action & 7
        } else {
            0
        };
        let mode = (*state).mode;
        let value = (*state).value;
        let lead = *bytes.cast::<u8>();
        let width = match lead {
            0x80 => 2,
            0x81 => 3,
            0x82 => 4,
            _ => panic!("converter called for an unmapped lead byte"),
        };
        // Read exactly the number of bytes promised by the lead-byte map. ASan
        // checks that even one-byte input feeds provide a complete wire character.
        let wire = std::slice::from_raw_parts(bytes.cast::<u8>(), width);
        let scalar = match mode {
            0 => [0xe9, 0x4e2d, 0x6f22][usize::from(lead - 0x80)],
            1 => value,
            2 => -1,
            3 => 0xd800 + (value & 0x7ff),
            4 => 0x10000 + value,
            5 => value & 0x7f,
            6 => wire[1..]
                .iter()
                .fold(0_i32, |scalar, byte| (scalar << 8) | i32::from(*byte)),
            _ => 0xfffe + (value & 1),
        };
        if action == 1 || action == 4 {
            let error = XML_GetErrorCode(parser);
            assert_eq!(XML_Parse(parser, c"<recursive/>".as_ptr(), 12, 1), 0);
            assert_eq!(XML_ParseBuffer(parser, 0, 0), 0);
            assert!(XML_GetBuffer(parser, 1).is_null());
            assert_eq!(XML_ParserReset(parser, ptr::null()), 0);
            // Callback-time Free is ignored: the outer driver still owns this
            // handle and frees it after parsing (or resuming) has returned.
            XML_ParserFree(parser);
            assert_eq!(XML_GetErrorCode(parser), error);
        }
        match action {
            2 | 4 => assert_eq!(XML_StopParser(parser, 1), 1),
            3 => assert_eq!(XML_StopParser(parser, 0), 1),
            _ => {}
        }
        scalar
    }
}

unsafe extern "C" fn release(data: *mut c_void) {
    // Each handler invocation transfers exactly one instance. The stack-owned
    // state remains live until this parser and its independent child are freed.
    unsafe {
        let state = data.cast::<State>();
        assert!((*state).active, "encoding instance released more than once");
        (*state).active = false;
        (*state).releases += 1;
        assert!((*state).releases <= (*state).handlers);
    }
}

unsafe extern "C" fn encoding(
    data: *mut c_void,
    _: *const c_char,
    info: *mut XML_Encoding,
) -> c_int {
    unsafe {
        let state = data.cast::<State>();
        assert!(
            !(*state).active,
            "encoding instance replaced without release"
        );
        (*state).active = true;
        (*state).handlers += 1;
        (*info).map = std::array::from_fn(|byte| if byte < 128 { byte as i32 } else { -1 });
        (*info).map[0x80] = if (*state).map_mode == 1 { -5 } else { -2 };
        (*info).map[0x81] = -3;
        (*info).map[0x82] = -4;
        if (*state).map_mode == 3 {
            (*info).map[usize::from(b'<')] = 0xe9;
        }
        (*info).data = data;
        (*info).convert = if (*state).map_mode == 2 {
            None
        } else {
            Some(convert)
        };
        (*info).release = Some(release);
    }
    1
}

unsafe extern "C" fn text(_: *mut c_void, bytes: *const c_char, length: c_int) {
    assert!(length >= 0);
    if length != 0 {
        let bytes = unsafe { std::slice::from_raw_parts(bytes.cast::<u8>(), length as usize) };
        assert!(std::str::from_utf8(bytes).is_ok());
    }
}

unsafe fn configure(state: *mut State, options: u8) {
    unsafe {
        let parser = (*state).parser;
        XML_SetUnknownEncodingHandler(parser, Some(encoding), state.cast());
        XML_SetCharacterDataHandler(parser, Some(text));
        XML_SetReparseDeferralEnabled(parser, u8::from(options & 1 != 0));
    }
}

unsafe fn document(parser: XML_Parser, input: &[u8], buffered: bool, width: usize) {
    let mut position = 0;
    for _ in 0..4096 {
        let end = input.len().min(position + width);
        let count = end - position;
        let final_input = c_int::from(end == input.len());
        let mut status = unsafe {
            if buffered {
                let buffer = XML_GetBuffer(parser, count as c_int);
                if buffer.is_null() {
                    return;
                }
                ptr::copy_nonoverlapping(input.as_ptr().add(position), buffer.cast(), count);
                XML_ParseBuffer(parser, count as c_int, final_input)
            } else {
                XML_Parse(
                    parser,
                    input.as_ptr().add(position).cast(),
                    count as c_int,
                    final_input,
                )
            }
        };
        // Only the first conversion requests a stop. Bound resumption even if a
        // parser bug leaves the status suspended without making progress.
        for _ in 0..8 {
            if status != 2 {
                break;
            }
            status = unsafe { XML_ResumeParser(parser) };
        }
        if status != 1 || final_input != 0 {
            return;
        }
        position = end;
    }
}

fuzz_target!(|data: &[u8]| {
    if !(16..=65_536).contains(&data.len()) {
        return;
    }
    assert_eq!(LIVE.get(), 0);
    CALLS.set(0);
    FAIL_AT.set(if data[0] & 1 == 0 {
        0
    } else {
        usize::from(u16::from_le_bytes([data[1], data[2]])) % 512 + 1
    });
    let input = &data[16..];
    let width = usize::from(data[3]) + 1;
    let buffered = data[0] & 2 != 0;
    let mut parent = State::new(data, data[8]);
    let mut child = State::new(data, data[9]);
    unsafe {
        let protocol = if data[0] & 64 == 0 {
            c"multibyte".as_ptr()
        } else {
            ptr::null()
        };
        let separator = if data[0] & 32 != 0 {
            c"|".as_ptr()
        } else {
            ptr::null()
        };
        parent.parser = XML_ParserCreate_MM(protocol, &SUITE, separator);
        if !parent.parser.is_null() {
            configure(&mut parent, data[11]);
            document(parent.parser, input, buffered, width);
            if data[0] & 4 != 0 {
                child.parser =
                    XML_ExternalEntityParserCreate(parent.parser, c"".as_ptr(), protocol);
                if !child.parser.is_null() {
                    // Child conversion data belongs to the child. Replacing the
                    // inherited handler before parsing avoids borrowing its parent.
                    configure(&mut child, data[12]);
                }
            }
            if data[0] & 8 != 0 {
                XML_ParserFree(parent.parser);
                parent.parser = ptr::null_mut();
            } else if data[0] & 16 != 0 && XML_ParserReset(parent.parser, protocol) != 0 {
                configure(&mut parent, data[11]);
                document(parent.parser, input, !buffered, width);
            }
            if !child.parser.is_null() {
                document(child.parser, input, !buffered, usize::from(data[13]) + 1);
                XML_ParserFree(child.parser);
                child.parser = ptr::null_mut();
            }
            if !parent.parser.is_null() {
                XML_ParserFree(parent.parser);
                parent.parser = ptr::null_mut();
            }
        }
    }
    for state in [&parent, &child] {
        assert!(!state.active, "encoding instance was not released");
        assert_eq!(state.handlers, state.releases);
    }
    assert_eq!(LIVE.get(), 0, "custom encoding parser leaked allocations");
});
