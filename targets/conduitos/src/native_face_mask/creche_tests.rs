//! CPU checked Source/Plan/Mask/scanout proof; actual QEMU remains separate.
use super::*;
use crate::native_face_scene::raster_layout_tests::Pixels;
use conduit_face::{CrecheComponentRecipe, prepare_component_layout};
use conduit_human::{KeyModifiers, KeyTransition};

#[test]
fn checked_creche_masks_admit_three_fields_and_present_both_column_counts() {
    let provider = crate::product_bases::fixture_surface_provider();
    let draft = tests::draft();
    for recipe in [
        CrecheComponentRecipe::SingleColumn,
        CrecheComponentRecipe::ResponsiveColumns,
    ] {
        for (width, height) in [(320, 240), (640, 480), (1280, 800)] {
            let mut mask = NativeFaceMask::prepare_creche(
                "host/creche-layout".into(),
                "boot/creche-layout".into(),
                OfferGeneration(1),
                "creche-layout",
                provider.entry.base_id.clone(),
                "surface/creche-layout",
                &provider,
                recipe,
            )
            .unwrap();
            let face = draft
                .face(&mask.birth_basis("arrival/creche-layout"))
                .unwrap();
            assert_eq!(
                face.subjects
                    .iter()
                    .filter(|subject| subject.role == FaceRole::TextEntry)
                    .count(),
                3
            );
            let configuration = mask.component_configuration().unwrap();
            assert_eq!(
                configuration.implementation_id.as_str(),
                "face/renderer-conduitos-layout-component@1"
            );
            let prepared = prepare_component_layout(
                conduit_core::StructuredInfoValue::from_canonical_bytes(
                    configuration
                        .layout
                        .as_ref()
                        .expect("selected layout contract")
                        .canonical_value(),
                )
                .unwrap(),
                conduit_core::StructuredInfoValue::from_canonical_bytes(
                    configuration.recipe.canonical_value(),
                )
                .unwrap(),
                &face,
            )
            .unwrap();
            assert_eq!(prepared.face(), &face);
            assert_eq!(
                prepared.structure().candidates().len(),
                if recipe == CrecheComponentRecipe::ResponsiveColumns {
                    2
                } else {
                    1
                }
            );
            let columns = if width == 1280 && recipe == CrecheComponentRecipe::ResponsiveColumns {
                2
            } else {
                1
            };
            let mut pixels = Pixels::new(width, height);
            mask.present(face.clone(), 1, 1, &mut pixels).unwrap();
            assert_eq!(mask.component_fallback(), None);
            assert_eq!(mask.show().unwrap().show.face_id, face.identity);
            let pages = mask.scene().unwrap().pages();
            let mut second_column_control = false;
            for page in 0..pages {
                let scene = mask.scene().unwrap();
                assert_eq!(scene.face(), &face);
                let frame = scene.frame().unwrap();
                assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
                second_column_control |= frame
                    .hits
                    .iter()
                    .any(|hit| hit.bounds.x > (width / 2) as i16);
                let result = mask
                    .key(
                        KeyEvent::new(78, KeyTransition::Pressed, KeyModifiers::NONE).unwrap(),
                        page as u64 + 2,
                        &mut pixels,
                    )
                    .unwrap();
                assert!(matches!(result, NativeFaceMaskInput::Redrawn(_)));
            }
            assert_eq!(second_column_control, columns == 2);
        }
    }
}

