//! CPU-only construction/publication proof. No Play, electrical completion or
//! physical manifestation is simulated, and no frame acknowledgement is issued.
use super::*;
use alloc::{boxed::Box, vec::Vec};
use conduit_environment_runtime::display_mask::EncodedFrame;
use conduit_seven_segment_mask::{DigitWiring, SevenSegmentLook, SevenSegmentShow, Tm1637Profile};
use core::cell::Cell;

const ENCODED: usize = conduit_tm1637_protocol::DISPLAY_FRAME_BYTES;
const FRAME: usize = ENCODED + 4;
std::thread_local! {
    static DROPS: Cell<usize> = const { Cell::new(0) };
    static LAST_DROPPED_READS: Cell<usize> = const { Cell::new(usize::MAX) };
}

/// Actually owned, inert CPU clock in one calibrated millisecond basis.
struct CpuClock {
    reads: Cell<usize>,
}
impl RendererClock for CpuClock {
    fn now_ms(&self) -> u64 {
        self.reads.set(self.reads.get() + 1);
        12
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
}
impl Drop for CpuClock {
    fn drop(&mut self) {
        DROPS.with(|n| n.set(n.get() + 1));
        LAST_DROPPED_READS.with(|n| n.set(self.reads.get()));
    }
}

/// Real prepared canonical codec with an independently owned output workspace.
struct CpuEncoder {
    codec: conduit_tm1637_protocol::PreparedDisplayFrame,
    bytes: Box<[u8]>,
}
impl PreparedFrameEncoder for CpuEncoder {
    type Registers = [u8; 6];
    fn validate_support(
        &self,
        profile: &Tm1637Profile,
    ) -> Result<(), conduit_environment_runtime::display_mask::Refusal> {
        profile
            .validate()
            .map_err(conduit_environment_runtime::display_mask::Refusal::Wiring)
    }
    fn maximum_frame_bytes(&self) -> usize {
        FRAME
    }
    fn retained_backing_bytes(&self) -> usize {
        self.bytes.len()
    }
    fn encode(
        &mut self,
        show: &SevenSegmentShow,
        profile: &Tm1637Profile,
    ) -> Result<EncodedFrame<'_, Self::Registers>, conduit_environment_runtime::display_mask::Refusal>
    {
        let registers = profile
            .map(show)
            .map_err(conduit_environment_runtime::display_mask::Refusal::Wiring)?;
        let encoded = self
            .codec
            .encode(registers, 2, true)
            .map_err(conduit_environment_runtime::display_mask::Refusal::Frame)?;
        self.bytes[..encoded.len()].copy_from_slice(encoded);
        Ok(EncodedFrame {
            registers,
            canonical: &self.bytes[..encoded.len()],
        })
    }
}

fn identity() -> Identity {
    Identity {
        host_id: "cpu/renderer-test".into(),
        boot_id: "cpu/boot".into(),
        base_id: "cpu/renderer".into(),
        provider_instance_id: "cpu/renderer/instance".into(),
        provider_generation: 1,
        resource_pool_id: "cpu/renderer/pool".into(),
        resource_generation_id: c::ResourceGenerationId("cpu/renderer/generation".into()),
        envelope_id: "cpu/renderer/envelope".into(),
        artifact_id: "cpu/renderer/artifact".into(),
        implementation_id: "cpu/renderer/implementation".into(),
        base_implementation_id: "cpu/renderer/implementation".into(),
    }
}
fn mask() -> PreparedDisplayMask<CpuEncoder> {
    PreparedDisplayMask::prepare_with_encoder(
        Tm1637Profile {
            digits: core::array::from_fn(|n| DigitWiring {
                address: n as u8,
                segment_bits: [0, 1, 2, 3, 4, 5, 6],
            }),
            decimal_points: [None; 4],
            colon: None,
        },
        SevenSegmentLook::fahrenheit(),
        10,
        20,
        CpuEncoder {
            codec: conduit_tm1637_protocol::PreparedDisplayFrame::prepare().unwrap(),
            bytes: alloc::vec![0; FRAME].into_boxed_slice(),
        },
    )
    .unwrap()
}
fn ready<const N: usize>(
    resource_bytes: u64,
) -> Result<ReadyRenderer<CpuClock, CpuEncoder, N>, Refusal> {
    // SAFETY: this CPU fixture transfers its actual initialized clock, codec and
    // workspace, with exclusively owned identity backing. The supplied storage
    // ceiling is checked by the real constructor. No grant, Ready hardware,
    // electrical effect, fabricated completion or physical proof is asserted.
    unsafe {
        ReadyRenderer::new_with_encoder(
            identity(),
            CpuClock {
                reads: Cell::new(0),
            },
            mask(),
            Bounds {
                face_bytes: 64,
                show_bytes: 64,
                maximum_frames: 3,
                resource_bytes,
            },
        )
    }
}
fn advertisement() -> c::HostAdvertisement {
    c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "cpu/renderer-test".into(),
        boot_id: "cpu/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "cpu/renderer-profile".into(),
        bases: Vec::new(),
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    }
}

