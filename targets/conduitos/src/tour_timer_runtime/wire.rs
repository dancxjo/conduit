//! Explicit bounded mechanism ABI; never copy a Rust object layout across domains.
use super::{CORDS, NODES, PORTS, PreparedTimerGraph, PreparedTimerRoute};
use conduit_kernel::{
    CordEndpoint, CordId, HostCallBinding, HostCallId, NodeId, PortId, RouteRange, RouteTarget,
    scheduler::{
        AssignedConnectionTrack, AssignedPressurePolicy, CordCapacity, CordSpec, NodeSpec,
        SchedulerError,
    },
};

impl PreparedTimerGraph {
    pub(crate) fn encode(&self, output: &mut [u8]) -> Result<usize, SchedulerError> {
        let mut writer = Writer {
            output,
            position: 0,
        };
        writer.u16(0x5451)?;
        writer.u16(1)?;
        for node in [self.timer, self.count, self.presentation] {
            writer.u16(node.0)?;
        }
        writer.bytes(&self.period.to_le_bytes())?;
        writer.bytes(&self.start.to_le_bytes())?;
        writer.u32(self.sign_bytes)?;
        for node in &self.nodes {
            for cord in node.input_cords {
                writer.u16(cord.map_or(u16::MAX, |cord| cord.0))?;
            }
            writer.u16(node.maximum_step_fuel)?;
        }
        for cord in &self.cords {
            writer.u16(cord.cord.0)?;
            writer.endpoint(cord.source)?;
            writer.endpoint(cord.sink)?;
            writer.u16(cord.slot_start)?;
            writer.u16(cord.item_capacity)?;
            writer.u32(cord.byte_capacity)?;
            writer.u32(cord.maximum_value_bytes)?;
            writer.bytes(&[cord.pressure_policy as u8, cord.track as u8])?;
        }
        for route in self.routes {
            writer.bytes(&[u8::from(route.is_some())])?;
            if let Some(route) = route {
                writer.u16(route.node.0)?;
                writer.u16(route.port.0)?;
                writer.u16(route.range.start)?;
                writer.u16(route.range.len)?;
                writer.u16(route.target.cord.0)?;
                writer.endpoint(route.target.sink)?;
            }
        }
        for binding in self.bindings {
            writer.bytes(&[u8::from(binding.is_some())])?;
            if let Some((node, binding)) = binding {
                writer.u16(node.0)?;
                writer.u16(binding.call.0)?;
                writer.u32(binding.maximum_input_bytes)?;
                writer.u32(binding.maximum_output_bytes)?;
            }
        }
        Ok(writer.position)
    }

    pub(crate) fn decode(input: &[u8]) -> Result<Self, SchedulerError> {
        let mut reader = Reader { input, position: 0 };
        if reader.u16()? != 0x5451 || reader.u16()? != 1 {
            return invalid();
        }
        let timer = NodeId(reader.u16()?);
        let count = NodeId(reader.u16()?);
        let presentation = NodeId(reader.u16()?);
        let period = u64::from_le_bytes(reader.take()?);
        let start = u64::from_le_bytes(reader.take()?);
        let sign_bytes = reader.u32()?;
        let mut nodes = [NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_fuel: 0,
        }; NODES];
        for node in &mut nodes {
            for cord in &mut node.input_cords {
                let id = reader.u16()?;
                *cord = (id != u16::MAX).then_some(CordId(id));
            }
            node.maximum_step_fuel = reader.u16()?;
        }
        let mut cords = [CordSpec::inactive(); CORDS];
        for cord in &mut cords {
            let id = CordId(reader.u16()?);
            let source = reader.endpoint()?;
            let sink = reader.endpoint()?;
            let slot_start = reader.u16()?;
            let item_capacity = reader.u16()?;
            let byte_capacity = reader.u32()?;
            let maximum = reader.u32()?;
            let pressure_policy = match reader.byte()? {
                0 => AssignedPressurePolicy::PreserveOrder,
                1 => AssignedPressurePolicy::CoalesceLatest,
                _ => return invalid(),
            };
            let track = match reader.byte()? {
                0 => AssignedConnectionTrack::Payload,
                1 => AssignedConnectionTrack::NormalClose,
                2 => AssignedConnectionTrack::AbnormalTerminal,
                3 => AssignedConnectionTrack::Quiescence,
                _ => return invalid(),
            };
            *cord = CordSpec::new(
                id,
                source,
                sink,
                CordCapacity {
                    slot_start,
                    item_capacity,
                    byte_capacity,
                    pressure_policy,
                },
            )
            .with_maximum_value_bytes(maximum)
            .with_track(track);
        }
        let mut routes = [None; CORDS];
        for route in &mut routes {
            if reader.present()? {
                *route = Some(PreparedTimerRoute {
                    node: NodeId(reader.u16()?),
                    port: PortId(reader.u16()?),
                    range: RouteRange {
                        start: reader.u16()?,
                        len: reader.u16()?,
                    },
                    target: RouteTarget {
                        cord: CordId(reader.u16()?),
                        sink: reader.endpoint()?,
                    },
                });
            }
        }
        let mut bindings = [None; NODES];
        for binding in &mut bindings {
            if reader.present()? {
                *binding = Some((
                    NodeId(reader.u16()?),
                    HostCallBinding {
                        call: HostCallId(reader.u16()?),
                        maximum_input_bytes: reader.u32()?,
                        maximum_output_bytes: reader.u32()?,
                    },
                ));
            }
        }
        if reader.position != input.len() {
            return invalid();
        }
        Ok(Self {
            nodes,
            cords,
            routes,
            bindings,
            timer,
            count,
            presentation,
            period,
            start,
            sign_bytes,
        })
    }
}

fn invalid<T>() -> Result<T, SchedulerError> {
    Err(SchedulerError::InvalidPlan)
}

struct Writer<'a> {
    output: &'a mut [u8],
    position: usize,
}
impl Writer<'_> {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), SchedulerError> {
        let end = self
            .position
            .checked_add(bytes.len())
            .ok_or(SchedulerError::InvalidPlan)?;
        self.output
            .get_mut(self.position..end)
            .ok_or(SchedulerError::InvalidPlan)?
            .copy_from_slice(bytes);
        self.position = end;
        Ok(())
    }
    fn u16(&mut self, value: u16) -> Result<(), SchedulerError> {
        self.bytes(&value.to_le_bytes())
    }
    fn u32(&mut self, value: u32) -> Result<(), SchedulerError> {
        self.bytes(&value.to_le_bytes())
    }
    fn endpoint(&mut self, value: CordEndpoint) -> Result<(), SchedulerError> {
        let CordEndpoint::Local { node, port } = value else {
            return invalid();
        };
        self.u16(node.0)?;
        self.u16(port.0)
    }
}

struct Reader<'a> {
    input: &'a [u8],
    position: usize,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], SchedulerError> {
        let end = self
            .position
            .checked_add(N)
            .ok_or(SchedulerError::InvalidPlan)?;
        let bytes = self
            .input
            .get(self.position..end)
            .ok_or(SchedulerError::InvalidPlan)?;
        self.position = end;
        bytes.try_into().map_err(|_| SchedulerError::InvalidPlan)
    }
    fn byte(&mut self) -> Result<u8, SchedulerError> {
        Ok(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, SchedulerError> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32, SchedulerError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn present(&mut self) -> Result<bool, SchedulerError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => invalid(),
        }
    }
    fn endpoint(&mut self) -> Result<CordEndpoint, SchedulerError> {
        Ok(CordEndpoint::local(
            NodeId(self.u16()?),
            PortId(self.u16()?),
        ))
    }
}
