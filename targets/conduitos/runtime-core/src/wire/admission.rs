use super::{Malformed, codec::*};
use crate::{
    admission::{Admission, BackKind},
    *,
};
use conduit_kernel::{
    scheduler::{AssignedConnectionTrack, AssignedPressurePolicy},
    *,
};
pub fn encode(admission: &Admission, bytes: &mut [u8]) -> Result<usize, Malformed> {
    let mut w = Writer { bytes, at: 0 };
    w.u16(admission.active_nodes)?;
    w.u16(admission.active_cords)?;
    for (node, back) in admission.nodes.iter().zip(admission.backs) {
        w.u16(node.maximum_step_fuel)?;
        for cord in node.input_cords {
            w.u16(cord.map_or(u16::MAX, |c| c.0))?;
        }
        w.u8(match back {
            BackKind::Keyboard => 0,
            BackKind::Keymap => 1,
            BackKind::Upper => 2,
            BackKind::Edit => 3,
            BackKind::Face => 4,
            BackKind::Application => 5,
        })?;
    }
    for cord in admission.cords {
        w.u16(cord.cord.0)?;
        endpoint_write(&mut w, cord.source)?;
        endpoint_write(&mut w, cord.sink)?;
        w.u16(cord.slot_start)?;
        w.u16(cord.item_capacity)?;
        w.u32(cord.byte_capacity)?;
        w.u32(cord.maximum_value_bytes)?;
        w.u8(cord.pressure_policy as u8)?;
        w.u8(cord.track as u8)?;
    }
    for range in admission.ranges {
        w.u8(u8::from(range.is_some()))?;
        if let Some(range) = range {
            w.u16(range.start)?;
            w.u16(range.len)?;
        }
    }
    for target in admission.targets {
        w.u8(u8::from(target.is_some()))?;
        if let Some(target) = target {
            w.u16(target.cord.0)?;
            endpoint_write(&mut w, target.sink)?;
        }
    }
    for binding in admission.bindings {
        w.u8(u8::from(binding.is_some()))?;
        if let Some(binding) = binding {
            w.u16(binding.call.0)?;
            w.u32(binding.maximum_input_bytes)?;
            w.u32(binding.maximum_output_bytes)?;
        }
    }
    Ok(w.at)
}
pub fn decode(bytes: &[u8]) -> Result<Admission, Malformed> {
    let mut r = Reader { bytes, at: 0 };
    let mut result = Admission::new(r.u16()?, r.u16()?);
    if result.active_nodes == 0
        || usize::from(result.active_nodes) > NODES
        || usize::from(result.active_cords) > CORDS
    {
        return Err(Malformed);
    }
    for (node, back) in result.nodes.iter_mut().zip(result.backs.iter_mut()) {
        node.maximum_step_fuel = r.u16()?;
        for cord in &mut node.input_cords {
            let n = r.u16()?;
            *cord = (n != u16::MAX).then_some(CordId(n));
        }
        *back = match r.u8()? {
            0 => BackKind::Keyboard,
            1 => BackKind::Keymap,
            2 => BackKind::Upper,
            3 => BackKind::Edit,
            4 => BackKind::Face,
            5 => BackKind::Application,
            _ => return Err(Malformed),
        };
    }
    for cord in &mut result.cords {
        cord.cord = CordId(r.u16()?);
        cord.source = endpoint_read(&mut r)?;
        cord.sink = endpoint_read(&mut r)?;
        cord.slot_start = r.u16()?;
        cord.item_capacity = r.u16()?;
        cord.byte_capacity = r.u32()?;
        cord.maximum_value_bytes = r.u32()?;
        cord.pressure_policy = match r.u8()? {
            0 => AssignedPressurePolicy::PreserveOrder,
            1 => AssignedPressurePolicy::CoalesceLatest,
            _ => return Err(Malformed),
        };
        cord.track = match r.u8()? {
            0 => AssignedConnectionTrack::Payload,
            1 => AssignedConnectionTrack::NormalClose,
            2 => AssignedConnectionTrack::AbnormalTerminal,
            3 => AssignedConnectionTrack::Quiescence,
            _ => return Err(Malformed),
        };
    }
    for range in &mut result.ranges {
        if r.flag()? {
            *range = Some(RouteRange {
                start: r.u16()?,
                len: r.u16()?,
            });
        }
    }
    for target in &mut result.targets {
        if r.flag()? {
            *target = Some(RouteTarget {
                cord: CordId(r.u16()?),
                sink: endpoint_read(&mut r)?,
            });
        }
    }
    for binding in &mut result.bindings {
        if r.flag()? {
            *binding = Some(HostCallBinding {
                call: HostCallId(r.u16()?),
                maximum_input_bytes: r.u32()?,
                maximum_output_bytes: r.u32()?,
            });
        }
    }
    r.finish()?;
    Ok(result)
}
fn endpoint_write(w: &mut Writer<'_>, endpoint: CordEndpoint) -> Result<(), Malformed> {
    match endpoint {
        CordEndpoint::Local { node, port } => {
            w.u8(0)?;
            w.u16(node.0)?;
            w.u16(port.0)
        }
        CordEndpoint::Remote(id) => {
            w.u8(1)?;
            w.u16(id.0)
        }
    }
}
fn endpoint_read(r: &mut Reader<'_>) -> Result<CordEndpoint, Malformed> {
    match r.u8()? {
        0 => Ok(CordEndpoint::Local {
            node: NodeId(r.u16()?),
            port: PortId(r.u16()?),
        }),
        1 => Ok(CordEndpoint::Remote(RemoteEndpointId(r.u16()?))),
        _ => Err(Malformed),
    }
}
