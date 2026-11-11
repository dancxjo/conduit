use super::*;
pub(super) fn evidence(value: &Value) -> Result<(u64, u64, u64)> {
    Ok((
        value
            .get("host_session")
            .and_then(Value::as_u64)
            .ok_or("evidence Host-session identity missing")?,
        value
            .get("execution")
            .and_then(Value::as_u64)
            .ok_or("evidence execution identity missing")?,
        value
            .get("sign")
            .and_then(Value::as_u64)
            .ok_or("evidence Sign identity missing")?,
    ))
}
pub(super) fn flags(
    properties: &mut Vec<FaceProperty>,
    subject: &str,
    summary: &Value,
) -> Result<()> {
    for (key, name) in [
        (
            "all_topology_stages_present",
            "dag-all-topology-stages-present",
        ),
        (
            "two_episode_temporal_feedback_chain",
            "dag-two-episode-feedback-chain",
        ),
        (
            "retained_explanation_complete",
            "dag-retained-explanation-complete",
        ),
        ("one_slot_queues", "dag-one-slot-queues"),
        ("exact_execution_identity", "dag-exact-execution-identity"),
        ("full_cognition_verified", "dag-full-cognition-verified"),
    ] {
        prop(
            properties,
            subject,
            name,
            FacePropertyValue::Flag(summary.get(key).and_then(Value::as_bool).unwrap_or(false)),
        );
    }
    let missing = summary
        .get("missing_branches")
        .and_then(Value::as_array)
        .ok_or("causal missing-branches field missing")?;
    for (index, branch) in missing.iter().enumerate() {
        prop(
            properties,
            subject,
            &format!("dag-missing-branch-{index}"),
            FacePropertyValue::Text(branch.as_str().ok_or("invalid missing branch")?.into()),
        );
    }
    Ok(())
}
pub(super) fn subject(
    subjects: &mut Vec<FaceSubject>,
    id: &str,
    role: FaceRole,
    name: &str,
) -> Result<()> {
    if let Some(existing) = subjects.iter().find(|s| s.identity == id) {
        if existing.role != role {
            return Err("historical subject role mismatch".into());
        }
        return Ok(());
    }
    subjects.push(FaceSubject {
        identity: id.into(),
        role,
        name: name.into(),
    });
    Ok(())
}
pub(super) fn prop(
    properties: &mut Vec<FaceProperty>,
    subject: &str,
    name: &str,
    value: FacePropertyValue,
) {
    properties.push(FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    });
}
pub(super) fn prop_once(
    properties: &mut Vec<FaceProperty>,
    subject: &str,
    name: &str,
    value: FacePropertyValue,
) {
    if !properties
        .iter()
        .any(|p| p.subject == subject && p.name == name)
    {
        prop(properties, subject, name, value);
    }
}

pub(super) fn lowered(
    plan: &BodyPlan,
) -> Result<conduit_plan_lowering::fragment_set::LoweredFragmentSet> {
    use conduit_plan_lowering::{
        fragment_set::{lower_local_fragment_set_from_plans, FragmentSetBounds},
        lowering::FIXED_KERNEL_STORAGE_PROFILE,
    };
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(32, 65536)
        .map_err(|e| format!("historical DAG loweringprofile: {e:?}"))?;
    let plans = plan.plots.iter().map(|p| &p.plan).collect::<Vec<_>>();
    lower_local_fragment_set_from_plans(
        &plans,
        profile,
        FragmentSetBounds {
            fragments: conduit_body::MAX_BODY_PLOTS as u16,
            nodes: 32,
            cords: 64,
            queue_slots: 64,
            value_bytes: 16 * 1024 * 1024,
            sign_items: 4096,
            sign_bytes: 4 * 1024 * 1024,
        },
    )
    .map_err(|e| format!("historical DAG Planlowering: {e:?}").into())
}
