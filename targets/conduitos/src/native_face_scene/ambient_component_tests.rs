//! Checked component/Look → selected native Back → actual CPU geometry.
//! Fixture offers are not live providers, a Show acknowledgement, or photo rendering.
use super::*;
use alloc::{collections::BTreeSet, vec};
use conduit_conduitos_mask_offer::{
    NativeMaskStorageProfile, native_layout_host_advertisement, prepare_layout_component_document,
};
use conduit_core::{
    BoundedResourceRef, CheckedValueContract, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, StructuredInfoValue, kind_id,
};
use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;

const CLOCK: &str = "ambient/test/time";
const WEATHER: &str = "ambient/test/weather";
const IMAGE: &str = "ambient/test/image";

fn refresh(face: Face) -> Face {
    Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap()
}

fn fixture() -> Face {
    let mut face = tests::face(8);
    for (identity, role, name) in [
        (CLOCK, "time/civil-observation", "Date and time"),
        (WEATHER, "weather/conditions", "Weather"),
        (IMAGE, "media/image", "Photograph"),
        (
            "ambient/test/unmatched",
            "test/unmatched-domain@1",
            "An unmatched observation",
        ),
    ] {
        face.subjects.push(FaceSubject {
            identity: identity.into(),
            role: FaceRole::Semantic(kind_id(role)),
            name: name.into(),
        });
    }
    for (subject, name, value) in [
        (CLOCK, "local-date", "2026-11-17"),
        (CLOCK, "local-time", "19:45:12"),
        (CLOCK, "timezone", "America/Los_Angeles"),
        (WEATHER, "availability", "fresh"),
        (WEATHER, "location", "Seattle"),
        (WEATHER, "conditions", "Cloudy"),
        (WEATHER, "attribution", "Fixture weather source"),
        (IMAGE, "attribution", "Fixture photographer"),
        (IMAGE, "license", "Fixture license"),
    ] {
        face.properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value: FacePropertyValue::Text(value.into()),
        });
    }
    for (subject, name, value) in [
        (CLOCK, "observed-at-unix-ms", 1_794_975_912_000),
        (WEATHER, "observed-at-unix-ms", 1_794_975_900_000),
        (WEATHER, "usable-until-unix-ms", 1_794_976_200_000),
    ] {
        face.properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value: FacePropertyValue::Count(value),
        });
    }
    face.properties.push(FaceProperty {
        subject: CLOCK.into(),
        name: "clock-basis".into(),
        value: FacePropertyValue::Identity("time/unix-utc@1".into()),
    });
    face.properties.push(FaceProperty {
        subject: WEATHER.into(),
        name: "temperature-celsius-milli".into(),
        value: FacePropertyValue::Signed(8_400),
    });
    for (subject, text) in [
        (
            CLOCK,
            "2026-11-17 · 19:45:12 · America/Los_Angeles (UTC−08:00)",
        ),
        (WEATHER, "Seattle: 8.4 °C, cloudy. Weather is current."),
        (WEATHER, "Weather source: Fixture weather source."),
        (
            IMAGE,
            "A landscape with complete photographer and license attribution.",
        ),
        (
            "ambient/test/unmatched",
            "This valid unfamiliar meaning must remain reachable.",
        ),
    ] {
        face.text.push(FaceText {
            subject: subject.into(),
            text: text.into(),
        });
    }
    let image = ImageResource::new(
        ImageColorProfile::named("srgb".into()).unwrap(),
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: kind_id("media/image-rgba8@1"),
            access_class: ResourceClassId::from("conduit.resource/image-content@1"),
            extent: ResourceExtent {
                bytes: 16,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        },
        ImagePixelExtent::new(2, 2).unwrap(),
        ImageFormat::rgba8(),
        "fixture/image".into(),
    )
    .unwrap()
    .into_structured()
    .unwrap();
    face.properties.push(FaceProperty {
        subject: IMAGE.into(),
        name: "image".into(),
        value: FacePropertyValue::TypedValue {
            contract: CheckedValueContract::new(
                image.value_type().profile().unwrap().value_kind().clone(),
                MAX_FACE_VALUE_BYTES,
                vec![],
            )
            .unwrap(),
            bytes: image.canonical_bytes().unwrap(),
        },
    });
    refresh(face)
}

fn configuration(root: &str) -> ConfiguredMaskConfiguration {
    let source = ambient_clock_mask_source(CLOCK, WEATHER, IMAGE).unwrap();
    configuration_for_source(&source, root)
}

