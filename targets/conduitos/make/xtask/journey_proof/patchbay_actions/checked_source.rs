//! Actual read-only Source selection, bounded graph reading and exact provenance.
use super::*;
use conduit_face::{FaceContributionBasis, FacePropertyValue, FaceRole};
use conduitos::native_workset;

const SOURCE: &str = include_str!("../../../../../../plots/vision/main.conduit");
const GRAPH: &str = include_str!("../../../../application-core/tests/fixtures/vision-graph.json");
const MAX_PAGES: u64 = 128;

pub(super) fn exercise(
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    initial: &Value,
) -> Result<Value, ConduitosError> {
    let initial_face = decode(initial)?;
    let original = initial_face
        .actions
        .iter()
        .find(|action| {
            action.intent == "patchbay/select-plot@1"
                && selection::property(&initial_face, &action.target, "patchbay-selected")
                    .is_ok_and(|value| value == &FacePropertyValue::Flag(true))
        })
        .ok_or_else(|| refusal("checked Source proof lacks its original planned target"))?;
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile)
        .map_err(refusal)?;
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(SOURCE), &startup)
            .map_err(|error| refusal(format!("{error:?}")))?;
    let authored = conduit_plot::expand_canonical_plot_for_authoring(&checked, "vision", &profile)
        .map_err(|error| refusal(format!("{error:?}")))?;
    let internal_cords = authored.expanded.connections.len();
    let witness: Value = serde_json::from_str(GRAPH).map_err(|error| refusal(error.to_string()))?;
    if internal_cords <= 12
        || witness["source_document_id"] != json!(authored.expanded.source_document_id)
        || witness["checked_plot_id"] != json!(authored.expanded.checked_plot_id)
        || witness["expanded_plot_id"] != json!(authored.expanded.expanded_plot_id)
        || witness["cords"].as_array().map(Vec::len)
            != Some(internal_cords + authored.input_bindings.len() + authored.output_bindings.len())
    {
        return Err(refusal(
            "reviewed Source does not match the admitted larger graph witness",
        ));
    }
    let target = format!("plot/{}", authored.expanded.checked_plot_id.as_str());
    let offered = initial_face
        .actions
        .iter()
        .find(|action| action.intent == "patchbay/select-plot@1" && action.target == target)
        .ok_or_else(|| refusal("native product does not offer checked Vision Source inspection"))?;
    let outward = submit(serial, child, qmp, reader, initial, &offered.identity, None)?;
    let selected = current(serial)?;
    let face = decode(&selected)?;
    for (name, expected) in [
        ("patchbay-selected", FacePropertyValue::Flag(true)),
        ("inspected-plan-present", FacePropertyValue::Flag(false)),
        ("inspected-play-present", FacePropertyValue::Flag(false)),
        (
            "inspected-source-document-id",
            FacePropertyValue::Identity(authored.expanded.source_document_id.as_str().into()),
        ),
        (
            "inspected-expanded-plot-id",
            FacePropertyValue::Identity(authored.expanded.expanded_plot_id.as_str().into()),
        ),
        (
            "observed-plot-state",
            FacePropertyValue::Text("checked-source".into()),
        ),
    ] {
        if selection::property(&face, &target, name)? != &expected {
            return Err(refusal(format!("checked Source differs at {name}")));
        }
    }
    if face
        .actions
        .iter()
        .any(|action| action.intent == "patchbay/edit-subject@1")
        || face.properties.iter().any(|property| {
            property.subject == target
                && matches!(
                    property.name.as_str(),
                    "observed-plan-id" | "observed-play-id"
                )
        })
        || face.relationships.iter().any(|relationship| {
            relationship.target == target
                && relationship.source.starts_with("body/")
                && relationship.kind == conduit_face::FaceRelationshipKind::Contains
        })
    {
        return Err(refusal(
            "read-only Source acquired realization, membership or Edit authority",
        ));
    }
    let expected = expected_fragment(&initial_face, &face, &original.target, witness)?;
    let subjects = selection::compare_graph(&face, &expected)?;
    let directory = serial
        .parent()
        .ok_or_else(|| refusal("Source proof lacks evidence directory"))?
        .join("patchbay-checked-source");
    fs::create_dir(&directory).map_err(io_error)?;
    let mut captures = Vec::with_capacity(2 * MAX_PAGES as usize + 4);
    capture(
        qmp,
        reader,
        &directory,
        "source-selected",
        &selected,
        &mut captures,
    )?;
    let action_count = records(serial, ACTION)?.len();
    let reading = read_key(serial, child, qmp, reader, "f3", 60, &selected)?;
    let diagram = continuation(
        serial,
        child,
        qmp,
        reader,
        &directory,
        &selected,
        reading,
        &mut captures,
    )?;
    let details = role_details(
        serial,
        child,
        qmp,
        reader,
        &directory,
        &selected,
        &face,
        subjects.len(),
        &mut captures,
    )?;
    // Details -> diagram -> ordinary controls, retaining this exact Face/Show.
    let diagram_return = read_key(serial, child, qmp, reader, "f3", 60, &selected)?;
    if diagram_return["diagram"] != true {
        return Err(refusal(
            "Source Details failed to return to the real diagram",
        ));
    }
    let ordinary_return = read_key(serial, child, qmp, reader, "f3", 60, &selected)?;
    // Diagram and Details are separate reading flags. Explicitly close retained
    // Details after leaving the diagram; this is a mode transition, not a retry.
    let ordinary_return = if ordinary_return["details"] == true {
        read_key(serial, child, qmp, reader, "f2", 59, &selected)?
    } else {
        ordinary_return
    };
    if ordinary_return["diagram"] != false || ordinary_return["details"] != false {
        return Err(refusal(
            "Source diagram failed to return to offered controls",
        ));
    }
    if records(serial, ACTION)?.len() != action_count {
        return Err(refusal(
            "Source reading unexpectedly emitted a semantic action",
        ));
    }
    let back = face
        .actions
        .iter()
        .find(|action| {
            action.intent == "patchbay/select-plot@1" && action.target == original.target
        })
        .ok_or_else(|| refusal("Source inspector lost its original planned target"))?;
    let inward = submit(serial, child, qmp, reader, &selected, &back.identity, None)?;
    let returned = current(serial)?;
    let returned_source = selection::source(&returned, &original.target)?;
    if selection::revision(&face)?
        != selection::revision(&initial_face)?
            .checked_add(1)
            .ok_or_else(|| refusal("Source proof actor revision overflow"))?
        || selection::revision(&decode(&returned)?)?
            != selection::revision(&face)?
                .checked_add(1)
                .ok_or_else(|| refusal("Source proof actor revision overflow"))?
    {
        return Err(refusal(
            "Source selection reset or skipped the retained actor revision",
        ));
    }
    for receipt in [&outward, &inward] {
        for field in ["body_id", "plan_id", "active_play_id"] {
            if receipt["before"][field].is_null()
                || receipt["before"][field] != receipt["after"][field]
            {
                return Err(refusal(
                    "read-only Source inspection replaced its owner Body/Plan/Play",
                ));
            }
        }
    }
    let packet = json!({"schema":"conduit.conduitos/checked-source-inspection@1",
        "source_path":"plots/vision/main.conduit", "source":SOURCE,
        "source_document_id":authored.expanded.source_document_id,
        "checked_plot_id":authored.expanded.checked_plot_id,
        "expanded_plot_id":authored.expanded.expanded_plot_id,
        "inspected_plan":null,"inspected_play":null,"internal_cords":internal_cords,
        "boundary_bindings":authored.input_bindings.len()+authored.output_bindings.len(),
        "expected_graph_subjects":subjects,"outward":outward,"selected_face":selected,
        "diagram_continuation":diagram,"details":details,"captures":captures,
        "returned_to_diagram":diagram_return,"returned_to_controls":ordinary_return,
        "inward":inward,"returned":returned_source,"same_owner_play":true,
        "limits":"real offered Source selection and acknowledged reading; diagrams retain explicit omitted links; Details captures show one exact Gear, Port and Cord focus; no inspected execution, Edit, Save, model or physical proof"});
    fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec_pretty(&packet).map_err(|error| refusal(error.to_string()))?,
    )
    .map_err(io_error)?;
    Ok(packet)
}

