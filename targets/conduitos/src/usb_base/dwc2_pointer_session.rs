//! Carry checked mouse context and samples across bounded native read quanta.
//! This performs no report decoding, coordinate scaling or gesture inference.
use super::dwc2_pointer_play::*;
use crate::{arch::dwc2::RegisterIo, source_pointer_sample::SourcePointerSampleDecoder};
use alloc::{string::String, vec::Vec};
use conduit_core::*;
use conduit_semantic_catalog::NormalizedPointerSample;
pub(crate) enum PointerSessionRefusal {
    Binding,
    Pressure,
    Source(&'static str),
    Value(StructuredInfoRefusal),
    Play(PointerPlayRefusal),
}
pub(crate) struct Dwc2PointerSession<I> {
    play: Dwc2PointerPlay<I>,
    initialize: PortId,
    read: PortId,
    parameters: ValuePayload,
    context: ValuePayload,
    outputs: Vec<(PortId, ValuePayload)>,
    context_composer: PreparedStructuredComposer,
    context_fields: Vec<String>,
    decoder: SourcePointerSampleDecoder,
    sequence: u64,
    initialized: bool,
    working: bool,
    received: bool,
    decoded: bool,
    point: bool,
    sample: Option<NormalizedPointerSample>,
}
impl<I: RegisterIo> Dwc2PointerSession<I> {
    pub(crate) fn prepare(
        mut play: Dwc2PointerPlay<I>,
        schemas: PointerPlaySchemas,
        selected: ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<Self, PointerSessionRefusal> {
        use PointerSessionRefusal as E;
        let layout = field(selected, "layout")?;
        let length = field(selected, "maximum-packet")?;
        let queue_type = field_type(&schemas.initialize, "queue-capacity")?;
        let mut queue = PreparedStructuredComposer::new(&queue_type, 64).map_err(E::Value)?;
        let capacity = queue.leaf(&1_u64.to_le_bytes()).map_err(E::Value)?;
        let capacity = validate_canonical_structured_value(capacity).map_err(E::Value)?;
        let names = record_names(&schemas.initialize)?;
        if names.len() != 3 {
            return Err(E::Binding);
        }
        let mut values = [layout; 3];
        for (index, name) in names.iter().enumerate() {
            values[index] = match name.as_str() {
                "layout" => layout,
                "length" => length,
                "queue-capacity" => capacity,
                _ => return Err(E::Binding),
            };
        }
        let mut composer =
            PreparedStructuredComposer::new(&schemas.initialize, 4096).map_err(E::Value)?;
        let parameters = composer.record(&values).map_err(E::Value)?.to_vec();
        let ready_type = variant_type(&schemas.update, "ready")?;
        let sample_type = field_type(&ready_type, "sample")?;
        let decoder = SourcePointerSampleDecoder::prepare(&sample_type).map_err(E::Value)?;
        let context_fields = record_names(&schemas.context)?;
        if context_fields.len() != 4 {
            return Err(E::Binding);
        }
        let context_composer =
            PreparedStructuredComposer::new(&schemas.context, 4096).map_err(E::Value)?;
        let boundary = &play.kernel_mut().definition().boundary;
        let input = |name: &str| {
            boundary
                .input_fronts
                .iter()
                .find(|p| p.external_port.port_id.as_str() == name)
                .map(|p| p.external_port.clone())
                .ok_or(E::Binding)
        };
        let initialize = input("initialize")?;
        let read = input("read")?;
        let outputs = boundary
            .output_fronts
            .iter()
            .map(|p| {
                (
                    p.external_port.port_id.clone(),
                    ValuePayload {
                        value_kind: p.external_port.value_kind.clone(),
                        encoded: Vec::with_capacity(4096),
                    },
                )
            })
            .collect();
        Ok(Self {
            play,
            initialize: initialize.port_id,
            read: read.port_id,
            parameters: ValuePayload {
                value_kind: initialize.value_kind,
                encoded: parameters,
            },
            context: ValuePayload {
                value_kind: read.value_kind,
                encoded: Vec::with_capacity(4096),
            },
            outputs,
            context_composer,
            context_fields,
            decoder,
            sequence: 0,
            initialized: false,
            working: false,
            received: false,
            decoded: false,
            point: false,
            sample: None,
        })
    }
    pub(crate) fn start(&mut self) -> Result<(), PointerSessionRefusal> {
        use PointerSessionRefusal as E;
        self.play.kernel_mut().start().map_err(|_| E::Binding)?;
        accepted(
            self.play
                .kernel_mut()
                .admit_input(&self.initialize, 0, &self.parameters)
                .map_err(|_| E::Binding)?,
        )
    }
    pub(crate) fn ready(&self) -> bool {
        self.initialized && !self.working
    }
    pub(crate) fn read(&mut self) -> Result<(), PointerSessionRefusal> {
        use PointerSessionRefusal as E;
        if !self.ready() {
            return Err(E::Binding);
        }
        let next = self.sequence.checked_add(1).ok_or(E::Binding)?;
        accepted(
            self.play
                .kernel_mut()
                .admit_input(&self.read, self.sequence, &self.context)
                .map_err(|_| E::Binding)?,
        )?;
        self.sequence = next;
        self.working = true;
        self.received = false;
        self.decoded = false;
        self.point = false;
        self.sample = None;
        Ok(())
    }
    pub(crate) fn step(
        &mut self,
    ) -> Result<Option<NormalizedPointerSample>, PointerSessionRefusal> {
        use PointerSessionRefusal as E;
        let status = self.play.step().map_err(E::Play)?;
        if status != conduit_composite::KernelCompositeStatus::Active {
            return Err(E::Binding);
        }
        for (port, payload) in &mut self.outputs {
            let Some(sequence) = self
                .play
                .kernel_mut()
                .output_into(port, payload)
                .map_err(|_| E::Binding)?
            else {
                continue;
            };
            let value = validate_canonical_structured_value(&payload.encoded).map_err(E::Value)?;
            match port.as_str() {
                "context" => {
                    if self.initialized {
                        return Err(E::Binding);
                    }
                    self.context.encoded.extend_from_slice(&payload.encoded);
                    self.initialized = true;
                    // Closing before the paired initialization has completed
                    // would correctly discard its unmatched finite value.
                    self.play
                        .kernel_mut()
                        .close_input(&self.initialize)
                        .map_err(|_| E::Binding)?;
                }
                "received" => {
                    if !self.working {
                        return Err(E::Binding);
                    }
                    if value.variant_payload("report").map_err(E::Value)?.is_some() {
                        self.received = true;
                    } else if value.variant_payload("idle").map_err(E::Value)?.is_some() {
                        self.working = false;
                    } else {
                        for tag in ["malformed", "stalled", "provider-lost", "unsupported"] {
                            if value.variant_payload(tag).map_err(E::Value)?.is_some() {
                                return Err(E::Source(tag));
                            }
                        }
                        return Err(E::Binding);
                    }
                }
                "decoded" => {
                    self.decoded = true;
                    if value.variant_payload("point").map_err(E::Value)?.is_some() {
                        self.point = true;
                    } else if value
                        .variant_payload("other-report")
                        .map_err(E::Value)?
                        .is_none()
                    {
                        for tag in ["short", "malformed", "invalid-layout"] {
                            if value.variant_payload(tag).map_err(E::Value)?.is_some() {
                                return Err(E::Source(tag));
                            }
                        }
                        return Err(E::Binding);
                    }
                }
                "update" => {
                    let ready = value
                        .variant_payload("ready")
                        .map_err(E::Value)?
                        .ok_or(E::Source("pointer-state-refused"))?;
                    let state = field(ready, "state")?;
                    let sample = self
                        .decoder
                        .decode_validated(field(ready, "sample")?)
                        .map_err(E::Value)?;
                    let context = validate_canonical_structured_value(&self.context.encoded)
                        .map_err(E::Value)?;
                    let mut values = [state; 4];
                    for (index, name) in self.context_fields.iter().enumerate() {
                        values[index] = if name == "state" {
                            state
                        } else {
                            field(context, name)?
                        };
                    }
                    let next = self.context_composer.record(&values).map_err(E::Value)?;
                    self.context.encoded.clear();
                    self.context.encoded.extend_from_slice(next);
                    if self.sample.replace(sample).is_some() {
                        return Err(E::Binding);
                    }
                }
                _ => return Err(E::Binding),
            }
            self.play
                .kernel_mut()
                .complete_output(port, sequence)
                .map_err(|_| E::Binding)?;
        }
        if self.working && self.received && self.decoded && (!self.point || self.sample.is_some()) {
            self.working = false;
            return Ok(self.sample.take());
        }
        Ok(None)
    }
}
fn accepted(
    outcome: conduit_kernel::scheduler::RemoteIngressOutcome,
) -> Result<(), PointerSessionRefusal> {
    match outcome {
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. } => Ok(()),
        _ => Err(PointerSessionRefusal::Pressure),
    }
}
fn field<'a>(
    value: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> Result<ValidatedCanonicalStructuredValue<'a>, PointerSessionRefusal> {
    value
        .record_field(name)
        .map_err(PointerSessionRefusal::Value)?
        .ok_or(PointerSessionRefusal::Binding)
}
fn field_type(
    ty: &StructuredInfoType,
    name: &str,
) -> Result<StructuredInfoType, PointerSessionRefusal> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(PointerSessionRefusal::Binding);
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .map(|f| f.value_type().clone())
        .ok_or(PointerSessionRefusal::Binding)
}
fn variant_type(
    ty: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, PointerSessionRefusal> {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        return Err(PointerSessionRefusal::Binding);
    };
    cases
        .iter()
        .find(|c| c.tag() == tag)
        .map(|c| c.payload_type().clone())
        .ok_or(PointerSessionRefusal::Binding)
}
fn record_names(ty: &StructuredInfoType) -> Result<Vec<String>, PointerSessionRefusal> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(PointerSessionRefusal::Binding);
    };
    Ok(fields.iter().map(|f| f.name().into()).collect())
}

impl core::fmt::Debug for PointerSessionRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Binding => f.write_str("Binding"),
            Self::Pressure => f.write_str("Pressure"),
            Self::Source(error) => f.debug_tuple("Source").field(error).finish(),
            Self::Value(error) => f.debug_tuple("Value").field(error).finish(),
            Self::Play(error) => f.debug_tuple("Play").field(error).finish(),
        }
    }
}
