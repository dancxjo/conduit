//! Source/recipe binding fixtures only. Parent owner-service tests own the
//! authenticated Show, membership, expiry, Line and rollback lifecycle proofs.
use super::*;
use conduit_core::*;
use conduit_face::*;

fn host() -> HostAdvertisement {
    let provider = BaseProviderEntry {
        base_id: "fixture/history-owner/surface".into(),
        provider_instance_id: "fixture/history-owner/provider".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![resource_offer(
            "fixture/history-owner/surface",
            SHOW_RESOURCE_CLASS,
            1,
        )],
    };
    conduit_conduitos_mask_offer::native_layout_host_advertisement(
        &"host/history-owner/fixture".into(),
        &"boot/history-owner/fixture".into(),
        1,
        &provider,
        conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
    )
}
fn face() -> Face {
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
    Face::new(
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
            name: "History binding fixture".into(),
        }],
        vec![],
        properties,
        vec![],
    )
    .unwrap()
}
#[test]
fn exact_independent_look_units_select_checked_layout_plans_on_one_recipe_bound_face() {
    let host = host();
    let face = face();
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
        let planned = plan(
            &host,
            &face,
            Some(&NativeSelection::History(Box::new(look.clone()))),
        )
        .unwrap();
        let (expected, configured) =
            conduit_conduitos_mask_offer::prepare_history_component_stage_with_look_from_offer(
                &host, &look,
            )
            .unwrap();
        assert_eq!(planned, expected.planned_mask);
        assert_eq!(configured.requested_look().unwrap(), look);
        assert_eq!(planned.mask.plot_name, HISTORY_LAYOUT_MASK_ROOT);
        assert!(ids.insert(planned.plan.plan_id));
        assert_eq!(face, original);
    }
}
#[test]
fn absent_or_mismatched_history_refuses_without_creating_facts_or_changing_default_planning() {
    let host = host();
    let selection = NativeSelection::History(Box::new(editorial_look().unwrap()));
    let mut absent = face();
    absent.subjects[0].identity = "other".into();
    absent.properties.clear();
    absent.reseal_prepared().unwrap();
    assert!(plan(&host, &absent, Some(&selection))
        .unwrap_err()
        .starts_with("native-history-face-unavailable"));
    let mut mismatched = face();
    mismatched
        .properties
        .iter_mut()
        .find(|p| p.name == "clock")
        .unwrap()
        .value = FacePropertyValue::Count(1);
    mismatched.reseal_prepared().unwrap();
    assert!(plan(&host, &mismatched, Some(&selection))
        .unwrap_err()
        .starts_with("native-history-face-unavailable"));
    let expected =
        conduit_conduitos_mask_offer::prepare_advertised_native_stage_from_offer(&host, None)
            .unwrap();
    assert_eq!(plan(&host, &absent, None).unwrap(), expected.planned_mask);
    let todo = NativeSelection::Todo(ComponentMaskLook::Compact, TodoComponentRecipe::Flat);
    let expected = conduit_conduitos_mask_offer::prepare_advertised_native_stage_from_offer(
        &host,
        Some((ComponentMaskLook::Compact.into(), TodoComponentRecipe::Flat)),
    )
    .unwrap();
    assert_eq!(
        plan(&host, &absent, Some(&todo)).unwrap(),
        expected.planned_mask
    );
}
