//! Exact external Fore validation from sealed selected port profiles.
use super::*;
use conduit_core::{QuantityValueProfile, QuantityWireContract};

/// Numeric kernel binding for one plan-sealed external Fore port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredForePort {
    pub front_port_id: PlanPortId,
    pub direction: PortDirection,
    pub track: conduit_core::ConnectionTrack,
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub value_kind: KindId,
    pub value_contract: Option<conduit_core::CheckedValueContract>,
    /// Selected codec support; this is a realization fact, not authored meaning.
    pub quantity_profile: QuantityValueProfile,
    pub abnormal_kind: Option<KindId>,
    pub temporal: conduit_core::PortTemporal,
    pub item_capacity: u16,
    pub byte_capacity: u32,
    /// Selected external carrier identity; execution must recheck its live
    /// availability before crossing this Fore.
    pub selected_line: Option<conduit_core::AdmittedLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForeValueRefusal {
    NotValueTrack,
    CordCapacity,
    Malformed(conduit_core::PrimitiveInfoRefusal),
    Constraint(conduit_core::ValueConstraintRefusal),
    ProfileConstraint(conduit_core::QuantityProfileContractRefusal),
    QuantityProfile(conduit_core::QuantityValueContractRefusal),
}

impl LoweredForePort {
    /// Revalidates one committed external value against the exact sealed Fore
    /// contract before the kernel observes it.
    pub fn validate_value(&self, canonical: &[u8]) -> Result<(), ForeValueRefusal> {
        let value_kind = match self.track {
            conduit_core::ConnectionTrack::Payload
            | conduit_core::ConnectionTrack::AbnormalTerminal => &self.value_kind,
            conduit_core::ConnectionTrack::NormalClose
            | conduit_core::ConnectionTrack::Quiescence => {
                return Err(ForeValueRefusal::NotValueTrack);
            }
        };
        if canonical.len() > self.byte_capacity as usize {
            return Err(ForeValueRefusal::CordCapacity);
        }
        let quantity = value_kind.as_str() == conduit_core::QUANTITY_INFO_ID
            || conduit_core::quantity_info_dimension(value_kind.as_str()).is_some()
            || conduit_core::parse_quantity_role_info_id(value_kind.as_str()).is_some();
        let explicit = quantity
            && (self.quantity_profile == QuantityValueProfile::Exact
                || canonical.first() == Some(&2));
        if let Some(contract) = &self.value_contract {
            if explicit {
                return contract
                    .validate_with_quantity_profile(canonical, self.quantity_profile)
                    .map_err(ForeValueRefusal::ProfileConstraint);
            }
            return contract
                .validate(canonical)
                .map_err(ForeValueRefusal::Constraint);
        }
        if explicit {
            return validate_quantity(self.quantity_profile, value_kind.as_str(), canonical);
        }
        conduit_core::validate_primitive_info(value_kind.as_str(), canonical)
            .map_err(ForeValueRefusal::Malformed)
    }
}

#[inline(never)]
fn validate_quantity(
    profile: QuantityValueProfile,
    kind: &str,
    bytes: &[u8],
) -> Result<(), ForeValueRefusal> {
    profile
        .decode(
            kind,
            bytes,
            &mut conduit_core::ExactNumericWork::new(
                conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK,
            ),
        )
        .map(|_| ())
        .map_err(ForeValueRefusal::QuantityProfile)
}

pub(super) fn selected_profile(
    fragment: &PlanFragment,
    fore: &conduit_core::PlannedForePort,
) -> Result<QuantityValueProfile, LoweringError> {
    if fore.track != ConnectionTrack::Payload {
        return Ok(QuantityValueProfile::Decimal);
    }
    let invalid = || {
        LoweringError::ConnectionContractMismatch(ConnectionId::from(fore.front_port_id.as_str()))
    };
    let placement = fragment
        .placements
        .iter()
        .find(|p| p.placement_id == fore.placement_id)
        .ok_or_else(invalid)?;
    if placement.realization_properties.is_empty() {
        return Ok(QuantityValueProfile::Decimal);
    }
    let kind = QuantityWireContract::property_kind();
    let Some(property) = placement
        .realization_properties
        .iter()
        .find(|p| p.profile() == &kind)
    else {
        return Ok(QuantityValueProfile::Decimal);
    };
    let contract = QuantityWireContract::from_property(property).map_err(|_| invalid())?;
    let profile = match fore.direction {
        PortDirection::Input => contract.input(&fore.gear_port_id),
        PortDirection::Output => contract.output(&fore.gear_port_id),
    };
    if profile == QuantityValueProfile::Exact
        && fore.byte_capacity < required_capacity(fore.direction, fore.value_contract.as_ref())
    {
        return Err(invalid());
    }
    Ok(profile)
}

// An input contract can restrict the accepted byte domain independently of its
// codec. Output storage still covers the selected producer's full wire ceiling.
fn required_capacity(
    direction: PortDirection,
    contract: Option<&conduit_core::CheckedValueContract>,
) -> u32 {
    let maximum = conduit_core::QUANTITY_VALUE_MAX_ENCODED_BYTES as u32;
    match direction {
        PortDirection::Input => contract.map_or(maximum, |c| c.maximum_bytes.min(maximum)),
        PortDirection::Output => maximum,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_exact_inputs_remain_distinct_from_native_codec_support() {
        let contract = conduit_core::CheckedValueContract::new(
            conduit_core::kind_id(conduit_core::DISTANCE_INFO_ID),
            conduit_core::QUANTITY_ENCODED_LEN as u32,
            alloc::vec![],
        )
        .unwrap();
        let native = conduit_core::Quantity::parse_plot_literal("1m").unwrap();
        let mut work =
            conduit_core::ExactNumericWork::new(conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
        let coordinate = conduit_core::ExactRational::from_i128(1)
            .checked_divide(conduit_core::ExactRational::from_i128(3), &mut work)
            .unwrap();
        let quantity =
            conduit_core::RationalQuantity::new(coordinate, native.unit(), native.role()).unwrap();
        let mut bytes = alloc::vec![0; conduit_core::QUANTITY_VALUE_MAX_ENCODED_BYTES];
        let n = conduit_core::encode_rational_quantity_value(quantity, &mut bytes).unwrap();
        bytes.truncate(n);
        let mut port = LoweredForePort {
            front_port_id: "value".into(),
            direction: PortDirection::Input,
            track: ConnectionTrack::Payload,
            endpoint: RemoteEndpointId(0),
            cord: CordId(0),
            value_kind: contract.value_kind.clone(),
            value_contract: Some(contract),
            quantity_profile: QuantityValueProfile::Exact,
            abnormal_kind: None,
            temporal: conduit_core::PortTemporal::Value,
            item_capacity: 1,
            byte_capacity: conduit_core::QUANTITY_ENCODED_LEN as u32,
            selected_line: None,
        };
        assert!(port.validate_value(&bytes).is_ok());
        assert_eq!(
            required_capacity(port.direction, port.value_contract.as_ref()),
            port.byte_capacity
        );
        port.quantity_profile = QuantityValueProfile::Decimal;
        assert!(matches!(
            port.validate_value(&bytes),
            Err(ForeValueRefusal::ProfileConstraint(
                conduit_core::QuantityProfileContractRefusal::Quantity(
                    conduit_core::QuantityValueContractRefusal::UnsupportedRepresentation { .. }
                )
            ))
        ));
        assert_eq!(
            required_capacity(PortDirection::Output, port.value_contract.as_ref()),
            conduit_core::QUANTITY_VALUE_MAX_ENCODED_BYTES as u32
        );
    }
}
