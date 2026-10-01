use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::{
    AdmittedNavigationDestination, ChoiceMultiplicity, EvidenceDisposition,
    FaceUtteranceClauseKind, GeneratedContentRole, GeneratedManifestationDisposition,
    GraphicsClipClass, LayoutAlignment, LayoutAxis, PresentationDisclosureLevel,
    PresentationMechanismKind, PresentationTemporalRole, StatusKind,
};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn presentation_vocabularies_round_trip_through_exact_native_types() {
    for value in [
        PresentationMechanismKind::Shell,
        PresentationMechanismKind::Workbench,
        PresentationMechanismKind::Panel,
        PresentationMechanismKind::Heading,
        PresentationMechanismKind::Separator,
        PresentationMechanismKind::Grid,
        PresentationMechanismKind::ActionGroup,
        PresentationMechanismKind::Action,
        PresentationMechanismKind::Status,
        PresentationMechanismKind::Disclosure,
        PresentationMechanismKind::Evidence,
        PresentationMechanismKind::DefinitionTable,
        PresentationMechanismKind::Definition,
        PresentationMechanismKind::CodeBlock,
        PresentationMechanismKind::FormField,
        PresentationMechanismKind::ChoiceGroup,
        PresentationMechanismKind::Navigation,
        PresentationMechanismKind::NavigationLink,
        PresentationMechanismKind::Link,
        PresentationMechanismKind::Stepper,
        PresentationMechanismKind::Progress,
        PresentationMechanismKind::Artifact,
        PresentationMechanismKind::Download,
        PresentationMechanismKind::DeviceChoice,
        PresentationMechanismKind::PatchbayCanvas,
    ] {
        assert_round_trip(value);
    }
    for value in [
        StatusKind::Ordinary,
        StatusKind::Warning,
        StatusKind::Failure,
        StatusKind::Success,
    ] {
        assert_round_trip(value);
    }
    for value in [
        EvidenceDisposition::Missing,
        EvidenceDisposition::Stale,
        EvidenceDisposition::Refused,
        EvidenceDisposition::Failed,
        EvidenceDisposition::Succeeded,
    ] {
        assert_round_trip(value);
    }
    for value in [
        ChoiceMultiplicity::Independent,
        ChoiceMultiplicity::Exclusive,
    ] {
        assert_round_trip(value);
    }
    for value in [
        AdmittedNavigationDestination::Home,
        AdmittedNavigationDestination::Tour,
        AdmittedNavigationDestination::Creche,
        AdmittedNavigationDestination::Patchbay,
        AdmittedNavigationDestination::Source,
    ] {
        assert_round_trip(value);
    }
    for value in [
        FaceUtteranceClauseKind::Subject,
        FaceUtteranceClauseKind::Relationship,
        FaceUtteranceClauseKind::Property,
        FaceUtteranceClauseKind::Composition,
        FaceUtteranceClauseKind::Text,
        FaceUtteranceClauseKind::Action,
        FaceUtteranceClauseKind::ActionArgument,
    ] {
        assert_round_trip(value);
    }
    for value in [
        PresentationDisclosureLevel::Primary,
        PresentationDisclosureLevel::CurrentAction,
        PresentationDisclosureLevel::Context,
        PresentationDisclosureLevel::SelectedDetail,
        PresentationDisclosureLevel::ExactProvenance,
    ] {
        assert_round_trip(value);
    }
    for value in [
        PresentationTemporalRole::Event,
        PresentationTemporalRole::Observation,
        PresentationTemporalRole::Ingestion,
    ] {
        assert_round_trip(value);
    }
    for value in [LayoutAxis::Horizontal, LayoutAxis::Vertical] {
        assert_round_trip(value);
    }
    for value in [
        LayoutAlignment::Start,
        LayoutAlignment::Center,
        LayoutAlignment::End,
    ] {
        assert_round_trip(value);
    }
    for value in [
        GraphicsClipClass::FullyVisible,
        GraphicsClipClass::PartiallyClipped,
        GraphicsClipClass::FullyClipped,
    ] {
        assert_round_trip(value);
    }
    for value in [
        GeneratedManifestationDisposition::Produced,
        GeneratedManifestationDisposition::Truncated,
        GeneratedManifestationDisposition::Refused,
        GeneratedManifestationDisposition::Failed,
        GeneratedManifestationDisposition::Cancelled,
        GeneratedManifestationDisposition::ProviderLost,
    ] {
        assert_round_trip(value);
    }
    for value in [
        GeneratedContentRole::Speech,
        GeneratedContentRole::PresentedThought,
    ] {
        assert_round_trip(value);
    }
}

#[test]
fn generated_manifestation_vocabularies_preserve_serde_and_postcard_shapes() {
    let dispositions = [
        (
            GeneratedManifestationDisposition::Produced,
            "\"Produced\"",
            vec![0],
        ),
        (
            GeneratedManifestationDisposition::Truncated,
            "\"Truncated\"",
            vec![1],
        ),
        (
            GeneratedManifestationDisposition::Refused,
            "\"Refused\"",
            vec![2],
        ),
        (
            GeneratedManifestationDisposition::Failed,
            "\"Failed\"",
            vec![3],
        ),
        (
            GeneratedManifestationDisposition::Cancelled,
            "\"Cancelled\"",
            vec![4],
        ),
        (
            GeneratedManifestationDisposition::ProviderLost,
            "\"ProviderLost\"",
            vec![5],
        ),
    ];
    for (value, json, bytes) in dispositions {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(postcard::to_allocvec(&value).unwrap(), bytes);
        assert_eq!(
            serde_json::from_str::<GeneratedManifestationDisposition>(json).unwrap(),
            value
        );
        assert_eq!(
            postcard::from_bytes::<GeneratedManifestationDisposition>(&bytes).unwrap(),
            value
        );
    }
    for (value, json, bytes) in [
        (GeneratedContentRole::Speech, "\"Speech\"", vec![0]),
        (
            GeneratedContentRole::PresentedThought,
            "\"PresentedThought\"",
            vec![1],
        ),
    ] {
        assert_eq!(serde_json::to_string(&value).unwrap(), json);
        assert_eq!(postcard::to_allocvec(&value).unwrap(), bytes);
        assert_eq!(
            serde_json::from_str::<GeneratedContentRole>(json).unwrap(),
            value
        );
        assert_eq!(
            postcard::from_bytes::<GeneratedContentRole>(&bytes).unwrap(),
            value
        );
    }
}
