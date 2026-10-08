//! Explicit structural weakening. This owner never admits native refinements or laws.
use crate::fixed_numeric_preparation::verify_fixed_placement;
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, PortId,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
pub const WEAKENING_IMPLEMENTATION: &str = "conduit.structure/nominal-weakening@1";
pub const MAXIMUM_WEAKENING_BYTES: u32 = 16384;
#[derive(Clone)]
pub struct PreparedNominalWeakening {
    input: StructuredInfoType,
    output: StructuredInfoType,
    input_maximum: u32,
    output_maximum: u32,
    identity: String,
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
impl PreparedNominalWeakening {
    pub fn prepare(input: StructuredInfoType) -> Result<Self, String> {
        let raw = raw_type(&input).map_err(|e| format!("{e:?}"))?;
        let mut schema = String::from("conduit.structure/candidate-");
        for byte in input.semantic_digest().map_err(|e| format!("{e:?}"))? {
            use core::fmt::Write;
            write!(&mut schema, "{byte:02x}").map_err(|_| "digest formatting")?;
        }
        if let StructuredInfoTypeShape::Record {
            schema: original,
            fields,
        } = input.shape()
        {
            if original.as_str().starts_with("conduitese/anonymous-tuple-") {
                let exact = tuple_info_type(
                    fields
                        .iter()
                        .map(|field| field.value_type().clone())
                        .collect(),
                )
                .map_err(|e| format!("{e:?}"))?;
                if exact != input {
                    return Err("foreign anonymous tuple schema".into());
                }
                if raw == input {
                    return Err("already bare tuple requires no weakening".into());
                }
            }
        }
        let output = match raw.shape() {
            StructuredInfoTypeShape::Record { fields, .. } => {
                StructuredInfoType::record(kind_id(&schema), fields.to_vec())
            }
            StructuredInfoTypeShape::Variant { cases, .. } => {
                StructuredInfoType::variant(kind_id(&schema), cases.to_vec())
            }
            StructuredInfoTypeShape::Leaf(_) => {
                return Err("structured weakening requires structured input".into());
            }
            _ => StructuredInfoType::nominal(kind_id(&schema), raw.clone()),
        }
        .map_err(|e| format!("{e:?}"))?;
        let input_maximum =
            crate::transport_envelope::maximum_prepared_transport_value_bytes(&input)
                .map_err(|e| format!("{e:?}"))?;
        let output_maximum =
            crate::transport_envelope::maximum_prepared_transport_value_bytes(&output)
                .map_err(|e| format!("{e:?}"))?;
        if input_maximum == 0
            || output_maximum == 0
            || input_maximum.max(output_maximum) > MAXIMUM_WEAKENING_BYTES
        {
            return Err("structural weakening envelope".into());
        }
        let mut identity = String::from("typed-");
        // Both exact Types bind the operation. The derived output is independently hashed.
        for digest in [input.semantic_digest(), output.semantic_digest()] {
            for byte in digest.map_err(|e| format!("{e:?}"))? {
                use core::fmt::Write;
                write!(&mut identity, "{byte:02x}").map_err(|_| "identity formatting")?;
            }
        }
        Ok(Self {
            input,
            output,
            input_maximum,
            output_maximum,
            identity,
        })
    }
    pub fn input_type(&self) -> &StructuredInfoType {
        &self.input
    }
    pub fn output_type(&self) -> &StructuredInfoType {
        &self.output
    }
    pub fn kind_identity(&self, flow: bool) -> String {
        format!(
            "structure/{}nominal-weakening/{}",
            if flow { "flow-" } else { "" },
            self.identity
        )
    }
    pub fn contract(&self, flow: bool) -> Result<Kind, String> {
        let temporal = if flow {
            PortTemporal::Flow { closes: true }
        } else {
            PortTemporal::Value
        };
        let port =
            |name: &str, ty: &StructuredInfoType, direction| -> Result<PortDescriptor, String> {
                Ok(PortDescriptor {
                    port_id: port_id(name),
                    value_kind: ty
                        .profile()
                        .map_err(|e| format!("{e:?}"))?
                        .value_kind()
                        .clone(),
                    direction,
                    temporal,
                    abnormal_kind: None,
                })
            };
        let input = port("value", &self.input, PortDirection::Input)?;
        let output = port("result", &self.output, PortDirection::Output)?;
        let contracts = [(&input, self.input_maximum), (&output, self.output_maximum)]
            .into_iter()
            .map(|(p, max)| FrontValueContract {
                location: match p.direction {
                    PortDirection::Input => FrontValueLocation::Input(p.port_id.clone()),
                    PortDirection::Output => FrontValueLocation::Output(p.port_id.clone()),
                },
                contract: CheckedValueContract::new(p.value_kind.clone(), max, vec![])
                    .expect("bounded schema"),
            })
            .collect();
        Ok(Kind {
            kind_id: kind_id(&self.kind_identity(flow)),
            kind_contract_revision: KindIdentity::from(WEAKENING_IMPLEMENTATION),
            startup_parameters: vec![],
            shorthand: None,
            configuration: vec![],
            inputs: vec![input],
            outputs: vec![output],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(contracts)],
            limits: CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 1,
                max_queue_bytes: self.input_maximum.max(self.output_maximum),
            },
        })
    }
    pub fn offer(&self, flow: bool) -> Result<CapabilityOffer, String> {
        Ok(BackOfferBuilder::new(
            self.contract(flow)?,
            Back {
                capability_id: format!("{WEAKENING_IMPLEMENTATION}/{}", self.kind_identity(flow))
                    .into(),
                execution_profile_id: WEAKENING_IMPLEMENTATION.into(),
                implementation_id: WEAKENING_IMPLEMENTATION.into(),
                artifact_id: WEAKENING_IMPLEMENTATION.into(),
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
        let id = kind.kind_id.as_str();
        startup.insert(conduit_plot::KindSignature {
            kind: id.into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(id, kind.checked_front())?;
        // Input registration supplies framing only, never a checked native admission receipt.
        startup.insert_structured_type(format!("{id}/value"), self.input.clone())?;
        let schema = match self.output.shape() {
            StructuredInfoTypeShape::Record { schema, .. }
            | StructuredInfoTypeShape::Variant { schema, .. }
            | StructuredInfoTypeShape::Nominal { schema, .. } => schema.clone(),
            _ => return Err("distinct output identity".into()),
        };
        let bare = conduit_plot::CheckedNativeType {
            name: format!("{id}/result"),
            identity: schema,
            value_type: self.output.clone(),
            value_contracts: vec![],
            invariants: vec![],
        };
        startup.insert_checked_native_type(bare.name.clone(), &bare)?;
        profiles.insert_kind(kind).map_err(|e| format!("{e:?}"))
    }
}
pub struct NominalWeakeningBack {
    input: PreparedStructuredValueValidator,
    output_validator: PreparedStructuredValueValidator,
    input_prefix: Vec<u8>,
    output_prefix: Vec<u8>,
    output: Vec<u8>,
    length: usize,
    flow: bool,
    staged: bool,
    finished: bool,
    cancelled: bool,
    committed: u64,
}
impl NominalWeakeningBack {
    pub fn prepare_planned<const PORTS: usize>(
        gear: &PlannedGear,
        fuel: u16,
        profile: &PreparedNominalWeakening,
        flow: bool,
    ) -> Result<Self, String> {
        if PORTS < 1 || fuel < 2 {
            return Err("weakening step bounds".into());
        }
        verify_fixed_placement(gear, &profile.offer(flow)?).map_err(|e| format!("{e:?}"))?;
        Ok(Self {
            input: PreparedStructuredValueValidator::new(
                &profile.input,
                profile.input_maximum as usize,
            )
            .map_err(|e| format!("{e:?}"))?,
            output_validator: PreparedStructuredValueValidator::new(
                &profile.output,
                profile.output_maximum as usize,
            )
            .map_err(|e| format!("{e:?}"))?,
            input_prefix: profile
                .input
                .canonical_bytes()
                .map_err(|e| format!("{e:?}"))?,
            output_prefix: profile
                .output
                .canonical_bytes()
                .map_err(|e| format!("{e:?}"))?,
            output: vec![0; profile.output_maximum as usize],
            length: 0,
            flow,
            staged: false,
            finished: false,
            cancelled: false,
            committed: 0,
        })
    }
    pub fn committed_frames(&self) -> u64 {
        self.committed
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
impl<const PORTS: usize> StepBack<PORTS> for NominalWeakeningBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 3000,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if io.input_closed(PortId(0)) {
            return if self.flow {
                StepOutcome::Complete
            } else {
                fail(3001)
            };
        }
        if PORTS < 1 || self.committed == u64::MAX {
            return fail(3002);
        }
        if io.input(PortId(0)).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = inputs.input(PortId(0)) else {
            return fail(3003);
        };
        if self.input.validate(bytes).is_err() {
            return fail(3004);
        }
        let Some(body) = bytes.strip_prefix(self.input_prefix.as_slice()) else {
            return fail(3005);
        };
        let Some(length) = self.output_prefix.len().checked_add(body.len()) else {
            return fail(3006);
        };
        if length > self.output.len() {
            return fail(3006);
        }
        self.output[..self.output_prefix.len()].copy_from_slice(&self.output_prefix);
        self.output[self.output_prefix.len()..length].copy_from_slice(body);
        self.length = length;
        if self
            .output_validator
            .validate(&self.output[..length])
            .is_err()
        {
            return fail(3007);
        }
        if io.consume(PortId(0)).is_err() || io.send_prepared(PortId(0), length as u32).is_err() {
            return fail(3008);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then_some(&self.output[..self.length])
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.committed += 1;
            self.finished = !self.flow;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}

impl NominalWeakeningBack {
    /// Retained requested local payload capacity. Shared tensor/model Arc owners,
    /// Box root, Arc headers, allocator bookkeeping and stack are separate charges.
    pub fn local_accounted_heap_bytes(&self) -> usize {
        self.input
            .owned_heap_bytes()
            .saturating_add(self.output_validator.owned_heap_bytes())
            .saturating_add(self.input_prefix.capacity())
            .saturating_add(self.output_prefix.capacity())
            .saturating_add(self.output.capacity())
    }
}

mod storage;
pub use storage::*;
