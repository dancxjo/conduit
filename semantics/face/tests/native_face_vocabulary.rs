use conduit_face::{
    AdmittedNavigationDestination, ChoiceMultiplicity, EvidenceDisposition, FaceDisclosureLevel,
    FaceMechanismKind, FaceTemporalRole, FaceUtteranceClauseKind, GeneratedContentRole,
    GeneratedManifestationDisposition, GraphicsClipClass, LayoutAlignment, LayoutAxis, StatusKind,
};
use conduit_plot::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn face_vocabularies_round_trip_through_exact_native_types() {
    for value in [
        FaceMechanismKind::Shell,
        FaceMechanismKind::Workbench,
        FaceMechanismKind::Panel,
        FaceMechanismKind::Heading,
        FaceMechanismKind::Separator,
        FaceMechanismKind::Grid,
        FaceMechanismKind::ActionGroup,
        FaceMechanismKind::Action,
        FaceMechanismKind::Status,
        FaceMechanismKind::Disclosure,
        FaceMechanismKind::Evidence,
        FaceMechanismKind::DefinitionTable,
        FaceMechanismKind::Definition,
        FaceMechanismKind::CodeBlock,
        FaceMechanismKind::FormField,
        FaceMechanismKind::ChoiceGroup,
        FaceMechanismKind::Navigation,
        FaceMechanismKind::NavigationLink,
        FaceMechanismKind::Link,
        FaceMechanismKind::Stepper,
        FaceMechanismKind::Progress,
        FaceMechanismKind::Artifact,
        FaceMechanismKind::Download,
        FaceMechanismKind::DeviceChoice,
        FaceMechanismKind::PatchbayCanvas,
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
        FaceUtteranceClauseKind::Disclosure,
        FaceUtteranceClauseKind::TemporalReference,
        FaceUtteranceClauseKind::TemporalFact,
    ] {
        assert_round_trip(value);
    }
    for value in [
        FaceDisclosureLevel::Primary,
        FaceDisclosureLevel::CurrentAction,
        FaceDisclosureLevel::Context,
        FaceDisclosureLevel::SelectedDetail,
        FaceDisclosureLevel::ExactProvenance,
    ] {
        assert_round_trip(value);
    }
    for value in [
        FaceTemporalRole::Event,
        FaceTemporalRole::Observation,
        FaceTemporalRole::Ingestion,
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
