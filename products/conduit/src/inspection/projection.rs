//! Inspect retained report claims without treating decoding as semantic admission.
use conduit_core::projection::{inspect_projection_evidence, ProjectionEvidenceView};
use conduit_observatory::RoutedValueArtifact;
use std::fmt::Write;
mod coordinates;
mod semantic;
pub(super) fn render(artifact: &RoutedValueArtifact) -> Result<Option<String>, String> {
    let sign = &artifact.sign.sign;
    let plan = artifact
        .sign
        .play
        .as_ref()
        .map(|p| &p.plan)
        .or(artifact.sign.plan.as_ref())
        .ok_or("routed value lacks Plan")?;
    let track = match &sign.kind {
        conduit_core::ObservationKind::ForeOutputContent { track, .. } => *track,
        conduit_core::ObservationKind::RoutedContent { .. } => {
            plan.fragments
                .iter()
                .flat_map(|f| &f.connections)
                .find(|c| Some(&c.connection_id) == sign.connection_id.as_ref())
                .ok_or("routed value lacks exact Cord track")?
                .track
        }
        _ => return Err("projection value lacks routed content context".into()),
    };
    if track != conduit_core::ConnectionTrack::Payload {
        return Ok(None);
    }
    let producer = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|p| Some(&p.placement_id) == sign.placement_id.as_ref())
        .ok_or("routed value lacks producer")?;
    if producer.execution_profile_id.as_str() == "std/exact-quantity-coordinates@1" {
        let port = match &sign.kind {
            conduit_core::ObservationKind::ForeOutputContent { evidence, .. }
            | conduit_core::ObservationKind::RoutedContent { evidence, .. } => {
                &evidence.output_port
            }
            _ => unreachable!("checked content context"),
        };
        if port.as_str() != "evidence" {
            return Ok(None);
        }
        return coordinates::render(artifact, producer).map(Some);
    }
    if !matches!(
        producer.execution_profile_id.as_str(),
        "std/quantity-near-exact-rational@1"
            | "std/quantity-near-exact-rational@2"
            | "std/quantity-near-binary32-single-round@1"
            | "std/quantity-near-binary32-single-round@2"
            | "std/quantity-near-enclosed-midpoint-binary32@1"
            | "std/quantity-near-enclosed-midpoint-binary32@2"
            | "std/quantity-near-enclosed-midpoint-binary32@3"
    ) {
        return Ok(None);
    }
    let frame = conduit_core::validate_canonical_structured_value(&artifact.bytes)
        .map_err(|e| format!("projection value framing: {e:?}"))?;
    let root = frame
        .record_field("projection-evidence")
        .map_err(|e| format!("projection evidence: {e:?}"))?;
    let field = match root {
        Some(field) => Some(field),
        None => {
            let representation = frame
                .record_field("representation")
                .map_err(|e| format!("projection representation: {e:?}"))?
                .ok_or("projection representation missing")?;
            match representation
                .variant_payload("enclosed_binary32")
                .map_err(|e| format!("projection representation: {e:?}"))?
            {
                Some(payload) => payload
                    .record_field("projection-evidence")
                    .map_err(|e| format!("projection evidence: {e:?}"))?,
                None => None,
            }
        }
    };
    let Some(field) = field else {
        let verified = semantic::verify(artifact, producer, frame)?;
        return Ok(Some(format!(
            "Legacy Near record: general projection report was not retained\n{verified}"
        )));
    };
    let node = field.value_node();
    if node.first() != Some(&0) || node.len() < 5 {
        return Err("projection evidence is not a byte leaf".into());
    }
    if u32::from_le_bytes(node[1..5].try_into().unwrap()) as usize != node.len() - 5 {
        return Err("projection evidence extent differs".into());
    }
    let view = inspect_projection_evidence(&node[5..])
        .map_err(|e| format!("projection evidence framing: {e:?}"))?;
    let verified = semantic::verify(artifact, producer, frame)?;
    let mut output = report(view);
    output.push_str(&verified);
    Ok(Some(output))
}
fn report(v: ProjectionEvidenceView<'_>) -> String {
    let mut out = format!(
        "Projection report claims\ncodec {}\nreport {}\nsource contract {}\ntarget contract {}\npolicy {}\nconfiguration {:?}\nsource identity {}\ntarget identity {:?}\nprojector {}\nrequested route {}\nboundary {:?}\nopaque policy bytes {}\nopaque source bytes {}\nopaque target bytes {:?}\nclaimed summary {:?}\nmechanism {:?}\nselected attempt {:?}\n",
        v.codec,
        v.report,
        v.source_contract,
        v.target_contract,
        v.policy,
        v.configuration_digest,
        v.source_identity,
        v.target_identity,
        v.projector,
        v.requested_route,
        v.boundary,
        v.policy_bytes.len(),
        v.source_bytes.len(),
        v.target_bytes.map(<[u8]>::len),
        v.claimed_summary,
        v.mechanism,
        v.selected_attempt
    );
    for fact in v.facts() {
        writeln!(out, "fact {fact:?}").unwrap();
    }
    for native in v.native() {
        writeln!(out, "native {native:?}").unwrap();
    }
    for score in v.scores() {
        writeln!(out, "score {score:?}").unwrap();
    }
    for admitted in v.admitted() {
        writeln!(out, "admitted {admitted:?}").unwrap();
    }
    for attempt in v.attempts() {
        writeln!(out, "attempt {attempt:?}").unwrap();
    }
    for diagnostic in v.diagnostics() {
        writeln!(out, "diagnostic {diagnostic}").unwrap();
    }
    out.push_str("report framing checked\n");
    out
}
