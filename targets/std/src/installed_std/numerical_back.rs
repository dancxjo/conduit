//! Source-specific numerical offers and the initialized std factory binding.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::*;
use conduit_kernel::HostedValueStore;
use conduit_semantic_catalog::{NumericalRealizationBack, NUMERICAL_REALIZE_KIND};
const EXACT: &str = "conduit.std/numeric-rational@1";
const INTEGER: &str = "conduit.std/numeric-outward-i64@1";
const FLOAT: &str = "conduit.std/numeric-binary64-nearest@1";
const ENCLOSED: &str = "conduit.std/numeric-enclosed-binary64-nearest@1";
const PROFILE: &str = "conduit.std/prepared-numerical-receipt@1";
const WORK: u64 = 100_000_000;
pub(super) static EXACT_FACTORY: BackFactory = BackFactory {
    implementation_id: EXACT,
    budget,
    prepare,
};
pub(super) static INTEGER_FACTORY: BackFactory = BackFactory {
    implementation_id: INTEGER,
    budget,
    prepare,
};
pub(super) static FLOAT_FACTORY: BackFactory = BackFactory {
    implementation_id: FLOAT,
    budget,
    prepare,
};
pub(super) static ENCLOSED_FACTORY: BackFactory = BackFactory {
    implementation_id: ENCLOSED,
    budget,
    prepare,
};
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut out, "{byte:02x}").unwrap();
    }
    out
}
fn offer(prepared: &PreparedNumericalRealization) -> Result<CapabilityOffer, String> {
    let implementation = match prepared.algorithm() {
        NumericalAlgorithm::ExactRational => EXACT,
        NumericalAlgorithm::OutwardI64 => INTEGER,
        NumericalAlgorithm::Binary64 {
            rounding: Binary64Rounding::NearestTiesEven,
        } => FLOAT,
        NumericalAlgorithm::EnclosedBinary64 {
            rounding: Binary64Rounding::NearestTiesEven,
        } => ENCLOSED,
        _ => return Err("std numerical rounding strategy is not installed".into()),
    };
    let property = prepared
        .property()
        .map_err(|why| format!("numeric certificate: {why:?}"))?;
    let source_identity = hex(&semantic_digest(
        "numeric/prepared-std-offer@1",
        property.canonical_value(),
    ));
    let artifact = hex(&semantic_digest(
        "numeric/compiled-std-back@1",
        concat!(
            include_str!("numerical_back.rs"),
            include_str!("../../../../semantics/catalog/src/numerical_realization.rs"),
            include_str!("../../../../semantics/catalog/src/numerical_realization_back.rs"),
            include_str!("../../../../semantics/catalog/numerical/types.conduit"),
            include_str!("../../../../architecture/core/src/exact_numeric/realization.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/enclosed_binary64.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/realization/enclosed.rs"),
            include_str!(
                "../../../../architecture/core/src/exact_numeric/realization/codec/enclosed.rs"
            ),
            include_str!("../../../../architecture/core/src/exact_numeric/symbolic.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/symbolic/enclosure.rs"),
            include_str!(
                "../../../../architecture/core/src/exact_numeric/symbolic/enclosure/interval.rs"
            ),
            include_str!(
                "../../../../architecture/core/src/exact_numeric/symbolic/enclosure/pi.rs"
            ),
            include_str!("../../../../architecture/core/src/exact_numeric/realization/codec.rs"),
            include_str!(
                "../../../../architecture/core/src/exact_numeric/realization/codec/float.rs"
            ),
            include_str!("../../../../architecture/core/src/exact_numeric/binary64.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/binary64/tininess.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/binary64/algorithm.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/rational/binary64.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/enclosure.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/natural.rs"),
            include_str!("../../../../architecture/core/src/exact_numeric/rational.rs")
        )
        .as_bytes(),
    ));
    let mut offer = BackOfferBuilder::new(
        conduit_semantic_catalog::numerical_realize_semantic_contract(),
        Back {
            capability_id: format!("{implementation}/{source_identity}").into(),
            execution_profile_id: PROFILE.into(),
            implementation_id: implementation.into(),
            artifact_id: format!("sha256:{artifact}").into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    offer.realization_properties = vec![property];
    Ok(offer)
}
pub(crate) fn owns_offer(offer: &CapabilityOffer) -> bool {
    offer.implementation.execution_profile_id.as_str() == PROFILE
        && matches!(
            offer.implementation.implementation_id.as_str(),
            EXACT | INTEGER | FLOAT | ENCLOSED
        )
}
/// Prepare the finite supported realizations of one exact source, independently
/// of the consumer's tolerance. The ordinary planner applies that tolerance.
pub(crate) fn offers(
    request: &NumericalRealizationRequest,
) -> Result<Vec<CapabilityOffer>, String> {
    if request.source().as_rational().is_none() {
        let prepared = PreparedNumericalRealization::prepare(
            request.source(),
            NumericalAlgorithm::EnclosedBinary64 {
                rounding: Binary64Rounding::NearestTiesEven,
            },
            request.maximum_error(),
            &mut ExactNumericWork::new(WORK),
        )
        .map_err(|why| format!("std symbolic numerical realization refused: {why:?}"))?;
        return Ok(vec![offer(&prepared)?]);
    }
    let mut result = vec![];
    for algorithm in [
        NumericalAlgorithm::ExactRational,
        NumericalAlgorithm::OutwardI64,
        NumericalAlgorithm::Binary64 {
            rounding: Binary64Rounding::NearestTiesEven,
        },
    ] {
        match PreparedNumericalRealization::prepare(
            request.source(),
            algorithm,
            if matches!(algorithm, NumericalAlgorithm::Binary64 { .. }) {
                request.maximum_error()
            } else {
                ExactRational::ONE
            },
            &mut ExactNumericWork::new(WORK),
        ) {
            Ok(prepared) => result.push(offer(&prepared)?),
            Err(NumericalRealizationRefusal::Integer(_))
                if algorithm == NumericalAlgorithm::OutwardI64 => {}
            Err(NumericalRealizationRefusal::Binary64(Binary64AlgorithmRefusal::Projection(
                Binary64Refusal::ToleranceExceeded
                | Binary64Refusal::Numeric(ExactNumericRefusal::OutOfRange),
            ))) if matches!(algorithm, NumericalAlgorithm::Binary64 { .. }) => {}
            Err(why) => return Err(format!("std numerical source realization refused: {why:?}")),
        }
    }
    Ok(result)
}
fn admitted(gear: &PlannedGear) -> Result<NumericalRealizationBack, String> {
    if gear.kind_id.as_str() != NUMERICAL_REALIZE_KIND {
        return Err("wrong numerical Kind".into());
    }
    let property = gear
        .realization_properties
        .iter()
        .find(|p| *p.profile() == numerical_realization_profile())
        .ok_or("numeric certificate missing")?;
    let selected =
        PreparedNumericalRealization::from_property(property, &mut ExactNumericWork::new(WORK))
            .map_err(|why| format!("numeric certificate: {why:?}"))?;
    let expected = PreparedNumericalRealization::prepare(
        selected.source(),
        selected.algorithm(),
        selected.prepared_error_limit(),
        &mut ExactNumericWork::new(WORK),
    )
    .map_err(|why| format!("numeric installed strategy: {why:?}"))?;
    let offer = offer(&expected)?;
    if gear.capability_id != offer.capability_id
        || gear.execution_profile_id != offer.implementation.execution_profile_id
        || gear.implementation_id != offer.implementation.implementation_id
        || gear.artifact_id != offer.implementation.artifact_id
        || gear.realization_properties != offer.realization_properties
        || gear.limits != offer.limits
    {
        return Err("planned numerical offer differs from initialized std Back".into());
    }
    NumericalRealizationBack::prepare(gear, selected.algorithm())
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    let back = admitted(gear)?;
    let bytes = back.result_bytes().len() as u32;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: bytes,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: bytes,
    })
}
fn prepare(gear: &PlannedGear, _: &mut HostedValueStore) -> Result<InstalledBack, String> {
    Ok(InstalledBack::Numerical(admitted(gear)?))
}

#[cfg(test)]
#[path = "numerical_back_tests.rs"]
mod tests;
