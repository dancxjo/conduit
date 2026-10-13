//! Authored finite Scalar byte bounds select intrinsic numerical profiles.
use crate::{Span, SyntaxCheckDiagnostic};
use conduit_core::{KindId, PortableNumericProfile, PrimitiveInfoKind};

pub(crate) fn bounded_kind(
    kind: &KindId,
    maximum: Option<u64>,
    span: Span,
) -> Result<KindId, SyntaxCheckDiagnostic> {
    let Some(maximum) = maximum else {
        return Ok(kind.clone());
    };
    let physical_role = conduit_core::parse_quantity_semantic_role(kind.as_str());
    let physical = conduit_core::PhysicalNumericProfile::parse_generic_info_id(kind.as_str()).is_some()
        || physical_role.is_some() || kind.as_str() == conduit_core::QUANTITY_INFO_ID;
    let unit = conduit_core::primitive_info_kind(kind.as_str()) == Some(PrimitiveInfoKind::Unit);
    if physical || unit {
        let invalid = |message| SyntaxCheckDiagnostic { code: "CND-FRM-057", span, message };
        let maximum = usize::try_from(maximum).map_err(|_| invalid("physical byte bound is too large".into()))?;
        let mut numeric = PortableNumericProfile::MAXIMAL;
        numeric.maximum_encoded_bytes = 10;
        let floor = conduit_core::PhysicalNumericProfile { coordinate: numeric, unit_law: numeric };
        let floor_bytes = if unit { floor.maximum_unit_encoded_bytes() } else { floor.maximum_quantity_encoded_bytes() }
            .map_err(|error| invalid(alloc::format!("physical profile: {error:?}")))?;
        if maximum < floor_bytes { return Err(invalid("physical byte bound cannot hold its immutable descriptor".into())); }
        numeric.maximum_encoded_bytes = (10 + (maximum - floor_bytes) / if unit { 2 } else { 3 })
            .min(conduit_core::MAX_NUMERIC_ENCODED_BYTES) as u32;
        let profile = conduit_core::PhysicalNumericProfile { coordinate: numeric, unit_law: numeric };
        let id = if unit {
            conduit_core::unit_numeric_profile_info_id(numeric).map_err(|error| invalid(alloc::format!("Unit profile: {error:?}")))?
        } else if let Some((family, role)) = physical_role {
            profile.info_id_for_identity(family, role).map_err(|error| invalid(alloc::format!("quantity profile: {error:?}")))?
        } else {
            profile.generic_info_id().map_err(|error| invalid(alloc::format!("quantity profile: {error:?}")))?
        };
        return Ok(conduit_core::kind_id(&id));
    }
    if conduit_core::primitive_info_kind(kind.as_str())
        != Some(PrimitiveInfoKind::MathematicalNumber)
    {
        return Ok(kind.clone());
    }
    let invalid = |message| SyntaxCheckDiagnostic {
        code: "CND-FRM-057",
        span,
        message,
    };
    let mut profile = PortableNumericProfile::from_info_id(kind.as_str())
        .unwrap_or(PortableNumericProfile::MAXIMAL);
    profile.maximum_encoded_bytes = profile.maximum_encoded_bytes.min(
        u32::try_from(maximum)
            .map_err(|_| invalid("Scalar byte bound exceeds portable numerical limits".into()))?,
    );
    let id = profile
        .info_id()
        .map_err(|reason| invalid(alloc::format!("invalid finite Scalar profile: {reason:?}")))?;
    Ok(conduit_core::kind_id(&id))
}