fn expected_fragment(
    initial: &Face,
    selected: &Face,
    original: &str,
    witness: Value,
) -> Result<conduit_face::FaceFragment, ConduitosError> {
    let resident = native_workset::inventory()
        .into_iter()
        .find(|plot| {
            native_workset::resident(*plot).is_ok_and(|resident| {
                format!("plot/{}", resident.checked_plot_id.as_str()) == original
            })
        })
        .ok_or_else(|| refusal("initial real resident Source absent"))?;
    let expanded = native_workset::checked(resident).map_err(|error| refusal(error.as_str()))?;
    let plan = match selection::property(initial, original, "observed-plan-id")? {
        FacePropertyValue::Identity(plan) => plan.clone().into(),
        _ => return Err(refusal("initial real resident Plan absent")),
    };
    let root = initial
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Document && subject.name == "Patchbay")
        .ok_or_else(|| refusal("initial inspector root absent"))?;
    let body_plan = port_body_plan(initial, &root.identity)?;
    let mut port =
        patchbay_application::PatchbayApplicationPort::open(&expanded, plan, body_plan.clone())
            .and_then(|port| port.with_prepared_graph_capacity(2))
            .map_err(|error| refusal(format!("{error:?}")))?;
    let index = port
        .append_prepared_source_graph(
            serde_json::from_value(witness).map_err(|error| refusal(error.to_string()))?,
            &body_plan,
        )
        .map_err(|error| refusal(format!("{error:?}")))?;
    port.select_prepared_plot(index)
        .map_err(|error| refusal(format!("{error:?}")))?;
    let basis = FaceContributionBasis {
        checked_plot_id: selected
            .basis
            .checked_plot_id
            .clone()
            .ok_or_else(|| refusal("owner Source absent"))?,
        plan_id: selected
            .basis
            .plan_id
            .clone()
            .ok_or_else(|| refusal("owner Plan absent"))?,
        active_play_id: selected
            .basis
            .active_play_id
            .clone()
            .ok_or_else(|| refusal("owner Play absent"))?,
        required_interaction_context: None,
    };
    port.observe_fragment(
        basis,
        selected
            .basis
            .body_id
            .as_ref()
            .ok_or_else(|| refusal("owner Body absent"))?,
    )
    .map_err(|error| refusal(format!("{error:?}")))
}
fn port_body_plan(face: &Face, root: &str) -> Result<conduit_core::PlanId, ConduitosError> {
    match selection::property(face, root, "body-plan-id")? {
        FacePropertyValue::Identity(plan) => Ok(plan.clone().into()),
        _ => Err(refusal("actual inspecting Body Plan absent")),
    }
}

