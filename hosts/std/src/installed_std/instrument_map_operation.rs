//! Bounded local realization of portable structured instrument controls.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_audio::{
    Gate, ModulationDestination, MusicalControl, MusicalControlEvent, MusicalNoteEvent,
    MusicalPitch, NoteOccurrenceId,
};
use conduit_core::{
    ConfigurationValue, PlannedGear, StructuredInfoValue, StructuredInfoValueShape,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{
    CanonicalValue, Failure, FailureCode, OperationAction, OperationInput, PortId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::INSTRUMENT_MAP_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct InstrumentMapOperation {
    mapping: InstrumentMapping,
    next_order: u32,
    emitted: bool,
}

struct InstrumentMapping {
    pitch_millihertz: [u64; 8],
    sustain_button: u64,
    modulation_control: u64,
    expression_control: u64,
}

impl InstrumentMapOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { .. } => InstalledOperation::fail(170),
            OperationInput::Closed { port: PortId(0) } if !self.emitted => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(174),
        }
    }

    pub(super) fn resume_value(&mut self, port: PortId, bytes: &[u8]) -> OperationAction {
        if port != PortId(0) || self.emitted {
            return InstalledOperation::fail(171);
        }
        if self.next_order >= u32::from(conduit_std_catalog::MAXIMUM_MUSICAL_EVENT_ITEMS) {
            return fail(FailureCode::StorageExhausted, 172);
        }
        let Some(next_order) = self.next_order.checked_add(1) else {
            return fail(FailureCode::StorageExhausted, 173);
        };
        if bytes.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return fail(FailureCode::InvalidInput, 174);
        }
        let Ok(control) = StructuredInfoValue::from_canonical_bytes(bytes) else {
            return fail(FailureCode::InvalidInput, 175);
        };
        if control.value_type() != &conduit_std_catalog::instrument_control_type() {
            return fail(FailureCode::InvalidInput, 176);
        }
        let event = match map_control(&self.mapping, &control, self.next_order) {
            Ok(event) => event,
            Err(detail) => return fail(FailureCode::InvalidInput, detail),
        };
        self.next_order = next_order;
        self.emitted = true;
        event
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if !self.emitted {
            return InstalledOperation::fail(175);
        }
        self.emitted = false;
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.emitted = false;
    }
}

enum MappedEvent {
    Note(MusicalNoteEvent),
    Control(MusicalControlEvent),
}

impl From<MappedEvent> for OperationAction {
    fn from(event: MappedEvent) -> Self {
        let (port, encoded) = match event {
            MappedEvent::Note(event) => (PortId(0), event.encode().to_vec()),
            MappedEvent::Control(event) => (PortId(1), event.encode().to_vec()),
        };
        OperationAction::EmitCanonical {
            port,
            value: CanonicalValue::new(&encoded).expect("portable music encodings are bounded"),
        }
    }
}

fn map_control(
    mapping: &InstrumentMapping,
    control: &StructuredInfoValue,
    order: u32,
) -> Result<OperationAction, u16> {
    let StructuredInfoValueShape::Variant { tag, payload } = control.shape() else {
        return Err(176);
    };
    let fields = record(payload, 177)?;
    match tag {
        "button" => {
            let index = count(field(fields, "index", 178)?, 179)?;
            let down = boolean(field(fields, "down", 180)?, 181)?;
            let event_time = count(field(fields, "event_time_micros", 182)?, 183)?;
            let occurrence = count(field(fields, "occurrence", 184)?, 185)?;
            if index == mapping.sustain_button {
                MusicalControlEvent::new(MusicalControl::Sustain { down }, event_time, order)
                    .map(MappedEvent::Control)
                    .map(Into::into)
                    .map_err(|_| 186)
            } else {
                let pitch = usize::try_from(index)
                    .ok()
                    .and_then(|index| mapping.pitch_millihertz.get(index))
                    .ok_or(187_u16)?;
                let pitch = MusicalPitch::new(*pitch, 440_000, 0).map_err(|_| 188_u16)?;
                MusicalNoteEvent::new(
                    NoteOccurrenceId(occurrence),
                    pitch,
                    if down { Gate::On } else { Gate::Off },
                    if down { u16::MAX } else { 0 },
                    event_time,
                    order,
                )
                .map(MappedEvent::Note)
                .map(Into::into)
                .map_err(|_| 189)
            }
        }
        "analog" => {
            let index = count(field(fields, "index", 190)?, 191)?;
            let amount = u32::try_from(count(field(fields, "value", 192)?, 193)?)
                .ok()
                .filter(|value| *value <= 1_000_000)
                .ok_or(194_u16)?;
            let event_time = count(field(fields, "event_time_micros", 195)?, 196)?;
            let destination = if index == mapping.modulation_control {
                ModulationDestination::FilterCutoff
            } else if index == mapping.expression_control {
                ModulationDestination::Amplitude
            } else {
                return Err(197);
            };
            MusicalControlEvent::new(
                MusicalControl::Modulation {
                    amount_millionths: amount,
                    destination,
                },
                event_time,
                order,
            )
            .map(MappedEvent::Control)
            .map(Into::into)
            .map_err(|_| 198)
        }
        _ => Err(199),
    }
}

