//! Allocation-stable std realization of the typed-record codec family.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
};

pub(super) static FRAME: InstalledFactory =
    factory(conduit_std_offers::TYPED_RECORD_FRAME_STD_IMPLEMENTATION);
pub(super) static DEFRAME: InstalledFactory =
    factory(conduit_std_offers::TYPED_RECORD_DEFRAME_STD_IMPLEMENTATION);
pub(super) static TEXT_TO_RECORD: InstalledFactory =
    factory(conduit_std_offers::TEXT_TO_TYPED_RECORD_STD_IMPLEMENTATION);
pub(super) static RECORD_TO_TEXT: InstalledFactory =
    factory(conduit_std_offers::TYPED_RECORD_TO_TEXT_STD_IMPLEMENTATION);

const fn factory(implementation_id: &'static str) -> InstalledFactory {
    InstalledFactory {
        implementation_id,
        budget,
        prepare,
    }
}

pub(super) struct TypedRecordOperation {
    pending: bool,
    complete: bool,
}

impl TypedRecordOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.complete => {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(
                        value,
                        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                    ) {
                        Ok(value) => value,
                        Err(_) => return InstalledOperation::fail(162),
                    },
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return InstalledOperation::fail(163);
                };
                self.pending = false;
                self.complete = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(164),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
        self.complete = true;
    }
}

#[derive(Copy, Clone)]
enum Codec {
    Frame,
    Deframe,
    TextToRecord,
    RecordToText,
}

pub(super) struct TypedRecordHost {
    codec: Codec,
    frame: [u8; conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES],
    text_type: Vec<u8>,
    text_kind: String,
    typed_type: Vec<u8>,
    framed_type: Vec<u8>,
    output: Vec<u8>,
}

impl TypedRecordHost {
    fn new(codec: Codec) -> Self {
        Self {
            codec,
            frame: [0; conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES],
            text_type: conduit_net::text_type()
                .canonical_bytes()
                .expect("text type is finite"),
            text_kind: conduit_net::text_type()
                .profile()
                .expect("text profile is finite")
                .value_kind()
                .as_str()
                .to_owned(),
            typed_type: conduit_net::typed_record_type()
                .canonical_bytes()
                .expect("typed-record type is finite"),
            framed_type: conduit_net::framed_typed_record_type()
                .canonical_bytes()
                .expect("framed-record type is finite"),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    pub(super) fn execute(&mut self, input: &[u8]) -> Result<&[u8], u16> {
        match self.codec {
            Codec::Frame => self.frame(input),
            Codec::Deframe => self.deframe(input),
            Codec::TextToRecord => self.text_to_record(input),
            Codec::RecordToText => self.record_to_text(input),
        }
    }

    fn frame(&mut self, input: &[u8]) -> Result<&[u8], u16> {
        let encoded = typed_leaf(input, &self.typed_type)?;
        let (kind, payload) = typed_parts(encoded)?;
        let written = conduit_net::encode_typed_record_into(
            conduit_net::TypedRecordRef::new(kind, payload).map_err(frame_detail)?,
            &mut self.frame,
        )
        .map_err(frame_detail)?;
        write_leaf(&mut self.output, &self.framed_type, &self.frame[..written])?;
        Ok(&self.output)
    }

    fn deframe(&mut self, input: &[u8]) -> Result<&[u8], u16> {
        let frame = typed_leaf(input, &self.framed_type)?;
        let record = conduit_net::decode_typed_record(frame).map_err(frame_detail)?;
        self.output.clear();
        self.output.extend_from_slice(&self.typed_type);
        let encoded_length = 2 + record.value_kind().len() + record.payload().len();
        push_leaf_header(&mut self.output, encoded_length)?;
        self.output
            .extend_from_slice(&(record.value_kind().len() as u16).to_le_bytes());
        self.output
            .extend_from_slice(record.value_kind().as_bytes());
        self.output.extend_from_slice(record.payload());
        Ok(&self.output)
    }

    fn text_to_record(&mut self, input: &[u8]) -> Result<&[u8], u16> {
        if input.len() > conduit_core::MAXIMUM_STRUCTURED_LEAF_BYTES {
            return Err(221);
        }
        self.output.clear();
        self.output.extend_from_slice(&self.typed_type);
        let text_payload_length = self.text_type.len() + 5 + input.len();
        let encoded_length = 2 + self.text_kind.len() + text_payload_length;
        push_leaf_header(&mut self.output, encoded_length)?;
        self.output
            .extend_from_slice(&(self.text_kind.len() as u16).to_le_bytes());
        self.output.extend_from_slice(self.text_kind.as_bytes());
        self.output.extend_from_slice(&self.text_type);
        push_leaf_header(&mut self.output, input.len())?;
        self.output.extend_from_slice(input);
        Ok(&self.output)
    }

    fn record_to_text(&mut self, input: &[u8]) -> Result<&[u8], u16> {
        let encoded = typed_leaf(input, &self.typed_type)?;
        let (kind, payload) = typed_parts(encoded)?;
        if kind != self.text_kind {
            return Err(222);
        }
        let text = typed_leaf(payload, &self.text_type)?;
        self.output.clear();
        self.output.extend_from_slice(text);
        Ok(&self.output)
    }
}

pub(super) fn typed_leaf<'a>(input: &'a [u8], value_type: &[u8]) -> Result<&'a [u8], u16> {
    let node = input.strip_prefix(value_type).ok_or(210_u16)?;
    leaf_bytes(node).ok_or(211)
}

