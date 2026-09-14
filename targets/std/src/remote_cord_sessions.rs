//! Generic exact session preparation for remote Cords in an admitted std fragment.

use conduit_core::{PlanFragment, PlannedConnection};
use conduit_kernel::{CordId, RemoteEndpointId};
use conduit_plan_lowering::lowering::{
    LoweredPlanFragment, LoweredRemoteEndpoint, RemoteCordDirection,
};
use conduit_wire::{SessionBinding, SessionMachine, SessionRole};

const MAXIMUM_REMOTE_ENDPOINTS: usize = 16;

pub struct RemoteCordSession {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub direction: RemoteCordDirection,
    binding: SessionBinding,
    machine: SessionMachine,
}

impl RemoteCordSession {
    pub fn binding(&self) -> &SessionBinding {
        &self.binding
    }

    pub fn machine(&self) -> &SessionMachine {
        &self.machine
    }

    pub fn machine_mut(&mut self) -> &mut SessionMachine {
        &mut self.machine
    }
}

pub struct RemoteCordSessions {
    sessions: Vec<RemoteCordSession>,
}

impl RemoteCordSessions {
    pub fn prepare(fragment: &PlanFragment, lowered: &LoweredPlanFragment) -> Result<Self, String> {
        if lowered.remote_endpoints.len() > MAXIMUM_REMOTE_ENDPOINTS {
            return Err("std fragment exceeds the remote Cord endpoint bound".into());
        }
        let mut sessions = Vec::with_capacity(lowered.remote_endpoints.len());
        for remote in &lowered.remote_endpoints {
            if sessions
                .iter()
                .any(|session: &RemoteCordSession| session.endpoint == remote.endpoint)
            {
                return Err("std fragment repeats a remote endpoint identity".into());
            }
            sessions.push(prepare_session(fragment, remote)?);
        }
        Ok(Self { sessions })
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn get(&self, endpoint: RemoteEndpointId) -> Option<&RemoteCordSession> {
        self.sessions
            .iter()
            .find(|session| session.endpoint == endpoint)
    }

    pub fn get_mut(&mut self, endpoint: RemoteEndpointId) -> Option<&mut RemoteCordSession> {
        self.sessions
            .iter_mut()
            .find(|session| session.endpoint == endpoint)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RemoteCordSession> {
        self.sessions.iter()
    }
}

fn prepare_session(
    fragment: &PlanFragment,
    remote: &LoweredRemoteEndpoint,
) -> Result<RemoteCordSession, String> {
    let connection = exact_connection(fragment, remote)?;
    let binding = SessionBinding::from_planned_connection_with_line(
        fragment.plan_id.clone(),
        remote.source_fragment_id.clone(),
        remote.sink_fragment_id.clone(),
        connection,
        &remote.line,
    )
    .map_err(|error| format!("prepare remote Cord binding: {error:?}"))?;
    let role = match remote.direction {
        RemoteCordDirection::Egress => SessionRole::Source,
        RemoteCordDirection::Ingress => SessionRole::Sink,
    };
    let machine = SessionMachine::new(binding.clone(), role)
        .map_err(|error| format!("prepare remote Cord session: {error:?}"))?;
    Ok(RemoteCordSession {
        endpoint: remote.endpoint,
        cord: remote.cord,
        direction: remote.direction,
        binding,
        machine,
    })
}

fn exact_connection<'a>(
    fragment: &'a PlanFragment,
    remote: &LoweredRemoteEndpoint,
) -> Result<&'a PlannedConnection, String> {
    let mut matches = fragment
        .connections
        .iter()
        .filter(|connection| connection.connection_id == remote.connection_id);
    let Some(connection) = matches.next() else {
        return Err("lowered remote endpoint names no fragment connection".into());
    };
    if matches.next().is_some() {
        return Err("lowered remote endpoint connection identity is ambiguous".into());
    }
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_fragments_prepare_the_same_exact_session_binding() {
        let exact = conduit_signal_conformance::exact_distributed_signal_plan().unwrap();
        let source_fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
            .unwrap();
        let sink_fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.sink_advertisement.host_id)
            .unwrap();
        let source_lowered =
            conduit_plan_lowering::lowering::lower_plan_fragment(source_fragment).unwrap();
        let sink_lowered =
            conduit_plan_lowering::lowering::lower_plan_fragment(sink_fragment).unwrap();
        let source = RemoteCordSessions::prepare(source_fragment, &source_lowered).unwrap();
        let sink = RemoteCordSessions::prepare(sink_fragment, &sink_lowered).unwrap();
        assert_eq!(source.len(), 1);
        assert_eq!(sink.len(), 1);
        assert_eq!(
            source.iter().next().unwrap().binding(),
            sink.iter().next().unwrap().binding()
        );
        assert_eq!(
            source.iter().next().unwrap().direction,
            RemoteCordDirection::Egress
        );
        assert_eq!(
            sink.iter().next().unwrap().direction,
            RemoteCordDirection::Ingress
        );
    }
}
