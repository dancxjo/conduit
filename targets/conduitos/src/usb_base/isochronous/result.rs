//! Prepared finite metadata composition; no audio or cadence interpretation.
use super::contract::{
    ISOCHRONOUS_OUT_MAXIMUM_BYTES, ISOCHRONOUS_OUT_MAXIMUM_PACKETS, IsochronousOutContract,
};
use crate::usb_base::isochronous_schedule::IsochronousOutSnapshot;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use conduit_core::{
    PreparedStructuredComposer, StructuredInfoRefusal as Error, StructuredInfoType,
    StructuredInfoTypeShape, validate_canonical_structured_value,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IsochronousCompletion {
    pub frame: u16,
    pub requested: u16,
    pub actual: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsochronousOutcome {
    Snapshot(IsochronousOutSnapshot),
    Completed,
    Short(IsochronousCompletion),
    MissedService { frame: u16 },
    Underrun,
    ProviderLost,
    Drained,
}
fn field(ty: &StructuredInfoType, wanted: &str) -> Result<StructuredInfoType, Error> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(Error::WrongType);
    };
    fields
        .iter()
        .find(|field| field.name() == wanted)
        .map(|field| field.value_type().clone())
        .ok_or(Error::WrongRecordFields)
}
fn case(ty: &StructuredInfoType, wanted: &str) -> Result<StructuredInfoType, Error> {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        return Err(Error::WrongType);
    };
    cases
        .iter()
        .find(|case| case.tag() == wanted)
        .map(|case| case.payload_type().clone())
        .ok_or(Error::UnknownVariantTag)
}
struct MetadataRecord {
    record: PreparedStructuredComposer,
    numbers: Vec<PreparedStructuredComposer>,
    names: Vec<String>,
    zero: Vec<u8>,
}
impl MetadataRecord {
    fn new(ty: &StructuredInfoType) -> Result<Self, Error> {
        let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
            return Err(Error::WrongType);
        };
        let mut numbers = Vec::with_capacity(fields.len());
        for field in fields {
            numbers.push(PreparedStructuredComposer::new(field.value_type(), 64)?);
        }
        let mut zero = PreparedStructuredComposer::new(
            &StructuredInfoType::leaf(conduit_core::kind_id("value/u64"))?,
            64,
        )?;
        Ok(Self {
            record: PreparedStructuredComposer::new(ty, 2048)?,
            numbers,
            names: fields
                .iter()
                .map(|field| field.name().to_string())
                .collect(),
            zero: zero.leaf(&0_u64.to_le_bytes())?.to_vec(),
        })
    }
    fn prepared_heap_bytes(&self) -> usize {
        self.record.prepared_heap_bytes()
            + self.numbers.capacity() * core::mem::size_of::<PreparedStructuredComposer>()
            + self
                .numbers
                .iter()
                .map(PreparedStructuredComposer::prepared_heap_bytes)
                .sum::<usize>()
            + self.names.capacity() * core::mem::size_of::<String>()
            + self.names.iter().map(String::capacity).sum::<usize>()
            + self.zero.capacity()
    }
    fn compose(&mut self, values: &[(&str, u64)]) -> Result<&[u8], Error> {
        if values.len() != self.numbers.len() || values.len() > 8 {
            return Err(Error::WrongRecordFields);
        }
        let mut children = [validate_canonical_structured_value(&self.zero)?; 8];
        for (index, (number, name)) in self.numbers.iter_mut().zip(&self.names).enumerate() {
            let value = values
                .iter()
                .find(|(candidate, _)| *candidate == name.as_str())
                .ok_or(Error::WrongRecordFields)?
                .1;
            children[index] =
                validate_canonical_structured_value(number.leaf(&value.to_le_bytes())?)?;
        }
        self.record.record(&children[..values.len()])
    }
    fn packet(&mut self, packet: IsochronousCompletion) -> Result<&[u8], Error> {
        self.compose(&[
            ("frame", u64::from(packet.frame)),
            ("requested", u64::from(packet.requested)),
            ("actual", u64::from(packet.actual)),
        ])
    }
}
pub struct PreparedIsochronousResultEncoder {
    result: PreparedStructuredComposer,
    completed: PreparedStructuredComposer,
    short: PreparedStructuredComposer,
    missed: PreparedStructuredComposer,
    stopped: PreparedStructuredComposer,
    sequence: PreparedStructuredComposer,
    packets: Vec<MetadataRecord>,
    fault_packet: MetadataRecord,
    snapshot: MetadataRecord,
    frame: PreparedStructuredComposer,
    zero_packet: Vec<u8>,
    unit: Vec<u8>,
}
impl PreparedIsochronousResultEncoder {
    pub fn new(contract: &IsochronousOutContract) -> Result<Self, Error> {
        let completed = case(contract.result_type(), "completed")?;
        let sequence = field(&completed, "packets")?;
        let StructuredInfoTypeShape::Sequence { element, .. } = sequence.shape() else {
            return Err(Error::WrongType);
        };
        let mut packets = Vec::with_capacity(ISOCHRONOUS_OUT_MAXIMUM_PACKETS);
        for _ in 0..ISOCHRONOUS_OUT_MAXIMUM_PACKETS {
            packets.push(MetadataRecord::new(element)?);
        }
        let mut zero_packet = MetadataRecord::new(element)?;
        let zero_packet = zero_packet
            .packet(IsochronousCompletion::default())?
            .to_vec();
        let mut unit = PreparedStructuredComposer::new(
            &StructuredInfoType::leaf(conduit_core::kind_id("value/empty"))?,
            64,
        )?;
        Ok(Self {
            result: PreparedStructuredComposer::new(
                contract.result_type(),
                ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize,
            )?,
            completed: PreparedStructuredComposer::new(
                &completed,
                ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize,
            )?,
            short: PreparedStructuredComposer::new(
                &case(contract.result_type(), "short")?,
                ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize,
            )?,
            missed: PreparedStructuredComposer::new(
                &case(contract.result_type(), "missed-service")?,
                ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize,
            )?,
            stopped: PreparedStructuredComposer::new(
                &case(contract.result_type(), "underrun")?,
                ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize,
            )?,
            sequence: PreparedStructuredComposer::new(
                &sequence,
                ISOCHRONOUS_OUT_MAXIMUM_BYTES as usize,
            )?,
            packets,
            fault_packet: MetadataRecord::new(element)?,
            snapshot: MetadataRecord::new(&case(contract.result_type(), "snapshot")?)?,
            frame: PreparedStructuredComposer::new(
                &StructuredInfoType::leaf(conduit_core::kind_id("value/u64"))?,
                64,
            )?,
            zero_packet,
            unit: unit.leaf(&[])?.to_vec(),
        })
    }
    /// Exact reserved Rust storage after preparation, excluding allocator overhead.
    /// Root must additionally admit native DMA (3072), input context/controller
    /// resources, capability-table storage and its installed HostCall buffers.
    pub fn prepared_storage_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
            + [
                &self.result,
                &self.completed,
                &self.short,
                &self.missed,
                &self.stopped,
                &self.sequence,
                &self.frame,
            ]
            .iter()
            .map(|value| value.prepared_heap_bytes())
            .sum::<usize>()
            + self.packets.capacity() * core::mem::size_of::<MetadataRecord>()
            + self
                .packets
                .iter()
                .map(MetadataRecord::prepared_heap_bytes)
                .sum::<usize>()
            + self.fault_packet.prepared_heap_bytes()
            + self.snapshot.prepared_heap_bytes()
            + self.zero_packet.capacity()
            + self.unit.capacity()
    }

    pub fn encode(
        &mut self,
        completed: &[IsochronousCompletion],
        outcome: IsochronousOutcome,
    ) -> Result<&[u8], Error> {
        if completed.len() > ISOCHRONOUS_OUT_MAXIMUM_PACKETS {
            return Err(Error::WrongCollectionLength);
        }
        let mut packets = [validate_canonical_structured_value(&self.zero_packet)?;
            ISOCHRONOUS_OUT_MAXIMUM_PACKETS];
        for (index, (encoder, packet)) in self.packets.iter_mut().zip(completed).enumerate() {
            packets[index] = validate_canonical_structured_value(encoder.packet(*packet)?)?;
        }
        let sequence = validate_canonical_structured_value(
            self.sequence.collection(&packets[..completed.len()])?,
        )?;
        let (tag, payload) = match outcome {
            IsochronousOutcome::Completed => ("completed", self.completed.record(&[sequence])?),
            IsochronousOutcome::Short(packet) => {
                let packet =
                    validate_canonical_structured_value(self.fault_packet.packet(packet)?)?;
                ("short", self.short.record(&[sequence, packet])?)
            }
            IsochronousOutcome::MissedService { frame } => {
                let frame = validate_canonical_structured_value(
                    self.frame.leaf(&u64::from(frame).to_le_bytes())?,
                )?;
                ("missed-service", self.missed.record(&[sequence, frame])?)
            }
            IsochronousOutcome::Underrun => ("underrun", self.stopped.record(&[sequence])?),
            IsochronousOutcome::ProviderLost => {
                ("provider-lost", self.stopped.record(&[sequence])?)
            }
            IsochronousOutcome::Drained => ("drained", self.unit.as_slice()),
            IsochronousOutcome::Snapshot(snapshot) => (
                "snapshot",
                self.snapshot.compose(&[
                    ("current-microframe", u64::from(snapshot.current_microframe)),
                    (
                        "scheduling-threshold",
                        u64::from(snapshot.scheduling_threshold),
                    ),
                    (
                        "service-interval-microframes",
                        u64::from(snapshot.service_interval_microframes),
                    ),
                    (
                        "maximum-packet-bytes",
                        u64::from(snapshot.maximum_packet_bytes),
                    ),
                    ("pending-packets", u64::from(snapshot.pending_packets)),
                    (
                        "maximum-pending-packets",
                        u64::from(snapshot.maximum_pending_packets),
                    ),
                    ("dma-resource-bytes", u64::from(snapshot.dma_resource_bytes)),
                ])?,
            ),
        };
        self.result
            .variant(tag, validate_canonical_structured_value(payload)?)
    }
}
