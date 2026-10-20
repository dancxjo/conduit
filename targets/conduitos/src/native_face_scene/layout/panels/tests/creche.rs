//! Actual Source draft and authored recipe; CPU proof, not guest enactment.
use super::*;

#[test]
fn actual_zero_body_creche_places_body_and_plot_groups_in_one_band() {
    use conduit_core::{BootId, CheckedPlotId, HostId, OfferGeneration, SourceDocumentId};
    use conduit_face::*;
    let mut door = crate::front_door::FrontDoor::new(
        HostId::from("host"),
        BootId::from("boot"),
        OfferGeneration(1),
        "profile",
        "build",
        "image",
        SourceDocumentId::from("source"),
        CheckedPlotId::from("checked"),
        7,
        true,
    );
    door.open_creche("00112233-4455-6677-8899-aabbccddeeff".into(), None)
        .unwrap();
    let producer = scene_tests::fixture::producer_plan();
    let face = door
        .creche_face(&conduit_birth_plot::BirthFaceBasis {
            host_id: "host".into(),
            boot_id: "boot".into(),
            encounter_id: "arrival/one".into(),
            producer_plot: conduit_core::PlotIdentity {
                source_document_id: producer.source_document_id,
                checked_plot_id: producer.checked_plot_id,
                expanded_plot_id: producer.expanded_plot_id,
            },
            producer_plan_id: producer.plan_id,
        })
        .unwrap();
    assert!(face.basis.body_id.is_none());
    let provider = scene_tests::fixture::framebuffer_provider_entry();
    let host = conduit_conduitos_mask_offer::native_host_advertisement(
        &"host/native".into(),
        &"boot/native".into(),
        1,
        &provider,
    );
    let (_, configuration) = conduit_conduitos_mask_offer::prepare_creche_stage_from_offer(
        &host,
        CrecheComponentRecipe::ResponsiveColumns,
    )
    .unwrap();
    let recipe = prepare_component_recipe(
        conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.recipe.canonical_value(),
        )
        .unwrap(),
        &face,
    )
    .unwrap();
    let expression = admit_visual_expression(
        Ok(visual_expression_from_info(
            conduit_core::StructuredInfoValue::from_canonical_bytes(
                configuration.expression.canonical_value(),
            )
            .unwrap(),
        )
        .unwrap()),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    let baseline = NativeFaceScene::prepare(face.clone(), 1280, 800).unwrap();
    let mut scene =
        NativeFaceScene::prepare_configured(face.clone(), 1280, 800, true, &recipe, &expression)
            .unwrap();
    assert_eq!(scene.face(), &face);
    assert_eq!(
        scene
            .primary
            .iter()
            .filter_map(|r| r.control)
            .collect::<Vec<_>>(),
        baseline
            .primary
            .iter()
            .filter_map(|r| r.control)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        scene
            .details
            .iter()
            .map(|r| r.text.as_str())
            .collect::<String>(),
        baseline
            .details
            .iter()
            .map(|r| r.text.as_str())
            .collect::<String>()
    );
    let row_for = |action: &str| {
        let target = &face
            .actions
            .iter()
            .find(|a| a.identity == action)
            .unwrap()
            .target;
        let subject = face
            .subjects
            .iter()
            .position(|s| &s.identity == target)
            .unwrap();
        scene
            .primary
            .iter()
            .find(|r| r.subject_index == Some(subject))
            .unwrap()
    };
    for row in &scene.primary {
        let region = row
            .subject_index
            .and_then(|subject| recipe.subject_regions[subject]);
        std::println!(
            "source_row subject={:?} region={region:?} page={} bounds={:?} text={}",
            row.subject_index,
            row.page,
            row.bounds,
            row.text
        );
    }
    let name = row_for("creche.name");
    let naming = row_for("creche.naming");
    let search = row_for("creche.search");
    assert_eq!(name.page, 0);
    assert_eq!(naming.page, name.page);
    assert_eq!(search.page, name.page);
    assert_eq!(name.bounds.x, naming.bounds.x);
    assert!(search.bounds.x > name.bounds.x);
    assert_eq!(
        name.bounds.y, search.bounds.y,
        "actual source fields must start in adjacent lanes"
    );
    let right_subjects: Vec<_> = recipe
        .subject_regions
        .iter()
        .enumerate()
        .filter_map(|(subject, region)| {
            region
                .and_then(|region| recipe.recipe.regions().as_slice().get(region))
                .filter(|region| *region.group() == 1)
                .map(|_| subject)
        })
        .collect();
    assert!(!right_subjects.is_empty());
    for row in &scene.primary {
        if row
            .subject_index
            .is_some_and(|subject| right_subjects.contains(&subject))
        {
            assert!(row.bounds.x >= search.bounds.x);
        }
    }
    assert_disjoint(&scene.primary);
    assert!(
        scene
            .primary
            .windows(2)
            .any(|rows| rows[0].page > rows[1].page)
    );
    let controls: Vec<_> = scene
        .primary
        .iter()
        .filter_map(|row| {
            let control = row.control?;
            face.actions[control.action]
                .availability
                .is_available()
                .then_some((control, row.page))
        })
        .collect();
    for (control, page) in controls {
        assert_eq!(scene.focus_next(true), Some(control));
        assert_eq!(scene.page(), page);
        assert!(
            scene
                .frame()
                .unwrap()
                .hits
                .iter()
                .any(|hit| hit.control == control)
        );
    }
    scene.focus = None;
    for page in 0..scene.pages() {
        scene.page = page;
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
        assert!(scene.primary.iter().filter(|row| row.page == page).count() <= MAX_PAGE_ROWS);
        assert!(frame.scene.commands().iter().all(|command| {
            command.payload().len() <= MAX_GRAPHICS_TEXT_BYTES
                && command.clip_class() == GraphicsClipClass::FullyVisible
        }));
    }
}