fn configuration_for_source(source: &str, root: &str) -> ConfiguredMaskConfiguration {
    let provider = tests::fixture::framebuffer_provider_entry();
    let profile = NativeMaskStorageProfile::Standard;
    let host = native_layout_host_advertisement(
        &"host/ambient-component-fixture".into(),
        &"boot/ambient-component-fixture".into(),
        1,
        &provider,
        profile,
    );
    let (stage, configuration) =
        prepare_layout_component_document(&host, source, root, profile).unwrap();
    assert_eq!(stage.planned_mask.mask.plot_name, root);
    assert!(configuration.layout.is_some());
    assert_eq!(
        configuration.implementation_id.as_str(),
        "face/renderer-conduitos-layout-component@1"
    );
    configuration
}

fn structure<'a>(
    configuration: &ConfiguredMaskConfiguration,
    face: &'a Face,
) -> Result<PreparedComponentLayout<'a>, ComponentLayoutRefusal> {
    prepare_component_layout(
        StructuredInfoValue::from_canonical_bytes(
            configuration.layout.as_ref().unwrap().canonical_value(),
        )
        .unwrap(),
        StructuredInfoValue::from_canonical_bytes(configuration.recipe.canonical_value()).unwrap(),
        face,
    )
}

fn look(configuration: &ConfiguredMaskConfiguration, scale: u16) -> AdmittedVisualLook {
    admit_visual_look(
        &configuration.requested_look().unwrap(),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: scale,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap()
}

#[test]
fn reviewed_picture_frame_component_reaches_real_native_layout_and_preserves_fallback() {
    let face = fixture();
    for root in ["ambient-clock-mask", "ambient-clock-mask-compact"] {
        let configuration = configuration(root);
        let structure = structure(&configuration, &face).unwrap();
        let selected = look(&configuration, 100);
        for (width, height) in [(1024, 600), (800, 600), (640, 480)] {
            let mut scene = NativeFaceScene::prepare_structured_look(
                &structure, width, height, true, &selected,
            )
            .unwrap();
            let first_page = scene
                .primary
                .iter()
                .filter(|row| row.page == 0)
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                first_page.contains("19:45:12"),
                "{root} {width}×{height}: {first_page}"
            );
            assert!(
                first_page.contains("Seattle: 8.4"),
                "{root} {width}×{height}: {first_page}"
            );
            let meaning = scene
                .primary
                .iter()
                .chain(&scene.details)
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(meaning.contains("unfamiliar meaning"));
            assert!(meaning.contains("Fixture weather source"));
            assert!(meaning.contains("photographer and license"));
            let baseline =
                NativeFaceScene::prepare_visual_look(face.clone(), width, height, true, &selected)
                    .unwrap();
            let controls = |scene: &NativeFaceScene| {
                scene
                    .primary
                    .iter()
                    .chain(&scene.details)
                    .filter_map(|row| {
                        row.control
                            .map(|control| (control.action, control.argument))
                    })
                    .collect::<BTreeSet<_>>()
            };
            assert_eq!(controls(&scene), controls(&baseline));
            for page in 0..scene.pages() {
                scene.page = page;
                let frame = scene.frame().unwrap();
                assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
                for command in frame.scene.commands() {
                    assert!(command.bounds.x >= 0 && command.bounds.y >= 0);
                    assert!(
                        i32::from(command.bounds.x) + i32::from(command.bounds.width)
                            <= i32::from(width)
                    );
                    assert!(
                        i32::from(command.bounds.y) + i32::from(command.bounds.height)
                            <= i32::from(height)
                    );
                }
            }
            assert_eq!(scene.face(), &face);
        }
    }
}

#[test]
fn semantic_bound_slots_refuse_missing_or_wrong_original_facts() {
    let configuration = configuration("ambient-clock-mask");
    let mut missing = fixture();
    missing.subjects.retain(|subject| subject.identity != CLOCK);
    missing
        .properties
        .retain(|property| property.subject != CLOCK);
    missing.text.retain(|text| text.subject != CLOCK);
    assert!(structure(&configuration, &refresh(missing)).is_err());
    let mut wrong = fixture();
    wrong
        .subjects
        .iter_mut()
        .find(|subject| subject.identity == CLOCK)
        .unwrap()
        .role = FaceRole::Info;
    assert!(structure(&configuration, &refresh(wrong)).is_err());
    let mut wrong_property = fixture();
    wrong_property
        .properties
        .iter_mut()
        .find(|property| property.subject == CLOCK && property.name == "local-time")
        .unwrap()
        .value = FacePropertyValue::Count(19);
    assert!(structure(&configuration, &refresh(wrong_property)).is_err());
}

