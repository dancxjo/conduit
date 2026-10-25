//! Pure history recipe fixture; the caller supplies actual admitted carrier offers.
use super::*;
use conduit_face::*;
pub(in crate::durable_host::owner::controller::participants::service) fn assert_history_selection_plans(
    owner: &conduit_core::HostAdvertisement,
    browser: &conduit_core::HostAdvertisement,
    evidence: &BrowserCarrierLineEvidence,
) {
    // The generic admission fixture has only the ordinary Back. Positive
    // history planning below is an isolated expanded-offer CPU fixture, not
    // a mutation of the admitted browser or evidence of live authorization.
    let mut expanded_offer = browser.clone();
    let configured = conduit_browser_mask_offer::configured_offer();
    if !expanded_offer.capabilities.contains(&configured) {
        expanded_offer.capabilities.push(configured);
    }
    let browser = &expanded_offer;
    use FacePropertyValue as V;
    let subject = "environment-history";
    let properties = [
        ("schema", V::Text("environment/history@1".into())),
        ("clock", V::Text("producer-boot-ms".into())),
        ("producer-host", V::Identity("fixture/source-host".into())),
        ("producer-boot", V::Identity("fixture/source-boot".into())),
        ("producer-plan", V::Identity("fixture/source-plan".into())),
        ("producer-play", V::Identity("fixture/source-play".into())),
        ("visible-samples", V::Count(0)),
        ("samples-outside-view", V::Count(0)),
    ]
    .into_iter()
    .map(|(name, value)| FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    })
    .collect();
    let face = Face::new(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: subject.into(),
            role: FaceRole::Collection,
            name: "History recipe fixture".into(),
        }],
        vec![],
        properties,
        vec![],
    )
    .unwrap();
    let original = face.clone();
    let base = editorial_look().unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for unit in [
        LookTemperatureUnit::Celsius,
        LookTemperatureUnit::Kelvin,
        LookTemperatureUnit::Fahrenheit,
    ] {
        let look = LookInfo::new(
            base.generative().clone(),
            base.identity().into(),
            LookQuantityFacetOption::some(unit).unwrap(),
            base.spoken().clone(),
            3,
            base.visual().clone(),
        )
        .unwrap();
        let selected = BrowserMaskSelection::History(Box::new(look.clone()));
        let planned = plan(Some(&selected), browser, owner, evidence, &face).unwrap();
        let expected =
            conduit_browser_mask_offer::planned_owner_face_show_interaction_mask_with_look(
                browser,
                owner,
                &evidence.face,
                &evidence.returned,
                &evidence.interaction,
                HISTORY_COMPONENT_MASK_SOURCE,
                HISTORY_COMPONENT_MASK_ROOT,
                &look,
            )
            .unwrap();
        assert_eq!(planned, expected);
        assert!(ids.insert(planned.plan.plan_id));
        assert_eq!(face, original);
        let mut wrong = face.clone();
        wrong.subjects[0].role = FaceRole::Info;
        wrong.reseal_prepared().unwrap();
        assert!(plan(Some(&selected), browser, owner, evidence, &wrong).is_err());
    }
}
