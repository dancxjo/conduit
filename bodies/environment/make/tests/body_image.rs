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

#[test]
fn compiled_bootstrap_reconstructs_the_same_creche_biography_and_both_plots() {
    let sign = conduit_core::bind_sign(
        &"construction/host".into(),
        &"construction/boot".into(),
        None,
        17,
    );
    let image = conduit_environment_make::EnvironmentBodyImage::prepare_from_sign(&sign).unwrap();
    let restored = image.body.reconstruct_bootstrap().unwrap();
    assert_eq!(&restored, image.body.evidence());
    assert_eq!(restored.body.workset.len(), 2);
    assert_eq!(restored.records[0].sign_id, sign.sign_id);
    let mut rust = String::new();
    image.body.write_bootstrap_rust(&mut rust).unwrap();
    assert!(rust.contains("restore_pristine_image"));
    assert!(!rust.contains("serde_json"));
}

#[test]
fn ordinary_mask_image_selects_exact_sampler_and_mask_without_changing_raw_profile() {
    use conduit_environment_make::{EnvironmentBodyImage, SevenSegmentMaskSource};
    let sign: conduit_core::SignId = "sign/build/mask-profile".into();
    let image = EnvironmentBodyImage::prepare_mask(UUID.into(), 7, sign.clone()).unwrap();
    let raw = EnvironmentBodyImage::prepare(UUID.into(), 7, sign.clone()).unwrap();
    let sampling = conduit_environment_runtime::construction::compile_default().unwrap();
    let mask = SevenSegmentMaskSource::compile().unwrap();
    let display = conduit_environment_make::DisplayOutputSource::compile().unwrap();
    let residents = |other: &conduit_plot::ExpandedCanonicalPlot| {
        [&sampling.expanded, other].map(|plot| {
            ResidentPlot::new(
                plot.source_document_id.clone(),
                plot.checked_plot_id.clone(),
            )
        })
    };
    let mask_residents = residents(&mask.plot.expanded);
    let evidence = image.body.evidence();
    assert_eq!(evidence.body.workset.len(), 2);
    for resident in &mask_residents {
        assert!(evidence.body.workset.plots().contains(resident));
    }
    assert_eq!(image.sampling, sampling.expanded.expanded_plot_id);
    assert_eq!(image.display, mask.role.plot_identity.expanded_plot_id);
    assert_eq!(
        image.sampling_software.planning_image(),
        conduit_plot::flat_image::indexed::encode(&sampling.expanded).unwrap()
    );
    let canonical =
        conduit_body::Body::born_with_plots(evidence.body.workset.clone(), 7, sign.clone())
            .unwrap();
    assert_eq!(evidence.body, canonical);
    assert_eq!(evidence.body.state, BodyState::Lulled);
    assert!(evidence.membership.parts.is_empty());
    assert_eq!(image.body.reconstruct_bootstrap().unwrap(), *evidence);

    // Recreate the preexisting raw-output construction contract independently:
    // exact titles, review order, explicit UUID and original checked identities.
    let raw_choices = [
        ("Environmental sampling", &sampling.expanded),
        ("Seven-segment output", &display.plot.expanded),
    ]
    .into_iter()
    .map(|(title, plot)| BirthPlotChoice {
        title: title.into(),
        search_text: title.into(),
        plot: ResidentPlot::new(
            plot.source_document_id.clone(),
            plot.checked_plot_id.clone(),
        ),
        selected: true,
        refusal: None,
    })
    .collect();
    let original_raw = PreparedBodyImage::prepare(UUID.into(), raw_choices, 7, sign).unwrap();
    assert_eq!(raw.body.evidence(), original_raw.evidence());
    assert_eq!(raw.display, display.plot.expanded.expanded_plot_id);
    assert_eq!(raw.sampling, image.sampling);
    assert_eq!(raw.body.evidence().friendly_name, evidence.friendly_name);
    assert_ne!(raw.body.evidence().body_id, evidence.body_id);
    assert!(!evidence
        .body
        .workset
        .plots()
        .contains(&residents(&display.plot.expanded)[1]));
}

#[test]
fn mask_sign_construction_shares_canonical_seed_and_bootstrap_without_admitting_a_host() {
    use conduit_environment_make::{EnvironmentBodyImage, EnvironmentBodyImageRefusal};
    let sign = conduit_core::bind_sign(
        &"construction/mask-host".into(),
        &"construction/mask-boot".into(),
        None,
        23,
    );
    let image = EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let repeated = EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let raw = EnvironmentBodyImage::prepare_from_sign(&sign).unwrap();
    let text = sign.sign_id.as_str();
    let seed = format!(
        "{}-{}-{}-{}-{}",
        &text[..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    );
    let explicit =
        EnvironmentBodyImage::prepare_mask(seed.clone(), sign.sequence, sign.sign_id.clone())
            .unwrap();
    let explicit_raw =
        EnvironmentBodyImage::prepare(seed, sign.sequence, sign.sign_id.clone()).unwrap();
    assert_eq!(image.body.evidence(), repeated.body.evidence());
    assert_eq!(image.body.evidence(), explicit.body.evidence());
    assert_eq!(raw.body.evidence(), explicit_raw.body.evidence());
    assert_eq!(
        image.body.evidence().friendly_name,
        raw.body.evidence().friendly_name
    );
    assert_eq!(
        image.body.evidence().body.sign_ids,
        vec![sign.sign_id.clone()]
    );
    let restored = image.body.reconstruct_bootstrap().unwrap();
    assert_eq!(restored, *image.body.evidence());
    let encoded = serde_json::to_vec(&restored).unwrap();
    let owner = BodyLifecycleSession::open(serde_json::from_slice(&encoded).unwrap()).unwrap();
    assert!(owner.realization().is_none());
    assert!(owner.evidence().membership.parts.is_empty());
    let mut rust = String::new();
    image.body.write_bootstrap_rust(&mut rust).unwrap();
    assert!(rust.contains("restore_pristine_image"));
    assert!(!rust.contains("serde_json"));
    for malformed in 0..4 {
        let mut forged = sign.clone();
        match malformed {
            0 => forged.sign_id = "not-a-canonical-sign".into(),
            1 => forged.sequence += 1,
            2 => forged.host_id = "foreign/host".into(),
            3 => forged.boot_id = "foreign/boot".into(),
            _ => unreachable!(),
        }
        assert!(matches!(
            EnvironmentBodyImage::prepare_mask_from_sign(&forged),
            Err(EnvironmentBodyImageRefusal::BirthSign)
        ));
        assert!(matches!(
            EnvironmentBodyImage::prepare_from_sign(&forged),
            Err(EnvironmentBodyImageRefusal::BirthSign)
        ));
    }
}
