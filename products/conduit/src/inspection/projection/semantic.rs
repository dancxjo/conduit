//! Offline replay under the selected physical Near profile, without execution.
use conduit_core::*;
use conduit_observatory::RoutedValueArtifact;
mod rational_text;
pub(super) fn verify(
    artifact: &RoutedValueArtifact,
    producer: &PlannedGear,
    frame: ValidatedCanonicalStructuredValue<'_>,
) -> Result<String, String> {
    let revision = producer.kind_contract_revision.as_str();
    let profile = producer.execution_profile_id.as_str();
    let supported = match revision {
        "numeric/quantity-near@1" => matches!(
            profile,
            "std/quantity-near-exact-rational@1" | "std/quantity-near-binary32-single-round@1"
        ),
        "numeric/quantity-near@2" => matches!(
            profile,
            "std/quantity-near-exact-rational@1"
                | "std/quantity-near-binary32-single-round@1"
                | "std/quantity-near-enclosed-midpoint-binary32@1"
        ),
        "numeric/quantity-near@3" => matches!(
            profile,
            "std/quantity-near-exact-rational@1"
                | "std/quantity-near-binary32-single-round@1"
                | "std/quantity-near-enclosed-midpoint-binary32@2"
        ),
        "numeric/quantity-near@4" => matches!(
            profile,
            "std/quantity-near-exact-rational@2"
                | "std/quantity-near-binary32-single-round@2"
                | "std/quantity-near-enclosed-midpoint-binary32@3"
        ),
        _ => false,
    };
    if producer.kind_id.as_str() != conduit_semantic_catalog::NUMERICAL_NEAR_KIND || !supported {
        return Err("projection producer differs from the supported Near contract".into());
    }
    let schema = StructuredInfoType::from_canonical_bytes(frame.type_bytes())
        .map_err(|e| format!("projection schema: {e:?}"))?;
    let [output] = producer.outputs.as_slice() else {
        return Err("projection output port inventory differs".into());
    };
    if output.port_id.as_str() != "result"
        || schema
            .profile()
            .map_err(|e| format!("projection schema: {e:?}"))?
            .value_kind()
            != &output.value_kind
    {
        return Err("projection schema differs from its planned output kind".into());
    }
    if revision == "numeric/quantity-near@4"
        && frame
            .record_field("projection-evidence")
            .map_err(|e| format!("projection evidence: {e:?}"))?
            .is_none()
    {
        return Err("current Near producer requires a complete projection report".into());
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
    let wire = frame
        .record_field("source-wire")
        .map_err(|e| format!("projection source: {e:?}"))?
        .ok_or("projection source missing")?;
    let node = wire.value_node();
    let mut work = ExactNumericWork::new(maximum_work);
    if node.get(5).is_some_and(|tag| matches!(tag, 1 | 2)) {
        let verifier = PreparedNearQuantityVerifier::new(&schema)
            .map_err(|e| format!("prepare projection replay: {e:?}"))?;
        let value = verifier
            .verify(
                &artifact.bytes,
                input.value_kind.as_str(),
                limit,
                None,
                &mut work,
            )
            .map_err(|e| format!("independent projection replay refused: {e:?}"))?;
        match value.representation() {
            VerifiedNearRepresentation::ExactRational
                if profile.starts_with("std/quantity-near-exact-rational@") => {}
            VerifiedNearRepresentation::Binary32 {
                rounding: Binary32Rounding::NearestTiesEven,
                ..
            } if !profile.starts_with("std/quantity-near-exact-rational@") => (),
            _ => {
                return Err(
                    "projection representation differs from its selected producer profile".into(),
                );
            }
        }
        return Ok(format!(
            "Numerical certificate independently replayed\noriginal Plan request matched\nsource unit {}\nsource role {:?}\nsource wire digest {:02x?}\nrepresentation {:?}\nmaximum absolute error {} {}\nrequested allowance {} {}\nvalidation work {}/{}\nexecution authenticity not established\n",
            value.source().unit().symbol(),
            value.source().role(),
            value.source_wire_digest(),
            value.representation(),
            rational_text::format(value.maximum_error())?,
            value.source().unit().symbol(),
            rational_text::format(limit.coordinate())?,
            limit.unit().symbol(),
            work.spent(),
            maximum_work
        ));
    }
    if !profile.starts_with("std/quantity-near-enclosed-midpoint-binary32@") {
        return Err("symbolic projection differs from its selected producer profile".into());
    }
    let mut verifier = PreparedMathematicalNearQuantityVerifier::new(
        &schema,
        conduit_std_offers::near_contract_requirements(),
        Binary32Rounding::NearestTiesEven,
        limit,
        maximum_work,
    )
    .map_err(|e| format!("prepare projection replay: {e:?}"))?;
    let value = verifier
        .verify(&artifact.bytes, input.value_kind.as_str(), None, &mut work)
        .map_err(|e| format!("independent projection replay refused: {e:?}"))?;
    let receipt = value.projection_receipt();
    let interval = receipt.enclosure().numerical().target();
    Ok(format!(
        "Numerical certificate independently replayed\noriginal Plan request matched\nsource unit {}\nsource role {:?}\nsource wire digest {:02x?}\ntarget binary32 0x{:08x}\nrounding {:?}\nenclosure lower {}\nenclosure upper {}\nmaximum absolute error {} {}\nrequested allowance {} {}\nvalidation work {}/{}\nexecution authenticity not established\n",
        value.source().unit().symbol(),
        value.source().role(),
        value.source_wire_digest(),
        value.target().bits(),
        value.rounding(),
        rational_text::format(interval.lower())?,
        rational_text::format(interval.upper())?,
        rational_text::format(value.maximum_error())?,
        value.source().unit().symbol(),
        rational_text::format(limit.coordinate())?,
        limit.unit().symbol(),
        work.spent(),
        maximum_work
    ))
}
