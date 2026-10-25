//! Construction fixtures use explicit checked-identity stand-ins, not hardware.
use conduit_birth_plot::{names::NamingCatalog, BirthDraftRefusal, BirthPlotChoice};
use conduit_body::{BodyBiographyEvidence, BodyLifecycleSession, BodyState, ResidentPlot};
use conduit_environment_make::{BodyImageRefusal, PreparedBodyImage};

const UUID: &str = "01234567-89ab-4cde-8fab-0123456789ab";

fn choices() -> Vec<BirthPlotChoice> {
    ["sampling", "display"]
        .into_iter()
        .map(|name| BirthPlotChoice {
            title: name.into(),
            search_text: name.into(),
            plot: ResidentPlot::new(
                format!("source/{name}").into(),
                format!("checked/{name}").into(),
            ),
            refusal: None,
            selected: true,
        })
        .collect()
}

#[test]
fn image_retains_creche_name_and_workset_without_claiming_a_boot_or_play() {
    let image =
        PreparedBodyImage::prepare(UUID.into(), choices(), 1, "sign/build/birth/1".into()).unwrap();
    let expected = NamingCatalog::shared()
        .unwrap()
        .name_for(UUID, "surprise", 0)
        .unwrap();
    assert_eq!(image.evidence().friendly_name, expected.name);
    assert_eq!(image.evidence().body.workset.len(), 2);
    assert_eq!(image.evidence().body.state, BodyState::Lulled);
    assert!(image.evidence().membership.parts.is_empty());
    // The installed ordinary biography format carries the generated name;
    // reloading image material does not run the naming catalogue again.
    let bytes = serde_json::to_vec(image.evidence()).unwrap();
    let restored: BodyBiographyEvidence = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored.friendly_name, expected.name);
    let owner = BodyLifecycleSession::open(restored).unwrap();
    assert!(owner.realization().is_none());
}

#[test]
fn attributable_birth_identity_remains_separate_from_generated_name() {
    let first =
        PreparedBodyImage::prepare(UUID.into(), choices(), 1, "sign/build/birth/1".into()).unwrap();
    let second =
        PreparedBodyImage::prepare(UUID.into(), choices(), 1, "sign/build/birth/2".into()).unwrap();
    assert_eq!(
        first.evidence().friendly_name,
        second.evidence().friendly_name
    );
    assert_ne!(first.evidence().body_id, second.evidence().body_id);
}

#[test]
fn invalid_or_unselected_workload_cannot_be_baked_as_a_running_appliance() {
    let mut unavailable = choices();
    unavailable[0].refusal = Some("capture unavailable in construction review".into());
    assert!(matches!(
        PreparedBodyImage::prepare(UUID.into(), unavailable, 1, "sign/birth".into()),
        Err(BodyImageRefusal::Draft(BirthDraftRefusal::InvalidInventory))
    ));
    let mut empty = choices();
    for choice in &mut empty {
        choice.selected = false;
    }
    assert!(matches!(
        PreparedBodyImage::prepare(UUID.into(), empty, 1, "sign/birth".into()),
        Err(BodyImageRefusal::Draft(BirthDraftRefusal::EmptySelection))
    ));
    assert!(matches!(
        PreparedBodyImage::prepare("not-a-uuid".into(), choices(), 1, "sign/birth".into()),
        Err(BodyImageRefusal::Draft(BirthDraftRefusal::Naming(_)))
    ));
}

#[test]
fn environment_image_selects_the_actual_checked_sources() {
    let image = conduit_environment_make::EnvironmentBodyImage::prepare(
        UUID.into(),
        1,
        "sign/build/checked-environment".into(),
    )
    .unwrap();
    let sampling = conduit_environment_runtime::construction::compile_default().unwrap();
    let display = conduit_environment_make::DisplayOutputSource::compile().unwrap();
    let workset = &image.body.evidence().body.workset;
    assert_eq!(workset.len(), 2);
    for plot in [&sampling.expanded, &display.plot.expanded] {
        assert!(workset
            .plots()
            .iter()
            .any(
                |resident| resident.source_document_id == plot.source_document_id
                    && resident.checked_plot_id == plot.checked_plot_id
            ));
    }
    assert_eq!(image.sampling, sampling.expanded.expanded_plot_id);
    assert_eq!(image.display, display.plot.expanded.expanded_plot_id);
    assert!(image.body.evidence().membership.parts.is_empty());
}

#[test]
fn build_sign_generates_and_retains_the_body_without_a_manual_name_or_uuid() {
    // Explicit CPU host/Boot fixtures, not a device-observation claim.
    let sign = conduit_core::bind_sign(
        &"fixture/build-host".into(),
        &"fixture/build-boot/1".into(),
        None,
        1,
    );
    let first = conduit_environment_make::EnvironmentBodyImage::prepare_from_sign(&sign).unwrap();
    let repeated =
        conduit_environment_make::EnvironmentBodyImage::prepare_from_sign(&sign).unwrap();
    assert_eq!(first.body.evidence(), repeated.body.evidence());
    assert_eq!(first.body.evidence().body.sign_ids[0], sign.sign_id);
    let bytes = serde_json::to_vec(first.body.evidence()).unwrap();
    let restored: BodyBiographyEvidence = serde_json::from_slice(&bytes).unwrap();
    let owner = BodyLifecycleSession::open(restored).unwrap();
    assert_eq!(
        owner.evidence().friendly_name,
        first.body.evidence().friendly_name
    );
    assert!(owner.realization().is_none());
    let mut forged = sign;
    forged.sign_id = "fixture/noncanonical-sign".into();
    assert!(matches!(
        conduit_environment_make::EnvironmentBodyImage::prepare_from_sign(&forged),
        Err(conduit_environment_make::EnvironmentBodyImageRefusal::BirthSign)
    ));
}
