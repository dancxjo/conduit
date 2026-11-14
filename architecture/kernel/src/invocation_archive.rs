//! Lossless bounded storage for repeated invocation Sign traces.
//! Only request IDs and per-remote-stream sequences may vary affinely.
use crate::{KernelEvent, RemoteLifecycleIdentity, RequestId, SignQuery, SignRetentionGap};

/// Exact owned Sign queries, including every remote identity sidecar.
/// Implementations must return stable, complete, finite queries while borrowed;
/// event sequences identify their exact optional sidecars. Fixed and hosted
/// production Sign logs satisfy this immutable query contract.
pub trait InvocationEvidence: SignQuery {
    fn events(&self) -> impl Iterator<Item = KernelEvent>;
}
impl<const N: usize> InvocationEvidence for crate::FixedSignLog<N> {
    fn events(&self) -> impl Iterator<Item = KernelEvent> {
        self.events()
    }
}
#[cfg(feature = "alloc")]
impl InvocationEvidence for crate::HostedSignLog {
    fn events(&self) -> impl Iterator<Item = KernelEvent> {
        self.events()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveRefusal {
    Capacity,
    InvocationOverflow,
    StreamCapacity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvocationMetadata {
    pub gap: Option<SignRetentionGap>,
    pub decisions: u32,
    pub logical_invocations: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchivedSign {
    pub event: KernelEvent,
    pub remote: Option<RemoteLifecycleIdentity>,
}
#[derive(Clone, Copy)]
struct Run<const STREAMS: usize> {
    start: usize,
    len: usize,
    copies: u32,
    request_step: Option<u32>,
    remote_steps: [Option<u64>; STREAMS],
    metadata: InvocationMetadata,
}
/// Owns complete original Signs as exact templates and checked affine deltas.
/// Refusal never changes retained evidence. No persistence is implied.
pub struct InvocationArchive<const EVENTS: usize, const RUNS: usize, const STREAMS: usize> {
    events: [Option<ArchivedSign>; EVENTS],
    runs: [Option<Run<STREAMS>>; RUNS],
    event_len: usize,
    run_len: usize,
    invocations: u32,
}
impl<const E: usize, const R: usize, const S: usize> Default for InvocationArchive<E, R, S> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const E: usize, const R: usize, const S: usize> InvocationArchive<E, R, S> {
    pub const fn new() -> Self {
        Self {
            events: [None; E],
            runs: [None; R],
            event_len: 0,
            run_len: 0,
            invocations: 0,
        }
    }
    pub fn len(&self) -> u32 {
        self.invocations
    }
    pub fn is_empty(&self) -> bool {
        self.invocations == 0
    }
    fn plan(
        &self,
        signs: &impl InvocationEvidence,
        metadata: InvocationMetadata,
    ) -> Result<Option<Run<S>>, ArchiveRefusal> {
        self.invocations
            .checked_add(1)
            .ok_or(ArchiveRefusal::InvocationOverflow)?;
        if let Some(run) = self.run_len.checked_sub(1).and_then(|i| self.runs[i]) {
            if run.metadata == metadata {
                if let Some(run) = self.matches(run, signs) {
                    return Ok(Some(run));
                }
            }
        }
        if self.run_len == R
            || signs.events().take(E.saturating_add(1)).count() > E - self.event_len
        {
            return Err(ArchiveRefusal::Capacity);
        }
        // Every fresh template must fit the finite stream parameter, too.
        let mut streams = [None; S];
        for event in signs.events().take(E) {
            if let Some(remote) = signs.remote_identity(event.sequence) {
                if !streams
                    .iter()
                    .flatten()
                    .any(|old| same_stream(*old, remote))
                {
                    let slot = streams
                        .iter_mut()
                        .find(|slot| slot.is_none())
                        .ok_or(ArchiveRefusal::StreamCapacity)?;
                    *slot = Some(remote);
                }
            }
        }
        Ok(None)
    }
    fn matches(&self, mut run: Run<S>, signs: &impl InvocationEvidence) -> Option<Run<S>> {
        let mut events = signs.events();
        let mut streams = [None; S];
        for index in run.start..run.start + run.len {
            let base = self.events[index]?;
            let actual = events.next()?;
            if base.event.sequence != actual.sequence
                || base.event.node != actual.node
                || base.event.port != actual.port
                || base.event.kind != actual.kind
            {
                return None;
            }
            match (base.event.request, actual.request) {
                (None, None) => {}
                (Some(base), Some(actual)) => {
                    let delta = actual.0.checked_sub(base.0)?;
                    let step = run.request_step.unwrap_or(delta / run.copies);
                    if step.checked_mul(run.copies)? != delta {
                        return None;
                    }
                    run.request_step = Some(step);
                }
                _ => return None,
            }
            match (base.remote, signs.remote_identity(actual.sequence)) {
                (None, None) => {}
                (Some(base), Some(actual)) if same_stream(base, actual) => {
                    let stream = match streams
                        .iter()
                        .position(|old| old.is_some_and(|old| same_stream(old, base)))
                    {
                        Some(index) => index,
                        None => {
                            let index = streams.iter().position(Option::is_none)?;
                            streams[index] = Some(base);
                            index
                        }
                    };
                    let delta = actual.sequence.checked_sub(base.sequence)?;
                    let step = run.remote_steps[stream].unwrap_or(delta / u64::from(run.copies));
                    if step.checked_mul(u64::from(run.copies))? != delta {
                        return None;
                    }
                    run.remote_steps[stream] = Some(step);
                }
                _ => return None,
            }
        }
        if events.next().is_some() {
            return None;
        }
        Some(run)
    }
    /// Read-only exact preflight, suitable before transferring drained evidence.
    pub fn can_append(
        &self,
        signs: &impl InvocationEvidence,
        decisions: u32,
        logical_invocations: u64,
    ) -> Result<(), ArchiveRefusal> {
        self.plan(
            signs,
            InvocationMetadata {
                gap: signs.retention_gap(),
                decisions,
                logical_invocations,
            },
        )
        .map(|_| ())
    }
    pub fn append(
        &mut self,
        signs: &impl InvocationEvidence,
        decisions: u32,
        logical_invocations: u64,
    ) -> Result<(), ArchiveRefusal> {
        let metadata = InvocationMetadata {
            gap: signs.retention_gap(),
            decisions,
            logical_invocations,
        };
        if let Some(mut run) = self.plan(signs, metadata)? {
            run.copies += 1;
            self.runs[self.run_len - 1] = Some(run);
        } else {
            let start = self.event_len;
            for event in signs.events().take(E) {
                self.events[self.event_len] = Some(ArchivedSign {
                    event,
                    remote: signs.remote_identity(event.sequence),
                });
                self.event_len += 1;
            }
            self.runs[self.run_len] = Some(Run {
                start,
                len: self.event_len - start,
                copies: 1,
                request_step: None,
                remote_steps: [None; S],
                metadata,
            });
            self.run_len += 1;
        }
        self.invocations += 1;
        Ok(())
    }
    pub fn invocation(&self, mut ordinal: u32) -> Option<ArchivedInvocation<'_, S>> {
        for run in self.runs[..self.run_len].iter().flatten() {
            if ordinal < run.copies {
                return Some(ArchivedInvocation {
                    metadata: run.metadata,
                    events: self.events[run.start..run.start + run.len].iter(),
                    request_delta: run.request_step.unwrap_or(0).checked_mul(ordinal)?,
                    remote_steps: run.remote_steps,
                    streams: [None; S],
                    ordinal,
                });
            }
            ordinal -= run.copies;
        }
        None
    }
}
fn same_stream(a: RemoteLifecycleIdentity, b: RemoteLifecycleIdentity) -> bool {
    a.endpoint == b.endpoint && a.cord == b.cord && a.direction == b.direction
}
pub struct ArchivedInvocation<'a, const S: usize> {
    pub metadata: InvocationMetadata,
    events: core::slice::Iter<'a, Option<ArchivedSign>>,
    request_delta: u32,
    remote_steps: [Option<u64>; S],
    streams: [Option<RemoteLifecycleIdentity>; S],
    ordinal: u32,
}
impl<const S: usize> Iterator for ArchivedInvocation<'_, S> {
    type Item = ArchivedSign;
    fn next(&mut self) -> Option<Self::Item> {
        let mut sign = (*self.events.next()?).expect("owned checked template");
        if let Some(request) = sign.event.request {
            sign.event.request = Some(RequestId(
                request
                    .0
                    .checked_add(self.request_delta)
                    .expect("checked affine request"),
            ));
        }
        if let Some(remote) = &mut sign.remote {
            let stream = match self
                .streams
                .iter()
                .position(|old| old.is_some_and(|old| same_stream(old, *remote)))
            {
                Some(index) => index,
                None => {
                    let index = self
                        .streams
                        .iter()
                        .position(Option::is_none)
                        .expect("checked stream capacity");
                    self.streams[index] = Some(*remote);
                    index
                }
            };
            let delta = self.remote_steps[stream]
                .unwrap_or(0)
                .checked_mul(u64::from(self.ordinal))
                .expect("checked affine delta");
            remote.sequence = remote
                .sequence
                .checked_add(delta)
                .expect("checked affine remote sequence");
        }
        Some(sign)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.events.size_hint()
    }
}
impl<const S: usize> ExactSizeIterator for ArchivedInvocation<'_, S> {}
#[cfg(test)]
mod tests;
