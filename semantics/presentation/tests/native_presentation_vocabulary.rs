use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::{
    AdmittedNavigationDestination, ChoiceMultiplicity, EvidenceDisposition,
    FaceUtteranceClauseKind, PresentationDisclosureLevel, PresentationMechanismKind,
    PresentationTemporalRole, StatusKind,
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
}
