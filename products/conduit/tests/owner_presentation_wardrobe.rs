//! Contract proof for the owner-held cross-route wardrobe before service wiring.
#[path = "../src/body_owner/presentation_wardrobe.rs"]
mod owner_presentation_wardrobe;

use conduit_body::{Body, BodyBiographyEvidence, BodyLifecycleSession, BodyMembership};
use conduit_core::{bind_active_play, BootId, HostAdvertisement, HostId, OfferGeneration, SignId};
use conduit_presentation::{
    CurrentOwnerPresentationRoute, LocalOwnerMaskRouteSeal, ManifestationLifecycle, MaskShow,
    MaskShowDisposition, MaskWardrobeAction, MaskWardrobeControlError, MaskWardrobeError,
    PlannedMaskPlot, Presentation, PresentationBasis, PresentationRole, PresentationSubject,
};
use conduit_std_host::{StdHost, StdHostConfig};
use owner_presentation_wardrobe::{OwnerPresentationWardrobe, OwnerPresentationWardrobeError};

fn owner_offer() -> HostAdvertisement {
    let mut offer = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("host/wardrobe-owner"),
        boot_id: BootId::from("boot/wardrobe-owner"),
        offer_generation: OfferGeneration(1),
    })
    .advertisement()
    .clone();
    offer.capabilities.push(conduit_browser_mask_offer::offer());
    offer
        .resources
        .retain(|resource| resource.class_id.as_str() != conduit_core::PRESENTATION_RESOURCE_CLASS);
    offer.resources.push(conduit_core::resource_offer(
        "browser/presentation",
        conduit_core::PRESENTATION_RESOURCE_CLASS,
        1,
    ));
    offer
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    offer.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    offer
}

struct Fixture {
    session: BodyLifecycleSession,
    face: Presentation,
    offer: HostAdvertisement,
    first: PlannedMaskPlot,
    second: PlannedMaskPlot,
    first_seal: LocalOwnerMaskRouteSeal,
    second_seal: LocalOwnerMaskRouteSeal,
}

impl Fixture {
    fn new() -> Self {
        let offer = owner_offer();
        let first = conduit_browser_mask_offer::planned_mask(
            &offer,
            conduit_browser_mask_offer::MASK_SOURCE,
            "browser-graphical",
        )
        .unwrap();
        let second = conduit_browser_mask_offer::planned_mask(
            &offer,
            conduit_browser_mask_offer::ALTERNATE_MASK_SOURCE,
            "browser-graphical-alternate",
        )
        .unwrap();
        let body = Body::born(
            "source/wardrobe".into(),
            "checked/wardrobe".into(),
            1,
            SignId::from("sign/wardrobe-born"),
        )
        .unwrap();
        let membership = BodyMembership::new(body.body_id.clone()).unwrap();
        let evidence =
            BodyBiographyEvidence::born(body, membership, "Wardrobe Body".into()).unwrap();
        let session = BodyLifecycleSession::open(evidence).unwrap();
        let face = Self::face(&session, 1);
        let first_seal =
            LocalOwnerMaskRouteSeal::seal_lulled(&session, &face, &offer, &first).unwrap();
        let second_seal =
            LocalOwnerMaskRouteSeal::seal_lulled(&session, &face, &offer, &second).unwrap();
        Self {
            session,
            face,
            offer,
            first,
            second,
            first_seal,
            second_seal,
        }
    }

    fn face(session: &BodyLifecycleSession, revision: u64) -> Presentation {
        Presentation::new(
            revision,
            PresentationBasis {
                body_id: Some(session.evidence().body_id.clone()),
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            vec![PresentationSubject {
                identity: "wardrobe/body".into(),
                role: PresentationRole::Plot,
                name: "Wardrobe Body".into(),
            }],
            vec![],
            vec![],
            vec![],
        )
        .unwrap()
    }

    fn first_witness(&self) -> CurrentOwnerPresentationRoute<'_> {
        CurrentOwnerPresentationRoute::Local {
            seal: &self.first_seal,
            owner_offer: &self.offer,
        }
    }

