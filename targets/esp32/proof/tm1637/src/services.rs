//! Actual socket-task ownership for the attended bench. No Body/Plan identity.
use conduit_environment_http::service_control::{
    ServiceControl, ServiceMailbox, ServiceMode, ServiceState,
};
use core::cell::Cell;
use embassy_futures::select::{select, Either};
use embassy_net::Stack;
pub static MAILBOX: ServiceMailbox = ServiceMailbox::new();
#[embassy_executor::task]
pub async fn run(stack: Stack<'static>) {
    let mut command = MAILBOX.next_command().await;
    loop {
        match command.mode {
            ServiceMode::Stopped => {
                conduit_environment_http::set_ap_unavailable();
                // The running future was dropped before reaching this branch.
                MAILBOX.acknowledge(command, ServiceState::Stopped);
                command = MAILBOX.next_command().await;
            }
            ServiceMode::Running => {
                conduit_environment_http::set_ap_unavailable();
                let ready = Cell::new(0_u8);
                let next = select(
                    crate::http_ap::serve_acknowledged(stack, |service| {
                        let bit = match service {
                            crate::http_ap::ServiceReady::Dhcp => 1,
                            crate::http_ap::ServiceReady::Http0 => 2,
                            crate::http_ap::ServiceReady::Http1 => 4,
                        };
                        ready.set(ready.get() | bit);
                        if ready.get() == 7 {
                            MAILBOX.acknowledge(command, ServiceState::Ready);
                        }
                    }),
                    MAILBOX.next_command(),
                )
                .await;
                // select owns and has now dropped all socket/request futures.
                match next {
                    Either::First(_) => {
                        conduit_environment_http::set_ap_unavailable();
                        MAILBOX.acknowledge(command, ServiceState::Failed);
                        command = MAILBOX.next_command().await;
                    }
                    Either::Second(next) => command = next,
                }
            }
        }
    }
}
pub fn control() -> ServiceControl<'static> {
    MAILBOX.claim_control(32).unwrap()
}