fn mapping(placement: &PlannedGear) -> Result<InstrumentMapping, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("instrument map requires one exact planned mapping".into());
    };
    let ("mapping", ConfigurationValue::Structured(configuration)) =
        (entry.key.as_str(), &entry.value)
    else {
        return Err("instrument map planned mapping is malformed".into());
    };
    let value = StructuredInfoValue::from_canonical_bytes(configuration.canonical_value())
        .map_err(|error| format!("decode planned instrument mapping: {error:?}"))?;
    if value.value_type() != &conduit_std_catalog::instrument_mapping_type()
        || configuration.profile()
            != conduit_std_catalog::instrument_mapping_type()
                .profile()
                .map_err(|error| format!("profile instrument mapping: {error:?}"))?
                .value_kind()
    {
        return Err("instrument map planned mapping type/profile mismatch".into());
    }
    let fields = record(&value, 200).map_err(|_| "instrument mapping is not a record")?;
    let pitches =
        collection(field(fields, "pitch_millihertz", 201).map_err(detail)?, 202).map_err(detail)?;
    let pitch_millihertz = pitches
        .iter()
        .map(|value| count(value, 203).map_err(detail))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "instrument mapping requires exactly eight pitches")?;
    let mapping = InstrumentMapping {
        pitch_millihertz,
        sustain_button: count(field(fields, "sustain_button", 204).map_err(detail)?, 205)
            .map_err(detail)?,
        modulation_control: count(
            field(fields, "modulation_control", 206).map_err(detail)?,
            207,
        )
        .map_err(detail)?,
        expression_control: count(
            field(fields, "expression_control", 208).map_err(detail)?,
            209,
        )
        .map_err(detail)?,
    };
    if mapping.sustain_button < mapping.pitch_millihertz.len() as u64
        || mapping.modulation_control == mapping.expression_control
        || mapping
            .pitch_millihertz
            .iter()
            .any(|pitch| MusicalPitch::new(*pitch, 440_000, 0).is_err())
    {
        return Err("instrument mapping contains overlapping controls or invalid pitches".into());
    }
    Ok(mapping)
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::instrument_map_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
    {
        return Err("planned instrument map differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    mapping(placement)?;
    Ok(OperationBudget {
        value_items: 4,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES
            + conduit_audio::NOTE_EVENT_ENCODED_LEN * 3) as u32,
        host_requests: 0,
        sign_items: conduit_std_catalog::MAXIMUM_MUSICAL_EVENT_ITEMS.saturating_mul(4),
        maximum_value_bytes: conduit_audio::NOTE_EVENT_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::InstrumentMap(InstrumentMapOperation {
        mapping: mapping(placement)?,
        next_order: 0,
        emitted: false,
    }))
}

fn record(
    value: &StructuredInfoValue,
    detail: u16,
) -> Result<&[conduit_core::StructuredFieldValue], u16> {
    match value.shape() {
        StructuredInfoValueShape::Record(fields) => Ok(fields),
        _ => Err(detail),
    }
}

fn collection(value: &StructuredInfoValue, detail: u16) -> Result<&[StructuredInfoValue], u16> {
    match value.shape() {
        StructuredInfoValueShape::Collection(values) => Ok(values),
        _ => Err(detail),
    }
}

fn field<'a>(
    fields: &'a [conduit_core::StructuredFieldValue],
    name: &str,
    detail: u16,
) -> Result<&'a StructuredInfoValue, u16> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(conduit_core::StructuredFieldValue::value)
        .ok_or(detail)
}

fn count(value: &StructuredInfoValue, detail: u16) -> Result<u64, u16> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(detail);
    };
    let text = core::str::from_utf8(bytes).map_err(|_| detail)?;
    let parsed = text.parse::<u64>().map_err(|_| detail)?;
    (parsed.to_string() == text).then_some(parsed).ok_or(detail)
}

fn boolean(value: &StructuredInfoValue, detail: u16) -> Result<bool, u16> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(detail);
    };
    match bytes {
        b"true" => Ok(true),
        b"false" => Ok(false),
        _ => Err(detail),
    }
}

fn detail(_: u16) -> String {
    "instrument mapping structured field mismatch".into()
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
#[path = "instrument_map_operation_tests.rs"]
mod tests;
