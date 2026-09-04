use super::*;

#[test]
fn canonical_round_trip_and_exact_icon_policy() {
    let value = PresentationComposition::icon("presentation", "Patchbay")
        .unwrap()
        .frame("panel", "Gear Face")
        .unwrap()
        .badge("warning", "Cord pressure")
        .unwrap();
    let bytes = value.encode();
    assert_eq!(
        PresentationComposition::decode(&bytes[..value.encoded_len()]),
        Ok(value)
    );
    assert_eq!(
        PresentationComposition::icon("name-guessed-from-kind", "bad"),
        Err(CompositionError::UnknownIcon)
    );
    assert_eq!(
        PresentationComposition::icon_or_fallback(None, None)
            .unwrap()
            .items()[0]
            .token(),
        "conduit-generic-gear"
    );
    assert_eq!(
        PresentationComposition::decode(&bytes[..value.encoded_len() - 1]),
        Err(CompositionError::MalformedEncoding)
    );
}

#[test]
fn finite_capacity_and_noncanonical_encodings_refuse() {
    let item = CompositionItem::new(
        CompositionItemKind::Badge,
        AccessibilityRole::Status,
        "ready",
        "ready",
    )
    .unwrap();
    let mut value = PresentationComposition::empty();
    for _ in 0..MAX_COMPOSITION_ITEMS {
        value.push(item).unwrap();
    }
    assert_eq!(value.push(item), Err(CompositionError::TooManyItems));
    assert_eq!(
        CompositionItem::new(
            CompositionItemKind::Frame,
            AccessibilityRole::Group,
            "panel",
            &"x".repeat(MAX_COMPOSITION_NAME_BYTES + 1),
        ),
        Err(CompositionError::TextTooLong)
    );
    let bytes = value.encode();
    let mut noncanonical = bytes[..value.encoded_len()].to_vec();
    noncanonical.push(0);
    assert_eq!(
        PresentationComposition::decode(&noncanonical),
        Err(CompositionError::NonCanonicalEncoding)
    );
}
