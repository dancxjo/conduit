//! Native software tests of default startup, distinct from emulator evidence.
use super::*;
use crate::{
    native_workset::{self, NativePlot},
    product_front_door::face_workspace::tests::CountingDisplay,
    product_journey::test_support::{fixture, key},
};
use conduit_birth_plot::BirthSelection;
use conduit_body::BodyWorkset;

pub(crate) struct Encounter {
    ids: BootIdentities,
    offer: HostOffer<'static>,
    pub(crate) journey: ProductJourney,
    door: FrontDoor,
    presenter: FrontDoorPresenter,
    workspace: FaceWorkspace,
    display: CountingDisplay,
    make: MakeRecord,
}

impl Encounter {
    pub(crate) fn born() -> Self {
        let (ids, offer, mut journey) = fixture();
        journey
            .birth_from_creche(BirthSelection {
                revision: 3,
                friendly_name: "Living Body".into(),
                workset: BodyWorkset::from_plots([
                    native_workset::resident(NativePlot::KeyboardCanvas).unwrap(),
                    native_workset::resident(NativePlot::MemoryLantern).unwrap(),
                ])
                .unwrap(),
            })
            .unwrap();
        let projection = journey.projection();
        let mut door = FrontDoor::new(
            projection.host_id.clone(),
            projection.boot_id.clone(),
            projection.offer_generation,
            "profile",
            "build",
            "image",
            projection.source_document_id.unwrap(),
            projection.checked_plot_id.unwrap(),
            7,
            true,
        );
        door.observe_product(&journey).unwrap();
        let provider = crate::product_bases::fixture_surface_provider();
        let workspace = FaceWorkspace::prepare(
            projection.host_id.clone(),
            projection.boot_id.clone(),
            projection.offer_generation,
            "build",
            provider.entry.base_id.clone(),
            &provider,
            conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
        )
        .unwrap();
        let mut presenter = FrontDoorPresenter::prepare(
            projection.host_id,
            projection.boot_id,
            projection.offer_generation,
            "profile",
            "image",
            provider.entry.base_id.clone(),
            2,
        )
        .unwrap();
        let mut display = CountingDisplay::default();
        presenter.present(&door, &mut display).unwrap();
        Self {
            ids,
            offer,
            journey,
            door,
            presenter,
            workspace,
            display,
            make: MakeRecord {
                build_id: "build",
                ..MakeRecord::legacy()
            },
        }
    }

    fn start(&mut self) -> Result<(), &'static str> {
        start(
            &mut self.door,
            &mut self.journey,
            &mut self.presenter,
            &mut self.workspace,
            &mut self.display,
            &self.ids,
            &self.offer,
            &self.make,
        )
    }

    pub(crate) fn remaining(&mut self) -> Result<(), &'static str> {
        remaining_stages(
            &mut self.door,
            &mut self.journey,
            &mut self.presenter,
            &mut self.workspace,
            &mut self.display,
            &self.ids,
            &self.offer,
            &self.make,
        )
    }

    pub(crate) fn assert_startup_refused(&self, stage: &str) {
        let face = alloc::format!("{:?}", self.door.face().unwrap());
        assert!(face.contains(stage), "stage-specific refusal: {face}");
        assert!(!self.workspace.active());
        assert!(self.presenter.route_keyboard().is_ok());
        assert!(self.journey.projection().active_play_id.is_none());
    }

    pub(crate) fn invoke(&mut self, action: PatchbayAction) -> Result<(), &'static str> {
        self.workspace
            .invoke_from_presenter(
                action,
                &mut self.door,
                &mut self.journey,
                &mut self.presenter,
                &mut self.display,
                &self.ids,
                &self.offer,
                &self.make,
            )
            .map(|_| ())
    }
}

#[test]
fn default_startup_keeps_birth_and_listens_with_both_residents_in_one_play() {
    let mut encounter = Encounter::born();
    let born = encounter.journey.projection();
    assert_eq!(born.status, JourneyStatus::BornLulled);
    assert!(born.plan_id.is_none());
    encounter.start().unwrap();
    assert!(encounter.door.home_open());
    assert_eq!(encounter.door.commons_place(), Some("commons/hub"));
    let active = encounter.journey.projection();
    assert_eq!(active.status, JourneyStatus::QuiescentAwaitingInput);
    assert_eq!(active.body_id, born.body_id);
    assert_eq!(active.born_sign_id, born.born_sign_id);
    assert!(active.wake_id.is_some());
    assert!(active.plan_id.is_some());
    assert!(active.active_play_id.is_some());
    encounter
        .journey
        .accept_play_input(key(4, KeyTransition::Pressed))
        .unwrap();
    encounter
        .journey
        .accept_play_input(key(4, KeyTransition::Released))
        .unwrap();
    assert_eq!(encounter.journey.projection().result.as_deref(), Some("A"));
    encounter
        .journey
        .select_next_plot(encounter.journey.revision())
        .unwrap();
    encounter
        .journey
        .accept_play_input(key(5, KeyTransition::Pressed))
        .unwrap();
    encounter
        .journey
        .accept_play_input(key(5, KeyTransition::Released))
        .unwrap();
    assert_eq!(encounter.journey.projection().result.as_deref(), Some("b"));
    encounter
        .journey
        .select_next_plot(encounter.journey.revision())
        .unwrap();
    let returned = encounter.journey.projection();
    assert_eq!(returned.result.as_deref(), Some("A"));
    assert_eq!(returned.active_play_id, active.active_play_id);
    assert_eq!(returned.plan_id, active.plan_id);
    assert_eq!(encounter.start(), Err("post-birth-startup-wrong-state"));
    assert_eq!(encounter.journey.projection(), returned);
}

