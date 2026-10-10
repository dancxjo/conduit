//! Shared raw slash-delimiter scanning for portable pattern consumers.
//! Scanning quotes closers but never decodes or admits a domain escape.

pub(crate) fn slash_pattern(source: &str) -> Option<(&str, bool, bool, bool, usize)> {
    if !source.starts_with('/') {
        return None;
    }
    let mut escaped = false;
    let mut class = false;
    for (offset, character) in source.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '[' => class = true,
            ']' => class = false,
            '/' if !class => {
                let flags_end = source[offset + 1..]
                    .find(|character: char| !character.is_ascii_alphabetic())
                    .map_or(source.len(), |relative| offset + 1 + relative);
                let flags = &source[offset + 1..flags_end];
                let case_insensitive = match flags {
                    "" => false,
                    "i" => true,
                    _ => return None,
                };
                let mut pattern = &source[1..offset];
                let anchored_start = pattern.starts_with('^');
                if anchored_start {
                    pattern = &pattern[1..];
                }
                let anchored_end = pattern.ends_with('$') && trailing_dollar_is_anchor(pattern);
                if anchored_end {
                    pattern = &pattern[..pattern.len() - 1];
                }
                if pattern.is_empty() {
                    return None;
                }
                return Some((
                    pattern,
                    case_insensitive,
                    anchored_start,
                    anchored_end,
                    flags_end,
                ));
            }
            _ => {}
        }
    }
    None
}

fn trailing_dollar_is_anchor(pattern: &str) -> bool {
    let preceding_backslashes = pattern[..pattern.len().saturating_sub(1)]
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count();
    preceding_backslashes % 2 == 0
}

#[cfg(test)]
mod tests {
    use super::slash_pattern;

    #[test]
    fn raw_payload_and_unicode_byte_offsets_survive_scanning() {
        let source = "/^t͡ʃ[ə/]+$/i finite";
        let (payload, insensitive, start, end, consumed) = slash_pattern(source).unwrap();
        assert_eq!(payload, "t͡ʃ[ə/]+");
        assert!(insensitive && start && end);
        assert_eq!(&source[consumed..], " finite");
        let payload_offset = payload.as_ptr() as usize - source.as_ptr() as usize;
        assert_eq!(payload_offset, 2);
        assert_eq!(
            &source[payload_offset..payload_offset + payload.len()],
            payload
        );
    }

    #[test]
    fn quoting_the_closer_does_not_admit_a_portable_escape() {
        let (payload, _, _, _, _) = slash_pattern(r"/a\/b/").unwrap();
        assert_eq!(payload, r"a\/b");
        assert!(crate::parse_text_pattern(payload).is_err());
        let (payload, _, _, _, _) = slash_pattern("/a[/]b/").unwrap();
        assert_eq!(payload, "a[/]b");
        assert!(crate::parse_text_pattern(payload).is_ok());
    }

    #[test]
    fn backslash_parity_and_quoted_anchors_preserve_existing_law() {
        let source = r"/path\\/ tail";
        let (payload, _, _, end, consumed) = slash_pattern(source).unwrap();
        assert_eq!(payload, r"path\\");
        assert!(!end);
        assert_eq!(&source[consumed..], " tail");
        let (payload, _, _, end, _) = slash_pattern(r"/path\$/").unwrap();
        assert_eq!(payload, r"path\$");
        assert!(!end);
        let (payload, _, _, end, _) = slash_pattern(r"/path\\$/").unwrap();
        assert_eq!(payload, r"path\\");
        assert!(end);
    }

    #[test]
    fn malformed_delimiters_empty_payload_and_flags_refuse() {
        for source in [
            "", "x/foo/", "/", "//", "/^$/", "/foo", r"/foo\/", "/[/", "/foo/ii", "/foo/m",
            "/foo/iX",
        ] {
            assert!(slash_pattern(source).is_none(), "{source:?}");
        }
    }
}