#[test]
fn full_inventory_fixture_keeps_long_and_empty_names_and_filtered_review_readable() {
    use conduit_birth_plot::{BirthDraft, BirthPlotChoice};
    // Declared inventory fixture, not sixteen installed or executed Plots.
    let choices = (0..conduit_body::MAX_BODY_PLOTS)
        .map(|index| BirthPlotChoice {
            title: alloc::format!("Plot {index:02} {}", "é".repeat(28)),
            search_text: alloc::string::String::new(),
            selected: true,
            refusal: None,
            plot: conduit_body::ResidentPlot::new(
                alloc::format!("source/fixture/{index}").into(),
                alloc::format!("checked/fixture/{index}").into(),
            ),
        })
        .collect();
    let mut draft =
        BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
    let provider = crate::product_bases::fixture_surface_provider();
    for query in ["", "no-match"] {
        draft.search_plots(draft.revision(), query).unwrap();
        for name in ["É".repeat(32), alloc::string::String::new()] {
            draft.edit_name(draft.revision(), name.clone()).unwrap();
            let mut mask = NativeFaceMask::prepare_creche(
                "host/creche-bounds".into(),
                "boot/creche-bounds".into(),
                OfferGeneration(1),
                "creche-bounds",
                provider.entry.base_id.clone(),
                "surface/creche-bounds",
                &provider,
                CrecheComponentRecipe::ResponsiveColumns,
            )
            .unwrap();
            let face = draft
                .face(&mask.birth_basis("arrival/creche-bounds"))
                .unwrap();
            let mut pixels = Pixels::new(320, 240);
            mask.present(face.clone(), 1, 1, &mut pixels).unwrap();
            assert_eq!(mask.component_fallback(), None);
            let mut words = alloc::string::String::new();
            let mut birth_hit = false;
            for page in 0..mask.scene().unwrap().pages() {
                let frame = mask.scene().unwrap().frame().unwrap();
                assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
                for command in frame.scene.commands() {
                    words.push_str(command.payload());
                }
                birth_hit |= frame
                    .hits
                    .iter()
                    .any(|hit| hit.action.identity == "creche.birth");
                mask.key(
                    KeyEvent::new(78, KeyTransition::Pressed, KeyModifiers::NONE).unwrap(),
                    page as u64 + 2,
                    &mut pixels,
                )
                .unwrap();
            }
            assert!(words.contains(&alloc::format!(
                "Friendly Body name: {}",
                if name.is_empty() { "(empty)" } else { &name }
            )));
            for choice in draft.choices() {
                assert!(
                    words.contains(&choice.title),
                    "lost full Plot name with query {query:?}"
                );
            }
            assert_eq!(birth_hit, !name.is_empty());
            assert_eq!(mask.scene().unwrap().face(), &face);
        }
    }
}

