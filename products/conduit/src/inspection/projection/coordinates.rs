//! Replay exact coordinate evidence against the selected Plan request.
use conduit_core::{projection::*, *};
use conduit_observatory::RoutedValueArtifact;

pub(super) fn render(
    artifact: &RoutedValueArtifact,
    producer: &PlannedGear,
) -> Result<String, String> {
    if producer.execution_profile_id.as_str() != "std/exact-quantity-coordinates@1"
        || producer.kind_id.as_str() != conduit_semantic_catalog::QUANTITY_COORDINATES_KIND
        || producer.kind_contract_revision.as_str()
            != conduit_semantic_catalog::QUANTITY_COORDINATES_REVISION
    {
        return Err("coordinate producer differs from the supported contract".into());
    }
    let contract = conduit_semantic_catalog::quantity_coordinates_semantic_contract();
    if producer.inputs != contract.inputs || producer.outputs != contract.outputs {
        return Err("coordinate port inventory differs from the supported contract".into());
    }
    let target_unit =
        conduit_semantic_catalog::quantity_coordinate_target(&producer.configuration)?;
    let schema = conduit_semantic_catalog::quantity_coordinate_evidence_type();
    let frame = validate_canonical_structured_value(&artifact.bytes)
        .map_err(|e| format!("coordinate report framing: {e:?}"))?;
    if frame.type_bytes()
        != schema
            .canonical_bytes()
            .map_err(|e| format!("coordinate schema: {e:?}"))?
    {
        return Err("coordinate report schema differs".into());
    }
    let output = producer
        .outputs
        .iter()
        .find(|p| p.port_id.as_str() == "evidence")
        .ok_or("coordinate evidence output missing")?;
    let port = match &artifact.sign.sign.kind {
        ObservationKind::ForeOutputContent { evidence, .. }
        | ObservationKind::RoutedContent { evidence, .. } => &evidence.output_port,
        _ => return Err("coordinate report lacks content context".into()),
    };
    if port != &output.port_id
        || schema
            .profile()
            .map_err(|e| format!("coordinate schema: {e:?}"))?
            .value_kind()
            != &output.value_kind
    {
        return Err("coordinate report differs from its planned evidence output".into());
    }
    let leaf = |name| -> Result<&[u8], String> {
        let value = frame
            .record_field(name)
            .map_err(|e| format!("coordinate evidence: {e:?}"))?
            .ok_or("coordinate evidence field missing")?;
        let node = value.value_node();
        if node.first() != Some(&0)
            || node.len() < 5
            || u32::from_le_bytes(node[1..5].try_into().unwrap()) as usize != node.len() - 5
        {
            return Err("coordinate evidence is not a canonical byte leaf".into());
        }
        Ok(&node[5..])
    };
    let head = leaf("projection-evidence-head")?;
    let tail = leaf("projection-evidence-tail")?;
    if head.len() != (head.len() + tail.len()).min(65536) || tail.len() > 65536 {
        return Err("coordinate evidence split differs".into());
    }
    let mut evidence = head.to_vec();
    evidence.extend_from_slice(tail);
    let view = inspect_projection_evidence(&evidence)
        .map_err(|e| format!("coordinate evidence framing: {e:?}"))?;
    let source = first_bytes(view.source_bytes)?;
    let target = first_bytes(view.target_bytes.ok_or("coordinate target missing")?)?;
    let ids = QuantityCoordinateContentIds::new(source, target, target_unit);
    // Fixed allowance of this exact producer profile, never adopted from the
    // untrusted receipt. Offline replay has its own larger consuming envelope.
    let producer_work = 200_000_000;
    let mut work = ExactNumericWork::new(900_000_000);
    let mut verifier = PreparedQuantityCoordinateConversion::new(500000, producer_work)
        .map_err(|e| format!("prepare coordinate replay: {e:?}"))?;
    let mut scratch = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    verifier
        .verify_evidence(
            source,
            target,
            QuantityCoordinateConversionPolicy { target_unit },
            producer_work,
            ids.basis(),
            &evidence,
            &mut scratch,
            &mut work,
        )
        .map_err(|e| format!("independent coordinate replay refused: {e:?}"))?;
    let mut rendered = super::report(view);
    rendered.push_str(&format!(
        "Coordinate certificate independently replayed\noriginal Plan request matched\ntarget unit {}\nvalidation work {}/900000000\nexecution authenticity not established\n",
        target_unit.symbol(), work.spent()));
    Ok(rendered)
}

fn first_bytes(bytes: &[u8]) -> Result<&[u8], String> {
    let prefix = bytes.get(..4).ok_or("coordinate wire extent missing")?;
    let length = u32::from_le_bytes(prefix.try_into().unwrap()) as usize;
    let end = 4usize
        .checked_add(length)
        .ok_or("coordinate wire extent overflow")?;
    bytes
        .get(4..end)
        .ok_or_else(|| "coordinate wire extent differs".into())
}
