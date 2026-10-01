//! Bounded transactional semantics for a Body-lived State realization.
//!
//! This is deliberately separate from [`super::RetainedTypedState`]. That type
//! is consumed ownership passed between graceful Plays; it is not a crash
//! checkpoint. This module also makes no filesystem or hard-loss claim. A Host
//! can use this exact protocol only when its separately admitted implementation
//! can truthfully provide the required residence and synchronization.

use conduit_core::{KindId, StateId};
use sha2::{Digest, Sha256};

const PROTOCOL_VERSION: u8 = 1;
const RECOVERY_ABSENT: u8 = 0;
const RECOVERY_PRESENT: u8 = 1;
const RECEIPT_BYTES: usize = 1 + 8 + 32;
const RECOVERY_HEADER_BYTES: usize = 2 + 8 + 32;
pub const MAXIMUM_DURABLE_STATE_BYTES: u32 =
    conduit_std_offers::STATE_VALUE_DURABLE_STD_MAXIMUM_BYTES;

/// Exact semantic owner of one durable State journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableStateBinding {
    pub body: String,
    pub state: StateId,
    pub value_kind: KindId,
    pub maximum_value_bytes: u32,
}

impl DurableStateBinding {
    pub fn validate(&self) -> Result<(), DurableStateRefusal> {
        if self.body.is_empty()
            || self.state.as_str().is_empty()
            || self.value_kind.as_str().is_empty()
            || self.maximum_value_bytes == 0
            || self.maximum_value_bytes > MAXIMUM_DURABLE_STATE_BYTES
        {
            return Err(DurableStateRefusal::InvalidBinding);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Generation {
    number: u64,
    digest: [u8; 32],
    value: Vec<u8>,
}

/// Recovery truth remains more exact than a generic I/O error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryDisposition {
    Absent,
    Recovered { generation: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableStateRefusal {
    InvalidBinding,
    ValueTooLarge,
    GenerationGap,
    ConflictingDigest,
    Corrupt,
    Incompatible,
    Lost,
    StaleRecovery,
    InvalidReceipt,
}

/// Deterministic Host-side transactional model. `candidate` represents bytes
/// that may have been written but have not acquired committed-generation truth.
pub struct DurableStateHost {
    binding: DurableStateBinding,
    committed: Option<Generation>,
    candidate: Option<Generation>,
    lost_generation: Option<u64>,
}

impl DurableStateHost {
    pub fn new(binding: DurableStateBinding) -> Result<Self, DurableStateRefusal> {
        binding.validate()?;
        Ok(Self {
            binding,
            committed: None,
            candidate: None,
            lost_generation: None,
        })
    }

    /// Stage a bounded candidate. It is intentionally invisible to recovery
    /// until [`commit_candidate`](Self::commit_candidate) publishes it.
    pub fn stage_candidate(
        &mut self,
        generation: u64,
        value: &[u8],
    ) -> Result<(), DurableStateRefusal> {
        if value.len() > self.binding.maximum_value_bytes as usize {
            return Err(DurableStateRefusal::ValueTooLarge);
        }
        let expected = self
            .committed
            .as_ref()
            .map_or(1, |current| current.number.saturating_add(1));
        let digest = digest(value);
        if let Some(current) = self
            .committed
            .as_ref()
            .filter(|item| item.number == generation)
        {
            return if current.digest == digest {
                Ok(())
            } else {
                Err(DurableStateRefusal::ConflictingDigest)
            };
        }
        if generation != expected {
            return Err(DurableStateRefusal::GenerationGap);
        }
        if let Some(candidate) = &self.candidate {
            return if candidate.number == generation && candidate.digest == digest {
                Ok(())
            } else if candidate.number == generation {
                Err(DurableStateRefusal::ConflictingDigest)
            } else {
                Err(DurableStateRefusal::GenerationGap)
            };
        }
        self.candidate = Some(Generation {
            number: generation,
            digest,
            value: value.to_vec(),
        });
        Ok(())
    }

    pub fn commit_candidate(&mut self) -> Result<[u8; RECEIPT_BYTES], DurableStateRefusal> {
        let candidate = self
            .candidate
            .take()
            .ok_or(DurableStateRefusal::GenerationGap)?;
        let receipt = encode_receipt(candidate.number, candidate.digest);
        self.committed = Some(candidate);
        self.lost_generation = None;
        Ok(receipt)
    }

    /// Exact Host Call semantics: the same binding, generation and digest is
    /// idempotent; the same generation with different bytes is a conflict.
    pub fn commit(
        &mut self,
        binding: &DurableStateBinding,
        generation: u64,
        value: &[u8],
    ) -> Result<[u8; RECEIPT_BYTES], DurableStateRefusal> {
        if binding != &self.binding {
            return Err(DurableStateRefusal::Incompatible);
        }
        let value_digest = digest(value);
        if let Some(current) = self
            .committed
            .as_ref()
            .filter(|item| item.number == generation)
        {
            return if current.digest == value_digest {
                Ok(encode_receipt(generation, value_digest))
            } else {
                Err(DurableStateRefusal::ConflictingDigest)
            };
        }
        self.stage_candidate(generation, value)?;
        self.commit_candidate()
    }

    /// Test/support hook for representing a separately observed loss of a
    /// generation that was known to have committed. This is not `Absent`.
    pub fn mark_committed_lost(&mut self) {
        self.lost_generation = self.committed.take().map(|item| item.number);
    }

    pub fn recover(
        &self,
        binding: &DurableStateBinding,
    ) -> Result<(RecoveryDisposition, Vec<u8>), DurableStateRefusal> {
        if binding != &self.binding {
            return Err(DurableStateRefusal::Incompatible);
        }
        if self.lost_generation.is_some() {
            return Err(DurableStateRefusal::Lost);
        }
        let Some(current) = &self.committed else {
            return Ok((RecoveryDisposition::Absent, encode_absent()));
        };
        if digest(&current.value) != current.digest {
            return Err(DurableStateRefusal::Corrupt);
        }
        let mut response = Vec::with_capacity(RECOVERY_HEADER_BYTES + current.value.len());
        response.extend_from_slice(&[PROTOCOL_VERSION, RECOVERY_PRESENT]);
        response.extend_from_slice(&current.number.to_be_bytes());
        response.extend_from_slice(&current.digest);
        response.extend_from_slice(&current.value);
        Ok((
            RecoveryDisposition::Recovered {
                generation: current.number,
            },
            response,
        ))
    }

    #[cfg(test)]
    fn corrupt_committed_value(&mut self) {
        self.committed.as_mut().expect("committed generation").value[0] ^= 0xff;
    }
}

fn digest(value: &[u8]) -> [u8; 32] {
    Sha256::digest(value).into()
}

fn encode_receipt(generation: u64, digest: [u8; 32]) -> [u8; RECEIPT_BYTES] {
    let mut receipt = [0; RECEIPT_BYTES];
    receipt[0] = PROTOCOL_VERSION;
    receipt[1..9].copy_from_slice(&generation.to_be_bytes());
    receipt[9..].copy_from_slice(&digest);
    receipt
}

fn decode_receipt(bytes: &[u8]) -> Option<(u64, [u8; 32])> {
    if bytes.len() != RECEIPT_BYTES || bytes[0] != PROTOCOL_VERSION {
        return None;
    }
    Some((
        u64::from_be_bytes(bytes[1..9].try_into().ok()?),
        bytes[9..].try_into().ok()?,
    ))
}

fn encode_absent() -> Vec<u8> {
    vec![PROTOCOL_VERSION, RECOVERY_ABSENT]
}

mod back;
pub use back::DurableStateBack;
mod residence;
pub use residence::FileDurableStateResidence;
pub(crate) use residence::InstalledDurableStateHost;

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{
        scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
        BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
    };

    fn binding() -> DurableStateBinding {
        DurableStateBinding {
            body: "body/notebook".into(),
            state: StateId::from("note/current"),
            value_kind: KindId::from("Text"),
            maximum_value_bytes: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
        }
    }

    #[test]
    fn incomplete_candidate_is_ignored_and_commit_is_idempotent_but_not_ambiguous() {
        let mut host = DurableStateHost::new(binding()).unwrap();
        host.stage_candidate(1, b"A").unwrap();
        assert_eq!(
            host.recover(&binding()).unwrap().0,
            RecoveryDisposition::Absent
        );
        let receipt = host.commit_candidate().unwrap();
        assert_eq!(decode_receipt(&receipt), Some((1, digest(b"A"))));
        assert_eq!(host.commit(&binding(), 1, b"A").unwrap(), receipt);
        assert_eq!(
            host.commit(&binding(), 1, b"different"),
            Err(DurableStateRefusal::ConflictingDigest)
        );
        host.commit(&binding(), 2, b"B").unwrap();
        assert_eq!(
            host.recover(&binding()).unwrap().0,
            RecoveryDisposition::Recovered { generation: 2 }
        );
    }

    #[test]
    fn recovery_distinguishes_absent_corrupt_incompatible_and_lost() {
        let mut host = DurableStateHost::new(binding()).unwrap();
        assert_eq!(
            host.recover(&binding()).unwrap().0,
            RecoveryDisposition::Absent
        );
        let mut other = binding();
        other.body = "body/other".into();
        assert_eq!(host.recover(&other), Err(DurableStateRefusal::Incompatible));
        host.commit(&binding(), 1, b"A").unwrap();
        host.corrupt_committed_value();
        assert_eq!(host.recover(&binding()), Err(DurableStateRefusal::Corrupt));

        let mut lost = DurableStateHost::new(binding()).unwrap();
        lost.commit(&binding(), 1, b"A").unwrap();
        lost.mark_committed_lost();
        assert_eq!(lost.recover(&binding()), Err(DurableStateRefusal::Lost));
    }

    #[test]
    fn transition_waits_for_exact_commit_receipt_before_consuming_or_emitting() {
        let probe = ValueRef {
            slot: 9,
            generation: 1,
            byte_len: 1,
        };
        let initial = ValueRef {
            slot: 10,
            generation: 1,
            byte_len: 7,
        };
        let mut back = DurableStateBack::new(binding(), initial, probe).unwrap();
        let mut recover_request = StepIo::test_frame([None], [false], [Some(48)], None, 8);
        assert_eq!(
            back.step(
                &mut recover_request,
                &StepInputBytes::test_frame([None], None)
            ),
            StepOutcome::Progress
        );
        assert_eq!(
            recover_request.test_host_request().map(|item| item.1),
            Some(HostCallId(1))
        );

        let absent = encode_absent();
        let absent_ref = ValueRef {
            slot: 8,
            generation: 1,
            byte_len: absent.len() as u32,
        };
        let outcome = conduit_kernel::HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(absent_ref, absent.len() as u32).unwrap()),
            failure: None,
        };
        let mut recovered = StepIo::test_frame(
            [None],
            [false],
            [Some(48)],
            Some((RequestId(0), outcome)),
            8,
        );
        assert_eq!(
            back.step(
                &mut recovered,
                &StepInputBytes::test_frame([None], Some(&absent))
            ),
            StepOutcome::Progress
        );
        assert_eq!(recovered.test_output(PortId(0)), Some(initial));

        let value = b"next";
        let value_ref = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: value.len() as u32,
        };
        let mut requested = StepIo::test_frame([Some(value_ref)], [false], [Some(48)], None, 8);
        assert_eq!(
            back.step(
                &mut requested,
                &StepInputBytes::test_frame([Some(value)], None)
            ),
            StepOutcome::Progress
        );
        assert!(!requested.test_consumed(PortId(0)));
        assert!(requested.test_output(PortId(0)).is_none());

        let receipt = encode_receipt(1, digest(value));
        let receipt_ref = ValueRef {
            slot: 7,
            generation: 1,
            byte_len: receipt.len() as u32,
        };
        let completion = conduit_kernel::HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(receipt_ref, receipt.len() as u32).unwrap()),
            failure: None,
        };
        let mut committed = StepIo::test_frame(
            [Some(value_ref)],
            [false],
            [Some(48)],
            Some((RequestId(2), completion)),
            8,
        );
        assert_eq!(
            back.step(
                &mut committed,
                &StepInputBytes::test_frame([Some(value)], Some(&receipt))
            ),
            StepOutcome::Progress
        );
        assert!(committed.test_consumed(PortId(0)));
        assert_eq!(committed.test_output(PortId(0)), Some(value_ref));
        assert_eq!(back.generation(), 1);
    }

    #[test]
    fn four_kib_recovery_is_bound_to_metadata_and_forwards_the_hosted_value() {
        let probe = ValueRef {
            slot: 9,
            generation: 1,
            byte_len: 1,
        };
        let initial = ValueRef {
            slot: 10,
            generation: 1,
            byte_len: 0,
        };
        let mut back = DurableStateBack::new(binding(), initial, probe).unwrap();
        let mut metadata_request = StepIo::test_frame([None], [false], [Some(4096)], None, 8);
        assert_eq!(
            back.step(
                &mut metadata_request,
                &StepInputBytes::test_frame([None], None)
            ),
            StepOutcome::Progress
        );

        let value = vec![b'n'; conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize];
        let mut metadata = Vec::with_capacity(RECOVERY_HEADER_BYTES);
        metadata.extend_from_slice(&[PROTOCOL_VERSION, RECOVERY_PRESENT]);
        metadata.extend_from_slice(&7_u64.to_be_bytes());
        metadata.extend_from_slice(&digest(&value));
        let metadata_ref = ValueRef {
            slot: 8,
            generation: 1,
            byte_len: metadata.len() as u32,
        };
        let metadata_outcome = conduit_kernel::HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(metadata_ref, metadata.len() as u32).unwrap()),
            failure: None,
        };
        let mut metadata_step = StepIo::test_frame(
            [None],
            [false],
            [Some(4096)],
            Some((RequestId(0), metadata_outcome)),
            8,
        );
        assert_eq!(
            back.step(
                &mut metadata_step,
                &StepInputBytes::test_frame([None], Some(&metadata))
            ),
            StepOutcome::Progress
        );
        assert!(metadata_step.test_host_completion_consumed());
        assert_eq!(
            metadata_step.test_host_request(),
            Some((
                RequestId(1),
                HostCallId(1),
                BoundedValueRef::new(metadata_ref, metadata.len() as u32).unwrap()
            ))
        );

        let value_ref = ValueRef {
            slot: 7,
            generation: 1,
            byte_len: value.len() as u32,
        };
        let value_outcome = conduit_kernel::HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(value_ref, value.len() as u32).unwrap()),
            failure: None,
        };
        let mut value_step = StepIo::test_frame(
            [None],
            [false],
            [Some(4096)],
            Some((RequestId(1), value_outcome)),
            8,
        );
        assert_eq!(
            back.step(
                &mut value_step,
                &StepInputBytes::test_frame([None], Some(&value))
            ),
            StepOutcome::Progress
        );
        assert_eq!(value_step.test_output(PortId(0)), Some(value_ref));
        assert_eq!(value_step.test_discards()[0], Some(initial));
        assert_eq!(back.generation(), 7);
    }

    #[test]
    fn durable_binding_refuses_more_than_the_canonical_text_bound() {
        let mut too_large = binding();
        too_large.maximum_value_bytes = conduit_data::MAXIMUM_DATA_TEXT_BYTES + 1;
        assert_eq!(
            too_large.validate(),
            Err(DurableStateRefusal::InvalidBinding)
        );
    }

    #[test]
    fn mismatched_receipt_never_publishes_candidate() {
        let probe = ValueRef {
            slot: 9,
            generation: 1,
            byte_len: 1,
        };
        let initial = ValueRef {
            slot: 10,
            generation: 1,
            byte_len: 7,
        };
        let mut back = DurableStateBack::new(binding(), initial, probe).unwrap();
        let mut request = StepIo::test_frame([None], [false], [Some(48)], None, 8);
        back.step(&mut request, &StepInputBytes::test_frame([None], None));
        let absent = encode_absent();
        let absent_ref = ValueRef {
            slot: 8,
            generation: 1,
            byte_len: 2,
        };
        let recovery = conduit_kernel::HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(absent_ref, 2).unwrap()),
            failure: None,
        };
        let mut recovered = StepIo::test_frame(
            [None],
            [false],
            [Some(48)],
            Some((RequestId(0), recovery)),
            8,
        );
        back.step(
            &mut recovered,
            &StepInputBytes::test_frame([None], Some(&absent)),
        );

        let value = b"next";
        let value_ref = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: 4,
        };
        let mut requested = StepIo::test_frame([Some(value_ref)], [false], [Some(48)], None, 8);
        back.step(
            &mut requested,
            &StepInputBytes::test_frame([Some(value)], None),
        );
        let wrong = encode_receipt(1, digest(b"other"));
        let wrong_ref = ValueRef {
            slot: 7,
            generation: 1,
            byte_len: wrong.len() as u32,
        };
        let completion = conduit_kernel::HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(wrong_ref, wrong.len() as u32).unwrap()),
            failure: None,
        };
        let mut refused = StepIo::test_frame(
            [Some(value_ref)],
            [false],
            [Some(48)],
            Some((RequestId(2), completion)),
            8,
        );
        assert!(matches!(
            back.step(
                &mut refused,
                &StepInputBytes::test_frame([Some(value)], Some(&wrong))
            ),
            StepOutcome::Fail(_)
        ));
        assert!(!refused.test_consumed(PortId(0)));
        assert!(refused.test_output(PortId(0)).is_none());
        assert_eq!(back.generation(), 0);
    }
}
