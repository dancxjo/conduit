// Independent installed Source/peer facts, never pins copied from preparation.
use alloc::{string::String, vec::Vec};
use conduit_body as b;
use conduit_core as c;
use conduit_mcu_host_runtime::{
    fore_bridge::{EgressSpec, PrimitiveIngressSpec},
    root::ForeCarrierBounds,
};

pub const SESSION_CONTRACT: &str = conduit_wire::PHYSICAL_FORE_SESSION_CONTRACT;
const PORTS: usize = crate::play::FORE_PORTS;

pub struct Hot {
    pub local: heapless::Vec<c::AssignedForeRequirement, PORTS>,
    pub lines: heapless::Vec<c::AssignedLineForeRequirement, PORTS>,
    pub bindings: heapless::Vec<c::AssignedRemoteBinding, PORTS>,
    input_temporal: &'static str,
    output_temporal: &'static str,
    input_quantity: c::QuantityValueProfile,
    output_quantity: c::QuantityValueProfile,
}
pub struct Cold {
    pub lines: Vec<c::AdmittedLine>,
    pub session: String,
}
impl Hot {
    pub fn profile(&self) -> c::AssignedLineForeProfile<'_> {
        c::AssignedLineForeProfile {
            local: &self.local,
            lines: &self.lines,
        }
    }
    /// Check the complete standard ABI cold; returned specs borrow only hot pins.
    pub fn specs(&self) -> Result<(PrimitiveIngressSpec<'_>, EgressSpec<'_>), &'static str> {
        let kind = conduit_resource_snapshot::read_contract::definition();
        let front = kind.checked_front();
        let input = self
            .lines
            .iter()
            .find(|p| p.fore().direction() == c::AssignedForeDirection::Input)
            .ok_or("missing Line input")?;
        let output = self
            .lines
            .iter()
            .find(|p| p.fore().direction() == c::AssignedForeDirection::Output)
            .ok_or("missing Line output")?;
        let input_contract = front.value_contract(&c::FrontValueLocation::Input(
            kind.inputs[0].port_id.clone(),
        ));
        let output_contract = front.value_contract(&c::FrontValueLocation::Output(
            kind.outputs[0].port_id.clone(),
        ));
        let ingress = PrimitiveIngressSpec::checked(
            input,
            c::AssignedForeAbi {
                value_kind: c::COUNT_INFO_ID,
                value_contract: input_contract,
                abnormal_kind: None,
                temporal: self.input_temporal,
                track: c::ConnectionTrack::Payload,
                quantity_profile: self.input_quantity,
                item_capacity: 1,
                byte_capacity: conduit_resource_snapshot::read_contract::READ_INPUT_BYTES,
            },
        )
        .map_err(|_| "read input ABI")?;
        let egress = EgressSpec::checked(
            output,
            c::AssignedForeAbi {
                value_kind: kind.outputs[0].value_kind.as_str(),
                value_contract: output_contract,
                abnormal_kind: None,
                temporal: self.output_temporal,
                track: c::ConnectionTrack::Payload,
                quantity_profile: self.output_quantity,
                item_capacity: 1,
                byte_capacity: conduit_resource_snapshot::read_result::READ_RESULT_BYTES as u32,
            },
        )
        .map_err(|_| "read result ABI")?;
        Ok((ingress, egress))
    }
}
struct Selected {
    pin: c::AssignedForeRequirement,
    temporal: &'static str,
    quantity: c::QuantityValueProfile,
}
fn select(
    all: &[c::AssignedForeRequirement],
    direction: c::AssignedForeDirection,
    kind: &str,
    contract: Option<&c::CheckedValueContract>,
    bytes: u32,
) -> Result<Selected, &'static str> {
    let mut selected = None;
    for pin in all.iter().filter(|p| p.direction() == direction) {
        for temporal in ["flow-closing", "flow-open"] {
            for quantity in [
                c::QuantityValueProfile::Decimal,
                c::QuantityValueProfile::Exact,
            ] {
                let digest = c::assigned_fore_abi_digest(c::AssignedForeAbi {
                    value_kind: kind,
                    value_contract: contract,
                    abnormal_kind: None,
                    temporal,
                    track: c::ConnectionTrack::Payload,
                    quantity_profile: quantity,
                    item_capacity: 1,
                    byte_capacity: bytes,
                });
                if pin.payload()[21..] == digest {
                    if selected.is_some() {
                        return Err("ambiguous read-service Fore");
                    }
                    selected = Some(Selected {
                        pin: *pin,
                        temporal,
                        quantity,
                    });
                }
            }
        }
    }
    selected.ok_or("installed Source lacks exact read-service Fore")
}

