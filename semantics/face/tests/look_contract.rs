//! Canonical Source contract admission; consumer migration is a separate check.
use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;

fn neutral(
    identity: &str,
    version: u16,
) -> Result<LookInfo, conduit_plot::rust_binding::NativeBindingRefusal> {
    LookInfo::new(
        LookGenerativeFacetOption::None,
        identity.into(),
        LookSpokenFacetOption::None,
        version,
        LookVisualFacetOption::None,
    )
}

#[test]
fn canonical_identity_version_and_retired_identities_are_exact() {
    let value = neutral("conduit.look/neutral@1", 1).unwrap();
    assert_eq!(
        LookInfo::decode(&value.clone().encode().unwrap()).unwrap(),
        value
    );
    for identity in [
        "",
        "conduit.face/phosphor@1",
        "conduit.presentation/phosphor@1",
    ] {
        assert!(neutral(identity, 1).is_err());
    }
    assert!(neutral(&"x".repeat(129), 1).is_err());
    assert!(neutral("conduit.look/neutral@1", 0).is_err());
    assert!(neutral("conduit.look/neutral@1", 2).is_err());
}

#[test]
fn spoken_and_generative_facets_are_distinct_closed_parameters() {
    let spoken =
        LookSpokenFacetOption::some(LookSpokenDelivery::Calm, LookVerbosity::Brief).unwrap();
    let generative =
        LookGenerativeFacetOption::some(LookGenerativeRegister::Technical, LookVerbosity::Expanded)
            .unwrap();
    let value = LookInfo::new(
        generative,
        "conduit.look/narration@1".into(),
        spoken,
        1,
        LookVisualFacetOption::None,
    )
    .unwrap();
    assert_eq!(
        LookInfo::decode(&value.clone().encode().unwrap()).unwrap(),
        value
    );
}

#[test]
fn unknown_facet_request_is_bounded_and_explicit() {
    let request = LookFacetRequest::unsupported("haptic".into()).unwrap();
    assert_eq!(
        LookFacetRequest::decode(&request.clone().encode().unwrap()).unwrap(),
        request
    );
    assert!(LookFacetRequest::unsupported(String::new()).is_err());
    assert!(LookFacetRequest::unsupported("x".repeat(65)).is_err());
}