    fn second_witness(&self) -> CurrentOwnerPresentationRoute<'_> {
        CurrentOwnerPresentationRoute::Local {
            seal: &self.second_seal,
            owner_offer: &self.offer,
        }
    }

    fn show(&self, planned: &PlannedMaskPlot, sequence: u64) -> MaskShow {
        let placement = planned.show_placement();
        let play = bind_active_play(
            &planned.plan.plan_id,
            &placement.host_id,
            &placement.boot_id,
            sequence,
        );
        MaskShow::prepared(
            planned,
            &self.face,
            play,
            "wardrobe/body".into(),
            "browser/document".into(),
            SignId::from(format!("sign/show/{sequence}/prepared")),
        )
        .unwrap()
        .transition(
            ManifestationLifecycle::Available,
            SignId::from(format!("sign/show/{sequence}/available")),
        )
        .unwrap()
    }

    fn wardrobe(&self) -> OwnerPresentationWardrobe {
        OwnerPresentationWardrobe::seal(
            &self.session,
            &self.face,
            &[self.first_witness(), self.second_witness()],
            vec![
                self.first.mask.plot_identity.clone(),
                self.second.mask.plot_identity.clone(),
            ],
            vec![
                self.first.mask.plot_identity.clone(),
                self.second.mask.plot_identity.clone(),
            ],
        )
        .unwrap()
    }
}

#[test]
fn explicit_selection_uses_an_admitted_route_without_replacing_the_owner_plan() {
    let fixture = Fixture::new();
    let mut wardrobe = fixture.wardrobe();
    let original_plan = wardrobe.plan().plan_id.clone();
    wardrobe
        .select_plot(
            &fixture.session,
            &fixture.face,
            &[fixture.first_witness(), fixture.second_witness()],
            &fixture.second.mask.plot_identity,
            &fixture.second_seal.route_plan_id,
        )
        .unwrap();
    assert_eq!(wardrobe.plan().plan_id, original_plan);
    assert_eq!(
        wardrobe.control().selected.as_ref().unwrap().mask_plot,
        fixture.second.mask.plot_identity
    );
    assert_eq!(
        wardrobe.selected_show(
            &fixture.session,
            &fixture.face,
            &[fixture.first_witness(), fixture.second_witness()],
        ),
        Err(OwnerPresentationWardrobeError::ShowNotAcknowledged)
    );
}