#[test]
fn protected_creche_exact_inventory_boolean_and_snapshot_round_trip() {
    use conduit_birth_plot::BirthWidgetFace;
    use conduitos_creche_core::{data, state::State, wire::*};
    fn input(state: &mut State, kind: InputKind, bytes: &[u8], transfer: &mut [u8]) -> Response {
        state
            .execute(
                Request::Begin {
                    kind,
                    total: bytes.len() as u32,
                },
                transfer,
            )
            .unwrap();
        for (index, chunk) in bytes.chunks(TRANSFER_BYTES as usize).enumerate() {
            transfer.fill(0);
            transfer[..chunk.len()].copy_from_slice(chunk);
            state
                .execute(
                    Request::Write {
                        offset: (index * TRANSFER_BYTES as usize) as u32,
                        window_offset: 0,
                        bytes: chunk.len() as u32,
                    },
                    transfer,
                )
                .unwrap();
        }
        state.execute(Request::Commit, transfer).unwrap()
    }
    fn snapshot(state: &mut State, revision: u32, transfer: &mut [u8]) -> data::DraftSnapshot {
        let Response::Blob {
            kind: OutputKind::Snapshot,
            bytes,
            ..
        } = state
            .execute(Request::Snapshot { revision }, transfer)
            .unwrap()
        else {
            panic!("snapshot response")
        };
        let mut encoded = alloc::vec::Vec::new();
        while encoded.len() < bytes as usize {
            let Response::Chunk { bytes: count, .. } = state
                .execute(
                    Request::Read {
                        offset: encoded.len() as u32,
                        maximum: TRANSFER_BYTES,
                    },
                    transfer,
                )
                .unwrap()
            else {
                panic!("snapshot chunk")
            };
            encoded.extend_from_slice(&transfer[..count as usize]);
        }
        data::decode(&encoded).unwrap()
    }
    let choices = crate::native_workset::candidate_inventory()
        .into_iter()
        .map(|plot| {
            let installed = crate::native_workset::profile().installed().contains(&plot);
            conduit_birth_plot::BirthPlotChoice {
                title: plot.title().into(),
                search_text: plot.source().into(),
                plot: crate::native_workset::resident(plot).unwrap(),
                refusal: (!installed).then(|| {
                    crate::native_workset::WorksetRefusal::UnknownPlot
                        .as_str()
                        .into()
                }),
                selected: installed,
            }
        })
        .collect::<alloc::vec::Vec<_>>();
    assert_eq!(choices.len(), 5, "exact current native candidate inventory");
    assert_eq!(choices.iter().filter(|choice| choice.selected).count(), 4);
    let mut draft = conduit_birth_plot::BirthDraft::new(
        "550e8400-e29b-41d4-a716-446655440000".into(),
        choices.clone(),
    )
    .unwrap();
    let mut state = State::default();
    let mut transfer = alloc::vec![0; TRANSFER_BYTES as usize];
    let initialize = data::Initialize {
        uuid: "550e8400-e29b-41d4-a716-446655440000".into(),
        choices,
        allow_idle_body: false,
    };
    assert_eq!(
        input(
            &mut state,
            InputKind::Initialize,
            &data::encode(&initialize).unwrap(),
            &mut transfer
        ),
        Response::Ready {
            revision: draft.revision()
        }
    );
    let provider = crate::product_bases::fixture_surface_provider();
    let mut mask = NativeFaceMask::prepare_creche(
        "host/protected-choice".into(),
        "boot/protected-choice".into(),
        OfferGeneration(1),
        "protected-choice",
        provider.entry.base_id.clone(),
        "surface/protected-choice",
        &provider,
        CrecheComponentRecipe::ResponsiveColumns,
    )
    .unwrap();
    let basis = mask.birth_basis("arrival/protected-choice");
    let mut pixels = Pixels::new(1280, 800);
    for (sequence, expected) in [(1, false), (2, true)] {
        let face = draft.face(&basis).unwrap();
        mask.present(face, sequence, sequence, &mut pixels).unwrap();
        mask.focus_named(
            &crate::native_face_scene::FaceFocusRequest {
                action_id: "creche.plot.0".into(),
                argument_name: None,
            },
            &mut pixels,
        )
        .unwrap();
        let show = mask.show().unwrap().clone();
        let NativeFaceMaskInput::Submitted { correlation, .. } = mask
            .key(
                KeyEvent::new(44, KeyTransition::Pressed, KeyModifiers::NONE).unwrap(),
                sequence,
                &mut pixels,
            )
            .unwrap()
        else {
            panic!("Boolean must submit through the actual Mask")
        };
        assert_eq!(
            correlation.interaction.arguments[0].value,
            alloc::vec![u8::from(expected)]
        );
        let request = data::InteractionRequest {
            revision: draft.revision(),
            basis: basis.clone(),
            show: show.clone(),
            input: correlation.interaction.clone(),
        };
        let encoded =
            data::encode(&request).expect("full checked Show fits unchanged protected input bound");
        let decoded: data::InteractionRequest =
            data::decode(&encoded).expect("typed request codec round trip");
        assert_eq!(decoded.input, request.input);
        assert_eq!(
            input(
                &mut state,
                InputKind::FaceInteraction,
                &encoded,
                &mut transfer
            ),
            Response::Ready {
                revision: draft.revision() + 1
            }
        );
        draft
            .apply_face_interaction(&basis, &show, &correlation.interaction)
            .unwrap();
        let observed = snapshot(&mut state, draft.revision(), &mut transfer);
        assert_eq!(observed.choices, draft.choices());
        assert_eq!(
            observed
                .choices
                .iter()
                .filter(|choice| choice.selected)
                .count(),
            if expected { 4 } else { 3 }
        );
        assert_eq!(observed.widget, draft.widget_face().unwrap());
    }
}
