//! Exclusive Application-profile lease and checked bounded blob exchange.
use crate::{
    arch::ServiceDomain,
    protected_region::{DomainBackend, DomainCost, DomainReturn},
    service_memory::{ServiceImage, ServiceProfile},
};
use alloc::vec::Vec;
use conduitos_application_core::{
    MAX_BLOB,
    data::{self, Command, Refusal, Reply},
    memory::TRANSFER_BYTES,
    wire::{self, Envelope, Request, Response},
};
pub(super) struct Service {
    domain: Option<ServiceDomain>,
    instance: u64,
    sequence: u64,
}
impl Service {
    pub fn install() -> Result<Self, Refusal> {
        #[cfg(conduitos_application_image)]
        let bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/application.elf"));
        #[cfg(not(conduitos_application_image))]
        let bytes: &[u8] = &[];
        let image =
            ServiceImage::for_profile(bytes, ServiceProfile::Application).map_err(|error| {
                record_refusal("image", 0, None, error, None);
                Refusal::Application
            })?;
        let domain = ServiceDomain::install(&image).map_err(|error| {
            record_refusal("install", 0, None, error, None);
            Refusal::Application
        })?;
        Ok(Self {
            instance: domain.instance(),
            domain: Some(domain),
            sequence: 1,
        })
    }
    pub fn quarantine(&mut self) {
        if let Some(mut domain) = self.domain.take() {
            domain.quarantine();
            drop(domain);
        }
    }
    fn request(&mut self, request: Request) -> Result<Response, Refusal> {
        let envelope = Envelope {
            instance: self.instance,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.checked_add(1).ok_or(Refusal::Capacity)?;
        let domain = self.domain.as_mut().ok_or(Refusal::Order)?;
        let exchange = domain.exchange().map_err(|_| Refusal::Application)?;
        exchange.fill(0);
        wire::encode_request(envelope, request, exchange)?;
        match domain.enter(1) {
            Ok(DomainReturn::Yielded) => {}
            result => {
                record_refusal(
                    "entry",
                    envelope.sequence,
                    Some(request),
                    result,
                    Some(domain.cost()),
                );
                return Err(Refusal::Application);
            }
        }
        let exchange = domain.exchange().map_err(|_| Refusal::Application)?;
        let (returned, response) =
            wire::decode_response(exchange.get(..wire::BYTES).ok_or(Refusal::Malformed)?)?;
        if returned != envelope {
            return Err(Refusal::Malformed);
        }
        match (request, response) {
            (_, Response::Refused(error)) => {
                record_refusal(
                    "reply",
                    envelope.sequence,
                    Some(request),
                    error,
                    Some(domain.cost()),
                );
                Err(error)
            }
            (
                Request::Begin { .. } | Request::Write { .. } | Request::Preflight,
                Response::Ready,
            )
            | (Request::Commit, Response::Blob { .. })
            | (Request::Read { .. }, Response::Chunk { .. }) => Ok(response),
            _ => Err(Refusal::Malformed),
        }
    }
    pub fn command(&mut self, command: &Command) -> Result<Reply, Refusal> {
        let result = self.exchange_command(command);
        if let Err(error) = &result {
            record_command_refusal(command, *error);
            self.quarantine();
        }
        result
    }
    fn exchange_command(&mut self, command: &Command) -> Result<Reply, Refusal> {
        let input = data::encode_command(command)?;
        self.request(Request::Begin {
            bytes: input.len() as u32,
        })?;
        let mut offset = 0;
        for chunk in input.chunks(TRANSFER_BYTES) {
            let window = self
                .domain
                .as_mut()
                .ok_or(Refusal::Order)?
                .transfer_window()
                .map_err(|_| Refusal::Application)?;
            window.fill(0);
            window
                .get_mut(..chunk.len())
                .ok_or(Refusal::Bounds)?
                .copy_from_slice(chunk);
            self.request(Request::Write {
                offset,
                bytes: chunk.len() as u32,
            })?;
            offset += chunk.len() as u32;
        }
        if matches!(command, Command::PrepareTarget { .. }) {
            self.request(Request::Preflight)?;
        }
        let Response::Blob { bytes: total } = self.request(Request::Commit)? else {
            return Err(Refusal::Malformed);
        };
        if total == 0 || total as usize > MAX_BLOB {
            return Err(Refusal::Bounds);
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(total as usize)
            .map_err(|_| Refusal::Capacity)?;
        while output.len() < total as usize {
            let offset = output.len() as u32;
            let maximum = (total - offset).min(TRANSFER_BYTES as u32);
            let Response::Chunk {
                offset: at,
                bytes,
                total: reported,
            } = self.request(Request::Read { offset, maximum })?
            else {
                return Err(Refusal::Malformed);
            };
            if at != offset || reported != total || bytes == 0 || bytes > maximum {
                return Err(Refusal::Malformed);
            }
            let window = self
                .domain
                .as_ref()
                .ok_or(Refusal::Order)?
                .output()
                .map_err(|_| Refusal::Application)?;
            output.extend_from_slice(window.get(..bytes as usize).ok_or(Refusal::Malformed)?);
        }
        data::decode_reply(&output)
    }
}

fn record_command_refusal(command: &Command, error: Refusal) {
    use core::fmt::Write;
    let (operation, slot, target) = match command {
        Command::PrepareInit { .. } => ("prepare-init", None, None),
        Command::PrepareTarget { slot, target, .. } => {
            ("prepare-target", Some(*slot), Some(*target))
        }
        Command::FinishTarget { slot, target } => ("finish-target", Some(*slot), Some(*target)),
        Command::PrepareTutorial { slot, .. } => ("prepare-tutorial", Some(*slot), None),
        Command::PrepareTour { slot } => ("prepare-tour", Some(*slot), None),
        Command::FinishInit => ("finish-init", None, None),
        Command::PrepareGraphProjection { .. } => ("prepare-graph-projection", None, None),
        Command::FinishGraphProjection => ("finish-graph-projection", None, None),
        Command::ProjectTutorial { .. } => ("project-tutorial", None, None),
        Command::Apply { slot, .. } => ("apply", Some(*slot), None),
        Command::Refresh { .. } => ("refresh", None, None),
        Command::Select { slot, target } => ("select", Some(*slot), Some(*target)),
        Command::Topology { .. } => ("topology", None, None),
        Command::CompleteTour { slot, .. } => ("complete-tour", Some(*slot), None),
        Command::Graph { slot } => ("graph", Some(*slot), None),
    };
    let mut line = crate::sign_format::FixedText::new();
    if writeln!(line,
        "CONDUIT_APPLICATION_COMMAND_REFUSAL operation={operation} slot={slot:?} target={target:?} refusal={error:?}"
    ).is_ok() {
        crate::arch::early_write(line.as_bytes());
    }
}

/// Capture the already observed refusal before quarantine discards the domain.
/// Closed protocol fields only; no payload, allocation, retry or extra entry.
fn record_refusal(
    stage: &str,
    sequence: u64,
    request: Option<Request>,
    detail: impl core::fmt::Debug,
    cost: Option<DomainCost>,
) {
    use core::fmt::Write;
    let mut line = crate::sign_format::FixedText::new();
    let counters = cost.map(|cost| (cost.entries, cost.interrupt_entries, cost.preemptions));
    if writeln!(line,
        "CONDUIT_APPLICATION_REFUSAL stage={stage} sequence={sequence} request={request:?} detail={detail:?} entries_interrupts_preemptions={counters:?}"
    ).is_ok() {
        crate::arch::early_write(line.as_bytes());
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.quarantine();
    }
}
