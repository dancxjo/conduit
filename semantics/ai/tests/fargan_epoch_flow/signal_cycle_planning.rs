use super::*;

pub(super) fn prepared_signal_cycle_plan() -> (
    Plan,
    EpochProfiles,
    conduitos::seeded_state::SeededStateOperationFactory,
) {
    let (context, seeded, ids) = prepared_signal_cycle_profiles_with_capacity(true);
    let cycle = include_str!("../../../speech/fargan_signal_cycle.conduit");
    for id in ids.values() {
        assert!(cycle.contains(id), "{id}");
    }
    let imports = cycle
        .lines()
        .filter(|line| line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let body = cycle
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let selected=format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    let body = body.replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {selected}\n"),
    );
    let source = format!(
        "{imports}\n{}\n{}\n{body}",
        epoch_source(),
        include_str!("../../../speech/fargan_epoch_feedback.conduit")
    );
    let offers = seeded.offers().cloned().collect();
    let (plan, context) = prepare_authored_epoch_entry(
        context,
        source,
        "speech/flow-fargan-signal-cycle",
        true,
        offers,
    )
    .unwrap();
    (plan, context, seeded)
}

pub(super) fn signal_cycle_template() -> String {
    let cycle = include_str!("../../../speech/fargan_signal_cycle.conduit");
    let imports = cycle
        .lines()
        .filter(|l| l.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let body = cycle
        .lines()
        .filter(|l| !l.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{imports}\n{}\n{}\n{body}",
        unbound_epoch_source(),
        include_str!("../../../speech/fargan_epoch_feedback.conduit")
    )
}
