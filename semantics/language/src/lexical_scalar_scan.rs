//! Borrowed scalar-token reconstruction. This stage supplies no lexical facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScalarToken<'a> {
    pub surface: &'a str,
    pub start: u32,
    pub end: u32,
    pub word: bool,
    pub partial: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScanRefusal {
    Text,
    Tokens,
    Surface,
}
pub(crate) struct ScalarTokens<'a> {
    pub tokens: [Option<ScalarToken<'a>>; 128],
    pub count: usize,
}
pub(crate) fn scan(
    text: &str,
    partial: bool,
    maximum_tokens: usize,
) -> Result<ScalarTokens<'_>, ScanRefusal> {
    if text.len() > 4096 || maximum_tokens > 128 {
        return Err(ScanRefusal::Text);
    }
    let mut out = ScalarTokens {
        tokens: [None; 128],
        count: 0,
    };
    let mut chars = text.char_indices().peekable();
    let mut scalar = 0u32;
    while let Some((start_byte, first)) = chars.next() {
        let start = scalar;
        scalar += 1;
        if first.is_whitespace() {
            continue;
        }
        if out.count == maximum_tokens {
            return Err(ScanRefusal::Tokens);
        }
        let word = first.is_alphanumeric();
        if word {
            loop {
                let mut look = chars.clone();
                let Some((_, next)) = look.next() else { break };
                if next.is_alphanumeric()
                    || (next == '\''
                        && look
                            .next()
                            .is_some_and(|(_, after)| after.is_alphanumeric()))
                {
                    chars.next();
                    scalar += 1;
                } else {
                    break;
                }
            }
        }
        let end_byte = chars.peek().map_or(text.len(), |(byte, _)| *byte);
        let surface = &text[start_byte..end_byte];
        if surface.len() > 256 {
            return Err(ScanRefusal::Surface);
        }
        out.tokens[out.count] = Some(ScalarToken {
            surface,
            start,
            end: scalar,
            word,
            partial: partial && word && end_byte == text.len(),
        });
        out.count += 1;
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};
    #[test]
    fn original_unicode_scalar_offsets_apostrophes_and_partial_frontier() {
        let r = scan("  naïve can't, 中文!", true, 128).unwrap();
        let t = r.tokens[..r.count]
            .iter()
            .map(|v| v.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            t.iter()
                .map(|v| (v.surface, v.start, v.end, v.word, v.partial))
                .collect::<Vec<_>>(),
            vec![
                ("naïve", 2, 7, true, false),
                ("can't", 8, 13, true, false),
                (",", 13, 14, false, false),
                ("中文", 15, 17, true, false),
                ("!", 17, 18, false, false)
            ]
        );
        assert!(scan("hello", true, 128).unwrap().tokens[0].unwrap().partial);
        assert!(
            !scan("hello ", true, 128).unwrap().tokens[0]
                .unwrap()
                .partial
        );
        let r = scan("'a a'", false, 128).unwrap();
        assert_eq!(r.count, 4);
        assert_eq!(r.tokens[0].unwrap().surface, "'");
        assert_eq!(r.tokens[2].unwrap().surface, "a");
    }
    #[test]
    fn bounded_text_surface_and_token_refusals() {
        assert_eq!(
            scan(&"a".repeat(4097), false, 128).err(),
            Some(ScanRefusal::Text)
        );
        assert_eq!(
            scan(&"a".repeat(257), false, 128).err(),
            Some(ScanRefusal::Surface)
        );
        assert_eq!(scan("a b", false, 1).err(), Some(ScanRefusal::Tokens));
        assert_eq!(scan("", false, 0).unwrap().count, 0);
    }
}
