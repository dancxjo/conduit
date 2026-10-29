use super::*;
use crate::hosted_local_model::{LocalModelAdapterTerminal, LocalModelStreamStep};
use conduit_ai::*;
use std::collections::VecDeque;

struct FakeIncremental {
    offer: LocalModelOffer,
    chunks: VecDeque<GeneratedTextChunk>,
    count: u64,
    bytes: u64,
    delivered: Arc<AtomicU64>,
    lost: bool,
    cancelled: bool,
}
impl HostedLocalModelAdapter for FakeIncremental {
    fn offer(&self) -> &LocalModelOffer {
        &self.offer
    }
    fn current_pool_health(&self) -> conduit_core::PoolRealizationHealth {
        conduit_core::PoolRealizationHealth::Ready
    }
    fn execute(
        &mut self,
        _: &conduit_core::PlannedGear,
        _: &[u8],
        _: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        panic!("streaming proof must use incremental Host Calls")
    }
    fn execute_stream_step(
        &mut self,
        _: &conduit_core::PlannedGear,
        input: &[u8],
    ) -> LocalModelStreamStep {
        assert_eq!(input, PROMPT.as_bytes());
        if let Some(chunk) = self.chunks.pop_front() {
            return LocalModelStreamStep::Chunk(chunk);
        }
        if !self.lost && !self.cancelled && self.count > 1 {
            assert!(
                self.delivered.load(Ordering::Relaxed) > 0,
                "delivery must begin before provider terminal"
            );
        }
        LocalModelStreamStep::Terminal(
            GeneratedTextFlowEvidence::new(
                self.count,
                self.bytes,
                if self.cancelled {
                    GeneratedTextFlowTerminal::Cancelled
                } else if self.lost {
                    GeneratedTextFlowTerminal::ProviderLost
                } else {
                    GeneratedTextFlowTerminal::Completed
                },
                false,
            )
            .unwrap(),
        )
    }
}
fn provider(chunks: &[(u64, String)], delivered: Arc<AtomicU64>) -> FakeIncremental {
    FakeIncremental {
        offer: LocalModelOffer {
            identity: LocalModelIdentity {
                runtime_name: "named-fake-incremental".into(),
                runtime_version: "1".into(),
                runtime_build_identity: "fixture/build1".into(),
                model_name: "fixture-model".into(),
                model_content_identity: "fixture/content1".into(),
                architecture: "fixture".into(),
                parameter_profile: "tiny".into(),
                quantization: "exact".into(),
            },
            limits: LocalModelLimits {
                work: LlmWorkBounds::new(4096, 1, 4096, 4096, 0).unwrap(),
                model_bytes: 1,
                admitted_memory_mib: 1,
                compute: LocalModelComputeNeed {
                    minimum_lanes: 1,
                    preferred_lanes: 2,
                    maximum_lanes: 4,
                    minimum_service_guarantee: conduit_core::ComputeServiceGuarantee::Shared,
                },
                maximum_in_flight: 1,
                maximum_queue_items: 4,
                maximum_queue_bytes: 16384,
                cancellation_supported: true,
                cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
            },
            supported_profiles: vec![LocalModelKindProfile::StreamGenerate],
            initialized: true,
            lifecycle: LocalModelLifecycleState::Ready,
            determinism: LlmDeterminismProfile::ProviderNondeterministic,
        },
        chunks: chunks
            .iter()
            .map(|(sequence, text)| GeneratedTextChunk::new(*sequence, text.clone()).unwrap())
            .collect(),
        count: chunks.len() as u64,
        bytes: chunks.iter().map(|(_, text)| text.len() as u64).sum(),
        delivered,
        lost: false,
        cancelled: false,
    }
}
#[test]
fn incremental_chunks_cross_flow_fore_before_provider_terminal() {
    let count = Arc::new(AtomicU64::new(0));
    let fake = provider(&[(0, "Hello ".into()), (1, "body.".into())], count.clone());
    let receipt = run_with_adapter(Box::new(fake), count, "named-fake-incremental").unwrap();
    assert_eq!(receipt.generated_text, "Hello body.");
    assert_eq!(receipt.chunk_count, 2);
    assert_eq!(receipt.value_capacity_before, receipt.value_capacity_after);
    assert!(receipt.first_delivery_us <= receipt.last_delivery_us);
    assert!(!receipt.terminal_sign_id.is_empty());
}
#[test]
fn reordered_and_overflowing_chunks_do_not_produce_success_receipts() {
    for chunks in [
        vec![(1, "wrong first sequence".into())],
        vec![(0, "x".repeat(MAXIMUM_TEXT_BYTES + 1))],
    ] {
        let count = Arc::new(AtomicU64::new(0));
        let fake = provider(&chunks, count.clone());
        assert!(run_with_adapter(Box::new(fake), count, "named-fake-incremental").is_err());
    }
}
#[test]
fn provider_loss_and_absent_stream_offer_are_refused() {
    for lost in [false, true] {
        let count = Arc::new(AtomicU64::new(0));
        let mut fake = provider(&[], count.clone());
        if lost {
            fake.lost = true;
        } else {
            fake.offer.supported_profiles = vec![LocalModelKindProfile::Generate];
        }
        assert!(run_with_adapter(Box::new(fake), count, "named-fake-incremental").is_err());
    }
}

#[test]
fn loss_after_chunks_preserves_incremental_delivery_and_refuses_success() {
    let count = Arc::new(AtomicU64::new(0));
    let mut fake = provider(
        &[(0, "Partial ".into()), (1, "reply".into())],
        count.clone(),
    );
    fake.lost = true;
    let error =
        run_with_adapter(Box::new(fake), count.clone(), "named-fake-incremental").unwrap_err();
    assert!(error.contains("54"), "{error}");
    assert!(count.load(Ordering::Relaxed) > 0);
}
#[test]
fn cancelled_provider_stream_is_distinct_from_loss_or_completed_flow() {
    let count = Arc::new(AtomicU64::new(0));
    let mut fake = provider(&[], count.clone());
    fake.cancelled = true;
    let error = run_with_adapter(Box::new(fake), count, "named-fake-incremental").unwrap_err();
    assert!(error.contains("Cancelled"), "{error}");
}
