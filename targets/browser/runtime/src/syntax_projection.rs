//! Finite browser projection of canonical Conduit syntax-highlight spans.

use conduit_form::{highlight_syntax, SyntaxHighlightKind, SyntaxHighlightRefusal};
use serde::Serialize;
use std::cell::RefCell;

const INPUT_BYTES: usize = 8 * 1_024;
const OUTPUT_BYTES: usize = 128 * 1_024;
const STATUS_READY: i32 = 0;
const ERROR_INPUT: i32 = -401;
const ERROR_OUTPUT: i32 = -404;
const ERROR_HIGHLIGHT: i32 = -409;
const SYNTAX_PROJECTION_VERSION: &str = "conduit.syntax-highlight-projection@1";
const SYNTAX_KINDS: [&str; 10] = [
    "whitespace",
    "comment",
    "keyword",
    "name",
    "identity",
    "string",
    "number",
    "literal",
    "operator",
    "delimiter",
];

#[derive(Debug, PartialEq, Eq, Serialize)]
struct SyntaxProjection {
    pub protocol: &'static str,
    pub source_bytes: u32,
    pub kinds: &'static [&'static str; 10],
    /// Compact tuples keep every admitted Tour projection inside the shared
    /// 128 KiB application-presentation aggregate bound.
    pub spans: Vec<(u32, u32, u8)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum SyntaxKind {
    Whitespace,
    Comment,
    Keyword,
    Name,
    Identity,
    String,
    Number,
    Literal,
    Operator,
    Delimiter,
}

