//! Shared support for fuzz harnesses.

pub mod allocator;

/// Select the UTF-8, DTD-free subset compared with the Expat oracle.
pub fn expat_differential_input_supported(bytes: &[u8]) -> bool {
    if bytes.len() > 8192 || bytes.contains(&0) {
        return false;
    }
    let Ok(input) = std::str::from_utf8(bytes) else {
        return false;
    };
    if input.contains("<!DOCTYPE") {
        return false;
    }
    // Keep malformed and incomplete version declarations in the corpus. Encoding
    // declarations still have documented differences; conservatively exclude the
    // substring even in incomplete declarations and XML-like processing instructions.
    !input.split("<?xml").skip(1).any(|tail| {
        tail.split_once("?>")
            .map_or(tail, |(declaration, _)| declaration)
            .contains("encoding")
    })
}

/// Decode an optional failure at any of the first 512 allocator requests.
/// Bit zero enables failure; the next nine bits select an independent ordinal.
pub fn allocation_failure_ordinal(control: u16) -> usize {
    if control & 1 == 0 {
        0
    } else {
        usize::from((control >> 1) & 511) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::{allocation_failure_ordinal, expat_differential_input_supported};

    #[test]
    fn version_declarations_reach_the_oracle_even_when_malformed() {
        for input in [
            "<?xml version='1.0'?><r/>",
            "<?xml version='1.1'?><r/>",
            "<?xml version='1.01'?><r/>",
            "<?xml version='2.0'?><r/>",
            "<?xml version='1.'?><r/>",
            "<?xml version='1.٠'?><r/>",
            "<?xml version=''?><r/>",
            "<?xml version='1.",
            "<?xml version='1.0' standalone='yes'?><r/>",
            "<?xml version='1.0'?><r>encoding</r>",
            "<r><?xml version='1.0'?></r>",
        ] {
            assert!(
                expat_differential_input_supported(input.as_bytes()),
                "{input}"
            );
        }
    }

    #[test]
    fn encoding_and_dtd_exclusions_remain_in_effect() {
        for input in [
            b"<?xml version='1.0' encoding='UTF-8'?><r/>".as_slice(),
            b"<?xml version='1.0' encoding=",
            b"<?xml version='1.0'?><?xml encoding='latin1'?><r/>",
            b"<?xml-stylesheet encoding='UTF-8'?><r/>",
            b"<!DOCTYPE r [<!ENTITY e 'text'>]><r>&e;</r>",
            b"<r>\0</r>",
            b"<r>\xff</r>",
        ] {
            assert!(!expat_differential_input_supported(input), "{input:?}");
        }
    }

    #[test]
    fn every_failure_ordinal_and_the_disabled_mode_are_reachable() {
        for ordinal in 1..=512 {
            let selection = ((ordinal - 1) * 2) as u16;
            assert_eq!(allocation_failure_ordinal(selection), 0);
            assert_eq!(allocation_failure_ordinal(selection | 1), ordinal);
        }
    }
}
