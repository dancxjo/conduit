use crate::protocol::{CpuBus, CpuClock};
use conduit_core as c;
use conduit_i2c_transaction::{
    installation::{I2cNativeIdentity, ReadyI2cBase},
    owner::I2cAttachment,
    I2cDisposition, I2cProvider, I2cTransaction,
};
use conduit_mcu_host_runtime::{
    execution::Policy,
    execution_factory::SoftwareInventory,
    root::{Configuration, Root},
    seed::Seed,
};
use conduit_monotonic_wait::clock_at::{
    codec::ClockDisposition,
    installation::*,
    owner::MonotonicDeadlineProvider,
    retained::{ClockAtAttachment, ClockAtNativeIdentity, ReadyClockAtBase},
};
use core::cell::Cell;
use ed25519_dalek::{Signer, SigningKey};
struct Messages(std::collections::VecDeque<Vec<u8>>);
impl conduit_mcu_host_runtime::transport::MessageTransport for Messages {
    type Error = ();
    async fn receive<'a>(&mut self, out: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let bytes = self.0.pop_front().ok_or(())?;
        out.get_mut(..bytes.len())
            .ok_or(())?
            .copy_from_slice(&bytes);
        Ok(&out[..bytes.len()])
    }
    async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
        Ok(())
    }
}
fn ready<F: core::future::Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    match future.as_mut().poll(&mut core::task::Context::from_waker(
        core::task::Waker::noop(),
    )) {
        core::task::Poll::Ready(result) => result,
        core::task::Poll::Pending => panic!("bounded CPU transport"),
    }
}
fn now() -> u64 {
    17
}
fn main() {
    // Generated-owner CPU construction may use host ABI temporaries. This
    // separately funded 32MiB CPU stack is not an RP2040 footprint assertion.
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(exercise)
        .unwrap()
        .join()
        .unwrap();
}
fn exercise() {
    assert_eq!(
        neutral_workset::generated::SOURCE_SHA256,
        EXPECTED_SOURCE_SHA256
    );
    assert_eq!(neutral_workset::N, 76);
    assert_eq!(neutral_workset::source::CORD_COUNT, 83);
    assert!(neutral_workset::C >= neutral_workset::source::CORD_COUNT);
    let bus_calls = Cell::new(0);
    let clock_calls = Cell::new(0);
    // These are original owned CPU providers under explicit fixture identities.
    // They represent no RP2040 electrical attachment or production authority.
    let (bus_base, bus_pool, bus_suffix) = match config::I2C_CONTROLLER {
        0 => ("rp2040/i2c/0", "pool/rp2040-i2c-0", "/i2c/0"),
        1 => ("rp2040/i2c/1", "pool/rp2040-i2c-1", "/i2c/1"),
        _ => panic!("checked I2C controller"),
    };
    let (minimum_address, maximum_address) = config::I2C_CALL_ADDRESS_RANGE.unwrap();
    let bus = unsafe {
        ReadyI2cBase::new(
            I2cNativeIdentity {
                host_id: "cpu/host".into(),
                boot_id: "cpu/boot".into(),
                base_id: bus_base.into(),
                base_implementation_id: config::I2C_BASE_IMPLEMENTATION.unwrap().into(),
                provider_instance_id: format!("cpu/boot{bus_suffix}").into(),
                provider_generation: 1,
                resource_pool_id: bus_pool.into(),
                resource_generation_id: c::ResourceGenerationId(format!("cpu/boot{bus_suffix}/1")),
                envelope_id: format!("cpu/boot{bus_suffix}/calls").into(),
                artifact_id: config::IMAGE_ID.into(),
            },
            I2cAttachment {
                generation: 1,
                minimum_address,
                maximum_address,
                resource_bytes: conduit_i2c_transaction::transaction::MAXIMUM_TRANSACTION_BYTES
                    as u64,
            },
            CpuBus(&bus_calls),
        )
    }
    .unwrap();
    let clock = unsafe {
        ReadyClockAtBase::new(
            ClockAtNativeIdentity {
                host_id: "cpu/host".into(),
                boot_id: "cpu/boot".into(),
                base_id: "rp2040/clock-at/0".into(),
                base_implementation_id: config::CLOCK_AT_BASE_IMPLEMENTATION.unwrap().into(),
                provider_instance_id: "cpu/boot/clock-at/0".into(),
                provider_generation: 1,
                resource_pool_id: "pool/rp2040/clock-at/0".into(),
                resource_generation_id: c::ResourceGenerationId("cpu/boot/clock-at/0/1".into()),
                envelope_id: "cpu/boot/clock-at/0/calls".into(),
                artifact_id: config::IMAGE_ID.into(),
                implementation_id: CLOCK_IMPLEMENTATION.into(),
                capability_id: CLOCK_IMPLEMENTATION.into(),
                execution_profile_id: CLOCK_EXECUTION_PROFILE.into(),
                authority_contract_id: CLOCK_AUTHORITY.into(),
                mechanism_family: CLOCK_RESOURCE_CLASS.into(),
                resource_class: CLOCK_RESOURCE_CLASS.into(),
            },
            ClockAtAttachment { resource_bytes: 1 },
            CpuClock(&clock_calls),
        )
    }
    .unwrap();
    let mut equipment = neutral_equipment::Equipment {
        bus: Some(bus),
        clock: Some(clock),
        identity_bytes: 0,
    };
    let mut actual = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "cpu/host".into(),
        boot_id: "cpu/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: config::CONFIGURATION_ID.into(),
        bases: vec![],
        resources: vec![c::resource_offer(
            config::RESIDENT_WORK_POOL,
            c::COOPERATIVE_STEP_WORK_CLASS_ID,
            config::RESIDENT_WORK_UNITS,
        )],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    assert!(equipment.publish(&mut actual, 0).is_err());
    equipment.publish(&mut actual, 64 * 1024).unwrap();
    assert_eq!(actual.bases.len(), 2);
    let owner = SigningKey::from_bytes(&[7; 32]);
    let payload=serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.body/member-bootstrap@2","body_id":"cpu/body","host_id":"cpu/host",
        "image_id":config::IMAGE_ID,"configuration_id":config::CONFIGURATION_ID,
        "owner":{"host_id":"cpu/owner","boot_id":"cpu/owner-boot","offer_generation":1,"verifying_key":owner.verifying_key().to_bytes().to_vec()},
        "admission":{"spawn":{"provision":{"protocol":conduit_body::PICO_SPAWN_PROTOCOL,"spore_id":"cpu/spore","image_id":config::IMAGE_ID,"invitation_id":"cpu/invitation","body_id":"cpu/body","nonce":vec![2;32],"expires_at_millis":1000,"secret":vec![3;32]}}}
    })).unwrap();
    let mut carrier = b"CONDUIT_SEED@2\0\0".to_vec();
    carrier.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    carrier.extend(payload);
    let seed = Seed::parse(
        &carrier,
        "cpu/host",
        config::IMAGE_ID,
        config::CONFIGURATION_ID,
    )
    .unwrap();
    let mut root = Root::<{ config::MAX_ASSIGNMENT_BYTES }>::new(
        seed,
        actual,
        Configuration {
            image_id: config::IMAGE_ID,
            configuration_id: config::CONFIGURATION_ID,
            friendly_label: "CPU",
            preparation: config::PREPARATION_DELEGATION.as_ref(),
            execution: config::EXECUTION_DELEGATION.as_ref(),
        },
    )
    .unwrap();
    assert!(!root.has_current_member());
    let mut residents = resident::generated::Initialized::prepare().unwrap();
    let runtime = neutral_preflight::Runtime {
        now,
        allocated_bytes: probe::current,
        challenge: || [1; 32],
    };
    let mut issuers = neutral_authority::Issuers::new();
    let operations = neutral_authority::Operations::selected().unwrap();
    assert_eq!(
        neutral_preflight::preflight(
            &mut root,
            &mut equipment,
            &mut residents,
            &mut issuers,
            &operations,
            &runtime,
            0
        ),
        Err("current-member-required")
    );
    assert_eq!(
        neutral_preflight::preflight(
            &mut root,
            &mut equipment,
            &mut residents,
            &mut issuers,
            &operations,
            &runtime,
            1024 * 1024
        ),
        Err("current-member-required")
    );
    let receipt = conduit_body::PortableAdmissionReceipt {
        schema: conduit_body::SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential: serde_json::from_value(serde_json::json!({
            "credential_id":"cpu/credential","body_id":"cpu/body","part_id":"cpu/part",
            "host_id":"cpu/host","boot_id":"cpu/boot","issued_at_millis":7,
        }))
        .unwrap(),
        host_advertisement: root.actual().clone(),
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
    };
    let owner_peer = conduit_body::FederationPeer {
        host_id: "cpu/owner".into(),
        boot_id: "cpu/owner-boot".into(),
        offer_generation: c::OfferGeneration(1),
        verifying_key: owner.verifying_key().to_bytes(),
    };
    let signed = conduit_body::attest_portable_admission_receipt(
        &receipt,
        owner_peer,
        SigningKey::from_bytes(&[3; 32]).verifying_key().to_bytes(),
        |bytes| owner.sign(bytes).to_bytes(),
    )
    .unwrap();
    let mut messages = Messages(
        [
            serde_json::to_vec(&signed.transport_metadata().unwrap()).unwrap(),
            signed.receipt_json.into_bytes(),
        ]
        .into(),
    );
    ready(root.spawn(&mut messages, &mut [0; 4096])).unwrap();
    assert!(root.has_current_member());
    {
        let context = root.return_context(now()).unwrap();
        let policy = Policy {
            preparation: root.configuration().preparation,
            execution: root.configuration().execution,
            active_worksets: 0,
            maximum_retained_bytes: 1024 * 1024,
        };
        issuers
            .authorize(context, policy, &equipment, &operations, 1024 * 1024)
            .unwrap();
    }
    positive::retain(&mut root, &issuers, &owner, now());
    execution::run(
        &mut root,
        &mut equipment,
        &mut residents,
        &mut issuers,
        &operations,
        &runtime,
    );
    assert_eq!(bus_calls.get(), if FAIL_DISPLAY { 49 } else { 54 });
    assert!(clock_calls.get() > 0);
}
