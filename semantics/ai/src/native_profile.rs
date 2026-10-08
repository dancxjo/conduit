use crate::native_traversal::{
    PreparedStructuredContractValidator, PreparedStructuredValueValidator,
};
// Exact Source-checked native admission from a distinct unrefined candidate.
use crate::fixed_numeric_preparation::verify_fixed_placement;
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
use conduit_plot::{CheckedNativeType, PreparedPortableExpressionEvaluator};
use sha2::{Digest, Sha256};
pub const NATIVE_PROFILE_IMPLEMENTATION: &str = "conduit.structure/source-native-profile@1";
pub const MAXIMUM_NATIVE_PROFILE_VALUE_BYTES: usize = 16384;
pub const MAXIMUM_NATIVE_PROFILE_SOURCE_BYTES: usize = 262144;
pub const MAXIMUM_NATIVE_PROFILE_PROGRAM_BYTES: usize = 4194304;
pub const MAXIMUM_NATIVE_PROFILE_LAWS: usize = 256;

/// Rechecks authored Type declarations; mutable checked records do not confer admission.
pub struct PreparedNativeProfile {
    checked: CheckedNativeType,
    context: Vec<CheckedNativeType>,
    candidate: StructuredInfoType,
    candidate_kind: KindId,
    native_kind: KindId,
    identity: String,
    candidate_maximum: u32,
    maximum: u32,
    definition_digest: [u8; 32],
}
fn raw_type(ty: &StructuredInfoType) -> Result<StructuredInfoType, StructuredInfoRefusal> {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => raw_type(representation),
        StructuredInfoTypeShape::Leaf(_) => Ok(ty.clone()),
        StructuredInfoTypeShape::Collection { element, length } => {
            StructuredInfoType::collection(raw_type(element)?, Some(length))
        }
        StructuredInfoTypeShape::Sequence {
            element,
            minimum_items,
            maximum_items,
        } => StructuredInfoType::bounded_sequence(raw_type(element)?, minimum_items, maximum_items),
        StructuredInfoTypeShape::Record { schema, fields } => StructuredInfoType::record(
            schema.clone(),
            fields
                .iter()
                .map(|f| StructuredFieldType::new(f.name(), raw_type(f.value_type())?))
                .collect::<Result<_, _>>()?,
        ),
        StructuredInfoTypeShape::Variant { schema, cases } => StructuredInfoType::variant(
            schema.clone(),
            cases
                .iter()
                .map(|c| StructuredVariantCase::new(c.tag(), raw_type(c.payload_type())?))
                .collect::<Result<_, _>>()?,
        ),
    }
}
fn contains(root: &StructuredInfoType, selected: &StructuredInfoType) -> bool {
    root == selected
        || match root.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                contains(representation, selected)
            }
            StructuredInfoTypeShape::Collection { element, .. }
            | StructuredInfoTypeShape::Sequence { element, .. } => contains(element, selected),
            StructuredInfoTypeShape::Record { fields, .. } => {
                fields.iter().any(|f| contains(f.value_type(), selected))
            }
            StructuredInfoTypeShape::Variant { cases, .. } => {
                cases.iter().any(|c| contains(c.payload_type(), selected))
            }
            StructuredInfoTypeShape::Leaf(_) => false,
        }
}
impl PreparedNativeProfile {
    pub fn check_definition(source: &str, name: &str) -> Result<Self, String> {
        if source.len() > MAXIMUM_NATIVE_PROFILE_SOURCE_BYTES {
            return Err("native profile Source byte bound".into());
        }
        let syntax = conduit_plot::parse_syntax_document(source);
        if !syntax.diagnostics.is_empty() || !syntax.plots.is_empty() || !syntax.uses.is_empty() {
            return Err(format!(
                "self-contained Type declarations required: {:?}",
                syntax.diagnostics
            ));
        }
        let document =
            conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new())
                .map_err(|e| format!("{e:?}"))?;
        let checked = document
            .native_types
            .iter()
            .find(|t| t.name == name)
            .cloned()
            .ok_or("selected native Type absent")?;
        let raw = raw_type(&checked.value_type).map_err(|e| format!("{e:?}"))?;
        let mut schema_identity = String::from("conduit.structure/candidate-");
        for byte in checked
            .value_type
            .semantic_digest()
            .map_err(|e| format!("{e:?}"))?
        {
            use core::fmt::Write;
            write!(&mut schema_identity, "{byte:02x}").map_err(|_| "digest formatting")?;
        }
        let candidate = match raw.shape() {
            StructuredInfoTypeShape::Record { fields, .. } => {
                StructuredInfoType::record(kind_id(&schema_identity), fields.to_vec())
            }
            StructuredInfoTypeShape::Variant { cases, .. } => {
                StructuredInfoType::variant(kind_id(&schema_identity), cases.to_vec())
            }
            StructuredInfoTypeShape::Leaf(_) => {
                return Err(
                    "structured candidate required; primitive guards use primitive owners".into(),
                );
            }
            _ => StructuredInfoType::nominal(kind_id(&schema_identity), raw.clone()),
        }
        .map_err(|e| format!("{e:?}"))?;
        if matches!(candidate.shape(), StructuredInfoTypeShape::Leaf(_)) {
            return Err(
                "structured candidate required; primitive guards use primitive owners".into(),
            );
        }
        let context = document
            .native_types
            .into_iter()
            .filter(|ty| contains(&checked.value_type, &ty.value_type))
            .collect::<Vec<_>>();
        let mut law_count = 0usize;
        let mut program_bytes = 0usize;
        for ty in &context {
            law_count = law_count
                .checked_add(ty.value_contracts.len() + ty.invariants.len())
                .ok_or("native law count overflow")?;
            for program in &ty.invariants {
                if program.input_type != ty.value_type
                    || program.output_type
                        != StructuredInfoType::leaf(kind_id(BOOL_INFO_ID))
                            .map_err(|e| format!("{e:?}"))?
                {
                    return Err("native invariant exact Boolean signature".into());
                }
                program_bytes = program_bytes
                    .checked_add(
                        program
                            .canonical_bytes()
                            .map_err(|e| format!("{e:?}"))?
                            .len(),
                    )
                    .ok_or("native program byte overflow")?;
                PreparedPortableExpressionEvaluator::new(program).map_err(|e| format!("{e:?}"))?;
            }
        }
        if law_count > MAXIMUM_NATIVE_PROFILE_LAWS
            || program_bytes > MAXIMUM_NATIVE_PROFILE_PROGRAM_BYTES
        {
            return Err("native profile law/program bound".into());
        }
        let maximum =
            crate::transport_envelope::maximum_prepared_transport_value_bytes(&checked.value_type)
                .map_err(|e| format!("{e:?}"))?;
        let candidate_maximum =
            crate::transport_envelope::maximum_prepared_transport_value_bytes(&candidate)
                .map_err(|e| format!("{e:?}"))?;
        if maximum as usize > MAXIMUM_NATIVE_PROFILE_VALUE_BYTES
            || candidate_maximum as usize > MAXIMUM_NATIVE_PROFILE_VALUE_BYTES
        {
            return Err("native profile value envelope".into());
        }
        let definition_digest: [u8; 32] = Sha256::digest(source.as_bytes()).into();
        let mut hash = Sha256::new();
        hash.update(b"conduit.source-native-profile.v1");
        hash.update(definition_digest);
        hash.update(name.as_bytes());
        hash.update(
            checked
                .value_type
                .canonical_bytes()
                .map_err(|e| format!("{e:?}"))?,
        );
        let mut identity = String::new();
        for byte in hash.finalize() {
            use core::fmt::Write;
            write!(&mut identity, "{byte:02x}").map_err(|_| "digest formatting")?;
        }
        let candidate_kind = candidate
            .profile()
            .map_err(|e| format!("{e:?}"))?
            .value_kind()
            .clone();
        let native_kind = checked
            .value_type
            .profile()
            .map_err(|e| format!("{e:?}"))?
            .value_kind()
            .clone();
        Ok(Self {
            candidate_kind,
            native_kind,
            checked,
            context,
            candidate,
            identity,
            candidate_maximum,
            maximum,
            definition_digest,
        })
    }
    pub fn value_type(&self) -> &StructuredInfoType {
        &self.checked.value_type
    }
    pub fn candidate_type(&self) -> &StructuredInfoType {
        &self.candidate
    }
    pub fn definition_digest(&self) -> [u8; 32] {
        self.definition_digest
    }
    pub fn kind_identity(&self, flow: bool) -> String {
        let flow = if flow { "flow-" } else { "" };
        let mut identity = String::with_capacity(
            "structure/".len() + flow.len() + "native-profile/".len() + self.identity.len(),
        );
        identity.push_str("structure/");
        identity.push_str(flow);
        identity.push_str("native-profile/");
        identity.push_str(&self.identity);
        identity
    }
    fn implementation_identity(identity: &str) -> String {
        let mut result =
            String::with_capacity(NATIVE_PROFILE_IMPLEMENTATION.len() + 1 + identity.len());
        result.push_str(NATIVE_PROFILE_IMPLEMENTATION);
        result.push('/');
        result.push_str(identity);
        result
    }
    pub fn contract(&self, flow: bool) -> Result<Kind, String> {
        let temporal = if flow {
            PortTemporal::Flow { closes: true }
        } else {
            PortTemporal::Value
        };
        let input = PortDescriptor {
            port_id: port_id("candidate"),
            value_kind: self.candidate_kind.clone(),
            direction: PortDirection::Input,
            temporal,
            abnormal_kind: None,
        };
        let output = PortDescriptor {
            port_id: port_id("result"),
            value_kind: self.native_kind.clone(),
            direction: PortDirection::Output,
            temporal,
            abnormal_kind: None,
        };
        let contracts = [(&input, self.candidate_maximum), (&output, self.maximum)]
            .into_iter()
            .map(|(p, max)| FrontValueContract {
                location: match p.direction {
                    PortDirection::Input => FrontValueLocation::Input(p.port_id.clone()),
                    PortDirection::Output => FrontValueLocation::Output(p.port_id.clone()),
                },
                contract: CheckedValueContract::new(p.value_kind.clone(), max, vec![])
                    .expect("checked finite schema envelope"),
            })
            .collect();
        let identity = self.kind_identity(flow);
        Ok(Kind {
            kind_id: kind_id(&identity),
            kind_contract_revision: KindIdentity::from(Self::implementation_identity(&identity)),
            startup_parameters: vec![],
            shorthand: None,
            configuration: vec![],
            inputs: vec![input],
            outputs: vec![output],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(contracts)],
            limits: CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 1,
                max_queue_bytes: self.maximum.max(self.candidate_maximum),
            },
        })
    }
    pub fn offer(&self, flow: bool) -> Result<CapabilityOffer, String> {
        let kind = self.contract(flow)?;
        let identity = String::from(kind.kind_id.as_str());
        Ok(BackOfferBuilder::new(
            kind,
            Back {
                capability_id: Self::implementation_identity(&identity).into(),
                execution_profile_id: NATIVE_PROFILE_IMPLEMENTATION.into(),
                implementation_id: NATIVE_PROFILE_IMPLEMENTATION.into(),
                artifact_id: NATIVE_PROFILE_IMPLEMENTATION.into(),
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
        flow: bool,
    ) -> Result<(), String> {
        let kind = self.contract(flow)?;
        let identity = kind.kind_id.as_str();
        startup.insert(conduit_plot::KindSignature {
            kind: identity.into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(identity, kind.checked_front())?;
        let candidate_identity = match self.candidate.shape() {
            StructuredInfoTypeShape::Record { schema, .. }
            | StructuredInfoTypeShape::Variant { schema, .. }
            | StructuredInfoTypeShape::Nominal { schema, .. } => schema.clone(),
            _ => return Err("candidate root has no distinct identity".into()),
        };
        let candidate_receipt = CheckedNativeType {
            name: format!("{identity}/candidate"),
            identity: candidate_identity,
            value_type: self.candidate.clone(),
            value_contracts: vec![],
            invariants: vec![],
        };
        startup.insert_checked_native_type(candidate_receipt.name.clone(), &candidate_receipt)?;
        startup.insert_checked_native_type(format!("{identity}/result"), &self.checked)?;
        for ty in &self.context {
            startup.insert_checked_native_type(format!("{identity}/native/{}", ty.name), ty)?;
        }
        profiles.insert_kind(kind).map_err(|e| format!("{e:?}"))?;
        Ok(())
    }
}
struct NestedLaw {
    value_type: StructuredInfoType,
    prefix: Vec<u8>,
    scratch: Vec<u8>,
    invariants: Vec<PreparedPortableExpressionEvaluator>,
}
pub struct NativeProfileBack {
    candidate: PreparedStructuredValueValidator,
    candidate_prefix: Vec<u8>,
    native: PreparedStructuredContractValidator,
    structure: PreparedStructuredValueValidator,
    native_prefix: Vec<u8>,
    output: Vec<u8>,
    output_length: usize,
    laws: Vec<NestedLaw>,
    flow: bool,
    staged: bool,
    finished: bool,
    cancelled: bool,
    committed_frames: u64,
}
impl NativeProfileBack {
    pub fn prepare_planned<const PORTS: usize>(
        gear: &PlannedGear,
        fuel: u16,
        profile: &PreparedNativeProfile,
        flow: bool,
    ) -> Result<Self, String> {
        if PORTS < 1 || fuel < 2 {
            return Err("native profile step bound".into());
        }
        verify_fixed_placement(gear, &profile.offer(flow)?).map_err(|e| format!("{e:?}"))?;
        let contracts = profile
            .checked
            .value_contracts
            .iter()
            .map(|c| (c.representation_path.clone(), c.contract.clone()))
            .collect::<Vec<_>>();
        let mut laws = Vec::new();
        for ty in &profile.context {
            if ty.invariants.is_empty() {
                continue;
            }
            let max =
                crate::transport_envelope::maximum_prepared_transport_value_bytes(&ty.value_type)
                    .map_err(|e| format!("{e:?}"))? as usize;
            if max > MAXIMUM_NATIVE_PROFILE_VALUE_BYTES {
                return Err("nested law value envelope".into());
            }
            laws.push(NestedLaw {
                value_type: ty.value_type.clone(),
                prefix: ty
                    .value_type
                    .canonical_bytes()
                    .map_err(|e| format!("{e:?}"))?,
                scratch: vec![0; max],
                invariants: ty
                    .invariants
                    .iter()
                    .map(|p| {
                        PreparedPortableExpressionEvaluator::new(p).map_err(|e| format!("{e:?}"))
                    })
                    .collect::<Result<_, _>>()?,
            });
        }
        Ok(Self {
            candidate: PreparedStructuredValueValidator::new(
                &profile.candidate,
                profile.candidate_maximum as usize,
            )
            .map_err(|e| format!("{e:?}"))?,
            candidate_prefix: profile
                .candidate
                .canonical_bytes()
                .map_err(|e| format!("{e:?}"))?,
            native: PreparedStructuredContractValidator::new(
                &profile.checked.value_type,
                profile.maximum as usize,
                &contracts,
            )
            .map_err(|e| format!("{e:?}"))?,
            structure: PreparedStructuredValueValidator::new(
                &profile.checked.value_type,
                profile.maximum as usize,
            )
            .map_err(|e| format!("{e:?}"))?,
            native_prefix: profile
                .checked
                .value_type
                .canonical_bytes()
                .map_err(|e| format!("{e:?}"))?,
            output: vec![0; profile.maximum as usize],
            output_length: 0,
            laws,
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
impl<const PORTS: usize> StepBack<PORTS> for NativeProfileBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 2900,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if io.input_closed(PortId(0)) {
            return if self.flow {
                StepOutcome::Complete
            } else {
                fail(2901)
            };
        }
        if self.committed_frames == u64::MAX {
            return fail(2902);
        }
        if io.input(PortId(0)).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = inputs.input(PortId(0)) else {
            return fail(2903);
        };
        if self.candidate.validate(bytes).is_err() {
            return fail(2904);
        }
        let Some(node) = bytes.strip_prefix(self.candidate_prefix.as_slice()) else {
            return fail(2905);
        };
        let Some(length) = self.native_prefix.len().checked_add(node.len()) else {
            return fail(2906);
        };
        if length > self.output.len() {
            return fail(2906);
        }
        self.output[..self.native_prefix.len()].copy_from_slice(&self.native_prefix);
        self.output[self.native_prefix.len()..length].copy_from_slice(node);
        self.output_length = length;
        let output = &self.output[..length];
        if self.native.validate(output).is_err() {
            return fail(2907);
        }
        let laws = &mut self.laws;
        if self
            .structure
            .visit_nodes(output, |ty, body| -> Result<(), ()> {
                for law in laws.iter_mut().filter(|law| &law.value_type == ty) {
                    let length = law.prefix.len().checked_add(body.len()).ok_or(())?;
                    if length > law.scratch.len() {
                        return Err(());
                    }
                    law.scratch[..law.prefix.len()].copy_from_slice(&law.prefix);
                    law.scratch[law.prefix.len()..length].copy_from_slice(body);
                    for program in &mut law.invariants {
                        let result = program.evaluate(&law.scratch[..length]).map_err(|_| ())?;
                        if !matches!(InfoBool::decode(result),Ok(value) if value.get()) {
                            return Err(());
                        }
                    }
                }
                Ok(())
            })
            .is_err()
        {
            return fail(2908);
        }
        if io.consume(PortId(0)).is_err() || io.send_prepared(PortId(0), length as u32).is_err() {
            return fail(2909);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then_some(&self.output[..self.output_length])
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

mod storage;
pub use storage::*;

#[cfg(test)]
mod storage_tests;
