//! Deterministic PROFILE-built, one-Body presentation capstone.

use conduit_body::{AuthenticatedHostObservation, Body, BodyMembership, MembershipProofId, PartId};
use conduit_core::{
    bind_active_play, resource_offer, ArtifactId, BootId, CapabilityId, GearId, HostId,
    HostProfileId, OfferGeneration, SignId,
};
use conduit_form::{parse, ProfileCatalog};
use conduit_host_fabrication::{
    bind_runtime_offer, build_default_host_image, BoundHostAdvertisement, BuildInputs,
    FabricationCatalog, HostProfile, RuntimeFacts, RuntimeOfferInputs,
};
use conduit_planner::{plan, PlacementChoice, PlacementChoices};
use conduit_presentation::{
    renderer_kind_definition, Manifestation, ManifestationAdmission, ManifestationLifecycle,
    ManifestationSet, Presentation, PresentationBasis, PresentationRole, PresentationSubject,
    PresentationText,
};
use std::collections::BTreeMap;

#[path = "host_capstone_receipt.rs"]
mod receipt;
use receipt::*;
#[path = "host_capstone_offers.rs"]
mod offers;
use offers::*;
#[path = "host_capstone_command.rs"]
mod command;
pub(super) use command::run;
#[path = "host_capstone_manifestations.rs"]
mod manifestations;
use manifestations::{identity_refusals, manifestation_for, mark_replaced};

const FORM_SOURCE: &str =
    "form shared-face {\n    native: presentation/renderer\n    browser: presentation/renderer\n}\n";
const NATIVE_PROFILE: &str =
    include_str!("../../../../targets/conduitos/profiles/conduitos-native.profile.json");
const BROWSER_PROFILE: &str =
    include_str!("../../../../targets/browser/profiles/browser-page.profile.json");
const HEADLESS_PROFILE: &str =
    include_str!("../../../../targets/conduitos/profiles/conduitos-headless.profile.json");

