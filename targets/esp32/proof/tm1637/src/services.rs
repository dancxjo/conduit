//! Actual socket-task ownership for the attended bench. No Body/Plan identity.
#[cfg(feature = "network-mdns")]
#[path = "native_mdns.rs"]
mod mdns;
use conduit_environment_http::network_setup::NetworkSetupWriter;
use conduit_environment_http::service_control::{
    ServiceControl, ServiceMailbox, ServiceMode, ServiceState,
};
use core::cell::Cell;
use embassy_futures::select::{select, Either};
use embassy_net::Stack;
pub static MAILBOX: ServiceMailbox = ServiceMailbox::new();
#[embassy_executor::task]
pub async fn run(
    stacks: (Stack<'static>, Stack<'static>),
    setup: Option<NetworkSetupWriter<'static>>,
) {
    #[cfg(feature = "native-face-root")]
    let reading = Some(
        crate::native_face::http_response as conduit_environment_http::exchange::ReadingProvider,
    );
    // Prepare once before serving; mode changes retain and reuse this backing.
    // Missing capacity refuses the selected service instead of allocating later.
    #[cfg(feature = "native-face-root")]
    let mut responses = {
        let stats = esp_alloc::HEAP.stats();
        if stats.size.saturating_sub(stats.current_usage)
            < core::mem::size_of::<crate::http_ap::ReadingBuffers>() + 8192
        {
            None
        } else {
            Some(alloc::boxed::Box::new([
                heapless::String::new(),
                heapless::String::new(),
            ]))
        }
    };
    #[cfg(feature = "network-mdns")]
    let mut mdns_buffers = {
        let stats = esp_alloc::HEAP.stats();
        if stats.size.saturating_sub(stats.current_usage) < mdns::BACKING_BYTES + 8192 {
            None
        } else {
            Some(alloc::boxed::Box::new(mdns::Buffers::new()))
        }
    };
    let mut command = MAILBOX.next_command().await;
    loop {
        match command.mode {
            ServiceMode::Stopped => {
                if setup.is_none() {
                    conduit_environment_http::set_ap_unavailable();
                }
                // The running future was dropped before reaching this branch.
                MAILBOX.acknowledge(command, ServiceState::Stopped);
                command = MAILBOX.next_command().await;
            }
            ServiceMode::Running | ServiceMode::Station => {
                let station = command.mode == ServiceMode::Station;
                let stack = if station { stacks.1 } else { stacks.0 };
                if setup.is_none() {
                    conduit_environment_http::set_ap_unavailable();
                }
                let ready = Cell::new(0_u8);
                let ready_callback = |service| {
                    let bit = match service {
                        crate::http_ap::ServiceReady::Dhcp => 1,
                        crate::http_ap::ServiceReady::Http0 => 2,
                        crate::http_ap::ServiceReady::Http1 => 4,
                    };
                    ready.set(ready.get() | bit);
                    if ready.get() == if station { 6 } else { 7 } {
                        MAILBOX.acknowledge(
                            command,
                            if station {
                                ServiceState::StationReady
                            } else {
                                ServiceState::Ready
                            },
                        );
                    }
                };
                #[cfg(not(feature = "native-face-root"))]
                let mut local_responses = [heapless::String::new(), heapless::String::new()];
                let serving = async {
                    #[cfg(feature = "native-face-root")]
                    let buffers = responses.as_deref_mut().ok_or(())?;
                    #[cfg(not(feature = "native-face-root"))]
                    let buffers = &mut local_responses;
                    #[cfg(not(feature = "native-face-root"))]
                    let reading = None;
                    crate::http_ap::serve_mode_with_reading(
                        stack,
                        // Credential ingress belongs to the AP. Installing its
                        // preparation-only route on a station denies otherwise
                        // admitted reading/history providers as bootstrap data.
                        if station { None } else { setup.clone() },
                        !station && setup.is_some(),
                        reading,
                        buffers,
                        !station,
                        ready_callback,
                    )
                    .await
                };
                #[cfg(feature = "network-mdns")]
                let next = {
                    let multicast = async {
                        if station {
                            let backing = mdns_buffers.as_deref_mut().ok_or(())?;
                            mdns::run(stack, backing).await
                        } else {
                            core::future::pending::<Result<(), ()>>().await
                        }
                    };
                    // Construct the HTTP future once. Neither mode duplicates
                    // its socket/request frame inside a second async branch.
                    match select(select(serving, multicast), MAILBOX.next_command()).await {
                        Either::First(_) => Either::First(()),
                        Either::Second(command) => Either::Second(command),
                    }
                };
                #[cfg(not(feature = "network-mdns"))]
                let next = select(serving, MAILBOX.next_command()).await;
                // select owns and has now dropped all socket/request futures.
                match next {
                    Either::First(_) => {
                        if setup.is_none() {
                            conduit_environment_http::set_ap_unavailable();
                        }
                        MAILBOX.acknowledge(command, ServiceState::Failed);
                        command = MAILBOX.next_command().await;
                    }
                    Either::Second(next) => command = next,
                }
            }
        }
    }
}
/// The composition root retains this unique handle across mode changes.
/// Cancelling an operation leaves the task owned; the next generation supersedes
/// its pending command. Completion is based on actual socket-task evidence.
pub struct Owner {
    control: ServiceControl<'static>,
    observations: bool,
}
impl conduit_esp32_network_switch::AccessPointServices for Owner {
    async fn withdraw(&mut self) -> Result<(), conduit_net::StationFailure> {
        if self.observations {
            conduit_environment_http::set_ap_unavailable();
        }
        self.control
            .request(ServiceMode::Stopped)
            .await
            .map_err(|_| conduit_net::StationFailure::ProviderUnavailable)
    }
    async fn station(&mut self) -> Result<(), conduit_net::StationFailure> {
        self.control
            .request(ServiceMode::Station)
            .await
            .map_err(|_| conduit_net::StationFailure::ProviderUnavailable)
    }
    async fn restore(&mut self) -> Result<(), conduit_net::StationFailure> {
        self.control
            .request(ServiceMode::Running)
            .await
            .map_err(|_| conduit_net::StationFailure::ProviderUnavailable)
    }
}
pub fn control(observations: bool) -> Owner {
    Owner {
        control: MAILBOX.claim_control(32).unwrap(),
        observations,
    }
}

#[cfg(feature = "network-scan")]
impl conduit_esp32_network_switch::RestoredAccessPointServices for Owner {
    fn access_point_ready(&self) -> bool {
        self.control.acknowledged_state() == Some(ServiceState::Ready)
    }
    fn remaining_service_commands(&self) -> u32 {
        self.control.remaining_commands()
    }
}

#[cfg(feature = "network-mdns")]
pub const MDNS_BACKING_BYTES: usize = mdns::BACKING_BYTES;
#[cfg(feature = "network-mdns")]
pub fn mdns_available() -> bool {
    mdns::available()
}
#[cfg(feature = "network-mdns")]
pub fn mdns_hostname() -> heapless::String<48> {
    mdns::hostname()
}
