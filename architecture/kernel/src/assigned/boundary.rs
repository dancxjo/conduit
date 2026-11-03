//! Numeric boundary validation from separately admitted resident Fore pins.
use super::*;
pub(super) fn endpoint<const N: usize, const P: usize>(
    data: &[u8],
    offset: usize,
    fore: Option<&[c::AssignedForeRequirement]>,
    cord: u16,
    source: bool,
) -> Result<k::CordEndpoint, AssignedKernelRefusal> {
    if data[offset] == 1 {
        let endpoint = u16_at(data, offset + 1)?;
        if u16_at(data, offset + 3)? != 0
            || !fore.unwrap_or(&[]).iter().any(|f| {
                f.endpoint() == endpoint
                    && f.cord() == cord
                    && (f.direction() == c::AssignedForeDirection::Input) == source
            })
        {
            return Err(AssignedKernelRefusal::Unsupported);
        }
        return Ok(k::CordEndpoint::Remote(k::RemoteEndpointId(endpoint)));
    }
    if data[offset] != 0 {
        return Err(AssignedKernelRefusal::Unsupported);
    }
    let node = u16_at(data, offset + 1)?;
    let port = u16_at(data, offset + 3)?;
    if usize::from(node) >= N || usize::from(port) >= P {
        return Err(AssignedKernelRefusal::Capacity);
    }
    Ok(k::CordEndpoint::local(k::NodeId(node), k::PortId(port)))
}
pub(super) fn route_sink(
    data: &[u8],
    fore: Option<&[c::AssignedForeRequirement]>,
) -> Result<k::CordEndpoint, AssignedKernelRefusal> {
    if data.len() == 6 {
        return Ok(k::CordEndpoint::local(
            k::NodeId(u16_at(data, 2)?),
            k::PortId(u16_at(data, 4)?),
        ));
    }
    endpoint::<0, 0>(data, 2, fore, u16_at(data, 0)?, false)
}