pub fn prove(source_identity: &str) -> Result<CapstoneReceipt, Box<dyn std::error::Error>> {
    let catalog = conduit_workspace_fabrication::catalog();
    let inputs = BuildInputs {
        source_identity: source_identity.into(),
        toolchain_available: true,
    };
    let native = build_profile("conduitos-native", NATIVE_PROFILE, &catalog, &inputs)?;
    let browser = build_profile("browser-page", BROWSER_PROFILE, &catalog, &inputs)?;
    let headless = build_profile("conduitos-headless", HEADLESS_PROFILE, &catalog, &inputs)?;

    let native_offer = presenter_offer(
        "presenter/native",
        "conduitos/native@1",
        "presenter/native-graphical@1",
        &native.image.manifest.image_id,
        "presentation/base/native-compositor@1",
    );
    let browser_offer = presenter_offer(
        "presenter/browser",
        "browser/dom-svg@1",
        "presenter/browser-dom-svg@1",
        &browser.image.manifest.image_id,
        "presentation/base/dom-svg@1",
    );
    let native_bound = bind_profile(
        &native,
        &catalog,
        "host/native",
        "boot/native/1",
        11,
        Some(native_offer.clone()),
        native_facts(true),
    )?;
    let browser_bound = bind_profile(
        &browser,
        &catalog,
        "host/browser",
        "boot/browser/1",
        12,
        Some(browser_offer.clone()),
        browser_facts(true),
    )?;
    let headless_bound = bind_profile(
        &headless,
        &catalog,
        "host/headless",
        "boot/headless/1",
        13,
        Some(native_offer.clone()),
        native_facts(true),
    )?;
    require(
        native_bound.advertisement().capabilities == vec![native_offer.clone()]
            && browser_bound.advertisement().capabilities == vec![browser_offer]
            && headless_bound.advertisement().capabilities.is_empty(),
        "runtime Presenter offers do not match exact PROFILE-built machinery",
    )?;

    let missing_live = bind_profile(
        &native,
        &catalog,
        "host/native",
        "boot/native/1",
        11,
        Some(native_offer),
        native_facts(false),
    )?;
    let missing_live_presenter = missing_live.advertisement().capabilities.is_empty();

    let form = checked_form()?;
    let body = Body::born(
        form.source_document_id.clone(),
        form.checked_form_id.clone(),
        1,
        SignId::from("capstone/body-born"),
    )
    .map_err(debug_error)?;
    let (body, wake) = body
        .wake(1, SignId::from("capstone/body-woke"))
        .map_err(debug_error)?;
    let advertisements = vec![
        native_bound.advertisement().clone(),
        browser_bound.advertisement().clone(),
        headless_bound.advertisement().clone(),
    ];
    let (membership, part_ids) = admit_hosts(&body, &advertisements)?;

    let choices = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("shared-face/native"),
                PlacementChoice {
                    host_id: advertisements[0].host_id.clone(),
                    capability_id: CapabilityId::from("presenter/native"),
                },
            ),
            (
                GearId::from("shared-face/browser"),
                PlacementChoice {
                    host_id: advertisements[1].host_id.clone(),
                    capability_id: CapabilityId::from("presenter/browser"),
                },
            ),
        ]),
    };
    let accepted_plan = plan(&form, &advertisements, &choices, &[]).map_err(debug_error)?;
    let admission = ManifestationAdmission::from_plan(&accepted_plan).map_err(debug_error)?;
    let initial = presentation(
        &form,
        &body,
        &wake.wake_id,
        &accepted_plan,
        1,
        "Body truth: idle",
        "capstone/body-truth/1",
    )?;
    let initial_manifestations = realize(&initial, &accepted_plan, &admission, "initial")?;

    let membership_before_refusals = membership.clone();
    let plan_before_refusals = accepted_plan.clone();
    let presentation_before_refusals = initial.clone();
    let headless_graphical_placement = headless_placement_refuses(&form, &advertisements);
    let (stale_boot, stale_generation, cross_wired_manifestation) =
        identity_refusals(&initial, &accepted_plan, &initial_manifestations);
    require(
        membership == membership_before_refusals
            && accepted_plan == plan_before_refusals
            && initial == presentation_before_refusals,
        "negative demonstrations mutated accepted truth",
    )?;

    let interaction_manifestation =
        manifestation_for(&initial_manifestations, "presenter/native-graphical@1")?;
    require(
        interaction_manifestation.lifecycle == ManifestationLifecycle::Available
            && interaction_manifestation.face_subject == "face/main"
            && interaction_manifestation
                .validate_against(&initial, &accepted_plan)
                .is_ok(),
        "semantic interaction did not originate through the accepted native Manifestation",
    )?;
    let interaction_manifestation_id = interaction_manifestation.manifestation_id.as_str().into();
    let revised = presentation(
        &form,
        &body,
        &wake.wake_id,
        &accepted_plan,
        2,
        "Body truth: engaged",
        "capstone/body-truth/2",
    )?;
    let replaced_manifestations = mark_replaced(
        &initial,
        &accepted_plan,
        &admission,
        &initial_manifestations,
    )?;
    let revised_manifestations = realize(&revised, &accepted_plan, &admission, "revised")?;
    require(
        revised_manifestations
            .manifestations
            .iter()
            .all(|item| item.presentation_id == revised.identity),
        "a Presenter did not independently consume revised Body truth",
    )?;
    let native_manifestation =
        manifestation_for(&revised_manifestations, "presenter/native-graphical@1")?;
    let browser_manifestation =
        manifestation_for(&revised_manifestations, "presenter/browser-dom-svg@1")?;
    let native_manifestation_id = native_manifestation.manifestation_id.as_str().into();
    let browser_manifestation_id = browser_manifestation.manifestation_id.as_str().into();

    let images = vec![native, browser, headless]
        .into_iter()
        .map(|built| ImageEvidence {
            profile_name: built.name.into(),
            artifact_id: ArtifactId::from(built.image.manifest.image_id.clone()),
            manifest: built.image.manifest.clone(),
            image: built.image,
            encoded_bytes: built.bytes.len(),
        })
        .collect();
    let receipt = CapstoneReceipt {
        schema: SCHEMA,
        images,
        boots: vec![
            native_bound.identity().clone(),
            browser_bound.identity().clone(),
            headless_bound.identity().clone(),
        ],
        membership,
        part_ids,
        plan: accepted_plan,
        initial_presentation: initial.clone(),
        initial_manifestations,
        replaced_manifestations,
        revised_presentation: revised.clone(),
        revised_manifestations,
        update: UpdateEvidence {
            source: "manifestation-semantic-action-to-body-truth",
            interaction_manifestation_id,
            semantic_subject: "face/main".into(),
            semantic_action: "engage".into(),
            sign_id: SignId::from("capstone/body-truth/2"),
            prior_presentation_id: initial.identity.as_str().into(),
            revised_presentation_id: revised.identity.as_str().into(),
            native_manifestation_id,
            browser_manifestation_id,
        },
        refusals: RefusalEvidence {
            missing_live_presenter,
            headless_graphical_placement,
            stale_boot,
            stale_generation,
            cross_wired_manifestation,
        },
    };
    require(
        serde_json::to_vec(&receipt)?.len() <= MAX_CAPSTONE_RECEIPT_BYTES,
        "capstone receipt exceeds its admitted byte bound",
    )?;
    Ok(receipt)
}

