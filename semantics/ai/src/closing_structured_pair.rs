//! Atomic pairing of exact bounded primitive or structured closing Flows.
//! This owner admits structure only. Native laws require their separate owner.
use crate::fixed_numeric_preparation::verify_fixed_placement;
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
pub const PAIR_IMPLEMENTATION: &str = "conduit.structure/closing-flow-atomic-pair@1";
pub const MAXIMUM_PAIR_BYTES: u32 = 16_384;
#[derive(Clone)]
pub struct ClosingStructuredPairProfile {
    left: StructuredInfoType,
    right: StructuredInfoType,
    paired: StructuredInfoType,
    maximum: [u32; 3],
    identity: String,
}
impl ClosingStructuredPairProfile {
    pub fn prepare(left: StructuredInfoType, right: StructuredInfoType) -> Result<Self, String> {
        let bound = |ty: &StructuredInfoType| {
            crate::transport_envelope::maximum_prepared_transport_value_bytes(ty)
                .map_err(|e| format!("{e:?}"))
        };
        let left_maximum = bound(&left)?;
        let right_maximum = bound(&right)?;
        let encoder = PreparedTypedTuplePairEncoder::new(
            left.clone(),
            left_maximum,
            right.clone(),
            right_maximum,
        )
        .map_err(|e| format!("{e:?}"))?;
        if encoder.maximum_bytes() > MAXIMUM_PAIR_BYTES {
            return Err("atomic pair exceeds selected finite envelope".into());
        }
        let mut hexadecimal = String::new();
        for ty in [&left, &right] {
            for byte in ty.semantic_digest().map_err(|e| format!("{e:?}"))? {
                use core::fmt::Write;
                write!(&mut hexadecimal, "{byte:02x}")
                    .map_err(|_| String::from("pair identity"))?;
            }
        }
        Ok(Self {
            left,
            right,
            paired: encoder.value_type().clone(),
            maximum: [left_maximum, right_maximum, encoder.maximum_bytes()],
            identity: format!("structure/flow-atomic-pair/typed-{hexadecimal}"),
        })
    }
    pub fn value_type(&self) -> &StructuredInfoType {
        &self.paired
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn maximum_bytes(&self) -> u32 {
        self.maximum[2]
    }
    pub fn contract(&self) -> Result<Kind, String> {
        let info_kind = |ty: &StructuredInfoType| match ty.shape() {
            StructuredInfoTypeShape::Leaf(kind) => Ok(kind.clone()),
            _ => ty
                .profile()
                .map(|p| p.value_kind().clone())
                .map_err(|e| format!("{e:?}")),
        };
        let port = |name, ty: &StructuredInfoType, direction| {
            Ok::<_, String>(PortDescriptor {
                port_id: port_id(name),
                value_kind: info_kind(ty)?,
                direction,
                temporal: PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            })
        };
        let left = port("left", &self.left, PortDirection::Input)?;
        let right = port("right", &self.right, PortDirection::Input)?;
        let paired = port("paired", &self.paired, PortDirection::Output)?;
        let contracts = [
            (&left, self.maximum[0]),
            (&right, self.maximum[1]),
            (&paired, self.maximum[2]),
        ]
        .into_iter()
        .map(|(p, maximum)| FrontValueContract {
            location: match p.direction {
                PortDirection::Input => FrontValueLocation::Input(p.port_id.clone()),
                PortDirection::Output => FrontValueLocation::Output(p.port_id.clone()),
            },
            contract: CheckedValueContract::new(p.value_kind.clone(), maximum, vec![])
                .expect("exact bounded shape"),
        })
        .collect();
        Ok(Kind {
            kind_id: kind_id(&self.identity),
            kind_contract_revision: KindIdentity::from(PAIR_IMPLEMENTATION),
            startup_parameters: vec![],
            shorthand: None,
            configuration: vec![],
            inputs: vec![left, right],
            outputs: vec![paired],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(contracts)],
            limits: CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 1,
                max_queue_bytes: MAXIMUM_PAIR_BYTES,
            },
        })
    }
    pub fn offer(&self) -> Result<CapabilityOffer, String> {
        Ok(BackOfferBuilder::new(
            self.contract()?,
            Back {
                capability_id: CapabilityId::from(format!(
                    "{PAIR_IMPLEMENTATION}/{}",
                    self.identity
                )),
                execution_profile_id: ExecutionProfileId::from(
                    "structure/atomic-closing-pair-frame16k@1",
                ),
                implementation_id: ImplementationId::from(PAIR_IMPLEMENTATION),
                artifact_id: ArtifactId::from(PAIR_IMPLEMENTATION),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build())
    }
    pub fn install(
        &self,
        startup: &mut conduit_plot::StartupCatalog,
        profiles: &mut conduit_plot::ProfileCatalog,
    ) -> Result<(), String> {
        let kind = self.contract()?;
        startup.insert(conduit_plot::KindSignature {
            kind: self.identity.clone(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(&self.identity, kind.checked_front())?;
        startup.insert_structured_type(format!("{}/paired", self.identity), self.paired.clone())?;
        profiles.insert_kind(kind).map_err(|e| format!("{e:?}"))
    }
}
pub struct ClosingStructuredPairBack {
    encoder: PreparedTypedTuplePairEncoder,
    staged: bool,
    committed_pairs: u64,
    cancelled: bool,
}
impl ClosingStructuredPairBack {
    pub fn prepare_planned<const PORTS: usize>(
        profile: &ClosingStructuredPairProfile,
        gear: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, String> {
        if PORTS < 2 || fuel < 3 {
            return Err("atomic pair Step budget".into());
        }
        verify_fixed_placement(gear, &profile.offer()?).map_err(|e| format!("{e:?}"))?;
        Ok(Self {
            encoder: PreparedTypedTuplePairEncoder::new(
                profile.left.clone(),
                profile.maximum[0],
                profile.right.clone(),
                profile.maximum[1],
            )
            .map_err(|e| format!("{e:?}"))?,
            staged: false,
            committed_pairs: 0,
            cancelled: false,
        })
    }
    pub fn committed_pairs(&self) -> u64 {
        self.committed_pairs
    }
}
impl<const PORTS: usize> StepBack<PORTS> for ClosingStructuredPairBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 2900,
            });
        }
        if PORTS < 2 || self.committed_pairs == u64::MAX {
            return fail(2901);
        }
        // A queued member remains unconsumed while its opposite computation
        // runs. Its producer's close is therefore not a drained input close.
        if (0..2).any(|p| io.input_closed(PortId(p))) {
            for p in 0..2 {
                if io.input(PortId(p)).is_some() {
                    if io.consume(PortId(p)).is_err() {
                        return fail(2902);
                    }
                    return StepOutcome::Progress; // discard unmatched finite tail
                }
            }
            return if (0..2).all(|p| io.input_closed(PortId(p))) {
                StepOutcome::Complete
            } else {
                StepOutcome::Await
            };
        }
        if (0..2).any(|p| io.input(PortId(p)).is_none()) || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(left), Some(right)) = (inputs.input(PortId(0)), inputs.input(PortId(1))) else {
            return fail(2903);
        };
        let encoded = match self.encoder.encode(left, right) {
            Ok(encoded) => encoded,
            Err(_) => return fail(2904),
        };
        if io.consume(PortId(0)).is_err()
            || io.consume(PortId(1)).is_err()
            || io.send_prepared(PortId(0), encoded.len() as u32).is_err()
        {
            return fail(2905);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then(|| self.encoder.encoded())
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.committed_pairs += 1;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
