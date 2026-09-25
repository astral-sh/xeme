#![no_main]

//! UTF-8, DTD-free acceptance and normalized element/attribute/text callbacks.
//! Expat runs in a persistent separate process, never in Xeme's symbol namespace.

use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::ptr;

use libfuzzer_sys::fuzz_target;
use xeme_expat::*;

#[derive(Default)]
struct Events {
    bytes: Vec<u8>,
    last_text: Option<usize>,
}

impl Events {
    fn number(&mut self, value: usize) {
        self.bytes
            .extend_from_slice(&u32::try_from(value).unwrap().to_be_bytes());
    }

    unsafe fn string(&mut self, value: *const c_char) {
        // SAFETY: The parser keeps callback strings NUL-terminated and live.
        let bytes = unsafe { CStr::from_ptr(value) }.to_bytes();
        self.number(bytes.len());
        self.bytes.extend_from_slice(bytes);
    }
}

unsafe extern "C" fn start(data: *mut c_void, name: *const c_char, attrs: *const *const c_char) {
    // SAFETY: Synchronous callbacks own this State exclusively; no API reentry.
    unsafe {
        let events = &mut *data.cast::<Events>();
        events.last_text = None;
        events.bytes.push(b'S');
        events.string(name);
        let mut count = 0;
        while !(*attrs.add(count * 2)).is_null() {
            count += 1;
        }
        events.number(count);
        for index in 0..count * 2 {
            events.string(*attrs.add(index));
        }
    }
}

unsafe extern "C" fn end(data: *mut c_void, name: *const c_char) {
    // SAFETY: Synchronous callbacks own this State and the provided name is live.
    unsafe {
        let events = &mut *data.cast::<Events>();
        events.last_text = None;
        events.bytes.push(b'E');
        events.string(name);
    }
}

unsafe extern "C" fn text(data: *mut c_void, bytes: *const c_char, count: c_int) {
    // SAFETY: The callback owns State and receives count initialized text bytes.
    unsafe {
        let events = &mut *data.cast::<Events>();
        let position = if let Some(position) = events.last_text {
            position
        } else {
            events.bytes.push(b'T');
            let position = events.bytes.len();
            events.number(0);
            events.last_text = Some(position);
            position
        };
        let previous = u32::from_be_bytes(events.bytes[position..position + 4].try_into().unwrap());
        events.bytes[position..position + 4]
            .copy_from_slice(&(previous + count as u32).to_be_bytes());
        events
            .bytes
            .extend_from_slice(std::slice::from_raw_parts(bytes.cast(), count as usize));
    }
}

struct Oracle {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
}

impl Oracle {
    fn new() -> Self {
        let executable = std::env::var_os("XEME_EXPAT_ORACLE")
            .expect("set XEME_EXPAT_ORACLE to the separately linked Expat 2.8.5 oracle");
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start Expat oracle");
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let mut oracle = Self {
            child,
            input,
            output,
        };
        let mut greeting = [0; 8];
        if oracle.output.read_exact(&mut greeting).is_err() || &greeting != b"EXPAT285" {
            oracle.stop();
            panic!("oracle must link Expat 2.8.5 and provide its protocol greeting");
        }
        oracle
    }

    fn parse(&mut self, bytes: &[u8], namespaces: bool, width: usize) -> (bool, Vec<u8>) {
        let result = (|| -> std::io::Result<(bool, Vec<u8>)> {
            self.input.write_all(&[u8::from(namespaces)])?;
            self.input.write_all(&(width as u16).to_be_bytes())?;
            self.input.write_all(&(bytes.len() as u32).to_be_bytes())?;
            self.input.write_all(bytes)?;
            self.input.flush()?;
            let mut header = [0; 5];
            self.output.read_exact(&mut header)?;
            let length = u32::from_be_bytes(header[1..].try_into().unwrap()) as usize;
            if header[0] > 1 || length > 32 * 1024 * 1024 {
                return Err(std::io::Error::other("invalid Expat oracle reply"));
            }
            let mut events = vec![0; length];
            self.output.read_exact(&mut events)?;
            Ok((header[0] == 1, events))
        })();
        result.unwrap_or_else(|error| {
            self.stop();
            panic!("Expat oracle failed: {error}");
        })
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Oracle {
    fn drop(&mut self) {
        self.stop();
    }
}

thread_local! { static ORACLE: RefCell<Option<Oracle>> = const { RefCell::new(None) }; }

fuzz_target!(|data: &[u8]| {
    let Some((&control, bytes)) = data.split_first() else {
        return;
    };
    // Encoding declarations and DTD grammar have separately documented differences.
    // Limit errors are outside this semantic oracle; dedicated targets cover them.
    if !xeme_fuzz::expat_differential_input_supported(bytes) {
        return;
    }
    let namespaces = control & 128 != 0;
    let width = usize::from(control & 127) + 1;
    let mut events = Events::default();
    // SAFETY: Every handle, input slice and callback state lives until parser free.
    let (accepted, error) = unsafe {
        let parser = if namespaces {
            XML_ParserCreateNS(ptr::null(), b'|' as c_char)
        } else {
            XML_ParserCreate(ptr::null())
        };
        assert!(!parser.is_null());
        XML_SetUserData(parser, ptr::from_mut(&mut events).cast());
        XML_SetElementHandler(parser, Some(start), Some(end));
        XML_SetCharacterDataHandler(parser, Some(text));
        let mut status = 1;
        for chunk in bytes.chunks(width) {
            status = XML_Parse(parser, chunk.as_ptr().cast(), chunk.len() as c_int, 0);
            if status != 1 {
                break;
            }
        }
        if status == 1 {
            status = XML_Parse(parser, ptr::null(), 0, 1);
        }
        let error = XML_GetErrorCode(parser);
        XML_ParserFree(parser);
        (status == 1, error)
    };
    if error == 43 {
        return;
    }
    ORACLE.with(|oracle| {
        let mut oracle = oracle.borrow_mut();
        let (reference_accepted, reference_events) = oracle
            .get_or_insert_with(Oracle::new)
            .parse(bytes, namespaces, width);
        if accepted != reference_accepted || (accepted && events.bytes != reference_events) {
            *oracle = None;
        }
        assert_eq!(accepted, reference_accepted, "Expat acceptance differs");
        if accepted {
            assert_eq!(
                events.bytes, reference_events,
                "Expat normalized callbacks differ"
            );
        }
    });
});
