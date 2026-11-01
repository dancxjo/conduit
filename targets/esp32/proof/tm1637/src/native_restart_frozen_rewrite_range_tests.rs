//! Recipe describes consumed immutable bytes, not an empty source cursor.
use super::*;
fn captured(reuse: Option<(usize, &[u8])>) -> Recipe {
    Recipe::capture(
        Limits {
            prefix: 1,
            append: 0,
            suffix: 1,
        },
        |sink| {
            sink(EncodedPiece::Changed {
                part: ChangedPart::Prefix,
                bytes: b"p",
            })
            .map_err(StreamRefusal::Sink)?;
            if let Some((source_offset, bytes)) = reuse {
                sink(EncodedPiece::Reused {
                    source_offset,
                    bytes,
                })
                .map_err(StreamRefusal::Sink)?;
            }
            sink(EncodedPiece::Changed {
                part: ChangedPart::Suffix,
                bytes: b"s",
            })
            .map_err(StreamRefusal::Sink)
        },
    )
    .unwrap()
    .0
}
#[test]
fn frozen_source_range_empty_cursor_matches_only_absent_consumed_bytes() {
    let recipe = captured(None);
    assert_eq!(
        recipe.source_range(),
        expected_source_range(archive::Span {
            start: 2164,
            end: 2164
        })
    );
    assert_ne!(
        recipe.source_range(),
        expected_source_range(archive::Span {
            start: 2164,
            end: 2165
        })
    );
    // A real nonempty consumed range cannot be hidden by an empty measurement.
    let nonempty = captured(Some((2164, b"x")));
    assert_ne!(
        nonempty.source_range(),
        expected_source_range(archive::Span {
            start: 2164,
            end: 2164
        })
    );
}
#[test]
fn frozen_source_range_nonempty_requires_exact_authenticated_span() {
    let recipe = captured(Some((2164, b"xy")));
    assert_eq!(
        recipe.source_range(),
        expected_source_range(archive::Span {
            start: 2164,
            end: 2166
        })
    );
    for span in [
        archive::Span {
            start: 2163,
            end: 2166,
        },
        archive::Span {
            start: 2164,
            end: 2167,
        },
        archive::Span { start: 0, end: 0 },
    ] {
        assert_ne!(recipe.source_range(), expected_source_range(span));
    }
}