fn typed_parts(encoded: &[u8]) -> Result<(&str, &[u8]), u16> {
    let length_bytes: [u8; 2] = encoded.get(..2).ok_or(212_u16)?.try_into().unwrap();
    let kind_length = usize::from(u16::from_le_bytes(length_bytes));
    let kind_end = 2_usize.checked_add(kind_length).ok_or(213_u16)?;
    let kind =
        core::str::from_utf8(encoded.get(2..kind_end).ok_or(214_u16)?).map_err(|_| 215_u16)?;
    Ok((kind, encoded.get(kind_end..).ok_or(216_u16)?))
}

fn write_leaf(output: &mut Vec<u8>, value_type: &[u8], bytes: &[u8]) -> Result<(), u16> {
    output.clear();
    output.extend_from_slice(value_type);
    push_leaf_header(output, bytes.len())?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn push_leaf_header(output: &mut Vec<u8>, length: usize) -> Result<(), u16> {
    let length = u32::try_from(length).map_err(|_| 217_u16)?;
    output.push(0);
    output.extend_from_slice(&length.to_le_bytes());
    Ok(())
}

fn leaf_bytes(node: &[u8]) -> Option<&[u8]> {
    if node.first() != Some(&0) || node.len() < 5 {
        return None;
    }
    let length = usize::try_from(u32::from_le_bytes(node[1..5].try_into().ok()?)).ok()?;
    (node.len() == 5 + length).then_some(&node[5..])
}

fn frame_detail(refusal: conduit_net::TypedRecordFrameRefusal) -> u16 {
    230 + refusal as u16
}

pub(super) fn prepare_hosts(fragment: &conduit_core::PlanFragment) -> Vec<Option<TypedRecordHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| codec(placement).map(TypedRecordHost::new))
        .collect()
}

fn codec(placement: &PlannedGear) -> Option<Codec> {
    match placement.implementation_id.as_str() {
        conduit_std_offers::TYPED_RECORD_FRAME_STD_IMPLEMENTATION => Some(Codec::Frame),
        conduit_std_offers::TYPED_RECORD_DEFRAME_STD_IMPLEMENTATION => Some(Codec::Deframe),
        conduit_std_offers::TEXT_TO_TYPED_RECORD_STD_IMPLEMENTATION => Some(Codec::TextToRecord),
        conduit_std_offers::TYPED_RECORD_TO_TEXT_STD_IMPLEMENTATION => Some(Codec::RecordToText),
        _ => None,
    }
}

fn selected_offer(placement: &PlannedGear) -> Result<conduit_core::CapabilityOffer, String> {
    conduit_std_offers::typed_record_codec_offers()
        .into_iter()
        .find(|offer| offer.implementation.implementation_id == placement.implementation_id)
        .ok_or_else(|| "unknown typed-record codec implementation".into())
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = selected_offer(placement)?;
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
    {
        return Err("planned typed-record codec differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        host_requests: 1,
        sign_items: 24,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::TypedRecord(TypedRecordOperation {
        pending: false,
        complete: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, StructuredInfoType, StructuredInfoValue};

    fn text(bytes: &[u8]) -> StructuredInfoValue {
        StructuredInfoValue::leaf(conduit_net::text_type(), bytes.to_vec()).unwrap()
    }

    #[test]
    fn all_four_hosts_round_trip_exact_text_without_growing_buffers() {
        let original = text(b"CALLING");
        let mut wrap = TypedRecordHost::new(Codec::TextToRecord);
        let typed = wrap.execute(b"CALLING").unwrap().to_vec();
        let expected_typed = conduit_net::typed_record_from_text(&original)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert_eq!(typed, expected_typed);

        let mut frame = TypedRecordHost::new(Codec::Frame);
        let framed = frame.execute(&typed).unwrap().to_vec();
        let mut deframe = TypedRecordHost::new(Codec::Deframe);
        let recovered = deframe.execute(&framed).unwrap().to_vec();
        assert_eq!(recovered, typed);

        let mut unwrap = TypedRecordHost::new(Codec::RecordToText);
        assert_eq!(unwrap.execute(&recovered).unwrap(), b"CALLING");
        assert_eq!(wrap.output.capacity(), MAXIMUM_STRUCTURED_CANONICAL_BYTES);
        assert_eq!(frame.output.capacity(), MAXIMUM_STRUCTURED_CANONICAL_BYTES);
        assert_eq!(
            deframe.output.capacity(),
            MAXIMUM_STRUCTURED_CANONICAL_BYTES
        );
        assert_eq!(unwrap.output.capacity(), MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    }

    #[test]
    fn text_decoder_refuses_a_quantity_record() {
        let quantity = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(conduit_core::QUANTITY_INFO_ID)).unwrap(),
            conduit_core::Quantity::new(42, conduit_core::QuantityUnit::Millivolt)
                .encode()
                .to_vec(),
        )
        .unwrap();
        let typed = conduit_net::typed_record_value(&quantity)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert_eq!(
            TypedRecordHost::new(Codec::RecordToText).execute(&typed),
            Err(222)
        );
    }
}
