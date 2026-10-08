//! Source-checked primitive U16 profile admission; no domain names or ranges in runtime.
use crate::fixed_numeric_preparation::verify_fixed_placement;
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
use conduit_plot::{CheckedNativeType, PreparedPortableExpressionEvaluator};
use sha2::{Digest, Sha256};
pub const U16_PROFILE_IMPLEMENTATION: &str = "conduit.numeric/source-u16-profile@1";
pub const MAXIMUM_U16_PROFILE_LAWS: usize = 32;
pub const MAXIMUM_U16_PROFILE_PROGRAM_BYTES: usize = 65536;
pub const MAXIMUM_U16_PROFILE_VALUE_BYTES: usize = 4096;
/// The constructor rechecks authored declaration text. Public mutable checked-Type
/// records cannot be supplied as a substitute for this private admission receipt.
pub struct PreparedU16Profile {
    checked: CheckedNativeType,
    identity: String,
    maximum_bytes: u32,
    definition_digest: [u8; 32],
}
impl PreparedU16Profile {
    pub fn check_definition(source: &str) -> Result<Self, String> {
        if source.len() > MAXIMUM_U16_PROFILE_PROGRAM_BYTES {
            return Err("U16 declaration byte bound".into());
        }
        let syntax = conduit_plot::parse_syntax_document(source);
        if !syntax.diagnostics.is_empty()
            || syntax.types.len() != 1
            || !syntax.plots.is_empty()
            || !syntax.uses.is_empty()
        {
            return Err("exact standalone U16 Type declaration required".into());
        }
        let document =
            conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new())
                .map_err(|e| format!("{e:?}"))?;
        let checked = document
            .native_types
            .into_iter()
            .next()
            .ok_or("absent checked Type")?;
        let StructuredInfoTypeShape::Nominal { representation, .. } = checked.value_type.shape()
        else {
            return Err("nominal scalar profile required".into());
        };
        if !matches!(representation.shape(),StructuredInfoTypeShape::Leaf(kind) if kind.as_str()=="value/u16")
        {
            return Err("exact U16 representation required".into());
        }
        if checked.value_contracts.len() > MAXIMUM_U16_PROFILE_LAWS
            || checked.invariants.len() > MAXIMUM_U16_PROFILE_LAWS
            || checked.value_contracts.iter().any(|c| {
                !c.representation_path.is_empty() || c.contract.value_kind != kind_id("value/u16")
            })
        {
            return Err("U16 profile law bound/path".into());
        }
        let mut program_bytes = 0usize;
        for program in &checked.invariants {
            if program.input_type != checked.value_type
                || program.output_type
                    != StructuredInfoType::leaf(kind_id(BOOL_INFO_ID))
                        .map_err(|e| format!("{e:?}"))?
            {
                return Err("exact Boolean invariant signature".into());
            }
            program_bytes = program_bytes
                .checked_add(
                    program
                        .canonical_bytes()
                        .map_err(|e| format!("{e:?}"))?
                        .len(),
                )
                .ok_or("invariant byte overflow")?;
            PreparedPortableExpressionEvaluator::new(program).map_err(|e| format!("{e:?}"))?;
        }
        if program_bytes > MAXIMUM_U16_PROFILE_PROGRAM_BYTES {
            return Err("U16 invariant byte bound".into());
        }
        let maximum_bytes =
            crate::transport_envelope::maximum_prepared_transport_value_bytes(&checked.value_type)
                .map_err(|e| format!("{e:?}"))?;
        if maximum_bytes as usize > MAXIMUM_U16_PROFILE_VALUE_BYTES {
            return Err("U16 profile value envelope".into());
        }
        let digest = Sha256::digest(
            checked
                .value_type
                .canonical_bytes()
                .map_err(|e| format!("{e:?}"))?,
        );
        let mut identity = String::from("sha256-");
        for byte in digest {
            use core::fmt::Write;
            write!(&mut identity, "{byte:02x}").map_err(|_| "digest formatting")?;
        }
        Ok(Self {
            checked,
            identity,
            maximum_bytes,
            definition_digest: Sha256::digest(source.as_bytes()).into(),
        })
    }
    pub fn value_type(&self) -> &StructuredInfoType {
        &self.checked.value_type
    }
    pub fn definition_digest(&self) -> [u8; 32] {
        self.definition_digest
    }
    pub fn kind_identity(&self, flow: bool) -> String {
        format!(
            "numeric/{}u16-profile/{}",
            if flow { "flow-" } else { "" },
            self.identity
        )
    }
    pub fn contract(&self, flow: bool) -> Result<Kind, String> {
        let output_kind = self
            .checked
            .value_type
            .profile()
            .map_err(|e| format!("{e:?}"))?
            .value_kind()
            .clone();
        let temporal = if flow {
            PortTemporal::Flow { closes: true }
        } else {
            PortTemporal::Value
        };
        let input = PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id("value/u16"),
            direction: PortDirection::Input,
            temporal,
            abnormal_kind: None,
        };
        let output = PortDescriptor {
            port_id: port_id("result"),
            value_kind: output_kind,
            direction: PortDirection::Output,
            temporal,
            abnormal_kind: None,
        };
        let laws = [(&input, 2), (&output, self.maximum_bytes)]
            .into_iter()
            .map(|(p, bound)| FrontValueContract {
                location: match p.direction {
                    PortDirection::Input => FrontValueLocation::Input(p.port_id.clone()),
                    PortDirection::Output => FrontValueLocation::Output(p.port_id.clone()),
                },
                contract: CheckedValueContract::new(p.value_kind.clone(), bound, vec![])
                    .expect("exact finite primitive/profile envelope"),
            })
            .collect();
        let identity = self.kind_identity(flow);
        Ok(Kind {
            kind_id: kind_id(&identity),
            kind_contract_revision: KindIdentity::from(format!(
                "{U16_PROFILE_IMPLEMENTATION}/{identity}"
            )),
            startup_parameters: vec![],
            shorthand: None,
            configuration: vec![],
            inputs: vec![input],
            outputs: vec![output],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(laws)],
            limits: CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 1,
                max_queue_bytes: self.maximum_bytes,
            },
        })
    }
    pub fn offer(&self, flow: bool) -> Result<CapabilityOffer, String> {
        let kind = self.contract(flow)?;
        let identity = String::from(kind.kind_id.as_str());
        Ok(BackOfferBuilder::new(
            kind,
            Back {
                capability_id: CapabilityId::from(format!(
                    "{U16_PROFILE_IMPLEMENTATION}/{identity}"
                )),
                execution_profile_id: ExecutionProfileId::from(U16_PROFILE_IMPLEMENTATION),
                implementation_id: ImplementationId::from(U16_PROFILE_IMPLEMENTATION),
                artifact_id: ArtifactId::from(U16_PROFILE_IMPLEMENTATION),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build())
    }
    /// Installs the exact executable Fore only; the authored document retains
    /// its own Type declaration and must expose that identical native profile.
    pub fn install(
        &self,
        startup: &mut conduit_plot::StartupCatalog,
        profiles: &mut conduit_plot::ProfileCatalog,
        flow: bool,
    ) -> Result<(), String> {
        let kind = self.contract(flow)?;
        startup.insert(conduit_plot::KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        profiles.insert_kind(kind).map_err(|e| format!("{e:?}"))?;
        Ok(())
    }
}
pub struct U16ProfileBack {
    output: Vec<u8>,
    scalar_offset: usize,
    validator: PreparedStructuredValueValidator,
    contracts: Vec<CheckedValueContract>,
    invariants: Vec<PreparedPortableExpressionEvaluator>,
    flow: bool,
    staged: bool,
    finished: bool,
    cancelled: bool,
    committed_frames: u64,
}
impl U16ProfileBack {
    pub fn prepare_planned<const PORTS: usize>(
        gear: &PlannedGear,
        fuel: u16,
        profile: &PreparedU16Profile,
        flow: bool,
    ) -> Result<Self, String> {
        if PORTS < 1 || fuel < 2 {
            return Err("U16 profile step bound".into());
        }
        verify_fixed_placement(gear, &profile.offer(flow)?).map_err(|e| format!("{e:?}"))?;
        let StructuredInfoTypeShape::Nominal { representation, .. } =
            profile.checked.value_type.shape()
        else {
            return Err("admitted nominal profile required".into());
        };
        let primitive = StructuredInfoValue::leaf(representation.clone(), vec![0, 0])
            .map_err(|e| format!("{e:?}"))?;
        let output = StructuredInfoValue::nominal(profile.checked.value_type.clone(), primitive)
            .map_err(|e| format!("{e:?}"))?
            .canonical_bytes()
            .map_err(|e| format!("{e:?}"))?;
        if output.len() != profile.maximum_bytes as usize {
            return Err("exact scalar framing envelope".into());
        }
        let scalar_offset = output.len().checked_sub(2).ok_or("scalar offset")?;
        let validator =
            PreparedStructuredValueValidator::new(&profile.checked.value_type, output.len())
                .map_err(|e| format!("{e:?}"))?;
        validator.validate(&output).map_err(|e| format!("{e:?}"))?;
        let invariants = profile
            .checked
            .invariants
            .iter()
            .map(|program| {
                PreparedPortableExpressionEvaluator::new(program).map_err(|e| format!("{e:?}"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            output,
            scalar_offset,
            validator,
            contracts: profile
                .checked
                .value_contracts
                .iter()
                .map(|c| c.contract.clone())
                .collect(),
            invariants,
            flow,
            staged: false,
            finished: false,
            cancelled: false,
            committed_frames: 0,
        })
    }
    pub fn committed_frames(&self) -> u64 {
        self.committed_frames
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
impl<const PORTS: usize> StepBack<PORTS> for U16ProfileBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 2890,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if io.input_closed(PortId(0)) {
            return if self.flow {
                StepOutcome::Complete
            } else {
                fail(2891)
            };
        }
        if self.committed_frames == u64::MAX {
            return fail(2892);
        }
        if io.input(PortId(0)).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = inputs.input(PortId(0)) else {
            return fail(2893);
        };
        if bytes.len() != 2 {
            return fail(2894);
        }
        self.output[self.scalar_offset..].copy_from_slice(bytes);
        if self.validator.validate(&self.output).is_err()
            || self
                .contracts
                .iter()
                .any(|contract| contract.validate(bytes).is_err())
        {
            return fail(2895);
        }
        for invariant in &mut self.invariants {
            let Ok(result) = invariant.evaluate(&self.output) else {
                return fail(2896);
            };
            if !matches!(InfoBool::decode(result),Ok(value) if value.get()) {
                return fail(2897);
            }
        }
        if io.consume(PortId(0)).is_err()
            || io
                .send_prepared(PortId(0), self.output.len() as u32)
                .is_err()
        {
            return fail(2898);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then_some(self.output.as_slice())
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.committed_frames += 1;
            self.finished = !self.flow;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}

impl U16ProfileBack {
    /// Retained requested local payload capacity. Shared tensor/model Arc owners,
    /// Box root, Arc headers, allocator bookkeeping and stack are separate charges.
    pub fn local_accounted_heap_bytes(&self) -> usize {
        self.output
            .capacity()
            .saturating_add(self.validator.owned_heap_bytes())
            .saturating_add(
                self.contracts
                    .capacity()
                    .saturating_mul(core::mem::size_of::<CheckedValueContract>()),
            )
            .saturating_add(self.contracts.iter().fold(0usize, |total, v| {
                total.saturating_add(v.owned_heap_bytes())
            }))
            .saturating_add(
                self.invariants
                    .capacity()
                    .saturating_mul(core::mem::size_of::<PreparedPortableExpressionEvaluator>()),
            )
            .saturating_add(self.invariants.iter().fold(0usize, |total, v| {
                total.saturating_add(v.owned_heap_bytes())
            }))
    }
}