#[allow(clippy::too_many_arguments)]
fn continuation(
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    directory: &Path,
    prior: &Value,
    mut reading: Value,
    captures: &mut Vec<Value>,
) -> Result<Value, ConduitosError> {
    let pages = reading["pages"]
        .as_u64()
        .filter(|pages| (2..=MAX_PAGES).contains(pages))
        .ok_or_else(|| refusal("larger Source diagram does not expose bounded continuation"))?;
    if reading["diagram"] != true || reading["page"] != 0 {
        return Err(refusal("larger Source diagram did not begin at page zero"));
    }
    let mut forward = Vec::with_capacity(pages as usize);
    for page in 0..pages {
        if reading["diagram"] != true || reading["page"] != page || reading["pages"] != pages {
            return Err(refusal("Source diagram skipped a forward page"));
        }
        capture(
            qmp,
            reader,
            directory,
            &format!("diagram-forward-{page:03}"),
            &reading,
            captures,
        )?;
        forward.push(reading.clone());
        if page + 1 < pages {
            reading = read_key(serial, child, qmp, reader, "pgdn", 78, prior)?;
        }
    }
    let mut backward = Vec::with_capacity(pages as usize - 1);
    for page in (0..pages - 1).rev() {
        reading = read_key(serial, child, qmp, reader, "pgup", 75, prior)?;
        if reading["diagram"] != true || reading["page"] != page || reading["pages"] != pages {
            return Err(refusal("Source diagram skipped a backward page"));
        }
        capture(
            qmp,
            reader,
            directory,
            &format!("diagram-backward-{page:03}"),
            &reading,
            captures,
        )?;
        backward.push(reading.clone());
    }
    Ok(json!({"pages":pages,"forward":forward,"backward":backward,
        "complete_page_round_trip":true,"all_cords_simultaneously_visible":false,
        "omission_policy":"ordinary diagram budget and explicit notices retained; complete exact subjects remain in acknowledged Details"}))
}

#[allow(clippy::too_many_arguments)]
fn role_details(
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    directory: &Path,
    prior: &Value,
    face: &Face,
    count: usize,
    captures: &mut Vec<Value>,
) -> Result<Value, ConduitosError> {
    let expected = face
        .subjects
        .iter()
        .filter(|subject| {
            matches!(
                subject.role,
                FaceRole::Gear | FaceRole::Port | FaceRole::Cord
            )
        })
        .collect::<Vec<_>>();
    if count == 0 || count > 128 || count != expected.len() {
        return Err(refusal(
            "Source graph navigation exceeds the finite proof subject bound",
        ));
    }
    let mut readings = Vec::with_capacity(count);
    let mut seen = [false; 3];
    for subject in expected {
        let reading = read_key(serial, child, qmp, reader, "tab", 43, prior)?;
        if reading["diagram"] != false
            || reading["details"] != true
            || reading["graph_focus"] != subject.identity
        {
            return Err(refusal(
                "Source Details navigation differs from exact portable subject order",
            ));
        }
        let role = match subject.role {
            FaceRole::Gear => 0,
            FaceRole::Port => 1,
            FaceRole::Cord => 2,
            _ => unreachable!(),
        };
        if !seen[role] {
            capture(
                qmp,
                reader,
                directory,
                ["details-gear", "details-port", "details-cord"][role],
                &reading,
                captures,
            )?;
            seen[role] = true;
        }
        readings.push(reading);
    }
    if !seen.into_iter().all(|seen| seen) {
        return Err(refusal(
            "Source Details did not visit Gear, Port and Cord roles",
        ));
    }
    Ok(json!({"subject_count":count,"readings":readings,
        "all_subjects_reached_in_source_order":true,
        "capture_scope":"one exact focused Gear, Port and Cord; full subject facts retained in checked Face; not every Details page photographed"}))
}

fn capture(
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    directory: &Path,
    name: &str,
    reading: &Value,
    captures: &mut Vec<Value>,
) -> Result<(), ConduitosError> {
    if captures.len() >= 2 * MAX_PAGES as usize + 4 {
        return Err(refusal(
            "checked Source proof exceeded its admitted capture bound",
        ));
    }
    let (image, health) = super::super::super::qmp_display::capture(qmp, reader, directory, name)?;
    if let Some(error) = health {
        return Err(error);
    }
    captures.push(json!({"checkpoint":name,"reading":reading,"image":image}));
    Ok(())
}
