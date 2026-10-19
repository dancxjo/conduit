//! Offline replay under the selected physical Near profile, without execution.
use conduit_core::*;
use conduit_observatory::RoutedValueArtifact;
mod rational_text;
pub(super) fn verify(
    artifact: &RoutedValueArtifact,
    producer: &PlannedGear,
    frame: ValidatedCanonicalStructuredValue<'_>,
) -> Result<String, String> {
    if producer.kind_id.as_str() != conduit_semantic_catalog::NUMERICAL_NEAR_KIND
        || producer.kind_contract_revision.as_str()
            != conduit_semantic_catalog::NUMERICAL_NEAR_REVISION
    {
        return Err("projection producer differs from the supported Near contract".into());
    }
    let limit = conduit_semantic_catalog::near_quantity_limit(&producer.configuration)?;
    let ConfigurationValue::Quantity(requested) = &producer.configuration[0].value else {
        unreachable!("checked Near configuration")
    };
    let requested_field = frame
        .record_field("requested-limit")
        .map_err(|e| format!("projection request framing: {e:?}"))?
        .ok_or("projection request missing")?;
    let node = requested_field.value_node();
    if node.first() != Some(&0) || node.len() < 5 || node[5..] != requested.value().encode() {
        return Err("projection request differs from its exact Plan configuration".into());
    }
    let [input] = producer.inputs.as_slice() else {
        return Err("projection input port inventory differs".into());
    };
    if input.port_id.as_str() != "value" || input.value_kind.as_str() != QUANTITY_INFO_ID {
        return Err("projection input differs from the supported physical contract".into());
    }
    let maximum_work = conduit_std_offers::NEAR_CONTRACT_REPLAY_WORK;
    let mut verifier = PreparedMathematicalNearQuantityVerifier::new(
        &conduit_semantic_catalog::near_quantity_type(),
        conduit_std_offers::near_contract_requirements(),
        Binary32Rounding::NearestTiesEven,
        limit,
        maximum_work,
    )
    .map_err(|e| format!("prepare projection replay: {e:?}"))?;
    let mut work = ExactNumericWork::new(maximum_work);
    let value = verifier
        .verify(&artifact.bytes, input.value_kind.as_str(), None, &mut work)
        .map_err(|e| format!("independent projection replay refused: {e:?}"))?;
    let receipt = value.projection_receipt();
    let interval = receipt.enclosure().numerical().target();
    Ok(format!("Numerical certificate independently replayed\noriginal Plan request matched\nsource unit {}\nsource role {:?}\nsource wire digest {:02x?}\ntarget binary32 0x{:08x}\nrounding {:?}\nenclosure lower {}\nenclosure upper {}\nmaximum absolute error {} {}\nrequested allowance {} {}\nvalidation work {}/{}\nexecution authenticity not established\n",
        value.source().unit().symbol(),value.source().role(),value.source_wire_digest(),
        value.target().bits(),value.rounding(),rational_text::format(interval.lower())?,rational_text::format(interval.upper())?,
        rational_text::format(value.maximum_error())?,value.source().unit().symbol(),
        rational_text::format(limit.coordinate())?,limit.unit().symbol(),work.spent(),maximum_work))
}
