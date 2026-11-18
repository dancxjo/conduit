//! Actual checked Source and CPU scene path; fixture offers are not live SDL,
//! provider initialization, a physical Show or an admission-budget proof.
use super::*;
use conduit_face::*;

fn fixture() -> (PlannedMaskPlot, ConfiguredMaskConfiguration, Face) {
    let subjects = [
        ("fixture/time", "time/civil-observation", "Clock"),
        ("fixture/weather", "weather/conditions", "Weather"),
        ("fixture/image", "media/image", "Picture"),
        ("fixture/other", "fixture/other", "Unmatched meaning"),
    ]
    .into_iter()
    .map(|(identity, role, name)| FaceSubject {
        identity: identity.into(),
        role: FaceRole::Semantic(kind_id(role)),
        name: name.into(),
    })
    .collect();
    let face = Face::new_with_semantics(
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
        subjects,
        vec![],
        [
            ("local-date", "2026-11-17"),
            ("local-time", "19:45:12"),
            ("timezone", "UTC"),
        ]
        .into_iter()
        .map(|(name, value)| FaceProperty {
            subject: "fixture/time".into(),
            name: name.into(),
            value: FacePropertyValue::Text(value.into()),
        })
        .collect(),
        vec![
            FaceText {
                subject: "fixture/time".into(),
                text: "19:45:12".into(),
            },
            FaceText {
                subject: "fixture/other".into(),
                text: "Original fallback information".into(),
            },
        ],
        vec![FaceAction {
            identity: "fixture/activate".into(),
            intent: "fixture/activate".into(),
            target: "fixture/other".into(),
            name: "Activate".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap();
    let provider = BaseProviderEntry {
        base_id: "fixture/display".into(),
        provider_instance_id: "fixture/display/provider".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![ResourceOffer {
            pool_id: "fixture/display/surface".into(),
            class_id: SHOW_RESOURCE_CLASS.into(),
            capacity_units: 1,
            compute: None,
            content: None,
        }],
    };
    let host = conduit_conduitos_mask_offer::native_layout_host_advertisement(
        &"fixture/native".into(),
        &"fixture/boot".into(),
        1,
        &provider,
        NativeMaskStorageProfile::Standard,
    );
    let source = ambient_clock_mask_source_for_face(&face).unwrap();
    let (stage, configuration) = conduit_conduitos_mask_offer::prepare_layout_component_document(
        &host,
        &source,
        "ambient-clock-tile",
        NativeMaskStorageProfile::Standard,
    )
    .unwrap();
    (stage.planned_mask, configuration, face)
}

#[test]
fn configured_native_consumer_keeps_exact_source_startup_and_original_face() {
    let (plan, configuration, face) = fixture();
    validate_binding(&configuration, &plan).unwrap();
    let expected = configured_mask_configuration_backing_bytes(&configuration).unwrap()
        + core::mem::size_of::<ConfiguredMaskConfiguration>();
    assert_eq!(configuration_bytes(Some(&configuration)), Some(expected));
    assert_eq!(configuration_bytes(None), Some(0));
    let prepared = PreparedConfiguredScene::prepare(&configuration, &face).unwrap();
    let mut scene = prepared.scene(640, 480, true).unwrap();
    assert_eq!(scene.face(), &face);
    let first = scene.frame().unwrap();
    assert!(
        first
            .scene
            .commands()
            .iter()
            .any(|command| command.payload().contains("19:45:12"))
    );
    drop(first);
    let mut reading = String::new();
    for _ in 0..scene.pages() {
        reading.extend(
            scene
                .frame()
                .unwrap()
                .scene
                .commands()
                .iter()
                .map(|command| command.payload()),
        );
        scene.turn_page(true);
    }
    assert!(reading.contains("Original fallback information"));
    assert!(reading.contains("Activate"));
    let mut wrong = configuration.clone();
    wrong.placement_id = "fixture/not-the-selected-renderer".into();
    assert!(validate_binding(&wrong, &plan).is_err());
    let mut wrong = configuration.clone();
    wrong.layout = None;
    assert!(validate_binding(&wrong, &plan).is_err());
}

#[test]
fn configured_consumer_refuses_changed_bound_semantic_facts() {
    let (_, configuration, face) = fixture();
    let properties = face
        .properties
        .into_iter()
        .filter(|property| property.name != "timezone")
        .collect();
    let incomplete = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    assert!(PreparedConfiguredScene::prepare(&configuration, &incomplete).is_err());
}

#[path = "configured/retained_tests.rs"]
mod retained_tests;
