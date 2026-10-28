//! Actual initialized resources and Root provisioning Control. Environmental
//! Host/Body construction and its sensor Plan remain separate.
//! No simulated provider, descriptor-derived availability or execution is used.
#[path = "native_body_root.rs"]
mod body;
#[cfg(feature = "display-physical-check")]
#[path = "native_display_check.rs"]
mod display_check;
#[path = "native_display_native_offer.rs"]
mod display_offers;
#[path = "native_install.rs"]
mod install;
#[path = "native_lifecycle.rs"]
mod lifecycle;
#[cfg(feature = "target-memory-check")]
#[path = "native_local_display_plan.rs"]
mod local_display_plan;
#[cfg(feature = "target-mask-memory-check")]
#[path = "native_local_mask.rs"]
mod local_mask;
#[cfg(feature = "local-appliance")]
#[path = "native_local_renewal.rs"]
mod local_renewal;
#[cfg(any(feature = "target-memory-check", feature = "target-mask-memory-check"))]
#[path = "native_local_sampler_plan.rs"]
mod local_sampler_plan;
#[cfg(feature = "target-mask-memory-check")]
#[path = "native_local_sampler_run.rs"]
mod local_sampler_run;
#[cfg(feature = "mask-root")]
#[path = "native_mask_root.rs"]
mod mask;
#[cfg(any(feature = "target-memory-check", feature = "target-mask-memory-check"))]
#[path = "native_memory_check.rs"]
pub(crate) mod memory_check;
#[path = "native_network_offer.rs"]
mod network_offer;
#[cfg(not(feature = "flat-network-appliance"))]
#[path = "native_network_plan.rs"]
mod network_plan;
#[cfg(all(
    feature = "local-appliance",
    feature = "body-persistence",
    target_os = "none"
))]
#[path = "native_root_observed_offer.rs"]
mod observed_offer;
#[path = "native_offer.rs"]
mod offers;
#[path = "native_root_policy.rs"]
mod policy;
#[path = "native_setup.rs"]
mod setup;
use conduit_core as c;
use conduit_esp32_pulse_capture::{Esp32PulseCapture as RawPulse, LIMITS};
#[cfg(all(feature = "local-appliance", target_os = "none"))]
pub(crate) type DevicePulse<'a> = crate::native_pulse_fence::GatedPulse<RawPulse<'a>>;
#[cfg(not(all(feature = "local-appliance", target_os = "none")))]
pub(crate) type DevicePulse<'a> = RawPulse<'a>;
use conduit_pulse_capture::{installation::*, owner::*};
use DevicePulse as Esp32PulseCapture;

include!("native_root_state.rs");

