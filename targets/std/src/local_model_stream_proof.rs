//! Repository-only bounded incremental generation through ordinary Plan/Play.
use crate::hosted_local_model::{HostedLocalModelAdapter, OllamaLocalModelAdapter};
use crate::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost,
    StdHostComposition, StdHostConfig, ThreadTimer,
};
use conduit_core::{
    BaseImplementationId, ConnectionTrack, ObservationKind, PortDirection, TerminalDisposition,
};
use conduit_plot::{ProfileCatalog, StartupCatalog};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};

const MAXIMUM_TEXT_BYTES: usize = 256;
const PROMPT: &str = "Reply in one short sentence: hello from Conduit.";

#[derive(Debug, Serialize)]
pub struct StreamProofReceipt {
    pub proof_class: &'static str,
    pub host_id: String,
    pub boot_id: String,
    pub model_content_identity: String,
    pub implementation_id: String,
    pub plan_id: String,
    pub play_id: String,
    pub terminal_sign_id: String,
    pub chunk_count: u64,
    pub generated_text: String,
    pub text_sha256: String,
    pub first_delivery_us: u64,
    pub last_delivery_us: u64,
    pub elapsed_us: u64,
    pub timing_scope: &'static str,
    pub value_capacity_before: (usize, usize),
    pub value_capacity_after: (usize, usize),
}