#[test]
fn fallback_doff_rewear_and_replacement_require_fresh_show() {
    let mut fixture = Fixture::new();
    let mut wardrobe = fixture.wardrobe();
    assert_eq!(wardrobe.plan().routes.len(), 2);
    assert_eq!(
        wardrobe.control().selected.as_ref().unwrap().mask_plot,
        fixture.first.mask.plot_identity
    );
    let first = fixture.show(&fixture.first, 1);
    wardrobe
        .acknowledge_selected_show(
            &fixture.session,
            &fixture.face,
            &[fixture.first_witness(), fixture.second_witness()],
            &fixture.first_seal.route_plan_id,
            &first,
        )
        .unwrap();
    assert_eq!(
        wardrobe
            .selected_show(
                &fixture.session,
                &fixture.face,
                &[fixture.first_witness(), fixture.second_witness()]
            )
            .unwrap(),
        &first
    );
    let original_plan = wardrobe.plan().plan_id.clone();
    wardrobe
        .admit_or_replace(
            &fixture.session,
            &fixture.face,
            &[fixture.first_witness(), fixture.second_witness()],
        )
        .unwrap();
    assert_eq!(wardrobe.plan().plan_id, original_plan);
    wardrobe.forget_show_for(&fixture.second_seal.route_plan_id);
    assert!(wardrobe
        .selected_show(
            &fixture.session,
            &fixture.face,
            &[fixture.first_witness(), fixture.second_witness()]
        )
        .is_ok());

    let fallback = wardrobe
        .reconcile(&fixture.session, &fixture.face, &[fixture.second_witness()])
        .unwrap();
    assert!(matches!(
        fallback.show,
        MaskShowDisposition::SelectSealed { .. }
    ));
    assert_eq!(
        wardrobe.control().selected.as_ref().unwrap().mask_plot,
        fixture.second.mask.plot_identity
    );
    assert_eq!(
        wardrobe.selected_show(&fixture.session, &fixture.face, &[fixture.second_witness()]),
        Err(OwnerPresentationWardrobeError::ShowNotAcknowledged)
    );
    let second = fixture.show(&fixture.second, 2);
    wardrobe
        .acknowledge_selected_show(
            &fixture.session,
            &fixture.face,
            &[fixture.second_witness()],
            &fixture.second_seal.route_plan_id,
            &second,
        )
        .unwrap();
    wardrobe.forget_show_for(&fixture.second_seal.route_plan_id);
    assert!(wardrobe.control().selected.is_none());
    assert_eq!(
        wardrobe.selected_show(&fixture.session, &fixture.face, &[fixture.second_witness()]),
        Err(OwnerPresentationWardrobeError::ShowNotAcknowledged)
    );
    let revision = wardrobe.control().scoped_wardrobe.wardrobe.revision;
    wardrobe
        .apply(
            &fixture.session,
            &fixture.face,
            &[fixture.second_witness()],
            revision,
            MaskWardrobeAction::Doff(fixture.second.mask.plot_identity.clone()),
        )
        .unwrap();
    assert_eq!(
        wardrobe.selected_show(&fixture.session, &fixture.face, &[fixture.second_witness()]),
        Err(OwnerPresentationWardrobeError::ShowNotAcknowledged)
    );
    assert_eq!(
        wardrobe.apply(
            &fixture.session,
            &fixture.face,
            &[fixture.second_witness()],
            revision,
            MaskWardrobeAction::Wear(fixture.second.mask.plot_identity.clone()),
        ),
        Err(OwnerPresentationWardrobeError::Control(
            MaskWardrobeControlError::Wardrobe(MaskWardrobeError::StaleRevision)
        ))
    );
    wardrobe
        .apply(
            &fixture.session,
            &fixture.face,
            &[fixture.second_witness()],
            revision + 1,
            MaskWardrobeAction::Wear(fixture.second.mask.plot_identity.clone()),
        )
        .unwrap();
    assert_eq!(
        wardrobe.selected_show(&fixture.session, &fixture.face, &[fixture.second_witness()]),
        Err(OwnerPresentationWardrobeError::ShowNotAcknowledged)
    );

    let old_plan = wardrobe.plan().plan_id.clone();
    fixture.face = Fixture::face(&fixture.session, 2);
    fixture.first_seal = LocalOwnerMaskRouteSeal::seal_lulled(
        &fixture.session,
        &fixture.face,
        &fixture.offer,
        &fixture.first,
    )
    .unwrap();
    fixture.second_seal = LocalOwnerMaskRouteSeal::seal_lulled(
        &fixture.session,
        &fixture.face,
        &fixture.offer,
        &fixture.second,
    )
    .unwrap();
    wardrobe
        .replace(
            &fixture.session,
            &fixture.face,
            &[fixture.first_witness(), fixture.second_witness()],
        )
        .unwrap();
    assert_ne!(wardrobe.plan().plan_id, old_plan);
    assert_eq!(
        wardrobe.control().scoped_wardrobe.wardrobe.revision,
        revision + 2
    );
    assert_eq!(
        wardrobe.selected_show(
            &fixture.session,
            &fixture.face,
            &[fixture.first_witness(), fixture.second_witness()]
        ),
        Err(OwnerPresentationWardrobeError::ShowNotAcknowledged)
    );
    assert_eq!(
        wardrobe.replace(
            &fixture.session,
            &fixture.face,
            &[fixture.first_witness(), fixture.second_witness()]
        ),
        Err(OwnerPresentationWardrobeError::ReusedPlan)
    );
}

#[test]
fn stale_or_unsealed_witness_cannot_change_selected_owner_route() {
    let fixture = Fixture::new();
    let mut wardrobe = fixture.wardrobe();
    let before = wardrobe.control().selected.clone();
    let mut stale = fixture.offer.clone();
    stale.offer_generation.0 += 1;
    assert!(wardrobe
        .reconcile(
            &fixture.session,
            &fixture.face,
            &[CurrentOwnerPresentationRoute::Local {
                seal: &fixture.first_seal,
                owner_offer: &stale,
            }]
        )
        .is_err());
    assert_eq!(wardrobe.control().selected, before);
    let unknown = LocalOwnerMaskRouteSeal::seal_lulled(
        &fixture.session,
        &fixture.face,
        &fixture.offer,
        &conduit_browser_mask_offer::planned_mask(
            &fixture.offer,
            &conduit_browser_mask_offer::ALTERNATE_MASK_SOURCE
                .replace("browser-graphical-alternate", "third-mask"),
            "third-mask",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(wardrobe
        .reconcile(
            &fixture.session,
            &fixture.face,
            &[CurrentOwnerPresentationRoute::Local {
                seal: &unknown,
                owner_offer: &fixture.offer,
            }]
        )
        .is_err());
    assert_eq!(wardrobe.control().selected, before);
}