fn build_profile(
    name: &'static str,
    source: &str,
    catalog: &FabricationCatalog,
    inputs: &BuildInputs,
) -> Result<BuiltProfile, Box<dyn std::error::Error>> {
    let profile: HostProfile = serde_json::from_str(source)?;
    let packages = conduit_workspace_fabrication::package_set();
    let (image, bytes) =
        build_default_host_image(profile, catalog, &packages, inputs).map_err(debug_error)?;
    Ok(BuiltProfile { name, image, bytes })
}

fn bind_profile(
    built: &BuiltProfile,
    catalog: &FabricationCatalog,
    host: &str,
    boot: &str,
    generation: u64,
    presenter: Option<conduit_core::CapabilityOffer>,
    facts: RuntimeFacts,
) -> Result<BoundHostAdvertisement, Box<dyn std::error::Error>> {
    let candidate_resources = if presenter.is_some() {
        vec![resource_offer(
            &format!("{host}/surface"),
            "presentation/surface",
            1,
        )]
    } else {
        vec![]
    };
    bind_runtime_offer(
        &built.image.manifest,
        &built.image,
        &built.bytes,
        catalog,
        RuntimeOfferInputs {
            host_id: HostId::from(host),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(generation),
            offer_sign_id: SignId::from(format!("{host}/offer/{generation}")),
            host_profile: HostProfileId::from(built.image.manifest.profile_id.clone()),
            candidate_resources,
            candidate_capabilities: presenter.into_iter().collect(),
            planner_capabilities: vec![],
            facts,
        },
    )
    .map_err(debug_error)
}

fn checked_form() -> Result<conduit_form::CheckedForm, Box<dyn std::error::Error>> {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(renderer_kind_definition())
        .map_err(debug_error)?;
    parse(FORM_SOURCE, &catalog).map_err(debug_error)
}

