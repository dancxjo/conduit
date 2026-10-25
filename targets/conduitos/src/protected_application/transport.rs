//! Exclusive Application-profile lease and checked bounded blob exchange.
use super::operation_budget::OperationBudget;
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
    wire::{self, Envelope, GraphPhase, GraphProgress, Request, Response},
};
pub(super) struct Service {
    domain: Option<ServiceDomain>,
    instance: u64,
    sequence: u64,
    operation: Option<OperationBudget>,
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
            operation: None,
        })
    }
    pub fn quarantine(&mut self) {
        self.operation = None;
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
        let remaining = self
            .operation
            .as_mut()
            .map(|operation| operation.before(envelope, request))
            .transpose()?;
        let domain = self.domain.as_mut().ok_or(Refusal::Order)?;
        let exchange = domain.exchange().map_err(|_| Refusal::Application)?;
        exchange.fill(0);
        wire::encode_request(envelope, request, exchange)?;
        let diagnostic = matches!(request, Request::Preflight | Request::Commit);
        if diagnostic {
            record_entry_cost("before", envelope.sequence, request, domain.cost());
        }
        let entered = if let Some(remaining) = remaining {
            let receipt = domain
                .enter_application_remaining(remaining)
                .map_err(|_| Refusal::Application)?;
            let observed = Envelope {
                instance: receipt.instance(),
                sequence: receipt.sequence(),
            };
            let result = receipt.result();
            record_operation_cost(envelope.sequence, request, &receipt);
            if self
                .operation
                .as_mut()
                .ok_or(Refusal::Order)?
                .observe(
                    observed,
                    receipt.admitted_rtc(),
                    receipt.consumed_rtc(),
                    result == DomainReturn::Yielded,
                )
                .is_err()
            {
                record_refusal(
                    "entry-budget",
                    envelope.sequence,
                    Some(request),
                    result,
                    Some(domain.cost()),
                );
                return Err(Refusal::Application);
            }
            Ok(result)
        } else {
            domain.enter(1)
        };
        if diagnostic {
            record_entry_cost("after", envelope.sequence, request, domain.cost());
        }
        match entered {
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
        if !matches!(response, Response::Refused(_)) {
            if let Some(operation) = self.operation.as_mut() {
                operation.respond(response)?;
            }
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
            | (Request::Commit, Response::Blob { .. } | Response::Ready)
            | (
                Request::GraphSerialize { .. } | Request::GraphValidate { .. },
                Response::Ready | Response::Progress { .. },
            )
            | (Request::GraphPack { .. }, Response::Ready | Response::Blob { .. })
            | (Request::Read { .. }, Response::Chunk { .. }) => Ok(response),
            _ => Err(Refusal::Malformed),
        }
    }
    pub fn command(&mut self, command: &Command) -> Result<Reply, Refusal> {
        let result = self.exchange_command(command).and_then(|reply| {
            if self
                .operation
                .as_ref()
                .is_some_and(|operation| !operation.complete())
            {
                return Err(Refusal::Order);
            }
            self.operation = None;
            Ok(reply)
        });
        if let Err(error) = &result {
            record_command_refusal(command, *error);
            self.quarantine();
        }
        result
    }
    fn prepare_graph_reply(&mut self) -> Result<Response, Refusal> {
        for (phase, begin, complete) in [
            (
                GraphPhase::Serialize,
                b"CONDUIT_APPLICATION_GRAPH_REPLY serialize begin\n".as_slice(),
                b"CONDUIT_APPLICATION_GRAPH_REPLY serialize complete\n".as_slice(),
            ),
            (
                GraphPhase::Validate,
                b"CONDUIT_APPLICATION_GRAPH_REPLY validate begin\n".as_slice(),
                b"CONDUIT_APPLICATION_GRAPH_REPLY validate complete\n".as_slice(),
            ),
        ] {
            write_observation(begin);
            let limit = match phase {
                GraphPhase::Serialize => data::GRAPH_SERIALIZE_TURNS,
                GraphPhase::Validate => data::GRAPH_VALIDATE_TURNS,
            };
            let mut progress = GraphProgress::new(phase, limit)?;
            loop {
                let response = self.request(progress.request()?)?;
                if progress.accept(response)? {
                    break;
                }
            }
            write_observation(complete);
        }
        write_observation(b"CONDUIT_APPLICATION_GRAPH_REPLY pack begin\n");
        // Every Ready consumes exactly GRAPH_PACK_BYTES input bytes. The last
        // turn must return Blob, within the unchanged decoded storage ceiling.
        for offset in (0..data::MAX_DECODED_JSON_BYTES).step_by(data::GRAPH_PACK_BYTES) {
            match self.request(Request::GraphPack {
                offset: offset as u32,
            })? {
                response @ Response::Blob { .. } => {
                    write_observation(b"CONDUIT_APPLICATION_GRAPH_REPLY pack complete\n");
                    return Ok(response);
                }
                Response::Ready => {}
                _ => return Err(Refusal::Malformed),
            }
        }
        Err(Refusal::Bounds)
    }
    fn exchange_command(&mut self, command: &Command) -> Result<Reply, Refusal> {
        let input = data::encode_command(command)?;
        record_target_input(command, &input);
        if self.operation.is_some() {
            return Err(Refusal::Order);
        }
        if matches!(
            command,
            Command::PrepareTarget { .. } | Command::PatchbayPublish { .. }
        ) {
            use sha2::{Digest, Sha256};
            let operation = OperationBudget::new(
                Envelope {
                    instance: self.instance,
                    sequence: self.sequence,
                },
                input.len(),
                Sha256::digest(&input).into(),
            )?;
            record_operation_begin(command, &operation);
            self.operation = Some(operation);
        }
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
        let projection = matches!(command, Command::PrepareGraphProjection { .. });
        if projection {
            write_observation(b"CONDUIT_APPLICATION_GRAPH_PROJECTION preflight begin\n");
        }
        if matches!(
            command,
            Command::PrepareTarget { .. }
                | Command::PrepareGraphProjection { .. }
                | Command::PatchbayPublish { .. }
        ) {
            // Full Face publications, like graphs, need decompression/framing
            // separated from typed decode and semantic validation.
            // Preflight freezes the same bytes consumed by the following Commit.
            self.request(Request::Preflight)?;
        }
        if projection {
            write_observation(b"CONDUIT_APPLICATION_GRAPH_PROJECTION preflight complete\n");
            write_observation(b"CONDUIT_APPLICATION_GRAPH_PROJECTION prepare begin\n");
        }
        let committed = self.request(Request::Commit)?;
        if projection {
            if !matches!(committed, Response::Blob { .. }) {
                return Err(Refusal::Malformed);
            }
            write_observation(b"CONDUIT_APPLICATION_GRAPH_PROJECTION prepare complete\n");
        }
        let response = if matches!(command, Command::Graph { .. }) {
            if committed != Response::Ready {
                return Err(Refusal::Malformed);
            }
            self.prepare_graph_reply()?
        } else {
            committed
        };
        let Response::Blob { bytes: total } = response else {
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
        Command::PatchbayCanonical { slot } => ("patchbay-canonical", Some(*slot), None),
        Command::PatchbayFragment { slot, .. } => ("patchbay-fragment", Some(*slot), None),
        Command::PatchbayPublish { slot, .. } => ("patchbay-publish", Some(*slot), None),
        Command::PatchbayConsume { slot, .. } => ("patchbay-consume", Some(*slot), None),
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
        write_observation(line.as_bytes());
    }
}

/// Root-only diagnostics: no new guest grant, entry, payload copy or authority.
fn record_target_input(command: &Command, input: &[u8]) {
    use core::fmt::Write;
    use sha2::{Digest, Sha256};
    let Command::PrepareTarget {
        slot,
        target,
        value,
    } = command
    else {
        return;
    };
    let compressed = input.starts_with(b"CAPZ\x01\0\0\0");
    let decoded_bytes = if compressed {
        input
            .get(8..12)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
            .map(|length| length as usize)
    } else {
        Some(input.len())
    };
    let digest = Sha256::digest(input);
    let mut line = crate::sign_format::FixedText::new();
    let result = (|| -> core::fmt::Result {
        write!(
            line,
            "CONDUIT_APPLICATION_TARGET_INPUT operation=prepare-target slot={slot} target={target} wire_bytes={} capz={compressed} decoded_bytes={decoded_bytes:?} sha256=",
            input.len()
        )?;
        for byte in digest {
            write!(line, "{byte:02x}")?;
        }
        writeln!(
            line,
            " source={} checked={} expanded={} body={:?} plan={:?}",
            value.graph.source_document_id.as_str(),
            value.graph.checked_plot_id.as_str(),
            value.graph.expanded_plot_id.as_str(),
            value.body,
            value.plan
        )
    })();
    if result.is_ok() {
        write_observation(line.as_bytes());
    }
}

fn record_entry_cost(stage: &str, sequence: u64, request: Request, cost: DomainCost) {
    use core::fmt::Write;
    let mut line = crate::sign_format::FixedText::new();
    let counters = (cost.entries, cost.interrupt_entries, cost.preemptions);
    if writeln!(line, "CONDUIT_APPLICATION_ENTRY_COST stage={stage} sequence={sequence} request={request:?} cumulative_entries_interrupts_preemptions={counters:?}").is_ok() {
        write_observation(line.as_bytes());
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
        write_observation(line.as_bytes());
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.quarantine();
    }
}

/// Guest serial evidence uses captured output when exercising CPU tests.
fn write_observation(bytes: &[u8]) {
    #[cfg(not(test))]
    crate::arch::early_write(bytes);
    #[cfg(test)]
    std::print!("{}", alloc::string::String::from_utf8_lossy(bytes));
}

/// Closed Root observations, separate RTC and Source counters, no extra entry.
fn record_operation_cost(
    sequence: u64,
    request: Request,
    receipt: &crate::arch::ApplicationEntryReceipt,
) {
    use core::fmt::Write;
    let mut line = crate::sign_format::FixedText::new();
    if writeln!(line, "CONDUIT_APPLICATION_OPERATION_COST sequence={sequence} request={request:?} instance={} cap={} rtc={} source={} result={:?}",
        receipt.instance(), receipt.admitted_rtc(), receipt.consumed_rtc(),
        receipt.source_interrupts(), receipt.result()).is_ok() {
        write_observation(line.as_bytes());
    }
}
fn record_operation_begin(command: &Command, operation: &OperationBudget) {
    use core::fmt::Write;
    let (name, slot, target) = match command {
        Command::PrepareTarget { slot, target, .. } => ("prepare-target", *slot, Some(*target)),
        Command::PatchbayPublish { slot, .. } => ("patchbay-publish", *slot, None),
        _ => return,
    };
    let mut line = crate::sign_format::FixedText::new();
    let result = (|| -> core::fmt::Result {
        write!(
            line,
            "CONDUIT_APPLICATION_OPERATION_BEGIN operation={name} slot={slot} target={target:?} rtc=64 sha256="
        )?;
        for byte in operation.digest() {
            write!(line, "{byte:02x}")?;
        }
        writeln!(line)
    })();
    if result.is_ok() {
        write_observation(line.as_bytes());
    }
}