#[test]
fn image_slot_pins_the_existing_image_owner_profile() {
    let configuration = configuration("ambient-clock-mask");
    let recipe = ComponentRecipeInfo::from_structured(
        StructuredInfoValue::from_canonical_bytes(configuration.recipe.canonical_value()).unwrap(),
    )
    .unwrap();
    let ComponentSubjectSelector::Bound(binding) = recipe.regions().as_slice()[2].selector() else {
        panic!("exact image slot")
    };
    assert_eq!(
        binding.properties().as_slice()[0].value_kind(),
        ImageResource::semantic_type()
            .unwrap()
            .profile()
            .unwrap()
            .value_kind()
            .as_str()
    );
}

#[test]
fn unavailable_weather_keeps_the_checked_component_and_truthful_message() {
    let configuration = configuration("ambient-clock-mask");
    let mut face = fixture();
    face.properties
        .retain(|property| property.subject != WEATHER);
    face.properties.push(FaceProperty {
        subject: WEATHER.into(),
        name: "availability".into(),
        value: FacePropertyValue::Text("unavailable".into()),
    });
    face.text.retain(|text| text.subject != WEATHER);
    face.text.push(FaceText {
        subject: WEATHER.into(),
        text: "Seattle weather is unavailable.".into(),
    });
    let face = refresh(face);
    let structure = structure(&configuration, &face).unwrap();
    let selected = look(&configuration, 100);
    let scene =
        NativeFaceScene::prepare_structured_look(&structure, 800, 600, true, &selected).unwrap();
    let reading = scene
        .primary
        .iter()
        .chain(&scene.details)
        .map(|row| row.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(reading.contains("Seattle weather is unavailable"));
    assert!(!reading.contains("Seattle: 8.4"));
    assert_eq!(scene.face(), &face);
}

#[test]
fn explicit_source_bindings_are_escaped_once_and_cannot_rebind_other_slots() {
    let subjects = ["clock/weather", "wallpaper/image", "clock/date-time"];
    let source = ambient_clock_mask_source(subjects[0], subjects[1], subjects[2]).unwrap();
    let configuration = configuration_for_source(&source, "ambient-clock-mask");
    let recipe = ComponentRecipeInfo::from_structured(
        StructuredInfoValue::from_canonical_bytes(configuration.recipe.canonical_value()).unwrap(),
    )
    .unwrap();
    for (region, expected) in recipe.regions().iter().zip(subjects) {
        let ComponentSubjectSelector::Bound(binding) = region.selector() else {
            panic!("original bound slot")
        };
        assert_eq!(binding.subject(), expected);
    }
    let escaped = "original/\"quoted\"\\subject";
    let source = ambient_clock_mask_source(escaped, WEATHER, IMAGE).unwrap();
    let configuration = configuration_for_source(&source, "ambient-clock-mask");
    let recipe = ComponentRecipeInfo::from_structured(
        StructuredInfoValue::from_canonical_bytes(configuration.recipe.canonical_value()).unwrap(),
    )
    .unwrap();
    let ComponentSubjectSelector::Bound(binding) = recipe.regions().as_slice()[0].selector() else {
        panic!("original bound slot")
    };
    assert_eq!(binding.subject(), escaped);
    assert_eq!(
        ambient_clock_mask_source(CLOCK, CLOCK, IMAGE),
        Err(AmbientClockComponentRefusal::DuplicateSubject)
    );
    assert_eq!(
        ambient_clock_mask_source("", WEATHER, IMAGE),
        Err(AmbientClockComponentRefusal::SubjectBound)
    );
    assert_eq!(
        ambient_clock_mask_source(&"x".repeat(257), WEATHER, IMAGE),
        Err(AmbientClockComponentRefusal::SubjectBound)
    );
}

#[test]
fn tile_and_fullscreen_context_choose_distinct_compositions_over_the_same_face() {
    let face = fixture();
    let identity = face.identity.clone();
    let basis = face.basis.clone();
    let mut recipes = BTreeSet::new();
    for (context, width, height) in [("tile", 320, 320), ("fullscreen", 1024, 600)] {
        let encounter = AmbientClockEncounter::try_from(context).unwrap();
        let configuration = configuration(encounter.root());
        let structure = structure(&configuration, &face).unwrap();
        recipes.insert(structure.recipe().recipe.identity().clone());
        let selected = look(&configuration, 100);
        let mut scene =
            NativeFaceScene::prepare_structured_look(&structure, width, height, true, &selected)
                .unwrap();
        assert_eq!(scene.face().identity, identity);
        assert_eq!(scene.face().basis, basis);
        assert_eq!(scene.face(), &face);
        let first_page = scene
            .primary
            .iter()
            .filter(|row| row.page == 0)
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(first_page.contains("19:45:12"), "{context}: {first_page}");
        let reading = scene
            .primary
            .iter()
            .chain(&scene.details)
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(reading.contains("Seattle: 8.4"));
        assert!(reading.contains("unfamiliar meaning"));
        for page in 0..scene.pages() {
            scene.page = page;
            assert!(scene.frame().unwrap().scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
        }
    }
    assert_eq!(recipes.len(), 2);
    for unsupported in ["", "inspector", "spoken", "tile fullscreen", "Tile"] {
        assert_eq!(
            AmbientClockEncounter::try_from(unsupported),
            Err(AmbientClockComponentRefusal::UnsupportedEncounter)
        );
    }
}

#[test]
fn authored_clock_components_check_before_host_planning() {
    let face = fixture();
    let source = ambient_clock_mask_source(CLOCK, WEATHER, IMAGE).unwrap();
    for component in ["ambient-clock-component", "ambient-clock-tile-component"] {
        let prepared = super::structured_source_tests::structure(&source, component, &face);
        assert_eq!(prepared.face(), &face);
    }
}

#[test]
fn short_tile_keeps_clock_content_together_or_refuses_geometry() {
    let face = fixture();
    let configuration = configuration(AmbientClockEncounter::Tile.root());
    let structure = structure(&configuration, &face).unwrap();
    let selected = look(&configuration, 100);
    match NativeFaceScene::prepare_structured_look(&structure, 320, 240, true, &selected) {
        Ok(scene) => {
            let first_page = scene
                .primary
                .iter()
                .filter(|row| row.page == 0)
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(first_page.contains("19:45:12"), "{first_page}");
            assert_eq!(scene.face(), &face);
        }
        Err(FaceSceneError::InvalidExtent) => {}
        Err(error) => panic!("unexpected tile refusal: {error:?}"),
    }
}

#[test]
fn picture_frame_draws_photo_and_local_weather_and_preserves_every_reader_page() {
    use conduit_core::{Quantity, TEMPERATURE_INFO_ID, Unit};
    let mut face = fixture();
    let value = Quantity::from_decimal(8400, -3, Unit::Celsius)
        .unwrap()
        .encode();
    face.properties.push(FaceProperty {
        subject: WEATHER.into(),
        name: "temperature".into(),
        value: FacePropertyValue::TypedValue {
            contract: CheckedValueContract::new(
                kind_id(TEMPERATURE_INFO_ID),
                value.len() as u32,
                vec![],
            )
            .unwrap(),
            bytes: value.to_vec(),
        },
    });
    let face = refresh(face);
    let expression = admit_visual_expression(
        Ok(VisualExpression::phosphor()),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    for (width, height) in [(1024, 600), (640, 480)] {
        let mut scene = NativeFaceScene::prepare_visual_fallback_with_temperature(
            face.clone(),
            width,
            height,
            true,
            &expression,
            Some(Unit::Fahrenheit),
        )
        .unwrap();
        let previous = scene.pages();
        scene.enable_picture_frame().unwrap();
        scene.enable_picture_frame().unwrap();
        assert_eq!(scene.pages(), previous + 1);
        let frame = scene.frame().unwrap();
        let commands = frame.scene.commands();
        assert_eq!(
            commands[0].image_key(),
            Some(GraphicsImageKey::new([1; 32], [2; 32]).unwrap())
        );
        assert_eq!(commands[0].bounds, scene.bounds());
        assert_eq!(commands[0].clip, scene.bounds());
        for value in ["2026-11-17", "19:45:12", "Seattle: 47.1 °F, Cloudy."] {
            assert!(commands.iter().any(|c| c.payload() == value), "{value}");
        }
        assert!(frame.hits.is_empty());
        scene.turn_page(true);
        assert_eq!(scene.frame().unwrap().page, 1);
        scene.show_details(true);
        assert_eq!(scene.face(), &face);
    }
}
