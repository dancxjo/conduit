//! Original initialized IO ownership, compact metadata and original grant scope.
//! CPU fixtures only; no controller operation, physical readiness or Play claim.
use super::*;
use conduit_i2c_transaction::{
    installation::{
        I2cNativeIdentity, ReadyI2cBase, I2C_AUTHORITY, I2C_CAPABILITY, I2C_IMPLEMENTATION,
    },
    owner::I2cAttachment,
    I2cDisposition, I2cProvider, I2cTransaction,
};
use conduit_monotonic_wait::clock_at::{
    codec::ClockDisposition,
    owner::MonotonicDeadlineProvider,
    retained::{ClockAtAttachment, ClockAtNativeIdentity, ReadyClockAtBase},
};
struct Bus;
impl I2cProvider for Bus {
    fn transact(&mut self, _: &I2cTransaction<'_>, _: &mut [u8]) -> Result<usize, I2cDisposition> {
        panic!("preparation must not transact")
    }
    fn revoke(&mut self) {
        panic!("preparation must preserve the owner")
    }
}
struct Clock;
impl MonotonicDeadlineProvider for Clock {
    fn poll_until(&mut self, _: u64) -> Result<Option<u64>, ClockDisposition> {
        panic!("preparation must not read time")
    }
    fn revoke(&mut self) {
        panic!("preparation must preserve the owner")
    }
}
fn owners(actual: &c::HostAdvertisement) -> (ReadyI2cBase<Bus>, ReadyClockAtBase<Clock>) {
    // SAFETY: this fixture exclusively owns inert providers and their declared
    // current-Boot identities. It tests custody plumbing, not physical truth.
    unsafe {
        let bus = ReadyI2cBase::new(
            I2cNativeIdentity {
                base_implementation_id: "fixture/base-i2c@1".into(),
                host_id: actual.host_id.clone(),
                boot_id: actual.boot_id.clone(),
                base_id: "fixture/bus".into(),
                provider_instance_id: "fixture/bus/instance".into(),
                provider_generation: 1,
                resource_pool_id: "fixture/bus/pool".into(),
                resource_generation_id: c::ResourceGenerationId("fixture/bus/generation".into()),
                envelope_id: "fixture/bus/envelope".into(),
                artifact_id: "image/1".into(),
            },
            I2cAttachment {
                generation: 1,
                minimum_address: 0x70,
                maximum_address: 0x76,
                resource_bytes: 32,
            },
            Bus,
        )
        .unwrap();
        let clock = ReadyClockAtBase::new(
            ClockAtNativeIdentity {
                host_id: actual.host_id.clone(),
                boot_id: actual.boot_id.clone(),
                base_id: "fixture/clock".into(),
                provider_instance_id: "fixture/clock/instance".into(),
                provider_generation: 1,
                resource_pool_id: "fixture/clock/pool".into(),
                resource_generation_id: c::ResourceGenerationId("fixture/clock/generation".into()),
                envelope_id: "fixture/clock/envelope".into(),
                artifact_id: "image/1".into(),
                implementation_id: "fixture/clock/implementation".into(),
                base_implementation_id: "fixture/clock/base-implementation".into(),
                capability_id: "fixture/clock/capability".into(),
                execution_profile_id: "fixture/clock/profile".into(),
                authority_contract_id: "fixture/clock/authority".into(),
                mechanism_family: "fixture/monotonic-read-counter".into(),
                resource_class: "machine/clock-attachment".into(),
            },
            ClockAtAttachment { resource_bytes: 1 },
            Clock,
        )
        .unwrap();
        (bus, clock)
    }
}
#[test]
fn io_facts_are_borrowed_without_allocation_and_publication_preserves_exact_pools() {
    let (_, mut actual, _, _) = crate::membership_tests::fixture();
    let (bus, clock) = owners(&actual);
    let (facts, measured) = crate::member_return_tests::return_allocation_probe::observe(|| {
        (NativeFacts::i2c(&bus), NativeFacts::clock_at(&clock))
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
    assert_eq!(facts.0.implementation, I2C_IMPLEMENTATION);
    assert_eq!(facts.0.capability, I2C_CAPABILITY);
    assert_eq!(facts.1.capability, clock.identity().capability_id.as_str());
    assert_eq!(facts.1.instance, &clock.identity().provider_instance_id);
    let original_base_count = actual.bases.len();
    let original_pool_count = actual.resources.len();
    let mut publication = crate::native_images::Publication::new(&mut actual, 2, 8192).unwrap();
    publication.i2c(&bus).unwrap();
    publication.clock_at(&clock).unwrap();
    publication.finish().unwrap();
    assert_eq!(actual.bases.len(), original_base_count + 2);
    assert_eq!(actual.resources.len(), original_pool_count + 2);
    let bus_base = actual
        .bases
        .iter()
        .find(|base| base.base_id == bus.native_identity().base_id)
        .unwrap();
    assert_eq!(
        bus_base.implementation_id,
        bus.native_identity().base_implementation_id
    );
    assert_ne!(bus_base.implementation_id.as_str(), I2C_CAPABILITY);
    let clock_base = actual
        .bases
        .iter()
        .find(|base| base.base_id == clock.identity().base_id)
        .unwrap();
    assert_eq!(
        clock_base.implementation_id,
        clock.identity().base_implementation_id
    );
    assert_eq!(
        clock_base.mechanism_family.as_str(),
        clock.identity().mechanism_family
    );
    assert_eq!(clock_base.capability_ids[0], clock.identity().capability_id);
    for (pool, class) in [
        (
            bus.native_identity().resource_pool_id.as_str(),
            "machine/i2c-attachment",
        ),
        (
            clock.identity().resource_pool_id.as_str(),
            "machine/clock-attachment",
        ),
    ] {
        let offered = actual
            .resources
            .iter()
            .find(|r| r.pool_id.as_str() == pool)
            .unwrap();
        assert_eq!(offered.class_id.as_str(), class);
        assert_eq!(
            offered.capacity_units, 1,
            "publication cannot enlarge the physical pool"
        );
    }
}
#[test]
fn current_io_grants_bind_installed_body_boot_and_operation_without_dispatch() {
    let (bootstrap, actual, _, _) = crate::membership_tests::fixture();
    let member = crate::membership_tests::receipt(&bootstrap, &actual).credential;
    let (bus, clock) = owners(&actual);
    let preparation = b::HostPreparationDelegation {
        authorizer: b::HostPreparationAuthorizer::SeedOwner,
        maximum_assignment_bytes: 4096,
        maximum_runtime_state_bytes: 4096,
    };
    let execution = b::HostExecutionDelegation {
        authorizer: b::HostExecutionAuthorizer::SeedOwner,
        maximum_active_worksets: 1,
        continuation: None,
    };
    let policy = crate::execution::Policy {
        preparation: Some(&preparation),
        execution: Some(&execution),
        active_worksets: 0,
        maximum_retained_bytes: 4096,
    };
    let context = b::NativePreparationContext {
        bootstrap: &bootstrap,
        member: &member,
        actual: &actual,
        image_id: "image/1",
        configuration_id: "configuration/1",
        now_millis: 20,
    };
    let cases = [
        (
            NativeFacts::i2c(&bus),
            Operation {
                kind: conduit_i2c_transaction::contract::I2C_KIND,
                authority: I2C_AUTHORITY,
                call: conduit_i2c_transaction::contract::I2C_CALL,
                maximum_parameter_bytes: 4096,
                maximum_result_bytes: 4096,
                maximum_work_units: 1,
                maximum_operations: 2,
            },
        ),
        (
            NativeFacts::clock_at(&clock),
            Operation {
                kind: conduit_monotonic_wait::clock_at::contract::CLOCK_KIND,
                authority: clock.identity().authority_contract_id.as_str(),
                call: conduit_monotonic_wait::clock_at::contract::CLOCK_CALL,
                maximum_parameter_bytes: 512,
                maximum_result_bytes: 512,
                maximum_work_units: conduit_monotonic_wait::clock_at::owner::MAXIMUM_POLL_STEPS,
                maximum_operations: 2,
            },
        ),
    ];
    for (facts, operation) in cases {
        let mut issuer = Issuer::new();
        assert!(matches!(
            issuer.authorize(context, policy, &facts, &operation, "original/io", 0),
            Err(Refusal::Bounds)
        ));
        assert!(issuer.grant().is_none());
        issuer
            .authorize(context, policy, &facts, &operation, "original/io", 4096)
            .unwrap();
        let original = issuer.grant().unwrap().clone();
        assert_eq!(original.capability_id.as_str(), facts.capability);
        assert_eq!(original.host_id, actual.host_id);
        assert_eq!(original.boot_id, actual.boot_id);
        issuer
            .authorize(context, policy, &facts, &operation, "original/io", 4096)
            .unwrap();
        let mut other = actual.clone();
        other.boot_id = "another/boot".into();
        assert!(matches!(
            issuer.authorize(
                b::NativePreparationContext {
                    actual: &other,
                    ..context
                },
                policy,
                &facts,
                &operation,
                "original/io",
                4096
            ),
            Err(Refusal::Scope)
        ));
        assert!(matches!(
            issuer.authorize(
                b::NativePreparationContext {
                    image_id: "another/image",
                    ..context
                },
                policy,
                &facts,
                &operation,
                "original/io",
                4096
            ),
            Err(Refusal::Scope)
        ));
        assert_eq!(issuer.grant(), Some(&original));
    }
}
#[test]
fn compact_io_projection_refuses_short_budget_foreign_boot_and_duplicate_pools() {
    let (_, actual, _, _) = crate::membership_tests::fixture();
    let (bus, clock) = owners(&actual);
    let original_bus = crate::native_images::i2c(&bus, &actual, 4096).unwrap();
    let original_clock = crate::native_images::clock_at(&clock, &actual, 4096).unwrap();
    assert!(matches!(
        crate::native_images::i2c(&bus, &actual, original_bus.owned_bytes - 1),
        Err(crate::native_images::Refusal::Capacity)
    ));
    assert!(matches!(
        crate::native_images::clock_at(&clock, &actual, original_clock.owned_bytes - 1),
        Err(crate::native_images::Refusal::Capacity)
    ));
    let mut other = actual.clone();
    other.boot_id = "another/boot".into();
    assert!(matches!(
        crate::native_images::i2c(&bus, &other, 4096),
        Err(crate::native_images::Refusal::Context)
    ));
    assert!(matches!(
        crate::native_images::clock_at(&clock, &other, 4096),
        Err(crate::native_images::Refusal::Context)
    ));
    let mut installed = actual.clone();
    installed.bases.push(original_clock.base);
    installed.resources.push(original_clock.resource);
    assert!(matches!(
        crate::native_images::clock_at(&clock, &installed, 4096),
        Err(crate::native_images::Refusal::Context)
    ));
    assert_eq!(NativeFacts::i2c(&bus).boot, &actual.boot_id);
    assert_eq!(NativeFacts::clock_at(&clock).boot, &actual.boot_id);
}