#[test]
fn missing_keyboard_stops_at_wake_and_retains_the_born_body_with_visible_refusal() {
    let mut encounter = Encounter::born();
    let born = encounter.journey.projection();
    encounter.offer.keyboard = None;
    encounter.start().unwrap();
    let refused = encounter.journey.projection();
    assert_eq!(refused.body_id, born.body_id);
    assert_eq!(refused.born_sign_id, born.born_sign_id);
    assert!(refused.active_play_id.is_none());
    assert!(refused.plan_id.is_none());
    let face = alloc::format!("{:?}", encounter.door.face().unwrap());
    assert!(face.contains("wake:"), "stage-specific refusal: {face}");
}

#[test]
fn restoring_input_requires_explicit_recovery_and_keeps_the_original_birth() {
    let mut encounter = Encounter::born();
    let born = encounter.journey.biography().unwrap().clone();
    let keyboard = encounter.offer.keyboard.take();
    encounter.start().unwrap();
    assert_eq!(encounter.journey.biography(), Some(&born));
    assert_eq!(encounter.journey.status(), JourneyStatus::BornLulled);
    encounter.offer.keyboard = keyboard;
    // Offer restoration alone cannot Wake, Plan or Play a retained Body.
    assert_eq!(encounter.journey.biography(), Some(&born));
    assert!(encounter.journey.projection().active_play_id.is_none());
    encounter.start().unwrap();
    let active = encounter.journey.projection();
    assert_eq!(active.body_id.as_ref(), Some(&born.body.body_id));
    assert_eq!(active.status, JourneyStatus::QuiescentAwaitingInput);
    assert!(active.active_play_id.is_some());
}

#[test]
fn insufficient_admitted_memory_keeps_birth_without_starting_a_partial_workset() {
    let mut encounter = Encounter::born();
    let born = encounter.journey.biography().unwrap().clone();
    encounter.offer = HostOffer::new(&encounter.ids, "build", encounter.offer.cpu_features, 1)
        .with_keyboard(encounter.offer.keyboard.unwrap().realization, "build")
        .unwrap();
    encounter.start().unwrap();
    assert_eq!(encounter.journey.status(), JourneyStatus::BornLulled);
    assert_eq!(encounter.journey.biography(), Some(&born));
    let refused = encounter.journey.projection();
    assert!(refused.plan_id.is_none());
    assert!(refused.active_play_id.is_none());
    let face = alloc::format!("{:?}", encounter.door.face().unwrap());
    assert!(face.contains("wake:"), "stage-specific refusal: {face}");
}

#[test]
fn unavailable_play_and_failed_plan_preserve_birth_and_allow_checked_recovery() {
    let mut encounter = Encounter::born();
    let born = encounter.journey.projection();
    encounter.invoke(PatchbayAction::Play).unwrap();
    assert_eq!(encounter.journey.projection(), born);
    assert!(
        alloc::format!("{:?}", encounter.door.face().unwrap()).contains("Play is not available")
    );
    encounter.invoke(PatchbayAction::Wake).unwrap();
    let awake = encounter.journey.projection();
    let evidence = encounter.journey.biography().unwrap().clone();
    let keyboard = encounter.offer.keyboard.take();
    assert!(encounter.invoke(PatchbayAction::Plan).is_err());
    assert_eq!(encounter.journey.projection(), awake);
    assert_eq!(encounter.journey.biography(), Some(&evidence));
    assert!(!encounter.workspace.active());
    assert!(encounter.presenter.route_keyboard().is_ok());
    encounter.offer.keyboard = keyboard;
    encounter.invoke(PatchbayAction::Plan).unwrap();
    let planned = encounter.journey.projection();
    assert_eq!(planned.body_id, born.body_id);
    assert_eq!(planned.born_sign_id, born.born_sign_id);
    assert_eq!(planned.wake_id, awake.wake_id);
    assert_eq!(planned.plan_id, awake.plan_id);
    assert!(planned.active_play_id.is_none());
    encounter.invoke(PatchbayAction::Play).unwrap();
    let active = encounter.journey.projection();
    assert_eq!(active.body_id, born.body_id);
    assert_eq!(active.plan_id, planned.plan_id);
    assert!(active.active_play_id.is_some());
}