fn project(source: &str) -> Result<SyntaxProjection, String> {
    let source_bytes = u32::try_from(source.len())
        .map_err(|_| "Tour syntax source length exceeds u32".to_owned())?;
    let spans = highlight_syntax(source)
        .map_err(refusal)?
        .into_iter()
        .map(|span| {
            let start = u32::try_from(span.start)
                .map_err(|_| "Tour syntax span start exceeds u32".to_owned())?;
            let end = u32::try_from(span.end)
                .map_err(|_| "Tour syntax span end exceeds u32".to_owned())?;
            Ok((start, end, SyntaxKind::from(span.kind) as u8))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(SyntaxProjection {
        protocol: SYNTAX_PROJECTION_VERSION,
        source_bytes,
        kinds: &SYNTAX_KINDS,
        spans,
    })
}

thread_local! {
    static INPUT: RefCell<[u8; INPUT_BYTES]> = const { RefCell::new([0; INPUT_BYTES]) };
    static OUTPUT: RefCell<[u8; OUTPUT_BYTES]> = const { RefCell::new([0; OUTPUT_BYTES]) };
    static OUTPUT_LEN: RefCell<usize> = const { RefCell::new(0) };
}

#[no_mangle]
pub extern "C" fn conduit_syntax_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_syntax_input_capacity() -> usize {
    INPUT_BYTES
}

#[no_mangle]
pub extern "C" fn conduit_syntax_output_ptr() -> usize {
    OUTPUT.with(|output| output.borrow().as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_syntax_output_len() -> usize {
    OUTPUT_LEN.with(|length| *length.borrow())
}

/// Projects finite exact UTF-8 byte spans from the canonical Form highlighter.
/// Parsing and checking remain separate, so incomplete edits still project.
#[no_mangle]
pub extern "C" fn conduit_syntax_project(source_length: usize) -> i32 {
    OUTPUT_LEN.with(|length| *length.borrow_mut() = 0);
    if source_length == 0 || source_length > INPUT_BYTES {
        return ERROR_INPUT;
    }
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let result = core::str::from_utf8(&input[..source_length])
            .map_err(|_| "syntax source is not UTF-8".to_owned())
            .and_then(project);
        input[..source_length].fill(0);
        match result {
            Ok(projection) => write_output(&projection).unwrap_or(ERROR_OUTPUT),
            Err(message) => {
                let _ = write_output(&serde_json::json!({ "message": message }));
                ERROR_HIGHLIGHT
            }
        }
    })
}

fn write_output(value: &impl Serialize) -> Result<i32, i32> {
    let encoded = serde_json::to_vec(value).map_err(|_| ERROR_OUTPUT)?;
    if encoded.len() > OUTPUT_BYTES {
        return Err(ERROR_OUTPUT);
    }
    OUTPUT.with(|output| output.borrow_mut()[..encoded.len()].copy_from_slice(&encoded));
    OUTPUT_LEN.with(|length| *length.borrow_mut() = encoded.len());
    Ok(STATUS_READY)
}

fn refusal(reason: SyntaxHighlightRefusal) -> String {
    match reason {
        SyntaxHighlightRefusal::SourceTooLarge => "Tour syntax source exceeds its byte bound",
        SyntaxHighlightRefusal::TooManyTokens => "Tour syntax source exceeds its token bound",
        SyntaxHighlightRefusal::TooManySpans => "Tour syntax source exceeds its span bound",
    }
    .to_owned()
}

impl From<SyntaxHighlightKind> for SyntaxKind {
    fn from(kind: SyntaxHighlightKind) -> Self {
        match kind {
            SyntaxHighlightKind::Whitespace => Self::Whitespace,
            SyntaxHighlightKind::Comment => Self::Comment,
            SyntaxHighlightKind::Keyword => Self::Keyword,
            SyntaxHighlightKind::Name => Self::Name,
            SyntaxHighlightKind::Identity => Self::Identity,
            SyntaxHighlightKind::String => Self::String,
            SyntaxHighlightKind::Number => Self::Number,
            SyntaxHighlightKind::Literal => Self::Literal,
            SyntaxHighlightKind::Operator => Self::Operator,
            SyntaxHighlightKind::Delimiter => Self::Delimiter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_source_projects_exact_utf8_byte_spans() {
        let source = "form café { value=text/constant(value=\"still typing\n";
        let projection = project(source).unwrap();
        assert_eq!(projection.source_bytes, source.len() as u32);
        assert_eq!(projection.spans.last().unwrap().2, SyntaxKind::String as u8);
        for (start, end, _) in projection.spans {
            assert!(source.is_char_boundary(start as usize));
            assert!(source.is_char_boundary(end as usize));
        }
    }

    #[test]
    fn inline_comment_projects_through_the_existing_browser_comment_kind() {
        let source = "form note { value = \"channel #7\" # visible note\n}\n";
        let projection = project(source).unwrap();
        assert!(projection
            .spans
            .iter()
            .any(|(start, end, kind)| *kind == SyntaxKind::Comment as u8
                && &source[*start as usize..*end as usize] == "# visible note"));
        assert!(projection
            .spans
            .iter()
            .any(|(start, end, kind)| *kind == SyntaxKind::String as u8
                && &source[*start as usize..*end as usize] == "\"channel #7\""));
    }

    #[test]
    fn worst_case_tour_input_fits_the_bounded_output_arena() {
        let source = "x ".repeat(INPUT_BYTES / 2);
        let encoded = serde_json::to_vec(&project(&source).unwrap()).unwrap();
        assert!(
            encoded.len() <= OUTPUT_BYTES,
            "{} > {OUTPUT_BYTES}",
            encoded.len()
        );
    }

    #[test]
    fn syntax_projection_accepts_incomplete_source_and_clears_input() {
        let source = b"form unfinished { value=\"still typing";
        INPUT.with(|input| input.borrow_mut()[..source.len()].copy_from_slice(source));
        assert_eq!(conduit_syntax_project(source.len()), STATUS_READY);
        let projection: serde_json::Value = OUTPUT.with(|output| {
            serde_json::from_slice(&output.borrow()[..conduit_syntax_output_len()]).unwrap()
        });
        assert_eq!(projection["protocol"], SYNTAX_PROJECTION_VERSION);
        assert_eq!(projection["source_bytes"], source.len());
        INPUT.with(|input| assert!(input.borrow()[..source.len()].iter().all(|byte| *byte == 0)));
    }
}
