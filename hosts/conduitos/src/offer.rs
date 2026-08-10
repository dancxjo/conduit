//! Fixed, boot-scoped Host offer assembled from truthful machine Bases.
//!
//! The allocation-backed portable advertisement is deliberately deferred to
//! the ordinary planning slice. This P2/P3 record preserves the same current
//! distinctions without adding an allocator or pretending Base presence is
//! semantic authority.

use crate::{identity::BootIdentities, machine::BaseKind};

pub const BASE_COUNT: usize = 7;
pub const RESOURCE_COUNT: usize = 4;
pub const CAPABILITY_COUNT: usize = 2;
pub const TIMER_SLOT_CAPACITY: u16 = 1;
pub const SERIAL_OPERATION_CAPACITY: u16 = 1;
pub const SERIAL_MAXIMUM_BYTES: u32 = 16;
pub const SIGN_ITEM_CAPACITY: u16 = 64;
pub const INTERRUPT_FACT_CAPACITY: u16 = 4;
pub const TIME_TICK_IMPLEMENTATION: &str = "conduitos/kernel-time-tick@1";
pub const TICK_PRESENTATION_IMPLEMENTATION: &str = "conduitos/kernel-serial-tick@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuFeatures {
    pub sse2: bool,
    pub rdrand: bool,
    pub invariant_tsc: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaseOffer {
    pub id: [u8; 32],
    pub kind: BaseKind,
    pub capacity: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceOffer {
    pub class: &'static str,
    pub capacity: u32,
    pub base: BaseKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortOffer {
    pub name: &'static str,
    pub value_kind: &'static str,
    pub direction: PortDirection,
    pub closes: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityOffer<'a> {
    pub kind: &'static str,
    pub contract_revision: &'static str,
    pub implementation: &'static str,
    pub artifact_build: &'a str,
    pub host_operation: &'static str,
    pub required_base: BaseKind,
    pub secondary_base: Option<BaseKind>,
    pub input: Option<PortOffer>,
    pub output: Option<PortOffer>,
    pub maximum_in_flight: u16,
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub struct HostOffer<'a> {
    pub host_id: [u8; 32],
    pub boot_id: [u8; 32],
    pub generation: u64,
    pub profile: &'static str,
    pub bases: [BaseOffer; BASE_COUNT],
    pub resources: [ResourceOffer; RESOURCE_COUNT],
    pub capabilities: [CapabilityOffer<'a>; CAPABILITY_COUNT],
    pub cpu_features: CpuFeatures,
    pub runtime_arena_bytes: u64,
    pub sign_item_capacity: u16,
    pub interrupt_fact_capacity: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfferError {
    EmptyIdentity,
    DuplicateBase,
    InvalidCapacity,
    MissingBase,
    StaleObservation,
    ArtifactRequirementMismatch,
    MissingIsaFeature,
}

impl OfferError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyIdentity => "empty-offer-identity",
            Self::DuplicateBase => "duplicate-base-identity",
            Self::InvalidCapacity => "invalid-offer-capacity",
            Self::MissingBase => "capability-base-unavailable",
            Self::StaleObservation => "stale-feature-observation",
            Self::ArtifactRequirementMismatch => "artifact-feature-mismatch",
            Self::MissingIsaFeature => "missing-isa-feature",
        }
    }
}

impl<'a> HostOffer<'a> {
    pub fn new(
        ids: &BootIdentities,
        build_id: &'a str,
        cpu_features: CpuFeatures,
        runtime_arena_bytes: u64,
    ) -> Self {
        let kinds = [
            BaseKind::Memory,
            BaseKind::Clock,
            BaseKind::Timer,
            BaseKind::Serial,
            BaseKind::Interrupt,
            BaseKind::Idle,
            BaseKind::ExecutionLane,
        ];
        let bases = kinds.map(|kind| BaseOffer {
            id: crate::identity::derive_base(&ids.boot, kind.as_str()),
            kind,
            capacity: match kind {
                BaseKind::Memory => u32::try_from(runtime_arena_bytes).unwrap_or(u32::MAX),
                BaseKind::Timer | BaseKind::Serial | BaseKind::ExecutionLane => 1,
                BaseKind::Interrupt => u32::from(INTERRUPT_FACT_CAPACITY),
                BaseKind::Clock | BaseKind::Idle => 1,
            },
        });
        Self {
            host_id: ids.host,
            boot_id: ids.boot,
            generation: 1,
            profile: "conduitos/single-lane-cooperative@1",
            bases,
            resources: [
                ResourceOffer {
                    class: "conduit.resource/runtime-memory@1",
                    capacity: u32::try_from(runtime_arena_bytes).unwrap_or(u32::MAX),
                    base: BaseKind::Memory,
                },
                ResourceOffer {
                    class: "conduit.resource/execution-lane@1",
                    capacity: 1,
                    base: BaseKind::ExecutionLane,
                },
                ResourceOffer {
                    class: "conduit.resource/timer-slot@1",
                    capacity: u32::from(TIMER_SLOT_CAPACITY),
                    base: BaseKind::Timer,
                },
                ResourceOffer {
                    class: "conduit.resource/presentation-slot@1",
                    capacity: u32::from(SERIAL_OPERATION_CAPACITY),
                    base: BaseKind::Serial,
                },
            ],
            capabilities: [
                CapabilityOffer {
                    kind: "time/tick",
                    contract_revision: "conduit.std/time-tick@2",
                    implementation: TIME_TICK_IMPLEMENTATION,
                    artifact_build: build_id,
                    host_operation: "conduit.host/wait@1",
                    required_base: BaseKind::Timer,
                    secondary_base: Some(BaseKind::Clock),
                    input: None,
                    output: Some(PortOffer {
                        name: "tick",
                        value_kind: "conduit.value/tick@1",
                        direction: PortDirection::Output,
                        closes: true,
                    }),
                    maximum_in_flight: TIMER_SLOT_CAPACITY,
                    maximum_input_bytes: 16,
                    maximum_output_bytes: 16,
                },
                CapabilityOffer {
                    kind: "presentation/tick",
                    contract_revision: "conduit.std/presentation-tick@1",
                    implementation: TICK_PRESENTATION_IMPLEMENTATION,
                    artifact_build: build_id,
                    host_operation: "conduit.host/present@1",
                    required_base: BaseKind::Serial,
                    secondary_base: None,
                    input: Some(PortOffer {
                        name: "tick",
                        value_kind: "conduit.value/tick@1",
                        direction: PortDirection::Input,
                        closes: true,
                    }),
                    output: None,
                    maximum_in_flight: SERIAL_OPERATION_CAPACITY,
                    maximum_input_bytes: SERIAL_MAXIMUM_BYTES,
                    maximum_output_bytes: 0,
                },
            ],
            cpu_features,
            runtime_arena_bytes,
            sign_item_capacity: SIGN_ITEM_CAPACITY,
            interrupt_fact_capacity: INTERRUPT_FACT_CAPACITY,
        }
    }

    pub fn validate(&self) -> Result<(), OfferError> {
        if self.host_id == [0; 32]
            || self.boot_id == [0; 32]
            || self.generation == 0
            || self.profile.is_empty()
        {
            return Err(OfferError::EmptyIdentity);
        }
        if self.bases.iter().enumerate().any(|(index, base)| {
            base.id == [0; 32]
                || base.capacity == 0
                || self.bases[..index]
                    .iter()
                    .any(|prior| prior.id == base.id || prior.kind == base.kind)
        }) {
            return Err(OfferError::DuplicateBase);
        }
        if self.sign_item_capacity == 0
            || self.interrupt_fact_capacity == 0
            || self.runtime_arena_bytes == 0
            || self.runtime_arena_bytes > u64::from(u32::MAX)
        {
            return Err(OfferError::InvalidCapacity);
        }
        for resource in self.resources {
            if resource.class.is_empty() || resource.capacity == 0 {
                return Err(OfferError::InvalidCapacity);
            }
            let Some(base) = self.bases.iter().find(|base| base.kind == resource.base) else {
                return Err(OfferError::MissingBase);
            };
            if resource.capacity > base.capacity {
                return Err(OfferError::InvalidCapacity);
            }
        }
        for capability in self.capabilities {
            if capability.kind.is_empty()
                || capability.contract_revision.is_empty()
                || capability.implementation.is_empty()
                || capability.artifact_build.is_empty()
                || capability.host_operation.is_empty()
                || capability.maximum_in_flight == 0
                || capability.maximum_input_bytes == 0
            {
                return Err(OfferError::InvalidCapacity);
            }
            let Some(required_base) = self
                .bases
                .iter()
                .find(|base| base.kind == capability.required_base)
            else {
                return Err(OfferError::MissingBase);
            };
            if u32::from(capability.maximum_in_flight) > required_base.capacity {
                return Err(OfferError::InvalidCapacity);
            }
            if capability
                .secondary_base
                .is_some_and(|kind| !self.bases.iter().any(|base| base.kind == kind))
            {
                return Err(OfferError::MissingBase);
            }
            for port in [capability.input, capability.output].into_iter().flatten() {
                if port.name.is_empty() || port.value_kind.is_empty() || !port.closes {
                    return Err(OfferError::InvalidCapacity);
                }
            }
            if capability
                .input
                .is_some_and(|port| port.direction != PortDirection::Input)
                || capability
                    .output
                    .is_some_and(|port| port.direction != PortDirection::Output)
                || (capability.input.is_none() && capability.output.is_none())
            {
                return Err(OfferError::InvalidCapacity);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IsaRequirement {
    pub sse2: bool,
    pub rdrand: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImplementationCandidate {
    pub id: &'static str,
    pub boot_id: [u8; 32],
    pub offer_requirement: IsaRequirement,
    pub artifact_requirement: IsaRequirement,
}

pub fn select_equal_face<'a>(
    offer: &HostOffer<'_>,
    candidates: &'a [ImplementationCandidate],
) -> Result<&'a ImplementationCandidate, OfferError> {
    for candidate in candidates {
        if candidate.boot_id != offer.boot_id {
            continue;
        }
        if candidate.offer_requirement != candidate.artifact_requirement {
            return Err(OfferError::ArtifactRequirementMismatch);
        }
        let requirement = candidate.offer_requirement;
        if (!requirement.sse2 || offer.cpu_features.sse2)
            && (!requirement.rdrand || offer.cpu_features.rdrand)
        {
            return Ok(candidate);
        }
    }
    if candidates
        .iter()
        .all(|candidate| candidate.boot_id != offer.boot_id)
    {
        Err(OfferError::StaleObservation)
    } else {
        Err(OfferError::MissingIsaFeature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(features: CpuFeatures) -> HostOffer<'static> {
        HostOffer::new(
            &BootIdentities {
                host: [1; 32],
                boot: [2; 32],
            },
            "build",
            features,
            262_144,
        )
    }

    #[test]
    fn exact_boot_offer_is_finite_and_bases_do_not_imply_authority() {
        let offer = offer(CpuFeatures {
            sse2: true,
            rdrand: false,
            invariant_tsc: true,
        });
        assert_eq!(offer.validate(), Ok(()));
        assert_eq!(offer.resources[0].capacity, 262_144);
        assert_eq!(offer.resources[0].base, BaseKind::Memory);
        assert_eq!(offer.capabilities[1].maximum_in_flight, 1);
        assert_eq!(
            offer.capabilities[1].host_operation,
            "conduit.host/present@1"
        );
        assert_eq!(
            offer.capabilities[0].output,
            Some(PortOffer {
                name: "tick",
                value_kind: "conduit.value/tick@1",
                direction: PortDirection::Output,
                closes: true,
            })
        );
        assert_eq!(offer.capabilities[0].input, None);
        assert_eq!(
            offer.capabilities[1].input.map(|port| port.direction),
            Some(PortDirection::Input)
        );
    }

    #[test]
    fn isa_admission_rejects_stale_missing_and_disagreeing_facts() {
        let offer = offer(CpuFeatures {
            sse2: true,
            rdrand: false,
            invariant_tsc: true,
        });
        let scalar = ImplementationCandidate {
            id: "scalar",
            boot_id: offer.boot_id,
            offer_requirement: IsaRequirement {
                sse2: true,
                rdrand: false,
            },
            artifact_requirement: IsaRequirement {
                sse2: true,
                rdrand: false,
            },
        };
        let vector = ImplementationCandidate {
            id: "rdrand",
            boot_id: offer.boot_id,
            offer_requirement: IsaRequirement {
                sse2: true,
                rdrand: true,
            },
            artifact_requirement: IsaRequirement {
                sse2: true,
                rdrand: true,
            },
        };
        assert_eq!(
            select_equal_face(&offer, &[vector, scalar]).unwrap().id,
            "scalar"
        );
        assert_eq!(
            select_equal_face(&offer, &[vector]),
            Err(OfferError::MissingIsaFeature)
        );

        let mut stale = scalar;
        stale.boot_id = [9; 32];
        assert_eq!(
            select_equal_face(&offer, &[stale]),
            Err(OfferError::StaleObservation)
        );

        let mut disagreeing = scalar;
        disagreeing.artifact_requirement.rdrand = true;
        assert_eq!(
            select_equal_face(&offer, &[disagreeing]),
            Err(OfferError::ArtifactRequirementMismatch)
        );
    }

    #[test]
    fn malformed_memory_and_port_facts_fail_closed() {
        let features = CpuFeatures {
            sse2: true,
            rdrand: false,
            invariant_tsc: true,
        };
        let mut missing_memory = offer(features);
        missing_memory.runtime_arena_bytes = 0;
        assert_eq!(missing_memory.validate(), Err(OfferError::InvalidCapacity));

        let mut oversized_resource = offer(features);
        oversized_resource.resources[2].capacity = 2;
        assert_eq!(
            oversized_resource.validate(),
            Err(OfferError::InvalidCapacity)
        );

        let mut wrong_direction = offer(features);
        wrong_direction.capabilities[1]
            .input
            .as_mut()
            .unwrap()
            .direction = PortDirection::Output;
        assert_eq!(wrong_direction.validate(), Err(OfferError::InvalidCapacity));
    }
}
