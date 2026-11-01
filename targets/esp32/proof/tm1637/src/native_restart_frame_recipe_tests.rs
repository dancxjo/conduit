use super::*;
use crate::native_restart::{checkpoint_store, park_tests};
#[test]
fn frozen_recipe_matches_shared_encoder_and_refuses_short_budgets() {
    let (mut store, mut owner, _, key) = park_tests::fixture();
    let append = store.archives.clone();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    checkpoint_store(&mut store, owner.as_mut().unwrap(), &key).unwrap();
    let previous = store.custody.as_ref().unwrap();
    let source = store
        .journal
        .read_snapshot(&previous.source, |reader| {
            let mut result = Vec::new();
            let mut bytes = [0; 128];
            loop {
                let n = reader.read(&mut bytes)?;
                if n == 0 {
                    break;
                }
                result.extend_from_slice(&bytes[..n]);
            }
            Ok(result)
        })
        .unwrap();
    let reuse = previous
        .proof
        .as_ref()
        .unwrap()
        .checked_frame_range_for_test()
        .unwrap();
    for pending in [&[][..], append.as_slice()] {
        let encode = |sink: &mut dyn FnMut(EncodedPiece<'_>) -> Result<(), CaptureRefusal>| {
            let mut at = 0;
            super::super::emit_pieces(
                owner.as_ref().unwrap().evidence(),
                pending,
                store.admissions.as_ref().unwrap(),
                previous.span,
                previous.prefix.as_ref(),
                previous.local_count,
                &previous.layouts,
                previous.network.as_ref(),
                previous.summary.clone(),
                Some(reuse),
                |out| {
                    let n = (source.len() - at).min(128);
                    out[..n].copy_from_slice(&source[at..at + n]);
                    at += n;
                    Ok(n)
                },
                sink,
            )
        };
        let mut expected = Vec::new();
        let mut dimensions = [0; 3];
        let metadata = encode(&mut |piece| {
            if let EncodedPiece::Changed { part, bytes } = &piece {
                let i = match part {
                    ChangedPart::Prefix => 0,
                    ChangedPart::Append => 1,
                    ChangedPart::Suffix => 2,
                };
                dimensions[i] += bytes.len();
            }
            expected.extend_from_slice(piece.bytes());
            Ok(())
        })
        .unwrap();
        let limits = Limits {
            prefix: dimensions[0],
            append: dimensions[1],
            suffix: dimensions[2],
        };
        let (recipe, actual_metadata) = Recipe::capture(limits, encode).unwrap();
        let parts = recipe.changed_parts();
        let mut rebuilt = parts[0].to_vec();
        if let Some(span) = recipe.source_range() {
            rebuilt.extend_from_slice(source.get(span.start..span.end).unwrap());
        }
        rebuilt.extend_from_slice(parts[1]);
        rebuilt.extend_from_slice(parts[2]);
        assert_eq!(rebuilt, expected);
        assert_eq!(recipe.changed_lengths(), dimensions);
        assert_eq!(recipe.source_range(), Some(reuse.encoded));
        assert_eq!(actual_metadata.1, metadata.1);
        assert_eq!(actual_metadata.2, metadata.2);
        println!(
            "frozen_recipe_cpu64 prefix={} append={} suffix={} reused={} source={} output={}",
            dimensions[0],
            dimensions[1],
            dimensions[2],
            reuse.encoded.end - reuse.encoded.start,
            source.len(),
            expected.len()
        );
        assert!(Recipe::capture(
            Limits {
                prefix: dimensions[0] - 1,
                ..limits
            },
            encode
        )
        .is_err());
    }
}
#[test]
fn reservation_and_source_order_refuse_without_unbounded_growth() {
    let mut called = false;
    assert!(Recipe::capture(
        Limits {
            prefix: usize::MAX,
            append: 1,
            suffix: 0
        },
        |_| {
            called = true;
            Ok(())
        }
    )
    .is_err());
    assert!(!called);
    assert!(Recipe::capture(
        Limits {
            prefix: 0,
            append: 0,
            suffix: 0
        },
        |sink| {
            sink(EncodedPiece::Reused {
                source_offset: 4,
                bytes: &[1; 2],
            })
            .map_err(StreamRefusal::Sink)?;
            sink(EncodedPiece::Reused {
                source_offset: 7,
                bytes: &[1; 2],
            })
            .map_err(StreamRefusal::Sink)?;
            Ok(())
        }
    )
    .is_err());
}