#[test]
fn automatic_remaining_plan_refusal_retains_the_awake_body_and_visible_stage() {
    let mut encounter = Encounter::born();
    encounter.invoke(PatchbayAction::Wake).unwrap();
    assert_eq!(encounter.journey.status(), JourneyStatus::Awake);
    let awake = encounter.journey.projection();
    let evidence = encounter.journey.biography().unwrap().clone();
    let keyboard = encounter.offer.keyboard.take();
    encounter.remaining().unwrap();
    assert_eq!(encounter.journey.projection(), awake);
    assert_eq!(encounter.journey.biography(), Some(&evidence));
    encounter.assert_startup_refused("plan:");
    // Recovery remains explicit and uses the unchanged admitted proposal.
    encounter.offer.keyboard = keyboard;
    encounter.invoke(PatchbayAction::Plan).unwrap();
    encounter.invoke(PatchbayAction::Play).unwrap();
    let active = encounter.journey.projection();
    assert_eq!(active.body_id, awake.body_id);
    assert_eq!(active.born_sign_id, awake.born_sign_id);
    assert_eq!(active.plan_id, awake.plan_id);
    assert!(active.active_play_id.is_some());
}

#[test]
fn prepared_birth_starts_through_acknowledged_face_without_rebirth() {
    use conduit_body::{
        Body, BodyBiographyEvidence, BodyMembership, PreparedBirthMedia, PreparedBirthMediaBinding,
    };
    let mut encounter = Encounter::born();
    let workset = encounter.journey.biography().unwrap().body.workset.clone();
    let body = Body::born_with_plots(workset, 41, "sign/media-birth".into()).unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence = BodyBiographyEvidence::born(body, membership, "Retained Cedar".into()).unwrap();
    let media = PreparedBirthMedia::from_evidence(
        &evidence,
        PreparedBirthMediaBinding {
            host_entry: "first".into(),
            first_host_entry: "first".into(),
            construction_id: "construction/reviewed".into(),
            configuration_id: "configuration/reviewed".into(),
            profile_id: encounter.make.profile_id.into(),
            build_id: encounter.make.build_id.into(),
            target: encounter.make.target.into(),
        },
    )
    .unwrap();
    encounter.journey = fixture().2;
    encounter
        .journey
        .import_prepared_birth(
            &media,
            encounter.make.target,
            encounter.make.profile_id,
            encounter.make.build_id,
        )
        .unwrap();
    // A boot owns a fresh front door. Reusing the previous encounter's Body
    // would correctly trigger its cross-Body continuity refusal.
    let projection = encounter.journey.projection();
    encounter.door = FrontDoor::new(
        projection.host_id.clone(),
        projection.boot_id.clone(),
        projection.offer_generation,
        "profile",
        "build",
        "image",
        projection.source_document_id.clone().unwrap(),
        projection.checked_plot_id.clone().unwrap(),
        7,
        true,
    );
    let provider = crate::product_bases::fixture_surface_provider();
    encounter.workspace = FaceWorkspace::prepare(
        projection.host_id.clone(),
        projection.boot_id.clone(),
        projection.offer_generation,
        "build",
        provider.entry.base_id.clone(),
        &provider,
        conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
    )
    .unwrap();
    encounter.presenter = FrontDoorPresenter::prepare(
        projection.host_id,
        projection.boot_id,
        projection.offer_generation,
        "profile",
        "image",
        provider.entry.base_id.clone(),
        2,
    )
    .unwrap();
    encounter.display = CountingDisplay::default();
    let imported = encounter.journey.biography().unwrap().clone();
    assert_eq!(encounter.journey.status(), JourneyStatus::BornLulled);
    encounter.door.observe_product(&encounter.journey).unwrap();
    encounter
        .presenter
        .present(&encounter.door, &mut encounter.display)
        .unwrap();
    after_prepared_boot(
        &mut encounter.door,
        &mut encounter.journey,
        &mut encounter.presenter,
        &mut encounter.workspace,
        &mut encounter.display,
        &encounter.ids,
        &encounter.offer,
        &encounter.make,
    )
    .unwrap();
    let active = encounter.journey.projection();
    assert_eq!(active.status, JourneyStatus::QuiescentAwaitingInput);
    assert_eq!(active.body_id.as_ref(), Some(&evidence.body_id));
    let retained = encounter.journey.biography().unwrap();
    assert_eq!(retained.friendly_name, evidence.friendly_name);
    assert_eq!(retained.records[0], evidence.records[0]);
    assert_eq!(retained.membership.parts.len(), 1);
    assert_eq!(retained.membership.parts, imported.membership.parts);
    assert!(
        active.wake_id.is_some() && active.plan_id.is_some() && active.active_play_id.is_some()
    );
    let before = encounter.journey.projection();
    assert_eq!(encounter.start(), Err("post-birth-startup-wrong-state"));
    assert_eq!(encounter.journey.projection(), before);
}
