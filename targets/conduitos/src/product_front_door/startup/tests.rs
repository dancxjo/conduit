//! Native software tests of default startup, distinct from emulator evidence.
use super::*;
use crate::{
    native_workset::{self, NativePlot},
    product_front_door::face_workspace::tests::CountingDisplay,
    product_journey::test_support::{fixture, key},
};
use conduit_birth_plot::BirthSelection;
use conduit_body::BodyWorkset;

struct Encounter {
    ids: BootIdentities,
    offer: HostOffer<'static>,
    journey: ProductJourney,
    door: FrontDoor,
    presenter: FrontDoorPresenter,
    workspace: FaceWorkspace,
    display: CountingDisplay,
    make: MakeRecord,
}

impl Encounter {
    fn born() -> Self {
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
}

#[test]
fn default_startup_keeps_birth_and_listens_with_both_residents_in_one_play() {
    let mut encounter = Encounter::born();
    let born = encounter.journey.projection();
    assert_eq!(born.status, JourneyStatus::BornLulled);
    assert!(born.plan_id.is_none());
    encounter.start().unwrap();
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