#[test]
fn custom_encoder_extent_and_actual_backing_are_required_before_readiness() {
    assert!(matches!(ready::<0>(4096), Err(Refusal::Bounds)));
    assert!(matches!(ready::<{ FRAME - 1 }>(4096), Err(Refusal::Bounds)));
    assert!(matches!(ready::<FRAME>(128), Err(Refusal::Bounds)));
    let owner = ready::<FRAME>(4096).unwrap();
    let id = owner.native_identity();
    let identity_bytes: usize = [
        &id.host_id.0,
        &id.boot_id.0,
        &id.base_id.0,
        &id.provider_instance_id.0,
        &id.resource_pool_id.0,
        &id.resource_generation_id.0,
        &id.envelope_id.0,
        &id.artifact_id.0,
        &id.implementation_id.0,
        &id.base_implementation_id.0,
    ]
    .iter()
    .map(|s| s.capacity())
    .sum();
    assert_eq!(
        owner.retained_backing_bytes(),
        Some(64 + FRAME + identity_bytes)
    );
    assert_eq!(owner.clock.reads.get(), 0);
    assert!(matches!(
        ready::<FRAME>((64 + FRAME + identity_bytes - 1) as u64),
        Err(Refusal::Bounds)
    ));
}

#[test]
fn publication_borrows_original_custom_owner_and_refuses_changed_host_or_boot() {
    let owner = ready::<FRAME>(4096).unwrap();
    let _facts = crate::execution_authority::NativeFacts::renderer(&owner);
    let actual = advertisement();
    let projected = crate::native_images::renderer(&owner, &actual, 4096).unwrap();
    assert_eq!(
        projected.base.provider_instance_id,
        owner.native_identity().provider_instance_id
    );
    assert_eq!(
        projected.base.provider_generation,
        owner.native_identity().provider_generation
    );
    assert_eq!(
        projected.resource.pool_id,
        owner.native_identity().resource_pool_id
    );
    let mut wrong = advertisement();
    wrong.host_id = "cpu/another-host".into();
    assert!(matches!(
        crate::native_images::renderer(&owner, &wrong, 4096),
        Err(crate::native_images::Refusal::Context)
    ));
    wrong = advertisement();
    wrong.boot_id = "cpu/another-boot".into();
    assert!(matches!(
        crate::native_images::renderer(&owner, &wrong, 4096),
        Err(crate::native_images::Refusal::Context)
    ));
    let mut published = advertisement();
    let mut publication = crate::native_images::Publication::new(&mut published, 1, 4096).unwrap();
    publication.renderer(&owner).unwrap();
    publication.finish().unwrap();
    assert_eq!(published.bases.len(), 1);
    assert_eq!(owner.clock.reads.get(), 0);
}

#[test]
fn dropping_cpu_owner_releases_clock_without_sampling_or_publishing_effects() {
    let before = DROPS.with(Cell::get);
    let owner = ready::<FRAME>(4096).unwrap();
    drop(owner);
    assert_eq!(DROPS.with(Cell::get), before + 1);
    assert_eq!(LAST_DROPPED_READS.with(Cell::get), 0);
}

#[cfg(feature = "ht16k33-mask")]
#[test]
fn actual_ht_encoder_has_the_generic_ready_constructor_at_its_source_extent() {
    use conduit_environment_runtime::display_mask::ht16k33::Ht16k33Encoder;
    type HtReady = ReadyRenderer<CpuClock, Ht16k33Encoder, { Ht16k33Encoder::FRAME_BYTES }>;
    // Compile the real constructor signature without fabricating an HT owner
    // or introducing a public helper merely to create test wiring metadata.
    let _constructor: unsafe fn(
        Identity,
        CpuClock,
        PreparedDisplayMask<Ht16k33Encoder>,
        Bounds,
    ) -> Result<HtReady, Refusal> = HtReady::new_with_encoder;
}