impl NativeRoot {
    #[inline(never)]
    pub fn prepare(
        mut sensor: Esp32PulseCapture<'static>,
        radio: crate::network::Radio,
        boot: &crate::boot::ObservedBoot,
        network_source: c::PlotIdentity,
        sdk_reservation: crate::sdk_memory::NativeSdkReservation,
        wait: conduit_monotonic_wait::ReadyMonotonicWaitBase<crate::native_wait::EspWait>,
        display: alloc::boxed::Box<crate::native_display::NativeDisplay>,
    ) -> Self {
        let issuer = crate::native_authority::RootIssuer::observe(boot)
            .expect("initialized hardware entropy for Root issuance");
        let (host, boot) = boot.ids();
        let host: c::HostId = host.into();
        let boot: c::BootId = boot.into();
        // One owned pin, actual provider/future backing and the reviewed finite
        // cooperative limits. A future installation must separately admit its
        // cadence, capability authority and complete kernel memory/work before Play.
        let identity = PulseNativeIdentity {
            host_id: host.clone(),
            boot_id: boot.clone(),
            base_id: "base/esp32/gpio27".into(),
            provider_instance_id: "instance/esp32/gpio27".into(),
            provider_generation: 1,
            resource_pool_id: "pool/esp32/gpio27".into(),
            resource_generation_id: c::ResourceGenerationId("generation/gpio27/1".into()),
            envelope_id: "envelope/esp32/gpio27".into(),
            artifact_id: "native/esp32-gpio-pulse-root@1".into(),
            implementation_id: "esp32/gpio27-pulse@1".into(),
            base_implementation_id: "esp32/gpio27-pulse-base@1".into(),
        };
        // SAFETY: Root consumes the actual exclusively initialized GPIO27 Flex
        // provider. Its read implementation has no allocation, releases low on
        // drop, and enforces these finite hold/timeout and capture bounds.
        let resource_bytes = {
            use conduit_pulse_capture::{
                owner::PulseReadProvider, CheckedPulseRead, PulseReadRequest,
            };
            let checked = CheckedPulseRead::admit(
                PulseReadRequest {
                    hold_low_us: LIMITS.maximum_hold_low_us,
                    skip_high_pulses: LIMITS.maximum_skipped_pulses,
                    // Choose a jointly representable request; independent
                    // maxima need not fit together inside the capture timeout.
                    // Sizing this future never polls or captures any input.
                    sample_after_rise_us: LIMITS.minimum_sample_offset_us,
                    octets: LIMITS.maximum_octets,
                    capture_timeout_us: LIMITS.maximum_capture_timeout_us,
                },
                LIMITS,
            )
            .expect("actual finite capture limits");
            let retained = core::mem::size_of_val(&sensor);
            let pending = PulseReadProvider::read(&mut sensor, checked);
            (retained + core::mem::size_of_val(&pending)).max(64) as u64
        };
        let pulse = unsafe {
            ReadyPulseBase::new(
                identity,
                PulseAttachment {
                    generation: 1,
                    resource_bytes,
                    limits: LIMITS,
                },
                sensor,
            )
        }
        .unwrap();
        let (credentials, credential_writer) = crate::native_credentials::prepare(&host, &boot);
        let mut seed = [0; 32];
        esp_hal::rng::Trng::try_new()
            .expect("Root admission entropy")
            .read(&mut seed);
        let admission = crate::native_admission::NativeAdmission::new(seed)
            .expect("Root admission requires nonzero entropy");
        seed.fill(0);
        mod owner_key {
            include!(concat!(env!("OUT_DIR"), "/owner-control-key.rs"));
        }
        let owner_control = crate::native_owner_control::OwnerControl::new(owner_key::PUBLIC_KEY)
            .expect("installed owner public key must be valid and non-weak");
        // Meaning is pinned by trusted Make material; local planning borrows
        // its index and reconstructs one checked Gear at a time when needed.
        let network_source_identity = network_source;
        Self {
            #[cfg(feature = "body-image-root")]
            body: None,
            #[cfg(all(
                feature = "local-appliance",
                feature = "body-persistence",
                target_os = "none"
            ))]
            observed_offer: None,
            #[cfg(all(
                feature = "local-appliance",
                feature = "body-persistence",
                target_os = "none"
            ))]
            observed_reclaim: None,
            retired_native: None,
            worker: None,
            #[cfg(feature = "native-face-root")]
            face_reader: None,
            worker_spawner: None,
            execution: None,
            native_bindings: None,
            owner_control,
            #[cfg(any(feature = "display-physical-check", feature = "mask-root"))]
            display_reservation: None,
            owner_reservation: None,
            assignment_seal: None,
            appliance_kernel: None,
            admission,
            admission_freshness: 0,
            #[cfg(not(feature = "flat-network-appliance"))]
            network_preparation: None,
            network_source: network_source_identity,
            credentials: Some(credentials),
            #[cfg(all(feature = "local-appliance", target_os = "none"))]
            reclaim_submitter: None,
            issuer,
            play: crate::native_play::NativePlay::new(),
            offer_generation: 1,
            #[cfg(all(not(feature = "display-ready-root"), not(feature = "local-appliance")))]
            software: Some(
                crate::native_software::prepare().expect("initialized resident software"),
            ),
            #[cfg(any(feature = "display-ready-root", feature = "local-appliance"))]
            software: None,
            credential_writer,
            assignment: None,
            network: crate::native_network::NativeNetwork::new(radio, sdk_reservation),
            pulse: Some(pulse),
            display,
            #[cfg(feature = "mask-root")]
            local_mask_scope: None,
            #[cfg(feature = "mask-root")]
            local_mask_worker: None,
            #[cfg(feature = "local-appliance")]
            pending_network_workspace: None,
            #[cfg(feature = "local-appliance")]
            local_renewal: None,
            #[cfg(feature = "local-appliance")]
            local_renewal_archive_refusal: None,
            #[cfg(feature = "local-appliance")]
            local_history_custody: None,
            wait: Some(wait),
            host,
            boot,
        }
    }
    /// Retain the unique profile-filtered queue endpoint. It alone admits no
    /// signed reclamation request and grants no actual wait window.
    #[cfg(all(feature = "local-appliance", target_os = "none"))]
    pub(crate) fn retain_reclaim_submitter(
        &mut self,
        submitter: crate::native_flash_domain::Submitter,
    ) -> Result<(), &'static str> {
        if self.reclaim_submitter.is_some() {
            return Err("reclaim-endpoint-already-owned");
        }
        self.reclaim_submitter = Some(submitter);
        Ok(())
    }
    pub fn owner_control_configured(&self) -> bool {
        self.owner_control.configured()
    }
    pub fn owner_control_sequence(&self) -> u64 {
        self.owner_control.last_sequence()
    }
    pub fn authenticate_owner<'a>(
        &mut self,
        request: crate::native_owner_control::Request<'a>,
        signature: &[u8; 64],
    ) -> Result<
        crate::native_owner_control::AuthenticatedRequest<'a>,
        crate::native_owner_control::Refusal,
    > {
        let generation =
            crate::native_owner_reservation::OwnerReservation::authentication_generation(
                request.operation,
                self.offer_generation,
                self.owner_reservation.as_ref(),
            );
        self.owner_control.authenticate(
            request,
            signature,
            self.host.as_str(),
            self.boot.as_str(),
            generation,
        )
    }
    pub fn next_play_sequence(&self) -> Option<u64> {
        self.issuer.next_play_sequence()
    }
    pub fn owner_reservation(&self) -> Option<&crate::native_owner_reservation::OwnerReservation> {
        self.owner_reservation.as_ref()
    }
    pub fn reserve_owner(
        &mut self,
        authenticated: crate::native_owner_control::AuthenticatedRequest<'_>,
    ) -> Result<(), &'static str> {
        if self.owner_reservation.is_some() || self.appliance_kernel.is_some() {
            return Err("already-reserved-or-prepared");
        }
        // Request IDs are bounded to 128 bytes. This covers retained scope,
        // prospective identity construction and Root response headroom.
        let memory = esp_alloc::HEAP.stats();
        if memory.size.saturating_sub(memory.current_usage) < 4096 {
            return Err("reservation-capacity");
        }
        #[cfg(feature = "display-ready-root")]
        {
            let assignment = self
                .display
                .assignment()
                .ok_or("display-assignment-unavailable")?;
            self.owner_reservation = Some(
                crate::native_owner_reservation::OwnerReservation::reserve_display(
                    authenticated,
                    &assignment.admitted,
                    &assignment.seal(),
                    &mut self.issuer,
                )?,
            );
            return Ok(());
        }
        #[cfg(not(feature = "display-ready-root"))]
        {
            let assignment = self.assignment.as_ref().ok_or("assignment-unavailable")?;
            let seal = self
                .assignment_seal
                .as_ref()
                .ok_or("assignment-seal-unavailable")?;
            self.owner_reservation =
                Some(crate::native_owner_reservation::OwnerReservation::reserve(
                    authenticated,
                    assignment,
                    seal,
                    &mut self.issuer,
                )?);
            Ok(())
        }
    }
    pub fn assignment_staged(&self) -> bool {
        self.assignment.is_some() || self.display.assignment_staged()
    }
    pub fn appliance_kernel_prepared(&self) -> bool {
        self.appliance_kernel.is_some()
            || self.display.kernel_prepared()
            || self.execution_context_prepared()
    }
    pub fn write_admission_offer(&mut self, output: &mut [u8]) -> Result<usize, ()> {
        self.admission_freshness = self.admission_freshness.checked_add(1).ok_or(())?;
        #[derive(serde::Serialize)]
        struct Offer<'a> {
            protocol: u16,
            advertisement: &'a c::HostAdvertisement,
            friendly_label: &'static str,
            verifying_key: [u8; 32],
            freshness_sequence: u64,
        }
        let advertisement = self.advertisement()?;
        let offer = Offer {
            protocol: conduit_body::PICO_ADMISSION_PROTOCOL,
            advertisement: &advertisement,
            friendly_label: "Conduit climate ESP32",
            verifying_key: self.admission.verifying_key(),
            freshness_sequence: self.admission_freshness,
        };
        let output = output
            .get_mut(..crate::native_admission::MAXIMUM_ENVELOPE_BYTES)
            .ok_or(())?;
        serde_json_core::to_slice(&offer, output).map_err(|_| ())
    }
    pub fn sign_admission(
        &self,
        challenge: &conduit_body::PicoAdmissionChallenge<'_>,
    ) -> Result<[u8; 64], ()> {
        self.admission.sign(
            challenge,
            self.host.as_str(),
            self.boot.as_str(),
            self.offer_generation,
        )
    }
    pub fn display_software_offers(
        &mut self,
    ) -> Result<
        conduit_environment_runtime::appliance::software_compact::CompactOutputOffers<'_, 'static>,
        (),
    > {
        if self.owner_reservation.is_some() || self.execution_context_prepared() {
            return Err(());
        }
        self.display.current_software_offers(
            &self.host,
            &self.boot,
            c::OfferGeneration(self.offer_generation),
        )
    }
    pub fn display_policy_grant(&self) -> Result<c::AuthorityGrant, ()> {
        self.issuer
            .grant_display(self.display.native_identity().ok_or(())?)
    }
    pub fn display_software_status(&self) -> (bool, &'static str) {
        (
            self.display.software_ready(),
            self.display.software_refusal().unwrap_or("none"),
        )
    }
    pub fn network_source(&self) -> &c::PlotIdentity {
        &self.network_source
    }
    /// Use current initialized offers only. No authority or local Base is
    /// inferred from authored meaning, SDK configuration or the UART request.
    pub fn network_plan_status(&self) -> &'static str {
        if self.play.attempted() {
            return self.play.status();
        }
        #[cfg(feature = "flat-network-appliance")]
        {
            "awaiting-current-offers"
        }
        #[cfg(not(feature = "flat-network-appliance"))]
        match &self.network_preparation {
            None => "unprepared",
            Some(Err(error)) => error.code(),
            Some(Ok(prepared)) => {
                // Read the retained ordinary result; this is not installation.
                debug_assert_eq!(prepared.metadata().fragment_count(), 1);
                debug_assert!(!prepared.metadata().preparation_metadata().fragments[0]
                    .placements
                    .is_empty());
                "prepared-uninstalled"
            }
        }
    }
    /// Fixed Root bootstrap service, independent of Body/Face installation. Two
    /// HTTP listeners and one DHCP socket share one finite private submission.
    pub async fn start_preparation_service(
        &mut self,
        spawner: embassy_executor::Spawner,
    ) -> Result<(), ()> {
        use conduit_esp32_network_switch::AccessPointServices;
        spawner.spawn(
            crate::services::run(
                self.network.stacks().ok_or(())?,
                Some(self.credential_writer.clone()),
            )
            .map_err(|_| ())?,
        );
        let mut services = crate::services::control(false);
        embassy_time::with_timeout(embassy_time::Duration::from_secs(5), services.restore())
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;
        self.network.retain_services(services);
        self.worker_spawner = Some(spawner);
        Ok(())
    }
}

#[cfg(feature = "display-ready-root")]
impl NativeRoot {
    pub fn stage_display_assignment(
        &mut self,
        wire: &[u8],
    ) -> Result<c::AssignedPlanView, &'static str> {
        if self.owner_reservation.is_some() || self.appliance_kernel_prepared() {
            return Err("already-reserved-or-prepared");
        }
        let memory = esp_alloc::HEAP.stats();
        let identity = self.display.stage_assignment(
            wire,
            conduit_environment_runtime::display_output::AssignedDisplayScope {
                host: &self.host,
                boot: &self.boot,
                generation: c::OfferGeneration(self.offer_generation),
            },
            memory.size.saturating_sub(memory.current_usage),
        )?;
        self.assignment_seal = self.display.assignment().map(|a| a.seal());
        Ok(identity)
    }
}

#[cfg(feature = "display-fit-root")]
impl NativeRoot {
    pub fn display_fit_telemetry(&self) -> crate::native_display::fit::Telemetry {
        self.display.fit_telemetry()
    }
}
