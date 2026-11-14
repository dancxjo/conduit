//! Canonical identity for observed original-kernel Host Call evidence. An ID
//! names evidence; it is neither a signature nor durable recording acceptance.
extern crate alloc;
use crate::execution::ExecutionSigns;
use alloc::string::String;
use conduit_core as c;
use conduit_kernel::{KernelEventKind, NodeId, RequestId};
use sha2::{Digest, Sha256};

/// Actual approved scope retained by Root. Fragment separates child kernels
/// which share one Body Play; request generations survive Mask session rearm.
pub struct Scope<'a> {
    pub body: &'a conduit_body::BodyId,
    pub active: &'a c::ActivePlayIdentity,
    pub fragment: &'a c::FragmentId,
}
#[derive(Clone, Copy)]
pub enum Phase {
    Requested,
    Completed,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Scope,
    Missing,
    Ambiguous,
    Capacity,
    Allocation,
}
/// The event must still be present in the actual kernel's finite Sign log.
/// Call synchronously after request/completion, before another actor advances
/// the same kernel. The returned 64-byte ID needs separately admitted retention.
pub fn host_call(
    started: &crate::execution_workset::StartedEvidence,
    scope: Scope<'_>,
    signs: &impl ExecutionSigns,
    node: NodeId,
    request: RequestId,
    phase: Phase,
    maximum_owned_bytes: usize,
) -> Result<c::SignId, Refusal> {
    let fields = [
        scope.body.as_str(),
        scope.active.host_id.as_str(),
        scope.active.boot_id.as_str(),
        scope.active.plan_id.as_str(),
        scope.active.active_play_id.as_str(),
        scope.fragment.as_str(),
    ];
    if fields.iter().any(|s| s.is_empty()) {
        return Err(Refusal::Scope);
    }
    let (kind, tag) = match phase {
        Phase::Requested => (KernelEventKind::HostCallRequested, 1u8),
        Phase::Completed => (KernelEventKind::HostCallCompleted, 2u8),
    };
    let mut matches = signs
        .events()
        .filter(|e| e.node == node && e.request == Some(request) && e.kind == kind);
    let event = matches.next().ok_or(Refusal::Missing)?;
    if matches.next().is_some() {
        return Err(Refusal::Ambiguous);
    }
    if maximum_owned_bytes < 64 {
        return Err(Refusal::Capacity);
    }
    let mut hash = Sha256::new();
    hash.update(b"conduit.kernel/observed-host-call-sign@1\0");
    for field in fields {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    hash.update(started.custody_identity());
    hash.update(scope.active.play_sequence.to_le_bytes());
    hash.update(event.sequence.to_le_bytes());
    hash.update(node.0.to_le_bytes());
    hash.update(request.0.to_le_bytes());
    hash.update([tag]);
    let mut id = String::new();
    id.try_reserve_exact(64).map_err(|_| Refusal::Allocation)?;
    if id.capacity() > maximum_owned_bytes {
        return Err(Refusal::Capacity);
    }
    use core::fmt::Write;
    for byte in hash.finalize() {
        write!(&mut id, "{byte:02x}").map_err(|_| Refusal::Capacity)?;
    }
    Ok(id.into())
}

/// Names the successful original authenticated Start as causal evidence for
/// preparing the first Show. This is distinct from a kernel request/completion
/// and does not claim that a renderer has already prepared or displayed a frame.
/// Face revision/content identity must remain part of the Show's own binding.
pub fn authenticated_start(
    started: &crate::execution_workset::StartedEvidence,
    scope: Scope<'_>,
    maximum_owned_bytes: usize,
) -> Result<c::SignId, Refusal> {
    let fields = [
        scope.body.as_str(),
        scope.active.host_id.as_str(),
        scope.active.boot_id.as_str(),
        scope.active.plan_id.as_str(),
        scope.active.active_play_id.as_str(),
        scope.fragment.as_str(),
    ];
    if fields.iter().any(|field| field.is_empty()) {
        return Err(Refusal::Scope);
    }
    if maximum_owned_bytes < 64 {
        return Err(Refusal::Capacity);
    }
    let mut hash = Sha256::new();
    hash.update(b"conduit.root/authenticated-start-sign@1\0");
    hash.update(started.custody_identity());
    for field in fields {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
    hash.update(scope.active.play_sequence.to_le_bytes());
    let mut id = String::new();
    id.try_reserve_exact(64).map_err(|_| Refusal::Allocation)?;
    if id.capacity() > maximum_owned_bytes {
        return Err(Refusal::Capacity);
    }
    use core::fmt::Write;
    for byte in hash.finalize() {
        write!(&mut id, "{byte:02x}").map_err(|_| Refusal::Capacity)?;
    }
    Ok(id.into())
}
