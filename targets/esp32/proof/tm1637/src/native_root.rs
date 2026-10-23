//! Actual initialized resources and Root provisioning Control. Environmental
//! Host/Body construction and its sensor Plan remain separate.
//! No simulated provider, descriptor-derived availability or execution is used.
use conduit_core as c;
use conduit_esp32_pulse_capture::{Esp32PulseCapture, LIMITS};
use conduit_pulse_capture::{installation::*, owner::*, PulseReadContract};

pub struct NativeRoot {
    network_preparation: Option<
        Result<
            crate::network_planning::PreparedNetworkSetup,
            crate::network_planning::NetworkPlanningRefusal,
        >,
    >,
    network_source: conduit_plot::ExpandedCanonicalPlot,
    credentials: Option<
        conduit_network_join_call::credential_installation::ReadyCredentialBase<
            crate::native_credentials::RootCredentialSource,
        >,
    >,
    issuer: crate::native_authority::RootIssuer,
    play: crate::native_play::NativePlay,
    offer_generation: u64,
    credential_writer: conduit_environment_http::network_setup::NetworkSetupWriter<'static>,
    network: crate::native_network::NativeNetwork,
    pulse: ReadyPulseBase<Esp32PulseCapture<'static>>,
    contract: PulseReadContract,
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
        Self {
            network_preparation: None,
            network_source,
            credentials: Some(credentials),
            issuer,
            play: crate::native_play::NativePlay::new(),
            offer_generation: 1,
            credential_writer,
            network: crate::native_network::NativeNetwork::new(radio, sdk_reservation),
            pulse,
            contract: PulseReadContract::prepare().unwrap(),
            host,
            boot,
        }
    }
    pub fn network_source(&self) -> &conduit_plot::ExpandedCanonicalPlot {
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
        self.network_preparation =
            Some(crate::network_planning::PreparedNetworkSetup::from_checked(
                &self.network_source,
                &host,
                &local_bases,
                grants.as_ref().map_or(&[], |v| v.as_slice()),
            ));
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
                self.network.ap_stack().ok_or(())?,
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
        Ok(())
    }
    /// Preparation never consumes the selected source or changes radio mode.
    /// Once configured, subsequent offers reuse the retained SDK configuration.
    pub fn prepare_network_configuration(&mut self) -> &'static [u8] {
        use conduit_environment_http::network_setup::NetworkSetupRefusal as E;
        if self.play.attempted() {
            return self.play.status().as_bytes();
        }
        if self.network.is_ready() {
            return b"prepared";
        }
        if self.network.has_staged_configuration() {
            // A refused preparation still retains its initialized owner. The
            // one-submission diagnostic never overwrites or silently retries it.
            return b"refused";
        }
        let configuration = {
            let Ok(mailbox) = self.credential_writer.try_lock() else {
                return b"busy";
            };
            mailbox
                .prepare_acknowledged(conduit_esp32_network_switch::prepare_station_configuration)
        };
        match configuration {
            Ok(Ok(config)) => {
                self.network.stage(config);
                if self.network.prepare(&self.host, &self.boot).is_err() {
                    return b"refused";
                }
                self.offer_generation += 1;
                // The retained provider changes the truthful offer generation.
                // Preparation is fresh; old refusal/Plan material is discarded.
                if self.prepare_network_plan().is_err() {
                    return b"refused";
                }
                let Some(Ok(prepared)) = &self.network_preparation else {
                    return b"planning-refused";
                };
                let result = self.play.install(
                    prepared,
                    &mut self.credentials,
                    self.network.ready_slot(),
                    &mut self.issuer,
                );
                self.network.mark_selected();
                self.offer_generation += 1;
                if result.is_err() {
                    return b"installation-refused";
                }
                b"installed"
            }
            Ok(Err(_)) => b"refused",
            Err(E::NoSubmission | E::NotAcknowledged) => b"waiting",
            Err(_) => b"refused",
        }
    }
    /// Poll independently of UART management. HTTP acknowledgement is required
    /// before preparation, selection or the control Play can consume input.
    pub async fn advance_setup(&mut self) {
        self.prepare_network_configuration();
        self.play.run().await;
    }
    pub fn control_binding(&self) -> Option<&c::ActivePlayIdentity> {
        self.play.active()
    }
    pub fn control_progress(&self) -> (u16, [u8; 3], bool) {
        let (steps, calls) = self.play.progress();
        (steps, calls, self.play.receipt_received())
    }
    pub fn control_installed(&self) -> bool {
        self.play.installed()
    }
    pub fn control_status(&self) -> &'static str {
        self.play.status()
    }
    pub fn control_failure(&self) -> (&'static str, &'static str) {
        (self.play.failure_phase(), self.play.failure_reason())
    }
    pub fn network_ready(&self) -> bool {
        self.network.is_ready()
    }
    /// Projection is made from the still-retained provider each time. It is an
    /// offer, not authority, membership, a Body birth or an installed native Play.
    pub fn write_offer(&self, output: &mut [u8]) -> Result<usize, ()> {
        serde_json_core::to_slice(&self.advertisement()?, output).map_err(|_| ())
    }
    fn advertisement(&self) -> Result<c::HostAdvertisement, ()> {
        let mut advertisement = c::HostAdvertisement {
            protocol_version: c::PROTOCOL_VERSION,
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            offer_generation: c::OfferGeneration(self.offer_generation),
            profile: "environment/esp32-native-root@1".into(),
            bases: alloc::vec![],
            resources: alloc::vec![],
            capabilities: alloc::vec![],
            planner_capabilities: alloc::vec![],
        };
        self.pulse
            .append_to_advertisement(&mut advertisement, &self.contract)
            .map_err(|_| ())?;
        if let Some(source) = &self.credentials {
            source
                .append_to_advertisement(&mut advertisement)
                .map_err(|_| ())?;
        }
        self.play.append_offer(&mut advertisement);
        self.network.append_to_advertisement(&mut advertisement)?;
        Ok(advertisement)
    }
}
