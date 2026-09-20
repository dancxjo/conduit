use super::*;

use alloc::string::ToString;
use conduit_protected_line::{
    CarrierFailure, EndpointBinding, ProtectedHandshake, ProtectedSessionPolicy, SessionLimits,
};

use crate::cryptographic_entropy::{EntropyProvider, EntropyReceipt};

const PSK: [u8; 32] = [0x31; 32];

struct FixtureEntropy {
    provider: EntropyProvider,
}

impl CryptographicEntropySource for FixtureEntropy {
    fn provider(&self) -> EntropyProvider {
        self.provider
    }

    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        output.fill(0x42);
        Ok(())
    }
}

struct RespondingCarrier {
    responder: ProtectedHandshake,
    response: [u8; 64],
    response_len: usize,
}

impl ProtectedFrameCarrier for RespondingCarrier {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), CarrierFailure> {
        self.responder
            .read_message(frame)
            .map_err(|_| CarrierFailure::Lost)?;
        self.response_len = self
            .responder
            .next_message_bytes()
            .map_err(|_| CarrierFailure::Lost)?;
        self.responder
            .write_message(&mut self.response[..self.response_len])
            .map_err(|_| CarrierFailure::Lost)
    }

    fn receive_frame(&mut self, output: &mut [u8], _: u32) -> Result<usize, CarrierFailure> {
        output[..self.response_len].copy_from_slice(&self.response[..self.response_len]);
        Ok(self.response_len)
    }

    fn close(&mut self) -> Result<(), CarrierFailure> {
        Ok(())
    }
}

fn binding() -> SessionBinding {
    SessionBinding {
        initiator: EndpointBinding {
            host_id: "host/conduitos".to_string(),
            boot_id: "boot/conduitos".to_string(),
        },
        responder: EndpointBinding {
            host_id: "host/std".to_string(),
            boot_id: "boot/std".to_string(),
        },
        negotiation_id: "negotiation/one".to_string(),
        line_session_id: "line/one".to_string(),
        candidate_binding: "candidate/one".to_string(),
        transport_binding: "tls/sha256/one".to_string(),
    }
}

fn policy() -> ProtectedSessionPolicy {
    ProtectedSessionPolicy {
        traffic: SessionLimits {
            maximum_payload_bytes: 256,
            maximum_frames_per_direction: 8,
            maximum_bytes_per_direction: 2_048,
        },
        maximum_simultaneous_sessions: 1,
        maximum_pending_frames_per_session: 1,
        handshake_work_units: 2,
        handshake_timeout_millis: 2_000,
        idle_timeout_millis: 5_000,
    }
}

fn entropy() -> CryptographicEntropyBase<FixtureEntropy, 1> {
    CryptographicEntropyBase::admit(FixtureEntropy {
        provider: EntropyProvider {
            base_id: "base/entropy/fixture",
            provider_instance_id: "fixture/one",
            provider_generation: 1,
        },
    })
    .unwrap()
}

#[test]
fn conduitos_establishes_the_shared_profile_with_one_admitted_entropy_request() {
    let binding = binding();
    let carrier = RespondingCarrier {
        responder: ProtectedHandshake::new(
            Role::Responder,
            &binding,
            policy().traffic,
            PSK,
            [0x53; 32],
        )
        .unwrap(),
        response: [0; 64],
        response_len: 0,
    };
    let mut entropy = entropy();
    let protected = establish_conduitos_protected_line(
        carrier,
        Role::Initiator,
        &binding,
        policy(),
        PSK,
        &mut entropy,
    )
    .unwrap();

    assert_eq!(entropy.requests_used(), 1);
    assert_eq!(protected.session().limits(), policy().traffic);
    assert_eq!(protected_line_implementation_id(), IMPLEMENTATION_ID);
}

#[test]
fn entropy_refusal_precedes_any_session_work() {
    struct NeverCarrier;
    impl ProtectedFrameCarrier for NeverCarrier {
        fn send_frame(&mut self, _: &[u8]) -> Result<(), CarrierFailure> {
            panic!("carrier must not run without entropy")
        }
        fn receive_frame(&mut self, _: &mut [u8], _: u32) -> Result<usize, CarrierFailure> {
            panic!("carrier must not run without entropy")
        }
        fn close(&mut self) -> Result<(), CarrierFailure> {
            panic!("carrier must not run without entropy")
        }
    }
    let mut entropy = entropy();
    let _: EntropyReceipt = entropy.fill(&mut [0_u8; 32]).unwrap();
    let refusal = establish_conduitos_protected_line(
        NeverCarrier,
        Role::Initiator,
        &binding(),
        policy(),
        PSK,
        &mut entropy,
    )
    .err()
    .unwrap();
    assert_eq!(
        refusal,
        ConduitOsProtectedLineRefusal::Entropy(EntropyRefusal::RequestCapacity)
    );
}
