//! Representation adaptation for serde-json-core 0.6, which cannot decode
//! surrogate-pair escapes. This is NOT JSON syntax admission: the caller must
//! first validate the complete original document with Web's JSON owner.
//!
//! Only validated UTF-16 surrogate pairs become equivalent literal UTF-8.
//! Original evidence bytes are never changed or used from this working copy.
pub(crate) fn adapt_validated_surrogate_pairs(input: &[u8], output: &mut [u8; 4096]) -> usize {
    let (mut read, mut written, mut in_string) = (0, 0, false);
    while read < input.len() {
        let byte = input[read];
        if in_string && byte == b'\\' {
            if input[read + 1] == b'u' {
                let high = hex(&input[read + 2..read + 6]);
                if (0xd800..=0xdbff).contains(&high) {
                    // Web admission establishes the following low surrogate.
                    let low = hex(&input[read + 8..read + 12]);
                    let scalar = 0x10000 + (((high - 0xd800) as u32) << 10) + (low - 0xdc00) as u32;
                    let mut encoded = [0; 4];
                    let text = char::from_u32(scalar).unwrap().encode_utf8(&mut encoded);
                    output[written..written + text.len()].copy_from_slice(text.as_bytes());
                    written += text.len();
                    read += 12;
                    continue;
                }
            }
            // Treat an escape as one indivisible spelling. In particular,
            // a literal escaped backslash must not activate a following 'u'.
            output[written..written + 2].copy_from_slice(&input[read..read + 2]);
            written += 2;
            read += 2;
            continue;
        }
        if byte == b'"' {
            in_string = !in_string;
        }
        output[written] = byte;
        written += 1;
        read += 1;
    }
    written
}
fn hex(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0, |value, byte| {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => unreachable!("Web validated the escape"),
        };
        (value << 4) | u16::from(digit)
    })
}