fn admit_hosts(
    body: &Body,
    advertisements: &[conduit_core::HostAdvertisement],
) -> Result<(BodyMembership, Vec<PartId>), Box<dyn std::error::Error>> {
    let mut membership = BodyMembership::new(body.body_id.clone()).map_err(debug_error)?;
    let mut part_ids = Vec::new();
    for (index, advertisement) in advertisements.iter().enumerate() {
        let part = PartId::bind(
            &body.body_id,
            advertisement.host_id.as_str(),
            index as u64 + 1,
        )
        .map_err(debug_error)?;
        let proof =
            MembershipProofId::bind(&format!("capstone/proof/{index}")).map_err(debug_error)?;
        membership
            .admit(
                &body.body_id,
                membership.revision,
                part.clone(),
                proof.clone(),
                SignId::from(format!("capstone/part/{index}/admitted")),
            )
            .map_err(debug_error)?;
        membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: advertisement.host_id.clone(),
                    boot_id: advertisement.boot_id.clone(),
                    offer_generation: advertisement.offer_generation,
                    proof_id: proof,
                    sequence: 1,
                },
                SignId::from(format!("capstone/part/{index}/present")),
            )
            .map_err(debug_error)?;
        part_ids.push(part);
    }
    membership.validate().map_err(debug_error)?;
    Ok((membership, part_ids))
}

fn presentation(
    form: &conduit_form::CheckedForm,
    body: &Body,
    wake_id: &conduit_body::WakeId,
    plan: &conduit_core::Plan,
    revision: u64,
    text: &str,
    sign: &str,
) -> Result<Presentation, Box<dyn std::error::Error>> {
    Presentation::new(
        revision,
        PresentationBasis {
            body_id: Some(body.body_id.clone()),
            wake_id: Some(wake_id.clone()),
            source_document_id: Some(form.source_document_id.clone()),
            checked_form_id: Some(form.checked_form_id.clone()),
            expanded_form_id: Some(form.expanded_form_id.clone()),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: vec![SignId::from(sign)],
        },
        vec![PresentationSubject {
            identity: "face/main".into(),
            role: PresentationRole::Form,
            label: "Shared Face".into(),
            accessibility_name: "One shared semantic Face".into(),
        }],
        vec![],
        vec![],
        vec![PresentationText {
            subject: "face/main".into(),
            text: text.into(),
        }],
    )
    .map_err(debug_error)
}

fn realize(
    presentation: &Presentation,
    plan: &conduit_core::Plan,
    admission: &ManifestationAdmission,
    phase: &str,
) -> Result<ManifestationSet, Box<dyn std::error::Error>> {
    let mut manifestations = Vec::new();
    for fragment in &plan.fragments {
        for placement in &fragment.placements {
            let prepared = Manifestation::prepared(
                presentation,
                plan,
                bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 1),
                placement.placement_id.clone(),
                "face/main".into(),
                format!("{}/surface", fragment.host_id.as_str()),
                SignId::from(format!(
                    "capstone/{phase}/{}/prepared",
                    fragment.host_id.as_str()
                )),
            )
            .map_err(debug_error)?;
            manifestations.push(
                prepared
                    .transition(
                        ManifestationLifecycle::Available,
                        SignId::from(format!(
                            "capstone/{phase}/{}/available",
                            fragment.host_id.as_str()
                        )),
                    )
                    .map_err(debug_error)?,
            );
        }
    }
    ManifestationSet::new(presentation, manifestations, plan, admission).map_err(debug_error)
}

fn headless_placement_refuses(
    form: &conduit_form::CheckedForm,
    advertisements: &[conduit_core::HostAdvertisement],
) -> bool {
    let choices = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("shared-face/native"),
                PlacementChoice {
                    host_id: advertisements[2].host_id.clone(),
                    capability_id: CapabilityId::from("presenter/native"),
                },
            ),
            (
                GearId::from("shared-face/browser"),
                PlacementChoice {
                    host_id: advertisements[1].host_id.clone(),
                    capability_id: CapabilityId::from("presenter/browser"),
                },
            ),
        ]),
    };
    plan(form, advertisements, &choices, &[]).is_err()
}

fn require(condition: bool, detail: &str) -> Result<(), Box<dyn std::error::Error>> {
    condition.then_some(()).ok_or_else(|| detail.into())
}

fn debug_error(error: impl std::fmt::Debug) -> Box<dyn std::error::Error> {
    format!("{error:?}").into()
}

#[cfg(test)]
#[path = "host_capstone_tests.rs"]
mod tests;
