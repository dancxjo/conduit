//! Actual initialized resources and Root provisioning Control. Environmental
//! Host/Body construction and its sensor Plan remain separate.
//! No simulated provider, descriptor-derived availability or execution is used.
#[path = "native_body_root.rs"]
mod body;
#[path = "native_install.rs"]
mod install;
#[path = "native_lifecycle.rs"]
mod lifecycle;
#[path = "native_offer.rs"]
mod offers;
#[path = "native_setup.rs"]
mod setup;
use conduit_core as c;
use conduit_esp32_pulse_capture::{Esp32PulseCapture, LIMITS};
use conduit_pulse_capture::{installation::*, owner::*, PulseReadContract};

pub(crate) type DeviceExecution =
    crate::native_worker::Execution<Esp32PulseCapture<'static>, crate::native_wait::EspWait>;
type DeviceProviders =
    crate::native_worker::RetiredProviders<Esp32PulseCapture<'static>, crate::native_wait::EspWait>;
pub struct NativeRoot {
    #[cfg(feature = "body-image-root")]
    body: Option<alloc::boxed::Box<conduit_body::BodyLifecycleSession>>,
    retired_native: Option<DeviceProviders>,
    worker: Option<crate::native_task::Handle>,
    #[cfg(feature = "native-face-root")]
    face_reader: Option<&'static crate::native_face::Reader>,
    worker_spawner: Option<embassy_executor::Spawner>,
    execution: Option<alloc::boxed::Box<DeviceExecution>>,
    native_bindings: Option<
        crate::native_authority::NativeBindings<
            Esp32PulseCapture<'static>,
            crate::native_wait::EspWait,
        >,
    >,
    owner_control: crate::native_owner_control::OwnerControl,
    owner_reservation: Option<crate::native_owner_reservation::OwnerReservation>,
    pub(crate) assignment_seal: Option<[u8; 32]>,
    appliance_kernel: Option<crate::native_kernel::PreparedKernel>,
    admission: crate::native_admission::NativeAdmission,
    admission_freshness: u64,
    network_preparation: Option<
        Result<
            crate::network_planning::PreparedNetworkSetup,
            crate::network_planning::NetworkPlanningRefusal,
        >,
    >,
    network_source: c::PlotIdentity,
    credentials: Option<
        conduit_network_join_call::credential_installation::ReadyCredentialBase<
            crate::native_credentials::RootCredentialSource,
        >,
    >,
    issuer: crate::native_authority::RootIssuer,
    play: crate::native_play::NativePlay,
    offer_generation: u64,
    software: Option<alloc::boxed::Box<crate::native_software::Software>>,
    pub(crate) assignment: Option<
        alloc::boxed::Box<
            conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance,
        >,
    >,
    credential_writer: conduit_environment_http::network_setup::NetworkSetupWriter<'static>,
    network: crate::native_network::NativeNetwork,
    pulse: Option<ReadyPulseBase<Esp32PulseCapture<'static>>>,
    display: alloc::boxed::Box<crate::native_display::NativeDisplay>,
    contract: PulseReadContract,
    wait: Option<conduit_monotonic_wait::ReadyMonotonicWaitBase<crate::native_wait::EspWait>>,
    host: c::HostId,
    boot: c::BootId,
}
impl NativeRoot {
    pub fn prepare(
        sensor: Esp32PulseCapture<'static>,
        radio: crate::network::Radio,
        boot: &crate::boot::ObservedBoot,
        network_source: conduit_plot::ExpandedCanonicalPlot,
        sdk_reservation: crate::sdk_memory::NativeSdkReservation,
        wait: conduit_monotonic_wait::ReadyMonotonicWaitBase<crate::native_wait::EspWait>,
        display: alloc::boxed::Box<crate::native_display::NativeDisplay>,
    ) -> Self {
        let issuer = crate::native_authority::RootIssuer::observe(boot)
            .expect("initialized hardware entropy for Root issuance");
        let (host, boot) = boot.ids();
        let host: c::HostId = host.into();
        let boot: c::BootId = boot.into();
        // One owned pin, 64 bytes of capture attachment and the reviewed finite
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
        let pulse = unsafe {
            ReadyPulseBase::new(
                identity,
                PulseAttachment {
                    generation: 1,
                    resource_bytes: 64,
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
        // Only the exact meaning identity survives construction. Network
        // planning later reloads the independently checked linked Source during
        // its admitted preparation workspace, never in the sensor Play.
        let network_source_identity = c::PlotIdentity {
            source_document_id: network_source.source_document_id.clone(),
            checked_plot_id: network_source.checked_plot_id.clone(),
            expanded_plot_id: network_source.expanded_plot_id.clone(),
        };
        drop(network_source);
        Self {
            #[cfg(feature = "body-image-root")]
            body: None,
            retired_native: None,
            worker: None,
            #[cfg(feature = "native-face-root")]
            face_reader: None,
            worker_spawner: None,
            execution: None,
            native_bindings: None,
            owner_control,
            owner_reservation: None,
            assignment_seal: None,
            appliance_kernel: None,
            admission,
            admission_freshness: 0,
            network_preparation: None,
            network_source: network_source_identity,
            credentials: Some(credentials),
            issuer,
            play: crate::native_play::NativePlay::new(),
            offer_generation: 1,
            #[cfg(not(feature = "display-ready-root"))]
            software: Some(
                crate::native_software::prepare().expect("initialized resident software"),
            ),
            #[cfg(feature = "display-ready-root")]
            software: None,
            credential_writer,
            assignment: None,
            network: crate::native_network::NativeNetwork::new(radio, sdk_reservation),
            pulse: Some(pulse),
            display,
            contract: PulseReadContract::prepare().unwrap(),
            wait: Some(wait),
            host,
            boot,
        }
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
    pub fn prepare_appliance_kernel(&mut self) -> Result<(), &'static str> {
        if self.appliance_kernel.is_some() {
            return Err("already-prepared");
        }
        if self.owner_reservation.is_some() {
            return Err("owner-reserved");
        }
        self.appliance_kernel = Some(crate::native_kernel::prepare(
            &mut self.software,
            &mut self.assignment,
        )?);
        self.assignment_seal = None;
        Ok(())
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
    pub fn offer_scope(&self) -> (&c::HostId, &c::BootId, c::OfferGeneration) {
        (
            &self.host,
            &self.boot,
            c::OfferGeneration(self.offer_generation),
        )
    }
    pub fn display_software_offers(
        &mut self,
    ) -> Result<conduit_environment_runtime::appliance::software::ReadySoftwareOffers<'_>, ()> {
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
    pub fn software(&self) -> Option<&crate::native_software::Software> {
        self.software.as_deref()
    }
    /// Root policy over still-retained providers. A projected grant is not a
    /// locally issued capability, Play reservation or Body start.
    pub fn appliance_policy_grants(&self) -> Result<[c::AuthorityGrant; 2], ()> {
        self.software.as_ref().ok_or(())?;
        let wait = self.wait.as_ref().ok_or(())?;
        let (_, cadence) = crate::native_software::source_facts();
        if cadence.count == 0 {
            return Err(());
        }
        Ok([
            self.issuer
                .grant_pulse(self.pulse.as_ref().ok_or(())?.native_identity())?,
            self.issuer.grant_wait(wait.native_identity())?,
        ])
    }
    pub fn network_source(&self) -> &c::PlotIdentity {
        &self.network_source
    }
    /// Use current initialized offers only. No authority or local Base is
    /// inferred from authored meaning, SDK configuration or the UART request.
    /// A prepared Plan remains uninstalled; admission and kernel storage are
    /// separate. Each call replaces preparation against this same actual Boot.
    pub fn prepare_network_plan(&mut self) -> Result<(), ()> {
        self.network_preparation = None;
        let host = self.advertisement()?;
        let local_bases = [c::BaseImplementationId::from("conduit.base/local@1")];
        let grants = match (self.network.ready(), self.credentials.as_ref()) {
            (Some(join), Some(source)) => Some([
                self.issuer.grant(
                    source.native_identity(),
                    crate::native_authority::Operation::Credentials,
                )?,
                self.issuer.grant(
                    join.native_identity(),
                    crate::native_authority::Operation::Join,
                )?,
            ]),
            _ => None,
        };
        let source = crate::native_source::load();
        self.network_preparation = Some(
            crate::network_planning::PreparedNetworkSetup::from_checked_for_identity(
                &self.network_source,
                &source,
                &host,
                &local_bases,
                grants.as_ref().map_or(&[], |v| v.as_slice()),
            ),
        );
        Ok(())
    }
    pub fn network_plan_status(&self) -> &'static str {
        if self.play.attempted() {
            return self.play.status();
        }
        match &self.network_preparation {
            None => "unprepared",
            Some(Err(error)) => error.code(),
            Some(Ok(prepared)) => {
                // Read the retained ordinary result; this is not installation.
                debug_assert_eq!(prepared.plan().fragments.len(), 1);
                debug_assert!(!prepared.lowered().nodes.is_empty());
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
