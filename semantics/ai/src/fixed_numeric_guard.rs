//! Exact bounded structured pass-through gated by an explicit true Boolean.
//! The profile admits structure only; native laws require their separate owner.
use crate::fixed_numeric_preparation::verify_fixed_placement;
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
pub const GUARD_IMPLEMENTATION: &str = "conduit.value/closing-flow-assert-true@1";
pub const MAXIMUM_GUARD_BYTES: u32 = 16_384;
#[derive(Clone)]
pub struct FixedGuardProfile {
    value_type: StructuredInfoType,
    maximum_bytes: u32,
    identity: String,
}
impl FixedGuardProfile {
    pub fn prepare(value_type: StructuredInfoType) -> Result<Self, String> {
        let maximum_bytes = conduit_plot::maximum_prepared_canonical_value_bytes(&value_type)
            .map_err(|e| format!("{e:?}"))?;
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_GUARD_BYTES {
            return Err("guard frame bound".into());
        }
        let digest = value_type.semantic_digest().map_err(|e| format!("{e:?}"))?;
        let mut hexadecimal = String::new();
        for byte in digest {
            use core::fmt::Write;
            write!(&mut hexadecimal, "{byte:02x}").map_err(|_| String::from("guard identity"))?;
        }
        Ok(Self {
            value_type,
            maximum_bytes,
            identity: format!("value/flow-assert-true/typed-{hexadecimal}"),
        })
    }
    pub fn value_type(&self) -> &StructuredInfoType {
        &self.value_type
    }
    pub fn contract(&self) -> Result<Kind, String> {
        let frame_kind = self
            .value_type
            .profile()
            .map_err(|e| format!("{e:?}"))?
            .value_kind()
            .clone();
        let port = |name: &str, value_kind: KindId, direction| PortDescriptor {
            port_id: port_id(name),
            value_kind,
            direction,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        };
        let value = port("value", frame_kind.clone(), PortDirection::Input);
        let condition = port("condition", kind_id(BOOL_INFO_ID), PortDirection::Input);
        let result = port("result", frame_kind, PortDirection::Output);
        let contracts = [
            (&value, self.maximum_bytes),
            (&condition, 1),
            (&result, self.maximum_bytes),
        ]
        .into_iter()
        .map(|(p, maximum)| FrontValueContract {
            location: match p.direction {
                PortDirection::Input => FrontValueLocation::Input(p.port_id.clone()),
                PortDirection::Output => FrontValueLocation::Output(p.port_id.clone()),
            },
            contract: CheckedValueContract::new(p.value_kind.clone(), maximum, vec![])
                .expect("bounded shape"),
        })
        .collect();
        Ok(Kind {
            kind_id: kind_id(&self.identity),
            kind_contract_revision: KindIdentity::from(GUARD_IMPLEMENTATION),
            startup_parameters: vec![],
            shorthand: None,
            configuration: vec![],
            inputs: vec![value, condition],
            outputs: vec![result],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(contracts)],
            limits: CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 1,
                max_queue_bytes: MAXIMUM_GUARD_BYTES,
            },
        })
    }
    pub fn offer(&self) -> Result<CapabilityOffer, String> {
        Ok(BackOfferBuilder::new(
            self.contract()?,
            Back {
                capability_id: CapabilityId::from(format!(
                    "{GUARD_IMPLEMENTATION}/{}",
                    self.identity
                )),
                execution_profile_id: ExecutionProfileId::from(GUARD_IMPLEMENTATION),
                implementation_id: ImplementationId::from(GUARD_IMPLEMENTATION),
                artifact_id: ArtifactId::from(GUARD_IMPLEMENTATION),
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
        startup.insert_structured_type(
            format!("{}/candidate", self.identity),
            self.value_type.clone(),
        )?;
        profiles.insert_kind(kind).map_err(|e| format!("{e:?}"))
    }
}
pub struct FixedGuardBack {
    validator: PreparedStructuredValueValidator,
    output: Vec<u8>,
    maximum_bytes: usize,
    staged: bool,
    committed_frames: u64,
    cancelled: bool,
}
impl FixedGuardBack {
    pub fn prepare_planned<const PORTS: usize>(
        profile: &FixedGuardProfile,
        placement: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, String> {
        if PORTS < 2 || fuel < 3 {
            return Err("guard step budget".into());
        }
        verify_fixed_placement(placement, &profile.offer()?).map_err(|e| format!("{e:?}"))?;
        let maximum_bytes = profile.maximum_bytes as usize;
        let validator = PreparedStructuredValueValidator::new(&profile.value_type, maximum_bytes)
            .map_err(|e| format!("{e:?}"))?;
        Ok(Self {
            validator,
            output: Vec::with_capacity(maximum_bytes),
            maximum_bytes,
            staged: false,
            committed_frames: 0,
            cancelled: false,
        })
    }
    pub fn committed_frames(&self) -> u64 {
        self.committed_frames
    }
}
impl<const PORTS: usize> StepBack<PORTS> for FixedGuardBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1300,
            });
        }
        if PORTS < 2 || self.committed_frames == u64::MAX {
            return fail(2800);
        }
        let closed = [io.input_closed(PortId(0)), io.input_closed(PortId(1))];
        if closed == [true, true] {
            return StepOutcome::Complete;
        }
        if (closed[0] && io.input(PortId(1)).is_some())
            || (closed[1] && io.input(PortId(0)).is_some())
        {
            return fail(2801);
        }
        if io.input(PortId(0)).is_none()
            || io.input(PortId(1)).is_none()
            || !io.output_ready(PortId(0))
        {
            return StepOutcome::Await;
        }
        let (Some(value), Some(condition)) = (inputs.input(PortId(0)), inputs.input(PortId(1)))
        else {
            return fail(2802);
        };
        if condition != [1]
            || value.len() > self.maximum_bytes
            || self.validator.validate(value).is_err()
        {
            return fail(2803);
        }
        self.output.clear();
        self.output.extend_from_slice(value);
        if io.consume(PortId(0)).is_err()
            || io.consume(PortId(1)).is_err()
            || io
                .send_prepared(PortId(0), self.output.len() as u32)
                .is_err()
        {
            return fail(2804);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.committed_frames += 1;
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then_some(&self.output)
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

impl FixedGuardBack {
    /// Retained requested local payload capacity. Shared tensor/model Arc owners,
    /// Box root, Arc headers, allocator bookkeeping and stack are separate charges.
    pub fn local_accounted_heap_bytes(&self) -> usize {
        self.validator
            .owned_heap_bytes()
            .saturating_add(self.output.capacity())
    }
}

mod storage;
pub use storage::*;