struct Receiver {
    delivered: Arc<AtomicU64>,
    text: [u8; MAXIMUM_TEXT_BYTES],
    length: usize,
    started: Instant,
    first: Option<u64>,
    last: u64,
}
impl ExternalForeOutputAdapter for Receiver {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        if output.front_port_id.as_str() != "chunks"
            || output.track != ConnectionTrack::Payload
            || output.value_kind.as_str() != conduit_ai::GENERATED_TEXT_CHUNK_VALUE_KIND
        {
            return Err("stream proof received an unexpected external Fore".into());
        }
        let chunk = conduit_ai::decode_generated_text_chunk(&output.bytes)
            .map_err(|e| format!("stream proof chunk: {e:?}"))?;
        let expected = self.delivered.load(Ordering::Relaxed);
        if output.sequence != expected || *chunk.sequence() != expected {
            return Err("stream proof chunk sequence is not contiguous".into());
        }
        let text = chunk.text().as_bytes();
        let end = self
            .length
            .checked_add(text.len())
            .ok_or("stream proof text overflow")?;
        if end > self.text.len() {
            return Err("stream proof exceeds admitted text bound".into());
        }
        self.text[self.length..end].copy_from_slice(text);
        self.length = end;
        let now = self.started.elapsed().as_micros() as u64;
        self.first.get_or_insert(now);
        self.last = now;
        self.delivered.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

pub fn run(adapter: OllamaLocalModelAdapter) -> Result<StreamProofReceipt, String> {
    run_with_adapter(
        Box::new(adapter),
        Arc::new(AtomicU64::new(0)),
        "ollama-stream-kernel-plan-play",
    )
}

fn run_with_adapter(
    adapter: Box<dyn HostedLocalModelAdapter>,
    delivered: Arc<AtomicU64>,
    proof_class: &'static str,
) -> Result<StreamProofReceipt, String> {
    let model_content_identity = adapter.offer().identity.model_content_identity.clone();
    let fresh = StdHost::new();
    let ad = fresh.advertisement();
    let mut host = StdHost::new_with_local_model(
        StdHostConfig {
            host_id: ad.host_id.clone(),
            boot_id: ad.boot_id.clone(),
            offer_generation: ad.offer_generation,
        },
        StdHostComposition::reference(),
        adapter,
    )?;
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)
        .map_err(|e| e.to_string())?;
    startup
        .insert_value_kind_alias(
            "ProofRequest",
            conduit_core::kind_id(conduit_ai::GENERATION_REQUEST_VALUE_KIND),
        )
        .map_err(|e| e.to_string())?;
    startup
        .insert_value_kind_alias(
            "ProofChunk",
            conduit_core::kind_id(conduit_ai::GENERATED_TEXT_CHUNK_VALUE_KIND),
        )
        .map_err(|e| e.to_string())?;
    let source = format!("plot stream-proof (\n >> request: ProofRequest\n chunks: ProofChunk...| >>\n) {{\n model: llm/generate-stream(maximum-input-bytes = 4096, maximum-context-items = 1, maximum-output-bytes = {MAXIMUM_TEXT_BYTES}, maximum-work-units = 4096, maximum-history-items = 0)\n request >> model.request\n model.result >> chunks\n}}.\n");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .map_err(|e| format!("stream proof check: {} {}", e.code, e.message))?;
    let authored =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "stream-proof", &profiles)
            .map_err(|e| format!("stream proof expansion: {} {}", e.code, e.message))?;
    let hosts = [host.advertisement().clone()];
    let choices = conduit_planner::default_expanded_placements(&authored.expanded, &hosts)
        .map_err(|e| e.to_string())?;
    let capacity = conduit_ai::MAXIMUM_GENERATED_TEXT_CHUNK_VALUE_BYTES as u32;
    let boundaries = [
        (PortDirection::Input, "request"),
        (PortDirection::Output, "chunks"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: capacity,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &choices,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(|e| e.to_string())?;
    if !conduit_core::verify_plan(&plan) {
        return Err("stream proof Plan verification refused".into());
    }
    let [fragment] = plan.fragments.as_slice() else {
        return Err("stream proof requires one exact local fragment".into());
    };
    let placement = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == conduit_ai::LLM_STREAM_GENERATE_KIND)
        .ok_or("stream proof has no exact streaming placement")?;
    let implementation_id = placement.implementation_id.as_str().to_owned();
    let mut receiver = Receiver {
        delivered,
        text: [0; MAXIMUM_TEXT_BYTES],
        length: 0,
        started: Instant::now(),
        first: None,
        last: 0,
    };
    let report = host.run_external_plot_to(
        fragment.clone(),
        &[ExternalForeInput {
            front_port_id: "request".into(),
            track: ConnectionTrack::Payload,
            bytes: PROMPT.as_bytes().to_vec(),
        }],
        &mut receiver,
        &mut Vec::new(),
        &mut ThreadTimer,
    )?;
    let elapsed_us = receiver.started.elapsed().as_micros() as u64;
    let terminal = report
        .observations
        .iter()
        .find(|event| {
            matches!(
                event.kind,
                ObservationKind::PlanTerminal {
                    disposition: TerminalDisposition::Completed
                }
            )
        })
        .ok_or("stream proof did not complete")?;
    let kernel = report
        .kernel
        .ok_or("stream proof did not use the execution kernel")?;
    if kernel.active_play.plan_id != plan.plan_id
        || kernel.value_allocation_capacity_before != kernel.value_allocation_capacity_after
    {
        return Err("stream proof identity or admitted value capacity changed".into());
    }
    let text =
        String::from_utf8(receiver.text[..receiver.length].to_vec()).map_err(|e| e.to_string())?;
    Ok(StreamProofReceipt {
        proof_class,
        host_id: hosts[0].host_id.as_str().into(),
        boot_id: hosts[0].boot_id.as_str().into(),
        model_content_identity,
        implementation_id,
        plan_id: plan.plan_id.as_str().into(),
        play_id: kernel.active_play.active_play_id.as_str().into(),
        terminal_sign_id: terminal.sign_id.as_str().into(),
        chunk_count: receiver.delivered.load(Ordering::Relaxed),
        text_sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
        generated_text: text,
        first_delivery_us: receiver.first.ok_or("stream proof produced no chunks")?,
        last_delivery_us: receiver.last,
        elapsed_us,
        timing_scope:
            "Plan/Play including preparation; excludes provider discovery and initialization",
        value_capacity_before: kernel.value_allocation_capacity_before,
        value_capacity_after: kernel.value_allocation_capacity_after,
    })
}

#[cfg(test)]
#[path = "local_model_stream_proof_tests.rs"]
mod tests;
