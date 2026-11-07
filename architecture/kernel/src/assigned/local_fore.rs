//! Descriptive local-Fore provenance created only after complete assigned checks.
//! It is not affine equipment possession, resource admission or Start authority.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LocalForeProvenance {
    cord: k::CordId,
    endpoint: k::RemoteEndpointId,
    node: k::NodeId,
    port: k::PortId,
    input: bool,
}
impl LocalForeProvenance {
    pub(crate) fn matches(&self, cord: &CordSpec) -> bool {
        let local = k::CordEndpoint::local(self.node, self.port);
        let external = k::CordEndpoint::Remote(self.endpoint);
        self.cord == cord.cord
            && (cord.source, cord.sink)
                == if self.input {
                    (external, local)
                } else {
                    (local, external)
                }
    }
}
/// Called only after Core decoding, exact resident layout and all numeric tables
/// have succeeded. A plain/Line Cord constructor cannot request this provenance.
pub(super) fn pin(
    cords: &mut [CordSpec],
    fore: &[c::AssignedForeRequirement],
) -> Result<(), AssignedKernelRefusal> {
    for port in fore {
        let cord = cords
            .get_mut(usize::from(port.cord()))
            .ok_or(AssignedKernelRefusal::Layout)?;
        let (endpoint, node, local_port, input) = match (port.direction(), cord.source, cord.sink) {
            (
                c::AssignedForeDirection::Input,
                k::CordEndpoint::Remote(endpoint),
                k::CordEndpoint::Local { node, port },
            ) => (endpoint, node, port, true),
            (
                c::AssignedForeDirection::Output,
                k::CordEndpoint::Local { node, port },
                k::CordEndpoint::Remote(endpoint),
            ) => (endpoint, node, port, false),
            _ => return Err(AssignedKernelRefusal::Layout),
        };
        if endpoint.0 != port.endpoint() {
            return Err(AssignedKernelRefusal::Layout);
        }
        cord.retain_local_fore(LocalForeProvenance {
            cord: cord.cord,
            endpoint,
            node,
            port: local_port,
            input,
        });
    }
    Ok(())
}
