//! Actual cross-Plot selection and exact portable Source graph correspondence.
use super::*;
use conduit_face::{FacePropertyValue, FaceRole};
use conduitos::native_workset;

fn property<'a>(
    face: &'a Face,
    subject: &str,
    name: &str,
) -> Result<&'a FacePropertyValue, ConduitosError> {
    let mut values = face
        .properties
        .iter()
        .filter(|value| value.subject == subject && value.name == name);
    let value = &values
        .next()
        .ok_or_else(|| refusal("Patchbay Source property absent"))?
        .value;
    if values.next().is_some() {
        return Err(refusal("Patchbay Source property duplicated"));
    }
    Ok(value)
}
fn revision(face: &Face) -> Result<u64, ConduitosError> {
    let mut values = face
        .properties
        .iter()
        .filter(|value| value.name == "application-revision");
    let Some(FacePropertyValue::Count(revision)) = values.next().map(|value| &value.value) else {
        return Err(refusal("Patchbay actor revision absent"));
    };
    if values.next().is_some() {
        return Err(refusal("Patchbay actor revision duplicated"));
    }
    Ok(*revision)
}
pub(super) fn source(record: &Value, target: &str) -> Result<Value, ConduitosError> {
    let face = decode(record)?;
    if property(&face, target, "patchbay-selected")? != &FacePropertyValue::Flag(true) {
        return Err(refusal(
            "actual graph is not the exact selected inventory Plot",
        ));
    }
    let plot = native_workset::inventory()
        .into_iter()
        .find(|plot| {
            native_workset::resident(*plot).is_ok_and(|resident| {
                format!("plot/{}", resident.checked_plot_id.as_str()) == target
            })
        })
        .ok_or_else(|| refusal("inspected Plot is outside the exact installed Source inventory"))?;
    let checked = native_workset::checked(plot).map_err(|error| refusal(error.as_str()))?;
    let plan = match property(&face, target, "observed-plan-id")? {
        FacePropertyValue::Identity(value) => value.clone().into(),
        _ => return Err(refusal("inspected Plot has no exact Plan")),
    };
    let body = face
        .basis
        .body_id
        .as_ref()
        .ok_or_else(|| refusal("Patchbay has no Body"))?;
    let root = face
        .subjects
        .iter()
        .find(|subject| subject.name == "Patchbay" && subject.role == FaceRole::Document)
        .ok_or_else(|| refusal("Patchbay source-owned root absent"))?;
    let body_plan = match property(&face, &root.identity, "body-plan-id")? {
        FacePropertyValue::Identity(value) => value.clone().into(),
        _ => return Err(refusal("Patchbay Body Plan absent")),
    };
    let port = patchbay_application::PatchbayApplicationPort::open(&checked, plan, body_plan)
        .map_err(|error| refusal(format!("{error:?}")))?;
    let basis = conduit_face::FaceContributionBasis {
        checked_plot_id: face
            .basis
            .checked_plot_id
            .clone()
            .ok_or_else(|| refusal("Patchbay owner Source absent"))?,
        plan_id: face
            .basis
            .plan_id
            .clone()
            .ok_or_else(|| refusal("Patchbay owner Plan absent"))?,
        active_play_id: face
            .basis
            .active_play_id
            .clone()
            .ok_or_else(|| refusal("Patchbay owner Play absent"))?,
        required_interaction_context: None,
    };
    let expected = port
        .observe_fragment(basis, body)
        .map_err(|error| refusal(format!("{error:?}")))?;
    let graph_role =
        |role: &FaceRole| matches!(role, FaceRole::Gear | FaceRole::Port | FaceRole::Cord);
    let subjects = expected
        .subjects
        .iter()
        .filter(|subject| graph_role(&subject.role))
        .collect::<Vec<_>>();
    if face
        .subjects
        .iter()
        .filter(|subject| graph_role(&subject.role))
        .count()
        != subjects.len()
        || subjects
            .iter()
            .any(|subject| !face.subjects.contains(subject))
    {
        return Err(refusal(
            "actual graph subjects differ from the exact installed Source",
        ));
    }
    let ids = subjects
        .iter()
        .map(|subject| subject.identity.as_str())
        .collect::<Vec<_>>();
    for value in expected
        .properties
        .iter()
        .filter(|value| ids.contains(&value.subject.as_str()))
    {
        if !face.properties.contains(value) {
            return Err(refusal(
                "actual graph lost exact Source contracts or provenance",
            ));
        }
    }
    for value in expected.relationships.iter().filter(|value| {
        ids.contains(&value.source.as_str()) || ids.contains(&value.target.as_str())
    }) {
        if !face.relationships.contains(value) {
            return Err(refusal(
                "actual graph lost exact Source endpoints or containment",
            ));
        }
    }
    Ok(
        json!({"plot":plot.name(),"source":plot.source(),"checked_plot_id":checked.checked_plot_id,
        "source_document_id":checked.source_document_id,"expanded_plot_id":checked.expanded_plot_id,
        "graph_subjects":subjects,"actor_revision":revision(&face)?,"actual_face":record}),
    )
}
pub(super) fn round_trip(
    serial: &Path,
    child: &mut Child,
    qmp: &mut UnixStream,
    reader: &mut qmp::Reader,
    initial: &Value,
) -> Result<Value, ConduitosError> {
    let face = decode(initial)?;
    let selected = face
        .actions
        .iter()
        .find(|action| {
            action.intent == "patchbay/select-plot@1"
                && property(&face, &action.target, "patchbay-selected")
                    .is_ok_and(|value| *value == FacePropertyValue::Flag(true))
        })
        .ok_or_else(|| refusal("Patchbay selected Plot action absent"))?;
    let other = face
        .actions
        .iter()
        .find(|action| {
            action.intent == "patchbay/select-plot@1" && action.target != selected.target
        })
        .ok_or_else(|| refusal("Patchbay offers no distinct admitted Plot"))?;
    let initial_source = source(initial, &selected.target)?;
    let outward = submit(serial, child, qmp, reader, initial, &other.identity, None)?;
    let next = current(serial)?;
    let next_source = source(&next, &other.target)?;
    let next_face = decode(&next)?;
    let back = next_face
        .actions
        .iter()
        .find(|action| {
            action.intent == "patchbay/select-plot@1" && action.target == selected.target
        })
        .ok_or_else(|| refusal("Patchbay no longer offers its original Plot"))?;
    let inward = submit(serial, child, qmp, reader, &next, &back.identity, None)?;
    let returned = current(serial)?;
    let returned_source = source(&returned, &selected.target)?;
    for receipt in [&outward, &inward] {
        if receipt["before"]["active_play_id"] != receipt["after"]["active_play_id"]
            || receipt["before"]["plan_id"] != receipt["after"]["plan_id"]
            || receipt["before"]["body_id"] != receipt["after"]["body_id"]
        {
            return Err(refusal("graph inspection replaced the admitted Body actor"));
        }
    }
    if next_source["actor_revision"].as_u64()
        != initial_source["actor_revision"]
            .as_u64()
            .and_then(|value| value.checked_add(1))
        || returned_source["actor_revision"].as_u64()
            != next_source["actor_revision"]
                .as_u64()
                .and_then(|value| value.checked_add(1))
    {
        return Err(refusal(
            "cross-Plot selection reset the retained actor revision",
        ));
    }
    Ok(
        json!({"initial":initial_source,"outward":outward,"other":next_source,
        "inward":inward,"returned":returned_source,"same_play":true}),
    )
}
