// Actual Root state shared by firmware and the CPU hardware-adapter proof.
pub(crate) type DeviceExecution =
    crate::native_worker::Execution<Esp32PulseCapture<'static>, crate::native_wait::EspWait>;
type DeviceProviders =
    crate::native_worker::RetiredProviders<Esp32PulseCapture<'static>, crate::native_wait::EspWait>;
pub struct NativeRoot {
    #[cfg(all(
        feature = "local-appliance",
        feature = "body-persistence",
        target_os = "none"
    ))]
    observed_offer: Option<alloc::boxed::Box<observed_offer::Job>>,
    #[cfg(all(feature = "local-appliance", target_os = "none"))]
    reclaim_submitter: Option<crate::native_flash_domain::Submitter>,
    #[cfg(feature = "body-image-root")]
    body: Option<alloc::boxed::Box<conduit_body::SealedBodyLifecycleSession>>,
    retired_native: Option<DeviceProviders>,
    worker: Option<crate::native_task::Handle>,
    #[cfg(feature = "native-face-root")]
    face_reader: Option<crate::native_face::Reader>,
    worker_spawner: Option<embassy_executor::Spawner>,
    execution: Option<alloc::boxed::Box<DeviceExecution>>,
    native_bindings: Option<
        crate::native_authority::NativeBindings<
            Esp32PulseCapture<'static>,
            crate::native_wait::EspWait,
        >,
    >,
    owner_control: crate::native_owner_control::OwnerControl,
    #[cfg(any(feature = "display-physical-check", feature = "mask-root"))]
    display_reservation: Option<crate::native_owner_reservation::OwnerReservation>,
    owner_reservation: Option<crate::native_owner_reservation::OwnerReservation>,
    pub(crate) assignment_seal: Option<[u8; 32]>,
    appliance_kernel: Option<crate::native_kernel::PreparedKernel>,
    admission: crate::native_admission::NativeAdmission,
    admission_freshness: u64,
    #[cfg(not(feature = "flat-network-appliance"))]
    network_preparation: Option<
        Result<
            crate::network_planning::PreparedVerifiedNetworkSetup,
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
    #[cfg(feature = "mask-root")]
    local_mask_scope: Option<crate::native_authority::MaskScopeGuard>,
    #[cfg(feature = "mask-root")]
    local_mask_worker: Option<crate::native_display::task::Handle>,
    #[cfg(feature = "local-appliance")]
    pending_network_workspace: Option<setup::PendingWorkspace>,
    #[cfg(feature = "local-appliance")]
    local_renewal: Option<alloc::boxed::Box<local_renewal::Renewal>>,
    #[cfg(feature = "local-appliance")]
    local_renewal_archive_refusal: Option<&'static str>,
    #[cfg(feature = "local-appliance")]
    local_history_custody: Option<alloc::boxed::Box<local_renewal::HistoryCustody>>,
    wait: Option<conduit_monotonic_wait::ReadyMonotonicWaitBase<crate::native_wait::EspWait>>,
    host: c::HostId,
    boot: c::BootId,
}
