//! Physical implementation invoked by the already admitted network/join Call.
//! Root supplies one deadline shared by association and DHCP configuration.
use conduit_net::{
    associate_and_configure, DeadlineExpired, OperationDeadline, StationAttemptFailure,
    StationFailure, StationOperations,
};
use core::future::Future;
use embassy_net::Stack;
use embassy_time::{with_timeout, Duration};

struct Deadline(Duration);
impl OperationDeadline for Deadline {
    async fn run<T>(&mut self, work: impl Future<Output = T>) -> Result<T, DeadlineExpired> {
        with_timeout(self.0, work)
            .await
            .map_err(|_| DeadlineExpired)
    }
}
struct Station<'a, 'd> {
    control: &'a mut cyw43::Control<'d>,
    stack: Stack<'static>,
    ssid: &'a str,
    credential: &'a [u8],
}
impl StationOperations for Station<'_, '_> {
    async fn associate(&mut self) -> Result<(), StationFailure> {
        crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::NetworkJoin);
        self.control
            .join(self.ssid, cyw43::JoinOptions::new(self.credential))
            .await
            .map_err(|_| StationFailure::Rejected)
    }
    async fn configure_address(&mut self) -> Result<(), StationFailure> {
        crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::NetworkConfiguration);
        self.stack.wait_config_up().await;
        Ok(())
    }
}
pub async fn join_and_configure(
    control: &mut cyw43::Control<'_>,
    stack: Stack<'static>,
    ssid: &str,
    credential: &[u8],
    maximum_duration: Duration,
) -> Result<(), StationAttemptFailure> {
    associate_and_configure(
        &mut Station {
            control,
            stack,
            ssid,
            credential,
        },
        &mut Deadline(maximum_duration),
    )
    .await
}
