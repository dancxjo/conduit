//! Explicit finite LIVE-prefix dictionary and ordered affine tape prototype.
//! This retains evidence, not grants, rearm, persistence or delivery ACKs.
//!
//! Each template stores exact prefix-relative event sequences and gaps. Request
//! IDs are normalized by the minimum ID separately for each finite Node lane;
//! remote sequences are normalized separately for each exact endpoint/Cord/
//! direction lane. All other event and sidecar fields remain exact. The ordered
//! tape selects dictionary templates and retains independent lane bases/steps.
//! Only adjacent copies with checked nonnegative affine steps share a tape run.
//! Non-affine or decreasing bases start another finite run rather than wrapping.
//!
//! Stocks are explicit const parameters. The inline codec is useful for bounded
//! CPU proof/static storage; its `new` constructs arrays by value. The separate
//! allocated live profile initializes its one allocation fieldwise and retains
//! the existing LiveSignLog owner and exclusive transfer token. Neither profile
//! is selected by Source, Make or firmware merely by exporting this module.
use crate::{
    invocation_archive::{ArchivedSign, InvocationEvidence},
    KernelEvent, NodeId, RemoteLifecycleIdentity, RequestId, SignRetentionGap,
};
use core::num::NonZeroU16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DictionaryRefusal {
    Capacity,
    NodeCapacity,
    StreamCapacity,
    Sequence,
    Overflow,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Stream {
    endpoint: crate::RemoteEndpointId,
    cord: crate::CordId,
    direction: crate::RemoteCordDirection,
}
impl From<RemoteLifecycleIdentity> for Stream {
    fn from(r: RemoteLifecycleIdentity) -> Self {
        Self {
            endpoint: r.endpoint,
            cord: r.cord,
            direction: r.direction,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Template {
    start: u16,
    len: NonZeroU16,
    span: u32,
    gap: Option<SignRetentionGap>,
}
#[derive(Clone, Copy)]
struct Segment<const N: usize, const S: usize> {
    template: NonZeroU16,
    copies: u32,
    requests: [u32; N],
    request_steps: [u32; N],
    remotes: [u64; S],
    remote_steps: [u64; S],
}
/// Stocks E/T/G/N/S are selected independently: normalized event rows,
/// dictionary templates, ordered tape runs, request-node lanes, remote lanes.
pub struct LivePrefixArchive<
    const E: usize,
    const T: usize,
    const G: usize,
    const N: usize,
    const S: usize,
> {
    events: [Option<ArchivedSign>; E],
    templates: [Option<Template>; T],
    tape: [Option<Segment<N, S>>; G],
    nodes: [Option<NodeId>; N],
    streams: [Option<Stream>; S],
    event_len: usize,
    template_len: usize,
    tape_len: usize,
    prefixes: u32,
    span: u32,
}
struct Plan<const N: usize, const S: usize> {
    nodes: [Option<NodeId>; N],
    streams: [Option<Stream>; S],
    template: Template,
    template_index: usize,
    segment: Segment<N, S>,
    extend: bool,
    prefixes: u32,
    span: u32,
}
impl<const E: usize, const T: usize, const G: usize, const N: usize, const S: usize> Default
    for LivePrefixArchive<E, T, G, N, S>
{
    fn default() -> Self {
        Self::new()
    }
}
impl<const E: usize, const T: usize, const G: usize, const N: usize, const S: usize>
    LivePrefixArchive<E, T, G, N, S>
{
    pub const fn new() -> Self {
        Self {
            events: [None; E],
            templates: [None; T],
            tape: [None; G],
            nodes: [None; N],
            streams: [None; S],
            event_len: 0,
            template_len: 0,
            tape_len: 0,
            prefixes: 0,
            span: 0,
        }
    }
    pub fn len(&self) -> u32 {
        self.prefixes
    }
    pub fn is_empty(&self) -> bool {
        self.prefixes == 0
    }
    pub fn template_count(&self) -> usize {
        self.template_len
    }
    pub fn template_event_count(&self) -> usize {
        self.event_len
    }
    pub fn tape_run_count(&self) -> usize {
        self.tape_len
    }
    fn plan(
        &self,
        evidence: &impl InvocationEvidence,
        span: u32,
    ) -> Result<Plan<N, S>, DictionaryRefusal> {
        use DictionaryRefusal as F;
        if E > u16::MAX as usize || T > u16::MAX as usize || span == 0 {
            return Err(F::Capacity);
        }
        let prefixes = self.prefixes.checked_add(1).ok_or(F::Overflow)?;
        let total_span = self.span.checked_add(span).ok_or(F::Overflow)?;
        let gap = evidence.retention_gap();
        if gap.is_some_and(|g| {
            g.entries == 0 || g.first_sequence > g.last_sequence || g.last_sequence >= span
        }) {
            return Err(F::Sequence);
        }
        let mut nodes = self.nodes;
        let mut streams = self.streams;
        let mut requests = [None; N];
        let mut remotes = [None; S];
        let mut count = 0usize;
        let mut prior = None;
        for event in evidence.events().take(E.saturating_add(1)) {
            if count == E {
                return Err(F::Capacity);
            }
            if event.sequence >= span || prior.is_some_and(|old| old >= event.sequence) {
                return Err(F::Sequence);
            }
            prior = Some(event.sequence);
            count += 1;
            if let Some(request) = event.request {
                let at = match nodes.iter().position(|node| *node == Some(event.node)) {
                    Some(i) => i,
                    None => {
                        let i = nodes
                            .iter()
                            .position(Option::is_none)
                            .ok_or(F::NodeCapacity)?;
                        nodes[i] = Some(event.node);
                        i
                    }
                };
                requests[at] = Some(requests[at].map_or(request.0, |old: u32| old.min(request.0)));
            }
            if let Some(remote) = evidence.remote_identity(event.sequence) {
                let identity = Stream::from(remote);
                let at = match streams.iter().position(|stream| *stream == Some(identity)) {
                    Some(i) => i,
                    None => {
                        let i = streams
                            .iter()
                            .position(Option::is_none)
                            .ok_or(F::StreamCapacity)?;
                        streams[i] = Some(identity);
                        i
                    }
                };
                remotes[at] =
                    Some(remotes[at].map_or(remote.sequence, |old: u64| old.min(remote.sequence)));
            }
        }
        let len =
            NonZeroU16::new(u16::try_from(count).map_err(|_| F::Capacity)?).ok_or(F::Sequence)?;
        let requests = requests.map(|r| r.unwrap_or(0));
        let remotes = remotes.map(|r| r.unwrap_or(0));
        let mut template_index = None;
        for index in 0..self.template_len {
            let template = self.templates[index].expect("initialized template");
            if template.len != len || template.span != span || template.gap != gap {
                continue;
            }
            let matched = evidence.events().enumerate().all(|(i, event)| {
                normalized(evidence, event, &nodes, &streams, &requests, &remotes)
                    == self.events[usize::from(template.start) + i]
            });
            if matched {
                template_index = Some(index);
                break;
            }
        }
        let index = template_index.unwrap_or(self.template_len);
        if template_index.is_none() && (index == T || count > E - self.event_len) {
            return Err(F::Capacity);
        }
        let template = template_index
            .map(|i| self.templates[i].unwrap())
            .unwrap_or(Template {
                start: self.event_len as u16,
                len,
                span,
                gap,
            });
        let identity = NonZeroU16::new(u16::try_from(index + 1).map_err(|_| F::Capacity)?)
            .ok_or(F::Capacity)?;
        let mut segment = Segment {
            template: identity,
            copies: 1,
            requests,
            request_steps: [0; N],
            remotes,
            remote_steps: [0; S],
        };
        let mut extend = false;
        if let Some(mut last) = self.tape_len.checked_sub(1).and_then(|i| self.tape[i]) {
            if last.template == identity {
                let mut compatible = true;
                for (i, request) in requests.iter().copied().enumerate() {
                    if last.copies == 1 {
                        if let Some(step) = request.checked_sub(last.requests[i]) {
                            last.request_steps[i] = step
                        } else {
                            compatible = false
                        }
                    } else if last.request_steps[i]
                        .checked_mul(last.copies)
                        .and_then(|v| last.requests[i].checked_add(v))
                        != Some(request)
                    {
                        compatible = false
                    }
                }
                for (i, remote) in remotes.iter().copied().enumerate() {
                    if last.copies == 1 {
                        if let Some(step) = remote.checked_sub(last.remotes[i]) {
                            last.remote_steps[i] = step
                        } else {
                            compatible = false
                        }
                    } else if last.remote_steps[i]
                        .checked_mul(u64::from(last.copies))
                        .and_then(|v| last.remotes[i].checked_add(v))
                        != Some(remote)
                    {
                        compatible = false
                    }
                }
                if compatible {
                    last.copies = last.copies.checked_add(1).ok_or(F::Overflow)?;
                    segment = last;
                    extend = true
                }
            }
        }
        if !extend && self.tape_len == G {
            return Err(F::Capacity);
        }
        Ok(Plan {
            nodes,
            streams,
            template,
            template_index: index,
            segment,
            extend,
            prefixes,
            span: total_span,
        })
    }
    pub fn can_append(
        &self,
        evidence: &impl InvocationEvidence,
        span: u32,
    ) -> Result<(), DictionaryRefusal> {
        self.plan(evidence, span).map(|_| ())
    }
    /// Stable evidence and an exclusive archive borrow make publication atomic.
    /// All refusal checks precede the first mutation. No allocation or external
    /// execution callback runs; evidence queries must satisfy the immutable,
    /// finite InvocationEvidence contract throughout the call.
    pub fn append(
        &mut self,
        evidence: &impl InvocationEvidence,
        span: u32,
    ) -> Result<(), DictionaryRefusal> {
        let plan = self.plan(evidence, span)?;
        if plan.template_index == self.template_len {
            for event in evidence.events() {
                self.events[self.event_len] = normalized(
                    evidence,
                    event,
                    &plan.nodes,
                    &plan.streams,
                    &plan.segment.requests,
                    &plan.segment.remotes,
                );
                self.event_len += 1;
            }
            self.templates[self.template_len] = Some(plan.template);
            self.template_len += 1;
        }
        let at = if plan.extend {
            self.tape_len - 1
        } else {
            let at = self.tape_len;
            self.tape_len += 1;
            at
        };
        self.tape[at] = Some(plan.segment);
        self.nodes = plan.nodes;
        self.streams = plan.streams;
        self.prefixes = plan.prefixes;
        self.span = plan.span;
        Ok(())
    }
    pub fn prefix(&self, index: u32) -> Option<Prefix<'_, E, T, G, N, S>> {
        let mut at = index;
        for segment in self.tape.iter().take(self.tape_len).flatten() {
            if at < segment.copies {
                let template = self.templates[usize::from(segment.template.get() - 1)]?;
                return Some(Prefix {
                    archive: self,
                    segment: *segment,
                    copy: at,
                    template,
                    at: 0,
                });
            }
            at -= segment.copies;
        }
        None
    }
    #[cfg(feature = "alloc")]
    unsafe fn initialize_at(output: *mut Self) {
        // SAFETY: caller owns complete exact-layout storage. Each array slot is
        // written once; scalar counters finish initialization before publication.
        // Per-slot temporaries remain; chosen N/S and target stack need qualification.
        unsafe {
            let p = core::ptr::addr_of_mut!((*output).events).cast::<Option<ArchivedSign>>();
            for i in 0..E {
                p.add(i).write(None)
            }
            let p = core::ptr::addr_of_mut!((*output).templates).cast::<Option<Template>>();
            for i in 0..T {
                p.add(i).write(None)
            }
            let p = core::ptr::addr_of_mut!((*output).tape).cast::<Option<Segment<N, S>>>();
            for i in 0..G {
                p.add(i).write(None)
            }
            let p = core::ptr::addr_of_mut!((*output).nodes).cast::<Option<NodeId>>();
            for i in 0..N {
                p.add(i).write(None)
            }
            let p = core::ptr::addr_of_mut!((*output).streams).cast::<Option<Stream>>();
            for i in 0..S {
                p.add(i).write(None)
            }
            core::ptr::addr_of_mut!((*output).event_len).write(0);
            core::ptr::addr_of_mut!((*output).template_len).write(0);
            core::ptr::addr_of_mut!((*output).tape_len).write(0);
            core::ptr::addr_of_mut!((*output).prefixes).write(0);
            core::ptr::addr_of_mut!((*output).span).write(0);
        }
    }
}
fn normalized<const N: usize, const S: usize>(
    evidence: &impl InvocationEvidence,
    mut event: KernelEvent,
    nodes: &[Option<NodeId>; N],
    streams: &[Option<Stream>; S],
    requests: &[u32; N],
    remotes: &[u64; S],
) -> Option<ArchivedSign> {
    if let Some(request) = event.request {
        let i = nodes.iter().position(|n| *n == Some(event.node))?;
        event.request = Some(RequestId(request.0.checked_sub(requests[i])?))
    }
    let remote = match evidence.remote_identity(event.sequence) {
        None => None,
        Some(mut r) => {
            let i = streams.iter().position(|s| *s == Some(Stream::from(r)))?;
            r.sequence = r.sequence.checked_sub(remotes[i])?;
            Some(r)
        }
    };
    Some(ArchivedSign { event, remote })
}
pub struct Prefix<
    'a,
    const E: usize,
    const T: usize,
    const G: usize,
    const N: usize,
    const S: usize,
> {
    archive: &'a LivePrefixArchive<E, T, G, N, S>,
    segment: Segment<N, S>,
    copy: u32,
    template: Template,
    at: usize,
}
impl<const E: usize, const T: usize, const G: usize, const N: usize, const S: usize>
    Prefix<'_, E, T, G, N, S>
{
    pub fn span(&self) -> u32 {
        self.template.span
    }
    pub fn gap(&self) -> Option<SignRetentionGap> {
        self.template.gap
    }
}
impl<const E: usize, const T: usize, const G: usize, const N: usize, const S: usize> Iterator
    for Prefix<'_, E, T, G, N, S>
{
    type Item = ArchivedSign;
    fn next(&mut self) -> Option<Self::Item> {
        if self.at >= usize::from(self.template.len.get()) {
            return None;
        }
        let mut sign = self.archive.events[usize::from(self.template.start) + self.at]
            .expect("initialized row");
        self.at += 1;
        if let Some(request) = sign.event.request {
            let i = self
                .archive
                .nodes
                .iter()
                .position(|n| *n == Some(sign.event.node))
                .expect("retained node");
            let base = self.segment.requests[i]
                .checked_add(
                    self.segment.request_steps[i]
                        .checked_mul(self.copy)
                        .expect("checked affine step"),
                )
                .expect("checked affine base");
            sign.event.request = Some(RequestId(
                base.checked_add(request.0)
                    .expect("checked original request"),
            ))
        }
        if let Some(mut remote) = sign.remote {
            let i = self
                .archive
                .streams
                .iter()
                .position(|s| *s == Some(Stream::from(remote)))
                .expect("retained stream");
            let base = self.segment.remotes[i]
                .checked_add(
                    self.segment.remote_steps[i]
                        .checked_mul(u64::from(self.copy))
                        .expect("checked affine step"),
                )
                .expect("checked affine base");
            remote.sequence = base
                .checked_add(remote.sequence)
                .expect("checked original remote");
            sign.remote = Some(remote)
        }
        Some(sign)
    }
}
#[cfg(feature = "alloc")]
mod allocated;
#[cfg(feature = "alloc")]
pub use allocated::DictionaryLiveSignLog;
#[cfg(test)]
mod tests;