pub fn prepare(
    context: b::NativePreparationContext<'_>,
    implementation: &str,
    bounds: ForeCarrierBounds,
    all_fore: &[c::AssignedForeRequirement],
) -> Result<(Hot, Cold), &'static str> {
    if all_fore.len() > PORTS
        || bounds.maximum_lines < 2
        || context.actual.profile.as_str() != context.configuration_id
        || context.member.body_id != context.bootstrap.body_id
        || implementation.is_empty()
    {
        return Err("physical Fore installation scope or capacity");
    }
    let kind = conduit_resource_snapshot::read_contract::definition();
    let front = kind.checked_front();
    let input = select(
        all_fore,
        c::AssignedForeDirection::Input,
        c::COUNT_INFO_ID,
        front.value_contract(&c::FrontValueLocation::Input(
            kind.inputs[0].port_id.clone(),
        )),
        conduit_resource_snapshot::read_contract::READ_INPUT_BYTES,
    )?;
    let output = select(
        all_fore,
        c::AssignedForeDirection::Output,
        kind.outputs[0].value_kind.as_str(),
        front.value_contract(&c::FrontValueLocation::Output(
            kind.outputs[0].port_id.clone(),
        )),
        conduit_resource_snapshot::read_result::READ_RESULT_BYTES as u32,
    )?;
    let mut hot = Hot {
        local: heapless::Vec::new(),
        lines: heapless::Vec::new(),
        bindings: heapless::Vec::new(),
        input_temporal: input.temporal,
        output_temporal: output.temporal,
        input_quantity: input.quantity,
        output_quantity: output.quantity,
    };
    for pin in all_fore {
        if pin != &input.pin && pin != &output.pin {
            hot.local.push(*pin).map_err(|_| "local Fore capacity")?;
        }
    }
    let peer = &context.bootstrap.owner;
    let installation = conduit_wire::PhysicalForeInstallation {
        body_id: context.member.body_id.as_str(),
        configuration_id: context.configuration_id,
        implementation_id: implementation,
        session_contract: SESSION_CONTRACT,
    };
    let local = conduit_wire::PhysicalForePeerFacts {
        host_id: context.actual.host_id.as_str(),
        boot_id: context.actual.boot_id.as_str(),
        offer_generation: context.actual.offer_generation.0,
    };
    let owner = conduit_wire::PhysicalForePeerFacts {
        host_id: peer.host_id.as_str(),
        boot_id: peer.boot_id.as_str(),
        offer_generation: peer.offer_generation.0,
    };
    let limits = c::LinkLimits {
        maximum_in_flight_items: bounds.maximum_in_flight_items,
        maximum_payload_bytes: bounds.maximum_payload_bytes,
        maximum_buffered_bytes: u32::try_from(bounds.maximum_buffered_bytes)
            .map_err(|_| "carrier backing width")?,
        maximum_frame_bytes: bounds.maximum_frame_bytes,
    };
    // These installed contract facts describe the supported original serial
    // wrapper. The unsafe native installation still owns the physical proof.
    let contract = c::LineContract {
        scope: c::LineScope::PointToPoint,
        traffic_shape: c::LineTrafficShape::Message,
        duplex: c::LineDuplex::FullDuplex,
        ordering: c::LineOrdering::Ordered,
        reliability: c::LineReliability::Reliable,
        continuation: c::LineContinuation::None,
        security: c::LineSecurity::PhysicalPossession,
    };
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(2)
        .map_err(|_| "cold physical Line allocation")?;
    let mut session = None;
    for pin in [input.pin, output.pin] {
        let described = conduit_wire::describe_physical_fore_line(
            installation,
            local,
            owner,
            &pin,
            limits,
            contract,
        )
        .map_err(|_| "physical Line description")?;
        if session
            .as_ref()
            .is_some_and(|prior| prior != &described.session_id)
        {
            return Err("physical carrier session differs");
        }
        if session.is_none() {
            session = Some(described.session_id);
        }
        let (device, caller) = match pin.direction() {
            c::AssignedForeDirection::Input => {
                (&described.line.binding.sink, &described.line.binding.source)
            }
            c::AssignedForeDirection::Output => {
                (&described.line.binding.source, &described.line.binding.sink)
            }
        };
        let binding = c::AssignedRemoteBinding {
            line: c::AssignedIdentity::from_text(described.line.line_id.as_str()),
            local_host: c::AssignedIdentity::from_text(device.host_id.as_str()),
            local_boot: c::AssignedIdentity::from_text(device.boot_id.as_str()),
            peer_host: c::AssignedIdentity::from_text(caller.host_id.as_str()),
            peer_boot: c::AssignedIdentity::from_text(caller.boot_id.as_str()),
        };
        let row = c::AssignedLineForeRequirement::new(
            pin,
            binding,
            c::AssignedIdentity::from_text(session.as_deref().ok_or("carrier session absent")?),
            c::AssignedIdentity::from_text(SESSION_CONTRACT),
            c::admitted_line_commitment(&described.line),
        );
        hot.lines.push(row).map_err(|_| "Line Fore capacity")?;
        hot.bindings
            .push(binding)
            .map_err(|_| "Line binding capacity")?;
        lines.push(described.line);
    }
    hot.specs()?;
    Ok((
        hot,
        Cold {
            lines,
            session: session.ok_or("carrier session absent")?,
        },
    ))
}
