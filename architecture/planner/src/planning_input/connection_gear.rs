//! Retain only the Source facts actually needed by Cord admission.
#[cfg(feature = "indexed-source")]
use alloc::vec::Vec;
#[cfg(feature = "indexed-source")]
use conduit_core::GearId;
use conduit_core::{FrontValueContract, FrontValueLocation, ResourcePortContract, ValueConstraint};
use conduit_plot::CheckedGear;

pub(crate) fn minimum_at(contracts: &[FrontValueContract], location: &FrontValueLocation) -> u32 {
    contracts
        .iter()
        .filter(|c| c.location == *location)
        .flat_map(|c| &c.contract.constraints)
        .filter_map(|c| match c {
            ValueConstraint::ByteLength { minimum, .. } => Some(*minimum),
            _ => None,
        })
        .max()
        .unwrap_or_default()
}
#[cfg(feature = "indexed-source")]
pub(crate) struct ConnectionFacts {
    pub gear_id: GearId,
    pub minima: Vec<(FrontValueLocation, u32)>,
    pub resource_ports: Vec<ResourcePortContract>,
}
#[cfg(feature = "indexed-source")]
impl ConnectionFacts {
    pub fn from_checked(gear: &CheckedGear) -> Self {
        // Implicit Front contracts have upper bounds only. Source ByteLength
        // constraints supply every nonzero minimum; matching already checked
        // port shapes, so retaining a second full Front would duplicate them.
        let mut minima = Vec::new();
        for contract in gear.semantic_contract.value_contracts() {
            // Match CheckedFront's last declaration at a location, including
            // replacement with an explicit zero minimum.
            minima.retain(|(at, _)| at != &contract.location);
            let minimum = minimum_at(core::slice::from_ref(contract), &contract.location);
            if minimum != 0 {
                minima.push((contract.location.clone(), minimum));
            }
        }
        Self {
            gear_id: gear.gear_id.clone(),
            minima,
            resource_ports: gear.resource_ports.clone(),
        }
    }
}
pub(crate) enum ConnectionGear<'a> {
    Checked(&'a CheckedGear),
    #[cfg(feature = "indexed-source")]
    Prepared(&'a ConnectionFacts),
}
impl ConnectionGear<'_> {
    pub fn minimum_at(&self, location: &FrontValueLocation) -> u32 {
        match self {
            Self::Checked(gear) => minimum_at(gear.checked_front().value_contracts(), location),
            #[cfg(feature = "indexed-source")]
            Self::Prepared(gear) => gear
                .minima
                .iter()
                .filter(|(at, _)| at == location)
                .map(|(_, minimum)| *minimum)
                .max()
                .unwrap_or_default(),
        }
    }
    pub fn resource_ports(&self) -> &[ResourcePortContract] {
        match self {
            Self::Checked(gear) => &gear.resource_ports,
            #[cfg(feature = "indexed-source")]
            Self::Prepared(gear) => &gear.resource_ports,
        }
    }
}

#[cfg(all(test, feature = "indexed-source"))]
mod tests {
    use super::*;
    use conduit_core::{kind_id, CheckedValueContract, KindSemanticLaw};
    #[test]
    fn cord_projection_matches_full_front_defaults_and_last_location_override() {
        let mut gear = crate::tests::plot().gears.remove(0);
        let location = FrontValueLocation::Output(gear.outputs[0].port_id.clone());
        gear.semantic_contract
            .laws
            .retain(|law| !matches!(law, KindSemanticLaw::ValueContracts(_)));
        let mut contracts = Vec::new();
        for minimum in [None, Some(24), Some(12), Some(0)] {
            if let Some(minimum) = minimum {
                contracts.push(FrontValueContract {
                    location: location.clone(),
                    contract: CheckedValueContract::new(
                        kind_id("value/bytes"),
                        256,
                        alloc::vec![ValueConstraint::ByteLength {
                            minimum,
                            maximum: 256
                        }],
                    )
                    .unwrap(),
                });
            }
            gear.semantic_contract
                .laws
                .retain(|law| !matches!(law, KindSemanticLaw::ValueContracts(_)));
            gear.semantic_contract
                .laws
                .push(KindSemanticLaw::ValueContracts(contracts.clone()));
            let front = gear.checked_front();
            let facts = ConnectionFacts::from_checked(&gear);
            assert_eq!(
                ConnectionGear::Prepared(&facts).minimum_at(&location),
                minimum_at(front.value_contracts(), &location)
            );
            assert_eq!(facts.resource_ports, gear.resource_ports);
        }
    }
}
